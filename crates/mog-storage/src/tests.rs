use super::*;

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
    let exercises = store.exercises().unwrap();
    assert_eq!(exercises.len(), BUILTIN_EXERCISES.len());
    let bench = exercises
        .iter()
        .find(|e| e.code.as_deref() == Some("bench_press"))
        .unwrap();
    assert_eq!(bench.lift, Some(Lift::BenchPress));
    // Volver a sembrar no duplica.
    store.seed_exercises().unwrap();
    assert_eq!(store.exercises().unwrap().len(), BUILTIN_EXERCISES.len());
}

#[test]
fn reopening_a_file_keeps_data() {
    let dir = std::env::temp_dir().join(format!("mog-test-{}", std::process::id()));
    let path = dir.join("mog.db");
    {
        let store = Store::open(&path).unwrap();
        store
            .add_custom_exercise("Curl martillo", ExerciseKind::Weighted)
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
