//! Pestaña EJERCICIO: conecta la interfaz con las sesiones guardadas en la base de datos.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use chrono::{Datelike, Local, TimeZone};
use mog_core::ranks;
use mog_storage::{Exercise, ExerciseId, ExerciseKind, FinishedSession, SetEntry, Store};
use mog_ui::{ActiveSession, AppWindow, ExerciseItem, LevelUp, RecordRow, SessionRow, SetRow};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

type SharedStore = Rc<RefCell<Store>>;

const RECENT_SESSIONS: u32 = 8;

pub fn connect(ui: &AppWindow, store: &SharedStore) {
    refresh(ui, &store.borrow());

    ui.on_start_session({
        let (weak, store) = (ui.as_weak(), store.clone());
        move |name| {
            let Some(ui) = weak.upgrade() else { return };
            let store = store.borrow();
            let name = name.trim();
            if store.active_session().ok().flatten().is_none() {
                report(store.start_session(name, crate::now()));
            }
            refresh(&ui, &store);
        }
    });

    ui.on_add_set({
        let (weak, store) = (ui.as_weak(), store.clone());
        move |exercise, reps, weight, duration| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let store = store.borrow();
            let Some(session) = store.active_session().ok().flatten() else {
                return false;
            };
            let Some(kind) = exercise_kind(&store, i64::from(exercise)) else {
                return false;
            };
            let Some(set) = parse_set(kind, &reps, &weight, &duration) else {
                return false;
            };
            let saved = report(store.add_set(session.id, i64::from(exercise), set)).is_some();
            refresh(&ui, &store);
            saved
        }
    });

    ui.on_undo_set({
        let (weak, store) = (ui.as_weak(), store.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let store = store.borrow();
            if let Some(session) = store.active_session().ok().flatten() {
                report(store.remove_last_set(session.id));
            }
            refresh(&ui, &store);
        }
    });

    ui.on_discard_session({
        let (weak, store) = (ui.as_weak(), store.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let store = store.borrow();
            if let Some(session) = store.active_session().ok().flatten() {
                report(store.discard_session(session.id));
            }
            refresh(&ui, &store);
        }
    });

    ui.on_finish_session({
        let (weak, store) = (ui.as_weak(), store.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut store = store.borrow_mut();
            let Some(session) = store.active_session().ok().flatten() else {
                return;
            };
            let exercises = exercises_by_id(&store);
            if let Some(done) = report(store.finish_session(session.id, crate::now())) {
                ui.set_level_up(level_up(&session.name, &done, &exercises));
            }
            refresh(&ui, &store);
            crate::refresh_progress(&ui, &store);
        }
    });
}

/// Vuelve a leer ejercicios, sesión activa, series e historial.
pub fn refresh(ui: &AppWindow, store: &Store) {
    let exercises = store.exercises().unwrap_or_default();
    let by_id: HashMap<ExerciseId, &Exercise> = exercises.iter().map(|e| (e.id, e)).collect();
    ui.set_exercises(model(exercises.iter().map(exercise_item).collect()));

    match store.active_session().ok().flatten() {
        Some(session) => {
            ui.set_session(ActiveSession {
                active: true,
                name: session.name.clone().into(),
                started_at: session.started_at as i32,
            });
            let sets = store
                .session_sets(session.id)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|logged| {
                    let exercise = by_id.get(&logged.exercise_id)?;
                    Some(SetRow {
                        code: text(&exercise.code),
                        name: text(&exercise.name),
                        kind: kind_index(exercise.kind),
                        reps: logged.set.reps.unwrap_or(0) as i32,
                        weight: logged.set.weight_kg.unwrap_or(0.0) as f32,
                        duration_s: logged.set.duration_s.unwrap_or(0) as i32,
                    })
                })
                .collect();
            ui.set_sets(model(sets));
        }
        None => {
            ui.set_session(ActiveSession::default());
            ui.set_sets(model(Vec::new()));
        }
    }

    let recent = store
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
        .collect();
    ui.set_recent(model(recent));
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
    let number = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok();
    match kind {
        ExerciseKind::Timed => Some(SetEntry {
            reps: None,
            weight_kg: None,
            duration_s: Some(parse_duration(duration)?),
        }),
        ExerciseKind::Weighted | ExerciseKind::Bodyweight => {
            let reps: u32 = reps.trim().parse().ok().filter(|r| (1..=200).contains(r))?;
            let weight = if weight.trim().is_empty() && kind == ExerciseKind::Bodyweight {
                0.0
            } else {
                number(weight).filter(|w| (0.0..=1000.0).contains(w))?
            };
            Some(SetEntry {
                reps: Some(reps),
                weight_kg: Some(weight),
                duration_s: None,
            })
        }
    }
}

/// Acepta segundos ("90") o minutos y segundos ("1:30").
fn parse_duration(input: &str) -> Option<u32> {
    let input = input.trim();
    let seconds = match input.split_once(':') {
        Some((min, sec)) => {
            min.trim().parse::<u32>().ok()? * 60
                + sec.trim().parse::<u32>().ok().filter(|s| *s < 60)?
        }
        None => input.parse().ok()?,
    };
    (1..=24 * 3600).contains(&seconds).then_some(seconds)
}

fn exercise_kind(store: &Store, id: ExerciseId) -> Option<ExerciseKind> {
    store
        .exercises()
        .ok()?
        .into_iter()
        .find(|e| e.id == id)
        .map(|e| e.kind)
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
    ExerciseItem {
        id: exercise.id as i32,
        code: text(&exercise.code),
        name: text(&exercise.name),
        kind: kind_index(exercise.kind),
    }
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
        assert_eq!(parse_duration("1:75"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("0"), None);
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
    }
}
