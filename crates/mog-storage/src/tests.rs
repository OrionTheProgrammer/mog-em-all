use super::*;
use mog_core::finance::{EntryKind, IncomeType};
use mog_core::tasks::{Priority, Repeat};

const HOUR: Timestamp = 3600;
const DAY: Timestamp = 24 * HOUR;
const WEEK: Timestamp = 7 * DAY;
/// Lunes 2 de noviembre de 2026, 18:00 UTC. Lejos de cambios de día en cualquier zona horaria común.
const MONDAY: Timestamp = 1_793_642_400;

fn store_with_profile() -> Store {
    let store = Store::open_in_memory().unwrap();
    store
        .save_profile(
            &Profile {
                name: "Orion".into(),
                bodyweight_kg: Some(80.0),
                height_cm: Some(176.0),
                body_profile: BodyProfile::Male,
                weekly_goal: 3,
            },
            MONDAY,
        )
        .unwrap();
    store
}

fn workout(
    store: &mut Store,
    start: Timestamp,
    minutes: i64,
    exercise: ExerciseId,
    sets: &[(u32, f64)],
) -> FinishedSession {
    let session = store.start_session("Empuje A", start).unwrap();
    for &(reps, weight) in sets {
        store
            .add_set(
                session,
                exercise,
                SetEntry {
                    reps: Some(reps),
                    weight_kg: Some(weight),
                    duration_s: None,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    store.finish_session(session, start + minutes * 60).unwrap()
}

#[test]
fn migrations_run_once_and_seed_exercises() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.schema_version().unwrap(), migrations::latest());
    assert_eq!(
        store.migration_history().unwrap().len(),
        migrations::MIGRATIONS.len()
    );
    let exercises = store.exercises().unwrap();
    assert_eq!(exercises.len(), catalog::all().len());
    let bench = exercises
        .iter()
        .find(|e| e.code.as_deref() == Some("bench_press"))
        .unwrap();
    assert_eq!(bench.lift, Some(Lift::BenchPress));
    // Volver a sembrar no duplica.
    store.seed_exercises().unwrap();
    assert_eq!(store.exercises().unwrap().len(), catalog::all().len());
}

#[test]
fn reopening_a_file_keeps_data() {
    let dir = std::env::temp_dir().join(format!("mog-test-{}", std::process::id()));
    let path = dir.join("mog.db");
    {
        let store = Store::open(&path).unwrap();
        store
            .add_custom_exercise(
                "Curl martillo",
                ExerciseKind::Weighted,
                Some(Muscle::Biceps),
            )
            .unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert!(
        store
            .exercises()
            .unwrap()
            .iter()
            .any(|e| e.name.as_deref() == Some("Curl martillo"))
    );
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn profile_round_trip() {
    let store = store_with_profile();
    let profile = store.profile().unwrap().unwrap();
    assert_eq!(profile.name, "Orion");
    assert_eq!(profile.weekly_goal, 3);
    let updated = Profile {
        weekly_goal: 4,
        ..profile
    };
    store.save_profile(&updated, MONDAY + DAY).unwrap();
    assert_eq!(store.profile().unwrap().unwrap().weekly_goal, 4);
}

#[test]
fn finishing_a_session_gives_xp() {
    let mut store = store_with_profile();
    let bench = store.exercise_by_code("bench_press").unwrap().unwrap();
    let done = workout(&mut store, MONDAY, 45, bench, &[(8, 60.0), (8, 60.0)]);
    assert_eq!(done.duration_min, 45);
    assert!(done.records.is_empty(), "la primera vez no es récord");
    assert_eq!(done.xp, 50 + 45);
    assert_eq!(store.total_xp().unwrap(), 95);
}

#[test]
fn records_compare_against_previous_sessions() {
    let mut store = store_with_profile();
    let bench = store.exercise_by_code("bench_press").unwrap().unwrap();
    workout(&mut store, MONDAY, 40, bench, &[(5, 70.0)]);
    let better = workout(&mut store, MONDAY + DAY, 40, bench, &[(5, 72.5)]);
    assert_eq!(better.records.len(), 1);
    let record = &better.records[0];
    assert!(record.new_kg > record.previous_kg);
    assert_eq!(better.xp, 50 + 40 + 25);
    let same = workout(&mut store, MONDAY + 2 * DAY, 40, bench, &[(5, 72.5)]);
    assert!(same.records.is_empty());
}

#[test]
fn bodyweight_lifts_use_profile_weight() {
    let mut store = store_with_profile();
    let pull_up = store.exercise_by_code("pull_up").unwrap().unwrap();
    workout(&mut store, MONDAY, 30, pull_up, &[(8, 0.0)]);
    let weighted = workout(&mut store, MONDAY + DAY, 30, pull_up, &[(8, 5.0)]);
    assert_eq!(weighted.records.len(), 1);
}

#[test]
fn only_two_sessions_per_day_give_xp() {
    let mut store = store_with_profile();
    let squat = store.exercise_by_code("squat").unwrap().unwrap();
    assert!(workout(&mut store, MONDAY, 30, squat, &[(5, 100.0)]).xp > 0);
    assert!(workout(&mut store, MONDAY + HOUR, 30, squat, &[(5, 100.0)]).xp > 0);
    assert_eq!(
        workout(&mut store, MONDAY + 2 * HOUR, 30, squat, &[(5, 100.0)]).xp,
        0
    );
}

#[test]
fn streak_follows_weekly_goal() {
    let mut store = store_with_profile();
    let squat = store.exercise_by_code("squat").unwrap().unwrap();
    // Dos semanas completas (3 sesiones cada una).
    for week in 0..2 {
        for day in 0..3 {
            workout(
                &mut store,
                MONDAY + week * WEEK + day * DAY,
                30,
                squat,
                &[(5, 100.0)],
            );
        }
    }
    let now = MONDAY + 2 * WEEK + HOUR;
    let stats = store.week_stats(now).unwrap();
    assert_eq!(
        stats,
        WeekStats {
            sessions: 0,
            streak_weeks: 2
        }
    );
    // Con la racha de 2 semanas, la siguiente sesión tiene +20 %.
    let done = workout(&mut store, now, 30, squat, &[(5, 100.0)]);
    assert_eq!(done.streak_weeks, 2);
    assert_eq!(done.xp, (50 + 30) * 120 / 100);
    // Una semana sin entrenar corta la racha.
    let later = store.week_stats(MONDAY + 4 * WEEK).unwrap();
    assert_eq!(later.streak_weeks, 0);
}

// Versionado del esquema

fn temp_db(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("mog-schema-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mog.db");
    (dir, path)
}

/// Migración de prueba que va después de la última real.
const NEXT: migrations::Migration = migrations::Migration {
    version: 7,
    name: "profile_nickname",
    sql: "ALTER TABLE profile ADD COLUMN nickname TEXT;",
};

fn with_next() -> Vec<migrations::Migration> {
    let mut list = migrations::MIGRATIONS.to_vec();
    assert_eq!(
        NEXT.version,
        migrations::latest() + 1,
        "actualizar NEXT al agregar migraciones"
    );
    list.push(NEXT);
    list
}

#[test]
fn upgrade_keeps_data_and_makes_a_backup() {
    let (dir, path) = temp_db("upgrade");
    {
        let mut store = Store::open(&path).unwrap();
        store
            .save_profile(
                &Profile {
                    name: "Orion".into(),
                    bodyweight_kg: Some(80.0),
                    height_cm: None,
                    body_profile: BodyProfile::Male,
                    weekly_goal: 3,
                },
                MONDAY,
            )
            .unwrap();
        let squat = store.exercise_by_code("squat").unwrap().unwrap();
        workout(&mut store, MONDAY, 30, squat, &[(5, 100.0)]);
    }
    let store = Store::open_with(&path, &with_next()).unwrap();
    assert_eq!(store.schema_version().unwrap(), NEXT.version);
    assert_eq!(store.profile().unwrap().unwrap().name, "Orion");
    assert!(store.total_xp().unwrap() > 0);
    let history = store.migration_history().unwrap();
    assert_eq!(history.last().unwrap().version, NEXT.version);
    let backups: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains(&format!(".v{}-", migrations::latest()))
        })
        .collect();
    assert_eq!(
        backups.len(),
        1,
        "debe quedar una copia de la versión anterior"
    );
    let old = Connection::open(backups[0].path()).unwrap();
    assert_eq!(
        migrations::current_version(&old).unwrap(),
        migrations::latest()
    );
    drop((store, old));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn no_backup_when_nothing_to_migrate() {
    let (dir, path) = temp_db("nobackup");
    drop(Store::open(&path).unwrap());
    drop(Store::open(&path).unwrap());
    let files = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(files, 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn newer_schema_is_refused_and_left_untouched() {
    let (dir, path) = temp_db("newer");
    drop(Store::open_with(&path, &with_next()).unwrap());
    match Store::open(&path) {
        Err(Error::NewerSchema { found, supported })
            if found == NEXT.version && supported == migrations::latest() => {}
        Err(other) => panic!("error inesperado: {other}"),
        Ok(_) => panic!("debió rechazar la base"),
    }
    let conn = Connection::open(&path).unwrap();
    assert_eq!(migrations::current_version(&conn).unwrap(), NEXT.version);
    drop(conn);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn edited_migration_is_detected() {
    let (dir, path) = temp_db("edited");
    drop(Store::open(&path).unwrap());
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE schema_migrations SET checksum = 'otra' WHERE version = 1",
            [],
        )
        .unwrap();
    }
    assert!(matches!(
        Store::open(&path),
        Err(Error::ModifiedMigration { version: 1 })
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn databases_from_before_the_history_table_are_adopted() {
    let (dir, path) = temp_db("legacy");
    {
        // Así quedaban las bases creadas antes de schema_migrations: solo user_version.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(migrations::MIGRATIONS[0].sql).unwrap();
        conn.pragma_update(None, "user_version", 1i64).unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), migrations::latest());
    assert_eq!(
        store.migration_history().unwrap().len(),
        migrations::MIGRATIONS.len()
    );
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn migration_files_are_numbered_without_gaps() {
    for (index, migration) in migrations::MIGRATIONS.iter().enumerate() {
        assert_eq!(migration.version as usize, index + 1);
        assert!(!migration.sql.trim().is_empty());
    }
}

#[test]
fn active_session_can_be_resumed_or_discarded() {
    let store = store_with_profile();
    assert_eq!(store.active_session().unwrap(), None);
    let bench = store.exercise_by_code("bench_press").unwrap().unwrap();
    let id = store.start_session("Empuje A", MONDAY).unwrap();
    store
        .add_set(
            id,
            bench,
            SetEntry {
                reps: Some(8),
                weight_kg: Some(60.0),
                duration_s: None,
                ..Default::default()
            },
        )
        .unwrap();
    store
        .add_set(
            id,
            bench,
            SetEntry {
                reps: Some(6),
                weight_kg: Some(62.5),
                duration_s: None,
                ..Default::default()
            },
        )
        .unwrap();
    let active = store.active_session().unwrap().unwrap();
    assert_eq!((active.id, active.name.as_str()), (id, "Empuje A"));
    assert_eq!(store.session_sets(id).unwrap().len(), 2);
    assert!(store.remove_last_set(id).unwrap());
    assert_eq!(store.session_sets(id).unwrap().len(), 1);
    store.discard_session(id).unwrap();
    assert_eq!(store.active_session().unwrap(), None);
    assert!(
        store.session_sets(id).unwrap().is_empty(),
        "las series se borran con la sesión"
    );
}

#[test]
fn finished_session_reports_volume_and_breakdown() {
    let mut store = store_with_profile();
    let bench = store.exercise_by_code("bench_press").unwrap().unwrap();
    let done = workout(
        &mut store,
        MONDAY,
        52,
        bench,
        &[(8, 60.0), (8, 60.0), (6, 65.0)],
    );
    assert_eq!(done.sets, 3);
    assert!((done.volume_kg - (480.0 + 480.0 + 390.0)).abs() < 1e-9);
    assert_eq!(done.breakdown.minutes, 52);
    assert_eq!(done.xp, done.breakdown.total);
    let recent = store.recent_sessions(5).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!((recent[0].sets, recent[0].xp), (3, done.xp));
}

#[test]
fn third_session_of_the_day_is_flagged() {
    let mut store = store_with_profile();
    let squat = store.exercise_by_code("squat").unwrap().unwrap();
    workout(&mut store, MONDAY, 30, squat, &[(5, 100.0)]);
    workout(&mut store, MONDAY + HOUR, 30, squat, &[(5, 100.0)]);
    let third = workout(&mut store, MONDAY + 2 * HOUR, 30, squat, &[(5, 100.0)]);
    assert!(third.over_daily_cap);
    assert!(third.breakdown.total > 0);
    assert_eq!(third.xp, 0);
}

#[test]
fn version_1_databases_upgrade_to_routines_keeping_sessions() {
    let (dir, path) = temp_db("v1to2");
    {
        // Datos escritos como los dejaba la versión 1, con SQL directo.
        let store = Store::open_with(&path, &migrations::MIGRATIONS[..1]).unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);
        store
            .conn
            .execute_batch(
                "INSERT INTO session (id, name, started_at, ended_at, xp) VALUES (7, 'Antigua', 1000, 3400, 90);
                 INSERT INTO workout_set (session_id, exercise_id, reps, weight_kg) VALUES (7, 2, 5, 70);",
            )
            .unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), migrations::latest());
    assert_eq!(store.total_xp().unwrap(), 90);
    let recent = store.recent_sessions(5).unwrap();
    assert_eq!((recent.len(), recent[0].sets), (1, 1));
    assert!(store.routines().unwrap().is_empty());
    let sets = store.session_sets(recent[0].id).unwrap();
    assert_eq!(
        sets[0].set.set_type,
        SetType::Normal,
        "las series viejas quedan como normales"
    );
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}

// Rutinas

#[test]
fn routines_with_days_and_ordered_exercises() {
    let mut store = store_with_profile();
    let bench = store.exercise_by_code("bench_press").unwrap().unwrap();
    let dip = store.exercise_by_code("dip").unwrap().unwrap();
    let press = store.exercise_by_code("overhead_press").unwrap().unwrap();
    let chest = store.create_routine("Pecho", MONDAY).unwrap();
    store.set_routine_days(chest, &[0, 3]).unwrap();
    let a = store.add_routine_exercise(chest, bench).unwrap();
    let b = store.add_routine_exercise(chest, dip).unwrap();
    let c = store.add_routine_exercise(chest, press).unwrap();
    store
        .update_routine_exercise(
            a,
            Target {
                sets: 4,
                reps_min: 6,
                reps_max: 8,
                weight_kg: Some(70.0),
                rest_s: 150,
            },
        )
        .unwrap();
    store.move_routine_exercise(c, -1).unwrap();
    store.move_routine_exercise(a, -1).unwrap(); // ya es el primero: no cambia

    let routine = store.routine(chest).unwrap().unwrap();
    assert_eq!(routine.weekdays, vec![0, 3]);
    assert_eq!(
        routine.exercises.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![a, c, b]
    );
    assert_eq!(routine.exercises[0].target.rest_s, 150);
    assert_eq!(store.routines_for(0).unwrap().len(), 1);
    assert!(store.routines_for(1).unwrap().is_empty());

    store.remove_routine_exercise(c).unwrap();
    store.set_routine_days(chest, &[2]).unwrap();
    let routine = store.routine(chest).unwrap().unwrap();
    assert_eq!(
        (routine.exercises.len(), routine.weekdays.clone()),
        (2, vec![2])
    );
}

#[test]
fn deleting_a_routine_keeps_its_sessions() {
    let mut store = store_with_profile();
    let squat = store.exercise_by_code("squat").unwrap().unwrap();
    let legs = store.create_routine("Pierna", MONDAY).unwrap();
    store.add_routine_exercise(legs, squat).unwrap();
    let session = store.start_routine_session(legs, MONDAY).unwrap();
    let active = store.active_session().unwrap().unwrap();
    assert_eq!(
        (active.name.as_str(), active.routine_id),
        ("Pierna", Some(legs))
    );
    store
        .add_set(
            session,
            squat,
            SetEntry {
                reps: Some(5),
                weight_kg: Some(100.0),
                ..Default::default()
            },
        )
        .unwrap();
    store.finish_session(session, MONDAY + 30 * 60).unwrap();
    store.delete_routine(legs).unwrap();
    assert!(store.routines().unwrap().is_empty());
    assert_eq!(store.recent_sessions(5).unwrap().len(), 1);
}

#[test]
fn warm_ups_do_not_count_for_records_or_volume() {
    let mut store = store_with_profile();
    let bench = store.exercise_by_code("bench_press").unwrap().unwrap();
    workout(&mut store, MONDAY, 40, bench, &[(5, 70.0)]);
    let session = store.start_session("Empuje", MONDAY + DAY).unwrap();
    let warm = SetEntry {
        reps: Some(10),
        weight_kg: Some(100.0),
        set_type: SetType::WarmUp,
        ..Default::default()
    };
    store.add_set(session, bench, warm).unwrap();
    store
        .add_set(
            session,
            bench,
            SetEntry {
                reps: Some(5),
                weight_kg: Some(70.0),
                set_type: SetType::Drop,
                completed_at: Some(MONDAY + DAY + 60),
                ..Default::default()
            },
        )
        .unwrap();
    let done = store
        .finish_session(session, MONDAY + DAY + 40 * 60)
        .unwrap();
    assert!(done.records.is_empty(), "el calentamiento no es récord");
    assert!((done.volume_kg - 350.0).abs() < 1e-9);
    assert_eq!(done.sets, 2);
    let last = store.last_work_set(bench).unwrap().unwrap();
    assert_eq!(
        (last.set_type, last.completed_at),
        (SetType::Drop, Some(MONDAY + DAY + 60))
    );
}

#[test]
fn catalog_exercises_bring_their_muscle_and_default_target() {
    let store = store_with_profile();
    let exercises = store.exercises().unwrap();
    let curl = exercises
        .iter()
        .find(|e| e.code.as_deref() == Some("hammer_curl"))
        .unwrap();
    assert_eq!(curl.muscle, Some(Muscle::Biceps));
    let custom = store
        .add_custom_exercise("Curl araña", ExerciseKind::Weighted, Some(Muscle::Biceps))
        .unwrap();
    let custom = store
        .exercises()
        .unwrap()
        .into_iter()
        .find(|e| e.id == custom)
        .unwrap();
    assert_eq!(
        (custom.muscle, custom.code.clone()),
        (Some(Muscle::Biceps), None)
    );

    let routine = store.create_routine("Brazos", MONDAY).unwrap();
    let deadlift = store.exercise_by_code("deadlift").unwrap().unwrap();
    let lateral = store.exercise_by_code("lateral_raise").unwrap().unwrap();
    store.add_routine_exercise(routine, deadlift).unwrap();
    store.add_routine_exercise(routine, lateral).unwrap();
    let r = store.routine(routine).unwrap().unwrap();
    assert_eq!(
        (r.exercises[0].target.reps_min, r.exercises[0].target.rest_s),
        (3, 180)
    );
    assert_eq!(
        (r.exercises[1].target.reps_max, r.exercises[1].target.rest_s),
        (15, 60)
    );
}

#[test]
fn approach_sets_are_not_work_sets() {
    let mut store = store_with_profile();
    let bench = store.exercise_by_code("bench_press").unwrap().unwrap();
    workout(&mut store, MONDAY, 40, bench, &[(5, 70.0)]);
    let session = store.start_session("Empuje", MONDAY + DAY).unwrap();
    let approach = SetEntry {
        reps: Some(3),
        weight_kg: Some(90.0),
        set_type: SetType::Approach,
        ..Default::default()
    };
    store.add_set(session, bench, approach).unwrap();
    let done = store
        .finish_session(session, MONDAY + DAY + 30 * 60)
        .unwrap();
    assert!(done.records.is_empty(), "la aproximación no es récord");
    assert_eq!(done.volume_kg, 0.0);
}

#[test]
fn sql_work_set_filter_matches_set_types() {
    // Las consultas filtran "set_type NOT IN (1, 5)"; si cambia SetType, hay que cambiarlas.
    let not_work: Vec<u8> = SetType::ALL
        .iter()
        .filter(|t| !t.counts_as_work())
        .map(|t| t.index())
        .collect();
    assert_eq!(not_work, vec![1, 5]);
}

// Horario

fn event(title: &str, start: Minute, end: Minute, recurrence: Recurrence) -> Event {
    Event {
        id: 0,
        title: title.into(),
        kind: EventKind::Class,
        start_min: start,
        end_min: end,
        recurrence,
        reminder_min: Some(10),
    }
}

#[test]
fn weekly_and_one_off_events_round_trip() {
    let mut store = store_with_profile();
    let class = store
        .save_event(
            &event("Cloud", 510, 600, Recurrence::Weekly(vec![0, 2])),
            MONDAY,
        )
        .unwrap();
    let exam = store
        .save_event(&event("Prueba", 900, 990, Recurrence::Once(20_770)), MONDAY)
        .unwrap();
    let events = store.events().unwrap();
    assert_eq!(events.len(), 2);
    let cloud = events.iter().find(|e| e.id == class).unwrap();
    assert_eq!(cloud.recurrence, Recurrence::Weekly(vec![0, 2]));
    assert_eq!(cloud.reminder_min, Some(10));
    assert_eq!(
        events.iter().find(|e| e.id == exam).unwrap().recurrence,
        Recurrence::Once(20_770)
    );

    // Editar: pasa de semanal a una vez y cambia de hora.
    let edited = Event {
        id: class,
        start_min: 540,
        recurrence: Recurrence::Once(20_766),
        ..cloud.clone()
    };
    assert_eq!(store.save_event(&edited, MONDAY).unwrap(), class);
    let cloud = store
        .events()
        .unwrap()
        .into_iter()
        .find(|e| e.id == class)
        .unwrap();
    assert_eq!(
        (cloud.start_min, cloud.recurrence),
        (540, Recurrence::Once(20_766))
    );

    store.delete_event(exam).unwrap();
    assert_eq!(store.events().unwrap().len(), 1);
}

#[test]
fn events_must_end_after_they_start() {
    let mut store = store_with_profile();
    assert!(
        store
            .save_event(&event("Mal", 600, 600, Recurrence::Weekly(vec![1])), MONDAY)
            .is_err()
    );
}

#[test]
fn routines_can_have_a_time() {
    let store = store_with_profile();
    let legs = store.create_routine("Pierna", MONDAY).unwrap();
    assert_eq!(store.routine(legs).unwrap().unwrap().start_min, None);
    store.set_routine_time(legs, Some(19 * 60), 75).unwrap();
    let r = store.routine(legs).unwrap().unwrap();
    assert_eq!((r.start_min, r.duration_min), (Some(1140), 75));
}

// Tareas

fn task(title: &str, due_day: Option<DayNumber>, priority: Priority) -> Task {
    Task {
        title: title.into(),
        due_day,
        priority,
        ..Default::default()
    }
}

#[test]
fn pending_tasks_are_ordered_by_date_then_priority() {
    let store = store_with_profile();
    let today = 20_766;
    store
        .save_task(&task("Sin fecha", None, Priority::High), MONDAY)
        .unwrap();
    store
        .save_task(&task("Mañana", Some(today + 1), Priority::Normal), MONDAY)
        .unwrap();
    store
        .save_task(&task("Hoy normal", Some(today), Priority::Normal), MONDAY)
        .unwrap();
    store
        .save_task(&task("Hoy alta", Some(today), Priority::High), MONDAY)
        .unwrap();
    let titles: Vec<String> = store
        .pending_tasks()
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(
        titles,
        vec!["Hoy alta", "Hoy normal", "Mañana", "Sin fecha"]
    );
}

#[test]
fn completing_and_reopening() {
    let mut store = store_with_profile();
    let id = store
        .save_task(&task("Informe", Some(20_766), Priority::Normal), MONDAY)
        .unwrap();
    assert_eq!(store.complete_task(id, MONDAY + 60, 20_766).unwrap(), None);
    assert!(store.pending_tasks().unwrap().is_empty());
    assert_eq!(store.done_tasks_since(MONDAY).unwrap().len(), 1);
    store.reopen_task(id).unwrap();
    assert_eq!(store.pending_tasks().unwrap().len(), 1);
}

#[test]
fn repeating_tasks_create_the_next_one() {
    let mut store = store_with_profile();
    let today = 20_766;
    let weekly = Task {
        repeat: Repeat::Weekly,
        due_min: Some(600),
        ..task("Regar", Some(today - 15), Priority::Low)
    };
    let id = store.save_task(&weekly, MONDAY).unwrap();
    let next = store.complete_task(id, MONDAY, today).unwrap().unwrap();
    let next = store.task(next).unwrap().unwrap();
    // Venía atrasada dos semanas: la siguiente cae desde hoy en adelante, no en el pasado.
    assert!(next.due_day.unwrap() >= today);
    assert_eq!((next.due_day.unwrap() - (today - 15)) % 7, 0);
    assert_eq!(
        (next.due_min, next.priority, next.repeat),
        (Some(600), Priority::Low, Repeat::Weekly)
    );
}

#[test]
fn editing_and_deleting_tasks() {
    let store = store_with_profile();
    let id = store
        .save_task(&task("Viejo", None, Priority::Normal), MONDAY)
        .unwrap();
    let edited = Task {
        id,
        title: "Nuevo".into(),
        tag: Some("  ".into()),
        ..task("", Some(20_770), Priority::High)
    };
    store.save_task(&edited, MONDAY).unwrap();
    let t = store.task(id).unwrap().unwrap();
    assert_eq!(
        (t.title.as_str(), t.due_day, t.tag),
        ("Nuevo", Some(20_770), None),
        "una etiqueta vacía no se guarda"
    );
    store.delete_task(id).unwrap();
    assert_eq!(store.task(id).unwrap(), None);
}

// Ajustes y finanzas

#[test]
fn settings_round_trip() {
    let store = store_with_profile();
    assert_eq!(store.setting("currency").unwrap(), None);
    store.set_setting("currency", "CLP").unwrap();
    store.set_setting("currency", "USD").unwrap();
    assert_eq!(store.setting("currency").unwrap().as_deref(), Some("USD"));
}

#[test]
fn default_categories_and_budgets() {
    let store = store_with_profile();
    let categories = store.categories().unwrap();
    assert_eq!(
        categories.len(),
        mog_core::finance::DEFAULT_CATEGORIES.len()
    );
    let food = categories
        .iter()
        .find(|c| c.code.as_deref() == Some("food"))
        .unwrap();
    store.set_category_budget(food.id, Some(150_000)).unwrap();
    let custom = store.add_category("Mascotas").unwrap();
    let categories = store.categories().unwrap();
    assert_eq!(
        categories.iter().find(|c| c.id == food.id).unwrap().budget,
        Some(150_000)
    );
    assert_eq!(categories.last().unwrap().id, custom);
    store.set_category_budget(food.id, None).unwrap();
    assert_eq!(store.categories().unwrap()[0].budget, None);
}

#[test]
fn entries_by_month_and_recurring_income() {
    let store = store_with_profile();
    let food = store.categories().unwrap()[0].id;
    let lunch = Entry {
        kind: EntryKind::Expense,
        amount: 12_500,
        category_id: Some(food),
        note: "almuerzo".into(),
        day: 20_766,
        ..Default::default()
    };
    store.add_entry(&lunch, MONDAY).unwrap();
    // Un ingreso no guarda categoría aunque se la pasen.
    let sale = Entry {
        kind: EntryKind::Income,
        amount: 20_000,
        category_id: Some(food),
        income_type: Some(IncomeType::Informal),
        note: "venta".into(),
        day: 20_770,
        ..Default::default()
    };
    store.add_entry(&sale, MONDAY).unwrap();
    store
        .add_entry(
            &Entry {
                day: 20_700,
                amount: 1,
                ..lunch.clone()
            },
            MONDAY,
        )
        .unwrap();

    let month = store.entries_between(20_758, 20_787).unwrap();
    assert_eq!(month.len(), 2);
    assert_eq!(month[0].note, "venta", "el más reciente primero");
    assert_eq!(
        (month[0].category_id, month[0].income_type),
        (None, Some(IncomeType::Informal))
    );

    let salary = store
        .save_recurring(
            &Recurring {
                kind: EntryKind::Income,
                amount: 650_000,
                note: "Sueldo".into(),
                day_of_month: 30,
                ..Default::default()
            },
            MONDAY,
        )
        .unwrap();
    let entry = store
        .register_recurring(salary, 20_787, MONDAY)
        .unwrap()
        .unwrap();
    let registered = store.entries_between(20_787, 20_787).unwrap();
    assert_eq!(
        (
            registered[0].id,
            registered[0].recurring_id,
            registered[0].income_type
        ),
        (entry, Some(salary), Some(IncomeType::Fixed))
    );

    store.delete_recurring(salary).unwrap();
    assert!(store.recurring().unwrap().is_empty());
    assert_eq!(
        store.entries_between(20_787, 20_787).unwrap()[0].recurring_id,
        None,
        "el movimiento queda, sin el vínculo"
    );
}
