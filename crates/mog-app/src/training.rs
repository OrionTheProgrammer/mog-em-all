//! Pestaña ENTRENAMIENTO: rutinas por día, sesiones, series y descanso.
//! Conecta el global `TrainingState` de la interfaz con la base de datos.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use chrono::{Datelike, Local, TimeZone};
use mog_core::catalog::{self, Muscle};
use mog_core::ranks::{self, Lift};
use mog_core::training::{SetType, Weekday};
use mog_storage::{
    Exercise, ExerciseId, ExerciseKind, FinishedSession, Routine, RoutineId, SetEntry, Store,
    Target,
};
use mog_ui::{
    ActiveSession, AppWindow, CurrentExercise, ExerciseItem, LevelUp, RecordRow, RoutineDetail,
    RoutineExerciseRow, RoutineItem, SessionRow, SetRow, TrainingState,
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

type SharedStore = Rc<RefCell<Store>>;

const RECENT_SESSIONS: u32 = 6;
/// Descanso para ejercicios que no están en la rutina de la sesión.
const DEFAULT_REST_S: u32 = 90;

/// Lo que la pestaña recuerda entre acciones y no está en la base de datos.
#[derive(Default)]
struct View {
    editing: Cell<Option<RoutineId>>,
    current: Cell<Option<ExerciseId>>,
}

struct Ctx {
    ui: slint::Weak<AppWindow>,
    store: SharedStore,
    view: Rc<View>,
}

impl Ctx {
    fn run(&self, action: impl FnOnce(&AppWindow, &mut Store, &View)) {
        if let Some(ui) = self.ui.upgrade() {
            let mut store = self.store.borrow_mut();
            action(&ui, &mut store, &self.view);
            refresh(&ui, &store, &self.view);
        }
    }
}

pub fn connect(ui: &AppWindow, store: &SharedStore) {
    let view = Rc::new(View::default());
    let ctx = Rc::new(Ctx {
        ui: ui.as_weak(),
        store: store.clone(),
        view: view.clone(),
    });
    let state = ui.global::<TrainingState>();

    // Rutinas

    state.on_create_routine({
        let ctx = ctx.clone();
        move |name| {
            ctx.run(|_, store, view| {
                let name = name.trim();
                if name.is_empty() {
                    return;
                }
                if let Some(id) = report(store.create_routine(name, crate::now())) {
                    view.editing.set(Some(id));
                }
            })
        }
    });
    state.on_select_routine({
        let ctx = ctx.clone();
        move |id| ctx.run(|_, _, view| view.editing.set(Some(i64::from(id))))
    });
    state.on_rename_routine({
        let ctx = ctx.clone();
        move |id, name| {
            ctx.run(|_, store, _| {
                if !name.trim().is_empty() {
                    report(store.rename_routine(i64::from(id), &name));
                }
            })
        }
    });
    state.on_delete_routine({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|_, store, view| {
                report(store.delete_routine(i64::from(id)));
                view.editing.set(None);
            })
        }
    });
    state.on_toggle_day({
        let ctx = ctx.clone();
        move |id, day| {
            ctx.run(|_, store, _| {
                let Some(routine) = report(store.routine(i64::from(id))).flatten() else {
                    return;
                };
                let day = day as Weekday;
                let mut days = routine.weekdays;
                match days.iter().position(|d| *d == day) {
                    Some(i) => {
                        days.remove(i);
                    }
                    None => days.push(day),
                }
                report(store.set_routine_days(routine.id, &days));
            })
        }
    });
    state.on_add_to_routine({
        let ctx = ctx.clone();
        move |routine, exercise| {
            ctx.run(|_, store, _| {
                report(store.add_routine_exercise(i64::from(routine), i64::from(exercise)));
            })
        }
    });
    state.on_remove_from_routine({
        let ctx = ctx.clone();
        move |row| {
            ctx.run(|_, store, _| {
                report(store.remove_routine_exercise(i64::from(row)));
            })
        }
    });
    state.on_move_in_routine({
        let ctx = ctx.clone();
        move |row, step| {
            ctx.run(|_, store, _| {
                report(store.move_routine_exercise(i64::from(row), step));
            })
        }
    });
    state.on_save_target({
        let ctx = ctx.clone();
        move |row, sets, reps_min, reps_max, weight, rest| {
            let Some(target) = parse_target(&sets, &reps_min, &reps_max, &weight, &rest) else {
                return false;
            };
            let mut saved = false;
            ctx.run(|_, store, _| {
                saved = report(store.update_routine_exercise(i64::from(row), target)).is_some()
            });
            saved
        }
    });

    // Biblioteca

    state.on_filter_changed({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, _| {})
    });
    state.on_create_exercise({
        let ctx = ctx.clone();
        move |name, kind, muscle| {
            let name = name.trim().to_string();
            if name.is_empty() || name.chars().count() > 60 {
                return false;
            }
            let kind = match kind {
                1 => ExerciseKind::Bodyweight,
                2 => ExerciseKind::Timed,
                _ => ExerciseKind::Weighted,
            };
            let mut created = false;
            ctx.run(|ui, store, _| {
                let muscle = Muscle::from_index(i64::from(muscle));
                created = report(store.add_custom_exercise(&name, kind, muscle)).is_some();
                if created {
                    // Mostrar el ejercicio nuevo aunque no coincida con la búsqueda escrita.
                    ui.global::<TrainingState>()
                        .set_filter_query(SharedString::new());
                }
            });
            created
        }
    });

    // Sesión

    state.on_start_routine({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|_, store, view| {
                if store.active_session().ok().flatten().is_none() {
                    report(store.start_routine_session(i64::from(id), crate::now()));
                    view.current.set(None);
                }
            })
        }
    });
    state.on_start_free({
        let ctx = ctx.clone();
        move |name| {
            ctx.run(|_, store, view| {
                if store.active_session().ok().flatten().is_none() {
                    report(store.start_session(name.trim(), crate::now()));
                    view.current.set(None);
                }
            })
        }
    });
    state.on_select_exercise({
        let ctx = ctx.clone();
        move |id| ctx.run(|_, _, view| view.current.set(Some(i64::from(id))))
    });
    state.on_add_set({
        let ctx = ctx.clone();
        move |exercise, reps, weight, duration, set_type| {
            let mut saved = false;
            ctx.run(|ui, store, view| {
                saved = log_set(
                    ui,
                    store,
                    view,
                    i64::from(exercise),
                    &reps,
                    &weight,
                    &duration,
                    set_type,
                );
            });
            saved
        }
    });
    state.on_undo_set({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, _| {
                if let Some(session) = store.active_session().ok().flatten() {
                    report(store.remove_last_set(session.id));
                }
                stop_rest(ui);
            })
        }
    });
    state.on_discard_session({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, view| {
                if let Some(session) = store.active_session().ok().flatten() {
                    report(store.discard_session(session.id));
                }
                view.current.set(None);
                stop_rest(ui);
            })
        }
    });
    state.on_finish_session({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, view| {
                let Some(session) = store.active_session().ok().flatten() else {
                    return;
                };
                let exercises = exercises_by_id(store);
                if let Some(done) = report(store.finish_session(session.id, crate::now())) {
                    ui.set_level_up(level_up(&session.name, &done, &exercises));
                }
                view.current.set(None);
                stop_rest(ui);
                crate::refresh_progress(ui, store);
            })
        }
    });
    state.on_adjust_rest({
        let weak = ui.as_weak();
        move |delta| {
            let Some(ui) = weak.upgrade() else { return };
            let state = ui.global::<TrainingState>();
            let now = crate::now() as i32;
            let until = (state.get_rest_until() + delta).max(now);
            state.set_rest_total((state.get_rest_total() + delta).max(1));
            state.set_rest_until(until);
        }
    });
    state.on_skip_rest({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                stop_rest(&ui);
            }
        }
    });

    refresh(ui, &store.borrow(), &view);
}

#[allow(clippy::too_many_arguments)]
fn log_set(
    ui: &AppWindow,
    store: &mut Store,
    view: &View,
    exercise: ExerciseId,
    reps: &str,
    weight: &str,
    duration: &str,
    set_type: i32,
) -> bool {
    let Some(session) = store.active_session().ok().flatten() else {
        return false;
    };
    let Some(kind) = exercises_by_id(store).get(&exercise).map(|e| e.kind) else {
        return false;
    };
    let set_type = SetType::from_index(i64::from(set_type));
    let Some(mut set) = parse_set(kind, reps, weight, duration) else {
        return false;
    };
    let now = crate::now();
    set.set_type = set_type;
    set.completed_at = Some(now);
    if report(store.add_set(session.id, exercise, set)).is_none() {
        return false;
    }

    let routine = session
        .routine_id
        .and_then(|id| store.routine(id).ok().flatten());
    let target = routine
        .as_ref()
        .and_then(|r| r.exercises.iter().find(|e| e.exercise_id == exercise))
        .map(|e| e.target);

    let state = ui.global::<TrainingState>();
    if set_type.rests_after() {
        let rest = target.map_or(DEFAULT_REST_S, |t| t.rest_s) as i32;
        state.set_rest_total(rest);
        state.set_rest_until(now as i32 + rest);
    } else {
        stop_rest(ui);
    }

    // Meta cumplida: pasar al siguiente ejercicio pendiente del plan.
    if let (Some(routine), Some(target)) = (&routine, target) {
        let done = work_sets_done(store, session.id);
        if done.get(&exercise).copied().unwrap_or(0) >= target.sets
            && let Some(next) = routine
                .exercises
                .iter()
                .find(|e| done.get(&e.exercise_id).copied().unwrap_or(0) < e.target.sets)
        {
            view.current.set(Some(next.exercise_id));
        }
    }
    true
}

fn stop_rest(ui: &AppWindow) {
    let state = ui.global::<TrainingState>();
    state.set_rest_until(0);
    state.set_rest_total(0);
}

/// Series de trabajo (sin calentamiento) por ejercicio en la sesión.
fn work_sets_done(store: &Store, session: mog_storage::SessionId) -> HashMap<ExerciseId, u32> {
    let mut done = HashMap::new();
    for logged in store.session_sets(session).unwrap_or_default() {
        if logged.set.set_type.counts_as_work() {
            *done.entry(logged.exercise_id).or_insert(0) += 1;
        }
    }
    done
}

/// Vuelve a leer todo lo que muestra la pestaña.
fn refresh(ui: &AppWindow, store: &Store, view: &View) {
    let state = ui.global::<TrainingState>();
    let now = Local::now();
    let today = now.weekday().num_days_from_monday() as Weekday;
    state.set_weekday(i32::from(today));

    let exercises = store.exercises().unwrap_or_default();
    let by_id: HashMap<ExerciseId, &Exercise> = exercises.iter().map(|e| (e.id, e)).collect();
    let (muscle, query) = (state.get_filter_muscle(), state.get_filter_query());
    state.set_exercises_total(exercises.len() as i32);
    state.set_exercises(model(
        exercises
            .iter()
            .filter(|e| passes_filter(e, muscle, &query))
            .map(exercise_item)
            .collect(),
    ));

    let routines = store.routines().unwrap_or_default();
    state.set_routines(model(
        routines
            .iter()
            .map(|r| RoutineItem {
                id: r.id as i32,
                name: r.name.clone().into(),
                days: days_model(&r.weekdays),
                exercise_count: r.exercises.len() as i32,
                today: r.weekdays.contains(&today),
            })
            .collect(),
    ));

    let editing = view
        .editing
        .get()
        .and_then(|id| routines.iter().find(|r| r.id == id));
    if editing.is_none() {
        view.editing.set(None);
    }
    state.set_editing(match editing {
        Some(r) => RoutineDetail {
            id: r.id as i32,
            name: r.name.clone().into(),
            days: days_model(&r.weekdays),
            exercises: model(plan_rows(r, &by_id, &HashMap::new())),
        },
        None => RoutineDetail {
            days: days_model(&[]),
            ..Default::default()
        },
    });

    let recent = recent_rows(store);
    // Primeros pasos: rutina creada, con ejercicios, con días, primera sesión hecha.
    state.set_steps(model(vec![
        !routines.is_empty(),
        routines.iter().any(|r| !r.exercises.is_empty()),
        routines.iter().any(|r| !r.weekdays.is_empty()),
        !recent.is_empty(),
    ]));
    state.set_recent(model(recent));

    let Some(session) = store.active_session().ok().flatten() else {
        state.set_session(ActiveSession::default());
        state.set_plan(model(Vec::new()));
        state.set_sets(model(Vec::new()));
        state.set_current(CurrentExercise::default());
        state.set_next(RoutineExerciseRow::default());
        return;
    };
    state.set_session(ActiveSession {
        active: true,
        name: session.name.clone().into(),
        started_at: session.started_at as i32,
    });

    let done = work_sets_done(store, session.id);
    let routine = session
        .routine_id
        .and_then(|id| routines.iter().find(|r| r.id == id));
    let plan = routine
        .map(|r| plan_rows(r, &by_id, &done))
        .unwrap_or_default();

    let logged = store.session_sets(session.id).unwrap_or_default();
    state.set_sets(model(
        logged
            .iter()
            .filter_map(|l| {
                let e = by_id.get(&l.exercise_id)?;
                Some(SetRow {
                    code: text(&e.code),
                    name: text(&e.name),
                    kind: kind_index(e.kind),
                    reps: l.set.reps.unwrap_or(0) as i32,
                    weight: l.set.weight_kg.unwrap_or(0.0) as f32,
                    duration_s: l.set.duration_s.unwrap_or(0) as i32,
                    set_type: i32::from(l.set.set_type.index()),
                })
            })
            .collect(),
    ));

    // Ejercicio actual: el elegido, o el primero pendiente del plan, o el primero de la lista.
    let current = view
        .current
        .get()
        .filter(|id| by_id.contains_key(id))
        .or_else(|| {
            routine.and_then(|r| {
                r.exercises
                    .iter()
                    .find(|e| done.get(&e.exercise_id).copied().unwrap_or(0) < e.target.sets)
                    .or(r.exercises.first())
                    .map(|e| e.exercise_id)
            })
        })
        .or_else(|| exercises.first().map(|e| e.id));
    view.current.set(current);
    // Siguiente pendiente del plan después del actual.
    let next = current
        .and_then(|id| plan.iter().position(|row| i64::from(row.exercise_id) == id))
        .map_or(0, |i| i + 1);
    let pending = |row: &&RoutineExerciseRow| {
        row.done < row.sets && Some(i64::from(row.exercise_id)) != current
    };
    state.set_next(
        plan.iter()
            .skip(next)
            .find(pending)
            .or_else(|| plan.iter().find(pending))
            .cloned()
            .unwrap_or_default(),
    );
    state.set_plan(model(plan));

    let current = current.and_then(|id| by_id.get(&id).copied());
    state.set_current(match current {
        Some(exercise) => current_exercise(store, exercise, routine, &done, &logged),
        None => CurrentExercise::default(),
    });
}

fn current_exercise(
    store: &Store,
    exercise: &Exercise,
    routine: Option<&Routine>,
    done: &HashMap<ExerciseId, u32>,
    logged: &[mog_storage::LoggedSet],
) -> CurrentExercise {
    let target = routine
        .and_then(|r| r.exercises.iter().find(|e| e.exercise_id == exercise.id))
        .map(|e| e.target);
    // Propuesta: lo último de esta sesión, si no la meta, si no la última vez.
    let in_session = logged
        .iter()
        .rev()
        .find(|l| l.exercise_id == exercise.id && l.set.set_type.counts_as_work())
        .map(|l| l.set);
    let last_time = store.last_work_set(exercise.id).ok().flatten();
    let reps = in_session
        .and_then(|s| s.reps)
        .or(target.map(|t| t.reps_max))
        .or(last_time.and_then(|s| s.reps));
    let weight = in_session
        .and_then(|s| s.weight_kg)
        .or(target.and_then(|t| t.weight_kg))
        .or(last_time.and_then(|s| s.weight_kg));
    let t = target.unwrap_or_default();
    CurrentExercise {
        id: exercise.id as i32,
        code: text(&exercise.code),
        name: text(&exercise.name),
        kind: kind_index(exercise.kind),
        in_routine: target.is_some(),
        sets: t.sets as i32,
        reps_min: t.reps_min as i32,
        reps_max: t.reps_max as i32,
        weight: t.weight_kg.map_or(-1.0, |w| w as f32),
        rest_s: t.rest_s as i32,
        done: done.get(&exercise.id).copied().unwrap_or(0) as i32,
        suggest_reps: reps.map(|r| r.to_string()).unwrap_or_default().into(),
        suggest_weight: weight
            .filter(|w| *w > 0.0)
            .map(format_kg)
            .unwrap_or_default()
            .into(),
    }
}

fn plan_rows(
    routine: &Routine,
    by_id: &HashMap<ExerciseId, &Exercise>,
    done: &HashMap<ExerciseId, u32>,
) -> Vec<RoutineExerciseRow> {
    routine
        .exercises
        .iter()
        .filter_map(|entry| {
            let e = by_id.get(&entry.exercise_id)?;
            Some(RoutineExerciseRow {
                id: entry.id as i32,
                exercise_id: e.id as i32,
                code: text(&e.code),
                name: text(&e.name),
                kind: kind_index(e.kind),
                sets: entry.target.sets as i32,
                reps_min: entry.target.reps_min as i32,
                reps_max: entry.target.reps_max as i32,
                weight: entry.target.weight_kg.map_or(-1.0, |w| w as f32),
                rest_s: entry.target.rest_s as i32,
                done: done.get(&e.id).copied().unwrap_or(0) as i32,
            })
        })
        .collect()
}

fn recent_rows(store: &Store) -> Vec<SessionRow> {
    store
        .recent_sessions(RECENT_SESSIONS)
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            let date = Local.timestamp_opt(s.ended_at, 0).single();
            SessionRow {
                name: s.name.into(),
                day: date.map_or(0, |d| d.day() as i32),
                month: date.map_or(0, |d| d.month() as i32),
                duration_min: ((s.ended_at - s.started_at).max(0) / 60) as i32,
                sets: s.sets as i32,
                xp: s.xp as i32,
            }
        })
        .collect()
}

fn level_up(
    name: &str,
    done: &FinishedSession,
    exercises: &HashMap<ExerciseId, Exercise>,
) -> LevelUp {
    let before = ranks::level_from_xp(done.total_xp_before);
    let after = ranks::level_from_xp(done.total_xp_after);
    let progress = |p: &ranks::LevelProgress| {
        if p.xp_for_next == 0 {
            1.0
        } else {
            p.xp_in_level as f32 / p.xp_for_next as f32
        }
    };
    let rank_change = match ranks::rank_change(before.level, after.level) {
        None => 0,
        Some(ranks::RankChange::LevelOnly) => 1,
        Some(ranks::RankChange::Division) => 2,
        Some(ranks::RankChange::Tier) => 3,
    };
    let records = done
        .records
        .iter()
        .filter_map(|r| {
            let exercise = exercises.get(&r.exercise_id)?;
            Some(RecordRow {
                code: text(&exercise.code),
                name: text(&exercise.name),
                new_kg: r.new_kg as f32,
                previous_kg: r.previous_kg as f32,
            })
        })
        .collect();
    LevelUp {
        visible: true,
        session_name: name.into(),
        duration_min: done.duration_min as i32,
        sets: done.sets as i32,
        volume_kg: done.volume_kg as f32,
        records: model(records),
        base: done.breakdown.base as i32,
        minutes: done.breakdown.minutes as i32,
        records_xp: done.breakdown.records as i32,
        streak_weeks: done.streak_weeks as i32,
        streak_pct: done.breakdown.streak_pct as i32,
        total: done.breakdown.total as i32,
        over_cap: done.over_daily_cap,
        level_before: before.level as i32,
        level_after: after.level as i32,
        progress_before: progress(&before),
        progress_after: progress(&after),
        xp_in_level: after.xp_in_level as i32,
        xp_for_next: after.xp_for_next as i32,
        tier: ranks::tier_for_level(after.level) as i32,
        division: i32::from(ranks::division(after.level)),
        rank_change,
    }
}

/// Lee una serie desde los campos de texto. Devuelve `None` si algo no es válido.
fn parse_set(kind: ExerciseKind, reps: &str, weight: &str, duration: &str) -> Option<SetEntry> {
    match kind {
        ExerciseKind::Timed => Some(SetEntry {
            duration_s: Some(parse_duration(duration).filter(|s| *s > 0)?),
            ..Default::default()
        }),
        ExerciseKind::Weighted | ExerciseKind::Bodyweight => {
            let reps: u32 = reps.trim().parse().ok().filter(|r| (1..=200).contains(r))?;
            let weight = if weight.trim().is_empty() && kind == ExerciseKind::Bodyweight {
                0.0
            } else {
                parse_kg(weight)?
            };
            Some(SetEntry {
                reps: Some(reps),
                weight_kg: Some(weight),
                ..Default::default()
            })
        }
    }
}

fn parse_kg(input: &str) -> Option<f64> {
    input
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|w| (0.0..=1000.0).contains(w))
}

fn parse_target(
    sets: &str,
    reps_min: &str,
    reps_max: &str,
    weight: &str,
    rest: &str,
) -> Option<Target> {
    let sets: u32 = sets.trim().parse().ok().filter(|s| (1..=20).contains(s))?;
    let defaults = Target::default();
    let reps = |s: &str, fallback: u32| -> Option<u32> {
        if s.trim().is_empty() {
            Some(fallback)
        } else {
            s.trim().parse().ok().filter(|r| (1..=200).contains(r))
        }
    };
    let reps_min = reps(reps_min, defaults.reps_min)?;
    let reps_max = reps(reps_max, reps_min.max(defaults.reps_max))?;
    if reps_min > reps_max {
        return None;
    }
    let weight_kg = if weight.trim().is_empty() {
        None
    } else {
        Some(parse_kg(weight)?)
    };
    let rest_s = parse_duration(rest).filter(|s| *s <= 3600)?;
    Some(Target {
        sets,
        reps_min,
        reps_max,
        weight_kg,
        rest_s,
    })
}

/// Acepta segundos ("90") o minutos y segundos ("1:30"). Permite 0.
fn parse_duration(input: &str) -> Option<u32> {
    let input = input.trim();
    let seconds = match input.split_once(':') {
        Some((min, sec)) => {
            min.trim().parse::<u32>().ok()? * 60
                + sec.trim().parse::<u32>().ok().filter(|s| *s < 60)?
        }
        None => input.parse().ok()?,
    };
    (seconds <= 24 * 3600).then_some(seconds)
}

/// "62.5" o "60" (sin decimales de sobra).
fn format_kg(kg: f64) -> String {
    let rounded = (kg * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        rounded.to_string()
    }
}

fn days_model(days: &[Weekday]) -> ModelRc<bool> {
    model((0..7).map(|d| days.contains(&d)).collect())
}

fn exercises_by_id(store: &Store) -> HashMap<ExerciseId, Exercise> {
    store
        .exercises()
        .unwrap_or_default()
        .into_iter()
        .map(|e| (e.id, e))
        .collect()
}

fn exercise_item(exercise: &Exercise) -> ExerciseItem {
    let info = exercise.catalog();
    ExerciseItem {
        id: exercise.id as i32,
        code: text(&exercise.code),
        name: text(&exercise.name),
        kind: kind_index(exercise.kind),
        muscle: exercise.muscle.map_or(-1, |m| i32::from(m.index())),
        secondary: model(
            info.map(|i| i.secondary.iter().map(|m| i32::from(m.index())).collect())
                .unwrap_or_default(),
        ),
        equipment: info.map_or(-1, |i| i32::from(i.equipment.index())),
        lift: exercise.lift.map_or(-1, lift_index),
        reps_min: info.map_or(8, |i| i.reps_min as i32),
        reps_max: info.map_or(12, |i| i.reps_max as i32),
        rest_s: info.map_or(90, |i| i.rest_s as i32),
        custom: info.is_none(),
    }
}

fn lift_index(lift: Lift) -> i32 {
    [
        Lift::Squat,
        Lift::BenchPress,
        Lift::Deadlift,
        Lift::OverheadPress,
        Lift::BarbellRow,
        Lift::PullUp,
    ]
    .iter()
    .position(|l| *l == lift)
    .map_or(-1, |i| i as i32)
}

/// Si el ejercicio pasa el filtro de músculo y la búsqueda de la biblioteca.
fn passes_filter(exercise: &Exercise, muscle: i32, query: &str) -> bool {
    let muscle_ok = muscle < 0
        || exercise.muscle.map(|m| i32::from(m.index())) == Some(muscle)
        || exercise
            .catalog()
            .is_some_and(|c| c.secondary.iter().any(|m| i32::from(m.index()) == muscle));
    let query_ok = query.trim().is_empty()
        || match exercise.catalog() {
            Some(info) => catalog::matches(info, query),
            None => catalog::text_matches(exercise.name.as_deref().unwrap_or_default(), query),
        };
    muscle_ok && query_ok
}

fn kind_index(kind: ExerciseKind) -> i32 {
    match kind {
        ExerciseKind::Weighted => 0,
        ExerciseKind::Bodyweight => 1,
        ExerciseKind::Timed => 2,
    }
}

fn text(value: &Option<String>) -> SharedString {
    value.as_deref().unwrap_or_default().into()
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

/// Escribe el error en la consola y lo convierte en `None`.
fn report<T>(result: mog_storage::Result<T>) -> Option<T> {
    result.map_err(|err| eprintln!("base de datos: {err}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("90"), Some(90));
        assert_eq!(parse_duration("1:30"), Some(90));
        assert_eq!(parse_duration("0"), Some(0));
        assert_eq!(parse_duration("1:75"), None);
        assert_eq!(parse_duration(""), None);
    }

    #[test]
    fn sets_from_text() {
        let weighted = parse_set(ExerciseKind::Weighted, "8", "62,5", "").unwrap();
        assert_eq!((weighted.reps, weighted.weight_kg), (Some(8), Some(62.5)));
        assert!(
            parse_set(ExerciseKind::Weighted, "8", "", "").is_none(),
            "falta el peso"
        );
        assert!(parse_set(ExerciseKind::Weighted, "0", "60", "").is_none());
        let bodyweight = parse_set(ExerciseKind::Bodyweight, "12", "", "").unwrap();
        assert_eq!(bodyweight.weight_kg, Some(0.0));
        let timed = parse_set(ExerciseKind::Timed, "", "", "2:00").unwrap();
        assert_eq!(timed.duration_s, Some(120));
        assert!(parse_set(ExerciseKind::Timed, "", "", "0").is_none());
    }

    #[test]
    fn targets_from_text() {
        let t = parse_target("4", "6", "8", "70", "2:30").unwrap();
        assert_eq!(
            (t.sets, t.reps_min, t.reps_max, t.weight_kg, t.rest_s),
            (4, 6, 8, Some(70.0), 150)
        );
        let no_weight = parse_target("3", "10", "12", "", "60").unwrap();
        assert_eq!(no_weight.weight_kg, None);
        assert!(
            parse_target("3", "12", "8", "", "60").is_none(),
            "mínimo mayor que máximo"
        );
        assert!(parse_target("0", "8", "12", "", "60").is_none());
        assert!(parse_target("3", "8", "12", "", "2 horas").is_none());
    }

    #[test]
    fn kilos_format() {
        assert_eq!(format_kg(60.0), "60");
        assert_eq!(format_kg(62.5), "62.5");
        assert_eq!(format_kg(77.499999), "77.5");
    }
}
