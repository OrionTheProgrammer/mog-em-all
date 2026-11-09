//! Base de datos local de mog 'em all.
//!
//! Guarda el perfil, los ejercicios y las sesiones de entrenamiento en un archivo
//! SQLite en el equipo del usuario. Los cálculos (XP, récords, rachas) los hace
//! `mog-core`; este crate solo lee, escribe y junta los datos que esos cálculos piden.

mod migrations;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, TimeZone};
use mog_core::catalog::{self, Muscle};
use mog_core::ranks::{self, BodyProfile, Lift, SessionSummary};
use mog_core::schedule::{DayNumber, Minute, Recurrence};
pub use mog_core::training::ExerciseKind;
use mog_core::training::{SetType, Weekday};
use rusqlite::{Connection, OptionalExtension, params};

pub use migrations::AppliedMigration;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    /// La base la creó una versión más nueva de la app. No se toca para no perder datos.
    NewerSchema {
        found: u32,
        supported: u32,
    },
    /// El SQL de una migración ya aplicada cambió. Es un error de desarrollo.
    ModifiedMigration {
        version: u32,
    },
    /// La base registra una migración que esta versión no conoce.
    UnknownMigration {
        version: u32,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(err) => write!(f, "SQLite: {err}"),
            Error::Io(err) => write!(f, "archivo: {err}"),
            Error::NewerSchema { found, supported } => write!(
                f,
                "la base de datos tiene el esquema {found} y esta versión de la app solo entiende hasta el {supported}; actualiza la app"
            ),
            Error::ModifiedMigration { version } => {
                write!(f, "la migración {version} cambió después de aplicarse")
            }
            Error::UnknownMigration { version } => {
                write!(
                    f,
                    "la base registra la migración {version}, que esta versión no conoce"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Sqlite(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

/// Segundos desde 1970 (UTC). Es la unidad de tiempo de toda la base de datos.
pub type Timestamp = i64;

pub type ExerciseId = i64;
pub type SessionId = i64;

/// Semanas hacia atrás que se miran para calcular la racha.
const STREAK_LOOKBACK_WEEKS: i64 = 52;

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: String,
    pub bodyweight_kg: Option<f64>,
    pub height_cm: Option<f64>,
    pub body_profile: BodyProfile,
    pub weekly_goal: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Exercise {
    pub id: ExerciseId,
    /// Código de los ejercicios incluidos (`"squat"`); la interfaz lo traduce.
    pub code: Option<String>,
    /// Nombre escrito por el usuario, para ejercicios propios.
    pub name: Option<String>,
    pub kind: ExerciseKind,
    pub lift: Option<Lift>,
    /// Músculo principal: del catálogo en los incluidos, elegido por el usuario en los propios.
    pub muscle: Option<Muscle>,
}

impl Exercise {
    /// Ficha del catálogo, si es un ejercicio incluido.
    pub fn catalog(&self) -> Option<&'static catalog::CatalogExercise> {
        self.code.as_deref().and_then(catalog::find)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SetEntry {
    pub reps: Option<u32>,
    /// En ejercicios de peso corporal es el lastre.
    pub weight_kg: Option<f64>,
    pub duration_s: Option<u32>,
    pub set_type: SetType,
    /// Momento en que se anotó la serie.
    pub completed_at: Option<Timestamp>,
}

pub type RoutineId = i64;
pub type RoutineExerciseId = i64;

/// Una rutina: un día de entrenamiento con sus ejercicios en orden.
#[derive(Debug, Clone, PartialEq)]
pub struct Routine {
    pub id: RoutineId,
    pub name: String,
    pub weekdays: Vec<Weekday>,
    pub exercises: Vec<RoutineExercise>,
    /// Hora a la que se suele hacer; `None` si no tiene hora fija.
    pub start_min: Option<Minute>,
    pub duration_min: u16,
}

pub type EventId = i64;

/// Tipo de evento del horario. El número es el que guarda la base de datos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventKind {
    #[default]
    Class,
    Work,
    Study,
    Personal,
    Other,
}

impl EventKind {
    pub const ALL: [EventKind; 5] = [
        EventKind::Class,
        EventKind::Work,
        EventKind::Study,
        EventKind::Personal,
        EventKind::Other,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0) as u8
    }

    pub fn from_index(index: i64) -> EventKind {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub id: EventId,
    pub title: String,
    pub kind: EventKind,
    pub start_min: Minute,
    pub end_min: Minute,
    pub recurrence: Recurrence,
    /// Minutos antes para avisar; `None` sin aviso.
    pub reminder_min: Option<u32>,
}

/// Un ejercicio dentro de una rutina, con su meta.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutineExercise {
    pub id: RoutineExerciseId,
    pub exercise_id: ExerciseId,
    pub target: Target,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub sets: u32,
    pub reps_min: u32,
    pub reps_max: u32,
    pub weight_kg: Option<f64>,
    pub rest_s: u32,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            sets: 3,
            reps_min: 8,
            reps_max: 12,
            weight_kg: None,
            rest_s: 90,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonalRecord {
    pub exercise_id: ExerciseId,
    pub previous_kg: f64,
    pub new_kg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FinishedSession {
    pub duration_min: u32,
    pub sets: u32,
    /// Suma de repeticiones por peso en los ejercicios con carga externa.
    pub volume_kg: f64,
    pub records: Vec<PersonalRecord>,
    pub streak_weeks: u32,
    /// Detalle de la XP calculada. Si la sesión pasó el tope diario, `xp` es 0
    /// aunque el detalle tenga valores.
    pub breakdown: ranks::XpBreakdown,
    pub over_daily_cap: bool,
    pub xp: u32,
    pub total_xp_before: u64,
    pub total_xp_after: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveSession {
    pub id: SessionId,
    pub name: String,
    pub started_at: Timestamp,
    pub routine_id: Option<RoutineId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoggedSet {
    pub exercise_id: ExerciseId,
    pub set: SetEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub id: SessionId,
    pub name: String,
    pub started_at: Timestamp,
    pub ended_at: Timestamp,
    pub sets: u32,
    pub xp: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeekStats {
    pub sessions: u32,
    pub streak_weeks: u32,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Ruta por defecto del archivo: la carpeta de datos del usuario del sistema.
    pub fn default_path(data_dir: &Path) -> PathBuf {
        data_dir.join("mog-em-all").join("mog.db")
    }

    /// Abre (o crea) la base en `path` y la lleva a la última versión del esquema.
    /// Si hay migraciones pendientes sobre una base con datos, antes guarda una copia
    /// del archivo junto al original (`mog.db.v1-<fecha>.bak`).
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, migrations::MIGRATIONS)
    }

    fn open_with(path: &Path, list: &[migrations::Migration]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let from = migrations::current_version(&conn)?;
        if from > 0 && migrations::pending(&conn, list)? > 0 {
            backup(&conn, path, from)?;
        }
        Self::init(conn, list)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?, migrations::MIGRATIONS)
    }

    fn init(mut conn: Connection, list: &[migrations::Migration]) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::run(&mut conn, list, chrono::Utc::now().timestamp())?;
        let store = Self { conn };
        store.seed_exercises()?;
        Ok(store)
    }

    /// Migraciones aplicadas a esta base, de la más antigua a la más nueva.
    pub fn migration_history(&self) -> Result<Vec<AppliedMigration>> {
        migrations::history(&self.conn)
    }

    /// Versión de esquema más nueva que entiende esta compilación.
    pub fn supported_schema_version() -> u32 {
        migrations::latest()
    }

    /// Agrega los ejercicios del catálogo que falten. Corre en cada apertura, así
    /// las bases existentes reciben los ejercicios nuevos de cada versión.
    fn seed_exercises(&self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT OR IGNORE INTO exercise (code, name, kind, lift) VALUES (?1, NULL, ?2, ?3)",
        )?;
        for e in catalog::all() {
            stmt.execute(params![
                e.code,
                kind_to_i64(e.kind),
                e.lift.map(lift_to_i64)
            ])?;
        }
        Ok(())
    }

    // Perfil

    pub fn profile(&self) -> Result<Option<Profile>> {
        Ok(self
            .conn
            .query_row(
                "SELECT name, bodyweight_kg, height_cm, body_profile, weekly_goal FROM profile WHERE id = 1",
                [],
                |row| {
                    Ok(Profile {
                        name: row.get(0)?,
                        bodyweight_kg: row.get(1)?,
                        height_cm: row.get(2)?,
                        body_profile: body_profile_from_i64(row.get(3)?),
                        weekly_goal: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn save_profile(&self, profile: &Profile, now: Timestamp) -> Result<()> {
        self.conn.execute(
            "INSERT INTO profile (id, name, bodyweight_kg, height_cm, body_profile, weekly_goal, created_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (id) DO UPDATE SET
                name = excluded.name,
                bodyweight_kg = excluded.bodyweight_kg,
                height_cm = excluded.height_cm,
                body_profile = excluded.body_profile,
                weekly_goal = excluded.weekly_goal",
            params![
                profile.name,
                profile.bodyweight_kg,
                profile.height_cm,
                body_profile_to_i64(profile.body_profile),
                profile.weekly_goal,
                now
            ],
        )?;
        Ok(())
    }

    // Ejercicios

    pub fn exercises(&self) -> Result<Vec<Exercise>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, code, name, kind, lift, muscle FROM exercise")?;
        let rows = stmt.query_map([], |row| {
            let code: Option<String> = row.get(1)?;
            let stored_muscle = row.get::<_, Option<i64>>(5)?.and_then(Muscle::from_index);
            let muscle = code
                .as_deref()
                .and_then(catalog::find)
                .map(|c| c.muscle)
                .or(stored_muscle);
            Ok(Exercise {
                id: row.get(0)?,
                code,
                name: row.get(2)?,
                kind: kind_from_i64(row.get(3)?),
                lift: row.get::<_, Option<i64>>(4)?.and_then(lift_from_i64),
                muscle,
            })
        })?;
        let mut exercises: Vec<Exercise> = rows.collect::<rusqlite::Result<_>>()?;
        // Orden del catálogo (agrupado por músculo); los propios al final, por nombre.
        let position = |e: &Exercise| {
            e.code
                .as_deref()
                .and_then(|c| catalog::all().iter().position(|x| x.code == c))
                .unwrap_or(usize::MAX)
        };
        exercises.sort_by(|a, b| {
            position(a)
                .cmp(&position(b))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(exercises)
    }

    pub fn exercise_by_code(&self, code: &str) -> Result<Option<ExerciseId>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM exercise WHERE code = ?1", [code], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn add_custom_exercise(
        &self,
        name: &str,
        kind: ExerciseKind,
        muscle: Option<Muscle>,
    ) -> Result<ExerciseId> {
        self.conn.execute(
            "INSERT INTO exercise (code, name, kind, lift, muscle) VALUES (NULL, ?1, ?2, NULL, ?3)",
            params![name.trim(), kind_to_i64(kind), muscle.map(Muscle::index)],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // Sesiones

    pub fn start_session(&self, name: &str, started_at: Timestamp) -> Result<SessionId> {
        self.conn.execute(
            "INSERT INTO session (name, started_at) VALUES (?1, ?2)",
            params![name, started_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_set(&self, session: SessionId, exercise: ExerciseId, set: SetEntry) -> Result<()> {
        self.conn.execute(
            "INSERT INTO workout_set (session_id, exercise_id, reps, weight_kg, duration_s, set_type, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session,
                exercise,
                set.reps,
                set.weight_kg,
                set.duration_s,
                set.set_type.index(),
                set.completed_at
            ],
        )?;
        Ok(())
    }

    /// Cierra la sesión: calcula duración, récords, racha y XP, y guarda la XP.
    pub fn finish_session(
        &mut self,
        session: SessionId,
        ended_at: Timestamp,
    ) -> Result<FinishedSession> {
        let started_at: Timestamp = self.conn.query_row(
            "SELECT started_at FROM session WHERE id = ?1",
            [session],
            |row| row.get(0),
        )?;
        let duration_min = u32::try_from((ended_at - started_at).max(0) / 60).unwrap_or(u32::MAX);
        let bodyweight = self.profile()?.and_then(|p| p.bodyweight_kg);
        let total_xp_before = self.total_xp()?;

        let records = self.records_in(session, started_at, bodyweight)?;
        let sessions_today = self.finished_sessions_on_day(ended_at)?;

        // Calentamiento (1) y aproximación (5) no son series de trabajo: ver SetType::counts_as_work.
        let (sets, volume_kg) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN e.kind = 0 AND s.set_type NOT IN (1, 5) THEN s.reps * s.weight_kg END), 0)
             FROM workout_set s JOIN exercise e ON e.id = s.exercise_id WHERE s.session_id = ?1",
            [session],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, f64>(1)?)),
        )?;

        self.conn.execute(
            "UPDATE session SET ended_at = ?1 WHERE id = ?2",
            params![ended_at, session],
        )?;
        let streak_weeks = self.week_stats(ended_at)?.streak_weeks;
        let summary = SessionSummary {
            duration_min,
            personal_records: records.len() as u32,
        };
        let breakdown = ranks::xp_breakdown(summary, streak_weeks);
        // Solo las primeras sesiones del día dan XP (ver ranks::MAX_SESSIONS_PER_DAY).
        let over_daily_cap = sessions_today >= ranks::MAX_SESSIONS_PER_DAY;
        let xp = if over_daily_cap { 0 } else { breakdown.total };
        self.conn.execute(
            "UPDATE session SET xp = ?1 WHERE id = ?2",
            params![xp, session],
        )?;

        Ok(FinishedSession {
            duration_min,
            sets,
            volume_kg,
            records,
            streak_weeks,
            breakdown,
            over_daily_cap,
            xp,
            total_xp_before,
            total_xp_after: total_xp_before + u64::from(xp),
        })
    }

    /// Sesión empezada y no terminada, si la hay (por ejemplo, si la app se cerró
    /// en medio de un entrenamiento).
    pub fn active_session(&self) -> Result<Option<ActiveSession>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, started_at, routine_id FROM session WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(ActiveSession {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        started_at: row.get(2)?,
                        routine_id: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn session_sets(&self, session: SessionId) -> Result<Vec<LoggedSet>> {
        let mut stmt = self.conn.prepare(
            "SELECT exercise_id, reps, weight_kg, duration_s, set_type, completed_at FROM workout_set WHERE session_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([session], |row| {
            Ok(LoggedSet {
                exercise_id: row.get(0)?,
                set: SetEntry {
                    reps: row.get(1)?,
                    weight_kg: row.get(2)?,
                    duration_s: row.get(3)?,
                    set_type: SetType::from_index(row.get(4)?),
                    completed_at: row.get(5)?,
                },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Borra la última serie de la sesión (para corregir un error al anotar).
    pub fn remove_last_set(&self, session: SessionId) -> Result<bool> {
        let removed = self.conn.execute(
            "DELETE FROM workout_set WHERE id = (SELECT MAX(id) FROM workout_set WHERE session_id = ?1)",
            [session],
        )?;
        Ok(removed > 0)
    }

    /// Borra una sesión sin terminar junto con sus series.
    pub fn discard_session(&self, session: SessionId) -> Result<()> {
        self.conn.execute(
            "DELETE FROM session WHERE id = ?1 AND ended_at IS NULL",
            [session],
        )?;
        Ok(())
    }

    /// Últimas sesiones terminadas, de la más nueva a la más antigua.
    pub fn recent_sessions(&self, limit: u32) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT x.id, x.name, x.started_at, x.ended_at, x.xp, COUNT(s.id)
             FROM session x LEFT JOIN workout_set s ON s.session_id = x.id
             WHERE x.ended_at IS NOT NULL
             GROUP BY x.id ORDER BY x.ended_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                started_at: row.get(2)?,
                ended_at: row.get(3)?,
                xp: row.get(4)?,
                sets: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // Rutinas

    pub fn routines(&self) -> Result<Vec<Routine>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, start_min, duration_min FROM routine ORDER BY position, id",
        )?;
        let heads = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, RoutineId>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<Minute>>(2)?,
                    row.get::<_, u16>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        heads
            .into_iter()
            .map(|(id, name, start_min, duration_min)| {
                Ok(Routine {
                    id,
                    name,
                    weekdays: self.routine_days(id)?,
                    exercises: self.routine_exercises(id)?,
                    start_min,
                    duration_min,
                })
            })
            .collect()
    }

    pub fn routine(&self, id: RoutineId) -> Result<Option<Routine>> {
        Ok(self.routines()?.into_iter().find(|r| r.id == id))
    }

    /// Rutinas asignadas a un día de la semana (0 lunes ... 6 domingo).
    pub fn routines_for(&self, weekday: Weekday) -> Result<Vec<Routine>> {
        Ok(self
            .routines()?
            .into_iter()
            .filter(|r| r.weekdays.contains(&weekday))
            .collect())
    }

    fn routine_days(&self, id: RoutineId) -> Result<Vec<Weekday>> {
        let mut stmt = self
            .conn
            .prepare("SELECT weekday FROM routine_day WHERE routine_id = ?1 ORDER BY weekday")?;
        let days = stmt.query_map([id], |row| row.get::<_, Weekday>(0))?;
        Ok(days.collect::<rusqlite::Result<_>>()?)
    }

    fn routine_exercises(&self, id: RoutineId) -> Result<Vec<RoutineExercise>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, exercise_id, target_sets, reps_min, reps_max, weight_kg, rest_s
             FROM routine_exercise WHERE routine_id = ?1 ORDER BY position, id",
        )?;
        let rows = stmt.query_map([id], |row| {
            Ok(RoutineExercise {
                id: row.get(0)?,
                exercise_id: row.get(1)?,
                target: Target {
                    sets: row.get(2)?,
                    reps_min: row.get(3)?,
                    reps_max: row.get(4)?,
                    weight_kg: row.get(5)?,
                    rest_s: row.get(6)?,
                },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn create_routine(&self, name: &str, now: Timestamp) -> Result<RoutineId> {
        self.conn.execute(
            "INSERT INTO routine (name, position, created_at)
             VALUES (?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM routine), ?2)",
            params![name.trim(), now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_routine(&self, id: RoutineId, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE routine SET name = ?1 WHERE id = ?2",
            params![name.trim(), id],
        )?;
        Ok(())
    }

    /// Borra la rutina. Las sesiones hechas con ella se conservan.
    pub fn delete_routine(&self, id: RoutineId) -> Result<()> {
        self.conn
            .execute("DELETE FROM routine WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_routine_days(&mut self, id: RoutineId, weekdays: &[Weekday]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM routine_day WHERE routine_id = ?1", [id])?;
        for day in weekdays.iter().filter(|d| **d <= 6) {
            tx.execute(
                "INSERT OR IGNORE INTO routine_day (routine_id, weekday) VALUES (?1, ?2)",
                params![id, day],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn add_routine_exercise(
        &self,
        routine: RoutineId,
        exercise: ExerciseId,
    ) -> Result<RoutineExerciseId> {
        // La meta inicial sale del catálogo (un básico va a 6–8 con descanso largo,
        // un aislamiento a 10–15 con descanso corto).
        let code: Option<String> = self.conn.query_row(
            "SELECT code FROM exercise WHERE id = ?1",
            [exercise],
            |row| row.get(0),
        )?;
        let t = match code.as_deref().and_then(catalog::find) {
            Some(c) => Target {
                reps_min: c.reps_min,
                reps_max: c.reps_max,
                rest_s: c.rest_s,
                ..Target::default()
            },
            None => Target::default(),
        };
        self.conn.execute(
            "INSERT INTO routine_exercise (routine_id, exercise_id, position, target_sets, reps_min, reps_max, weight_kg, rest_s)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), -1) + 1 FROM routine_exercise WHERE routine_id = ?1),
                     ?3, ?4, ?5, ?6, ?7)",
            params![routine, exercise, t.sets, t.reps_min, t.reps_max, t.weight_kg, t.rest_s],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_routine_exercise(&self, id: RoutineExerciseId, target: Target) -> Result<()> {
        self.conn.execute(
            "UPDATE routine_exercise SET target_sets = ?1, reps_min = ?2, reps_max = ?3, weight_kg = ?4, rest_s = ?5
             WHERE id = ?6",
            params![target.sets, target.reps_min, target.reps_max, target.weight_kg, target.rest_s, id],
        )?;
        Ok(())
    }

    pub fn remove_routine_exercise(&self, id: RoutineExerciseId) -> Result<()> {
        self.conn
            .execute("DELETE FROM routine_exercise WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Mueve un ejercicio un lugar arriba (`-1`) o abajo (`1`) dentro de su rutina.
    pub fn move_routine_exercise(&mut self, id: RoutineExerciseId, step: i32) -> Result<()> {
        let routine: RoutineId = self.conn.query_row(
            "SELECT routine_id FROM routine_exercise WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        let mut order: Vec<RoutineExerciseId> = self
            .routine_exercises(routine)?
            .iter()
            .map(|e| e.id)
            .collect();
        let Some(from) = order.iter().position(|e| *e == id) else {
            return Ok(());
        };
        let to = from as i64 + i64::from(step);
        if !(0..order.len() as i64).contains(&to) {
            return Ok(());
        }
        order.swap(from, to as usize);
        let tx = self.conn.transaction()?;
        for (position, exercise) in order.iter().enumerate() {
            tx.execute(
                "UPDATE routine_exercise SET position = ?1 WHERE id = ?2",
                params![position as i64, exercise],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Empieza una sesión a partir de una rutina; toma su nombre.
    pub fn start_routine_session(
        &self,
        routine: RoutineId,
        started_at: Timestamp,
    ) -> Result<SessionId> {
        self.conn.execute(
            "INSERT INTO session (name, started_at, routine_id) SELECT name, ?1, id FROM routine WHERE id = ?2",
            params![started_at, routine],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Última serie de trabajo hecha en un ejercicio (para proponer el peso de la siguiente).
    pub fn last_work_set(&self, exercise: ExerciseId) -> Result<Option<SetEntry>> {
        Ok(self
            .conn
            .query_row(
                "SELECT reps, weight_kg, duration_s, set_type, completed_at FROM workout_set
                 WHERE exercise_id = ?1 AND set_type NOT IN (1, 5) ORDER BY id DESC LIMIT 1",
                [exercise],
                |row| {
                    Ok(SetEntry {
                        reps: row.get(0)?,
                        weight_kg: row.get(1)?,
                        duration_s: row.get(2)?,
                        set_type: SetType::from_index(row.get(3)?),
                        completed_at: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// Hora fija de la rutina en el horario (`None` la deja sin hora).
    pub fn set_routine_time(
        &self,
        id: RoutineId,
        start_min: Option<Minute>,
        duration_min: u16,
    ) -> Result<()> {
        let duration = duration_min.clamp(10, 600);
        self.conn.execute(
            "UPDATE routine SET start_min = ?1, duration_min = ?2 WHERE id = ?3",
            params![start_min, duration, id],
        )?;
        Ok(())
    }

    // Horario

    pub fn events(&self) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, kind, start_min, end_min, on_day, reminder_min FROM event ORDER BY start_min, id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, EventId>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Minute>(3)?,
                    row.get::<_, Minute>(4)?,
                    row.get::<_, Option<DayNumber>>(5)?,
                    row.get::<_, Option<u32>>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut days_stmt = self
            .conn
            .prepare("SELECT weekday FROM event_day WHERE event_id = ?1 ORDER BY weekday")?;
        rows.into_iter()
            .map(
                |(id, title, kind, start_min, end_min, on_day, reminder_min)| {
                    let recurrence = match on_day {
                        Some(day) => Recurrence::Once(day),
                        None => Recurrence::Weekly(
                            days_stmt
                                .query_map([id], |row| row.get::<_, Weekday>(0))?
                                .collect::<rusqlite::Result<_>>()?,
                        ),
                    };
                    Ok(Event {
                        id,
                        title,
                        kind: EventKind::from_index(kind),
                        start_min,
                        end_min,
                        recurrence,
                        reminder_min,
                    })
                },
            )
            .collect()
    }

    /// Guarda un evento. Con `id` 0 lo crea; si no, lo reemplaza. Devuelve su id.
    pub fn save_event(&mut self, event: &Event, now: Timestamp) -> Result<EventId> {
        let on_day = match &event.recurrence {
            Recurrence::Once(day) => Some(*day),
            Recurrence::Weekly(_) => None,
        };
        let tx = self.conn.transaction()?;
        let id = if event.id == 0 {
            tx.execute(
                "INSERT INTO event (title, kind, start_min, end_min, on_day, reminder_min, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![event.title.trim(), event.kind.index(), event.start_min, event.end_min, on_day, event.reminder_min, now],
            )?;
            tx.last_insert_rowid()
        } else {
            tx.execute(
                "UPDATE event SET title = ?1, kind = ?2, start_min = ?3, end_min = ?4, on_day = ?5, reminder_min = ?6
                 WHERE id = ?7",
                params![event.title.trim(), event.kind.index(), event.start_min, event.end_min, on_day, event.reminder_min, event.id],
            )?;
            event.id
        };
        tx.execute("DELETE FROM event_day WHERE event_id = ?1", [id])?;
        if let Recurrence::Weekly(days) = &event.recurrence {
            for day in days.iter().filter(|d| **d <= 6) {
                tx.execute(
                    "INSERT OR IGNORE INTO event_day (event_id, weekday) VALUES (?1, ?2)",
                    params![id, day],
                )?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn delete_event(&self, id: EventId) -> Result<()> {
        self.conn.execute("DELETE FROM event WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn total_xp(&self) -> Result<u64> {
        let total: i64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(xp), 0) FROM session", [], |row| {
                    row.get(0)
                })?;
        Ok(total.max(0) as u64)
    }

    /// Sesiones de la semana actual y racha de semanas, a la fecha `now`.
    pub fn week_stats(&self, now: Timestamp) -> Result<WeekStats> {
        let goal = self.profile()?.map_or(3, |p| p.weekly_goal);
        let current = week_index(now);
        let mut counts = vec![0u32; STREAK_LOOKBACK_WEEKS as usize + 1];
        let mut stmt = self.conn.prepare(
            "SELECT ended_at FROM session WHERE ended_at IS NOT NULL AND ended_at <= ?1",
        )?;
        let ends = stmt.query_map([now], |row| row.get::<_, Timestamp>(0))?;
        for end in ends {
            let back = current - week_index(end?);
            if (0..=STREAK_LOOKBACK_WEEKS).contains(&back) {
                counts[(STREAK_LOOKBACK_WEEKS - back) as usize] += 1;
            }
        }
        Ok(WeekStats {
            sessions: *counts.last().unwrap_or(&0),
            streak_weeks: ranks::streak_weeks(&counts, goal),
        })
    }

    fn finished_sessions_on_day(&self, at: Timestamp) -> Result<usize> {
        let day = local_date(at);
        let mut stmt = self
            .conn
            .prepare("SELECT ended_at FROM session WHERE ended_at IS NOT NULL AND xp > 0")?;
        let ends = stmt.query_map([], |row| row.get::<_, Timestamp>(0))?;
        let mut count = 0;
        for end in ends {
            if local_date(end?) == day {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Ejercicios de la sesión cuyo 1RM estimado supera al mejor anterior.
    /// La primera vez que se hace un ejercicio no cuenta como récord.
    fn records_in(
        &self,
        session: SessionId,
        started_at: Timestamp,
        bodyweight: Option<f64>,
    ) -> Result<Vec<PersonalRecord>> {
        let best_now = self.best_e1rm(
            "SELECT s.exercise_id, s.reps, s.weight_kg, e.kind FROM workout_set s
             JOIN exercise e ON e.id = s.exercise_id WHERE s.session_id = ?1 AND s.set_type NOT IN (1, 5)",
            session,
            bodyweight,
        )?;
        let best_before = self.best_e1rm(
            "SELECT s.exercise_id, s.reps, s.weight_kg, e.kind FROM workout_set s
             JOIN exercise e ON e.id = s.exercise_id
             JOIN session x ON x.id = s.session_id
             WHERE x.ended_at IS NOT NULL AND x.ended_at <= ?1 AND s.set_type NOT IN (1, 5)",
            started_at,
            bodyweight,
        )?;
        let mut records: Vec<PersonalRecord> = best_now
            .into_iter()
            .filter_map(|(exercise_id, new_kg)| {
                let previous_kg = *best_before.get(&exercise_id)?;
                (new_kg > previous_kg + 1e-9).then_some(PersonalRecord {
                    exercise_id,
                    previous_kg,
                    new_kg,
                })
            })
            .collect();
        records.sort_by_key(|r| r.exercise_id);
        Ok(records)
    }

    fn best_e1rm(
        &self,
        sql: &str,
        param: i64,
        bodyweight: Option<f64>,
    ) -> Result<HashMap<ExerciseId, f64>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([param], |row| {
            Ok((
                row.get::<_, ExerciseId>(0)?,
                row.get::<_, Option<u32>>(1)?,
                row.get::<_, Option<f64>>(2)?,
                kind_from_i64(row.get(3)?),
            ))
        })?;
        let mut best = HashMap::new();
        for row in rows {
            let (exercise, reps, weight, kind) = row?;
            let load = match kind {
                ExerciseKind::Weighted => weight,
                ExerciseKind::Bodyweight => bodyweight.map(|bw| bw + weight.unwrap_or(0.0)),
                ExerciseKind::Timed => None,
            };
            if let (Some(load), Some(reps)) = (load, reps)
                && let Some(e1rm) = ranks::estimated_one_rep_max(load, reps)
            {
                let entry = best.entry(exercise).or_insert(e1rm);
                if e1rm > *entry {
                    *entry = e1rm;
                }
            }
        }
        Ok(best)
    }

    pub fn schema_version(&self) -> Result<u32> {
        migrations::current_version(&self.conn)
    }
}

/// Copia el archivo antes de migrar, con la API de respaldo de SQLite
/// (así la copia es consistente aunque la base esté abierta).
fn backup(conn: &Connection, path: &Path, from_version: u32) -> Result<PathBuf> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mog.db");
    let target = path.with_file_name(format!("{file}.v{from_version}-{stamp}.bak"));
    conn.backup(
        rusqlite::MAIN_DB,
        &target,
        None::<fn(rusqlite::backup::Progress)>,
    )?;
    Ok(target)
}

/// Semana local como número continuo (semanas desde una fecha fija, empezando en lunes).
fn week_index(at: Timestamp) -> i64 {
    let date = local_date(at);
    let days = i64::from(date.num_days_from_ce());
    let monday = days - i64::from(date.weekday().num_days_from_monday());
    monday.div_euclid(7)
}

fn local_date(at: Timestamp) -> chrono::NaiveDate {
    Local
        .timestamp_opt(at, 0)
        .single()
        .map(|dt| dt.date_naive())
        .unwrap_or_default()
}

fn kind_to_i64(kind: ExerciseKind) -> i64 {
    match kind {
        ExerciseKind::Weighted => 0,
        ExerciseKind::Bodyweight => 1,
        ExerciseKind::Timed => 2,
    }
}

fn kind_from_i64(value: i64) -> ExerciseKind {
    match value {
        1 => ExerciseKind::Bodyweight,
        2 => ExerciseKind::Timed,
        _ => ExerciseKind::Weighted,
    }
}

const LIFTS: [Lift; 6] = [
    Lift::Squat,
    Lift::BenchPress,
    Lift::Deadlift,
    Lift::OverheadPress,
    Lift::BarbellRow,
    Lift::PullUp,
];

fn lift_to_i64(lift: Lift) -> i64 {
    LIFTS.iter().position(|l| *l == lift).unwrap_or(0) as i64
}

fn lift_from_i64(value: i64) -> Option<Lift> {
    usize::try_from(value)
        .ok()
        .and_then(|i| LIFTS.get(i).copied())
}

fn body_profile_to_i64(profile: BodyProfile) -> i64 {
    match profile {
        BodyProfile::Male => 0,
        BodyProfile::Female => 1,
        BodyProfile::Unspecified => 2,
    }
}

fn body_profile_from_i64(value: i64) -> BodyProfile {
    match value {
        0 => BodyProfile::Male,
        1 => BodyProfile::Female,
        _ => BodyProfile::Unspecified,
    }
}

#[cfg(test)]
mod tests;
