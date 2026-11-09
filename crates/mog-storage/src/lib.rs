//! Base de datos local de mog 'em all.
//!
//! Guarda el perfil, los ejercicios y las sesiones de entrenamiento en un archivo
//! SQLite en el equipo del usuario. Los cálculos (XP, récords, rachas) los hace
//! `mog-core`; este crate solo lee, escribe y junta los datos que esos cálculos piden.

mod migrations;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, TimeZone};
use mog_core::ranks::{self, BodyProfile, Lift, SessionSummary};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExerciseKind {
    /// Carga externa: peso y repeticiones.
    Weighted,
    /// Peso corporal, con lastre opcional.
    Bodyweight,
    /// Se mide en tiempo (plancha, correr).
    Timed,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SetEntry {
    pub reps: Option<u32>,
    /// En ejercicios de peso corporal es el lastre.
    pub weight_kg: Option<f64>,
    pub duration_s: Option<u32>,
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

/// Ejercicios que vienen con la app. El código es estable; no se cambia nunca.
const BUILTIN_EXERCISES: &[(&str, ExerciseKind, Option<Lift>)] = &[
    ("squat", ExerciseKind::Weighted, Some(Lift::Squat)),
    (
        "bench_press",
        ExerciseKind::Weighted,
        Some(Lift::BenchPress),
    ),
    ("deadlift", ExerciseKind::Weighted, Some(Lift::Deadlift)),
    (
        "overhead_press",
        ExerciseKind::Weighted,
        Some(Lift::OverheadPress),
    ),
    (
        "barbell_row",
        ExerciseKind::Weighted,
        Some(Lift::BarbellRow),
    ),
    ("pull_up", ExerciseKind::Bodyweight, Some(Lift::PullUp)),
    ("dip", ExerciseKind::Bodyweight, None),
    ("push_up", ExerciseKind::Bodyweight, None),
    ("plank", ExerciseKind::Timed, None),
    ("run", ExerciseKind::Timed, None),
];

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

    fn seed_exercises(&self) -> Result<()> {
        for (code, kind, lift) in BUILTIN_EXERCISES {
            self.conn.execute(
                "INSERT OR IGNORE INTO exercise (code, name, kind, lift) VALUES (?1, NULL, ?2, ?3)",
                params![code, kind_to_i64(*kind), lift.map(lift_to_i64)],
            )?;
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
            .prepare("SELECT id, code, name, kind, lift FROM exercise ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok(Exercise {
                id: row.get(0)?,
                code: row.get(1)?,
                name: row.get(2)?,
                kind: kind_from_i64(row.get(3)?),
                lift: row.get::<_, Option<i64>>(4)?.and_then(lift_from_i64),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn exercise_by_code(&self, code: &str) -> Result<Option<ExerciseId>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM exercise WHERE code = ?1", [code], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn add_custom_exercise(&self, name: &str, kind: ExerciseKind) -> Result<ExerciseId> {
        self.conn.execute(
            "INSERT INTO exercise (code, name, kind, lift) VALUES (NULL, ?1, ?2, NULL)",
            params![name.trim(), kind_to_i64(kind)],
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
            "INSERT INTO workout_set (session_id, exercise_id, reps, weight_kg, duration_s) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session, exercise, set.reps, set.weight_kg, set.duration_s],
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

        let (sets, volume_kg) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN e.kind = 0 THEN s.reps * s.weight_kg END), 0)
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
                "SELECT id, name, started_at FROM session WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(ActiveSession {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        started_at: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn session_sets(&self, session: SessionId) -> Result<Vec<LoggedSet>> {
        let mut stmt = self.conn.prepare(
            "SELECT exercise_id, reps, weight_kg, duration_s FROM workout_set WHERE session_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([session], |row| {
            Ok(LoggedSet {
                exercise_id: row.get(0)?,
                set: SetEntry {
                    reps: row.get(1)?,
                    weight_kg: row.get(2)?,
                    duration_s: row.get(3)?,
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
             JOIN exercise e ON e.id = s.exercise_id WHERE s.session_id = ?1",
            session,
            bodyweight,
        )?;
        let best_before = self.best_e1rm(
            "SELECT s.exercise_id, s.reps, s.weight_kg, e.kind FROM workout_set s
             JOIN exercise e ON e.id = s.exercise_id
             JOIN session x ON x.id = s.session_id
             WHERE x.ended_at IS NOT NULL AND x.ended_at <= ?1",
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
