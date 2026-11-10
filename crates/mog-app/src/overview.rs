//! Tarjetas del PANEL que juntan datos de varios módulos: el entrenamiento de hoy y la semana.

use std::collections::HashMap;

use chrono::{Local, NaiveDate, TimeZone};
use mog_core::schedule::{self, DayNumber};
use mog_core::training::Weekday;
use mog_storage::{Store, Timestamp};
use mog_ui::{AppWindow, PlanLine, ShellState, TodayTraining};
use slint::{ComponentHandle, ModelRc, VecModel};

pub fn refresh(ui: &AppWindow, store: &Store) {
    let shell = ui.global::<ShellState>();
    let today = day_of(crate::now());
    let weekday = schedule::weekday_of(today);

    shell.set_today(today_training(store, weekday));
    shell.set_week_goal(
        store
            .profile()
            .ok()
            .flatten()
            .map_or(3, |p| p.weekly_goal as i32),
    );

    // Lunes a domingo: 2 si hubo sesión, 1 si hay una rutina asignada, 0 si nada.
    let monday = schedule::monday_of(today);
    let mut days = [0i32; 7];
    for routine in store.routines().unwrap_or_default() {
        for d in routine.weekdays {
            if let Some(slot) = days.get_mut(usize::from(d)) {
                *slot = 1;
            }
        }
    }
    for session in store.recent_sessions(30).unwrap_or_default() {
        let offset = day_of(session.started_at) - monday;
        if (0..7).contains(&offset) {
            days[offset as usize] = 2;
        }
    }
    shell.set_week_days(ModelRc::new(VecModel::from(days.to_vec())));
}

fn today_training(store: &Store, weekday: Weekday) -> TodayTraining {
    let Some(routine) = store
        .routines_for(weekday)
        .unwrap_or_default()
        .into_iter()
        .next()
    else {
        return TodayTraining::default();
    };
    let exercises: HashMap<_, _> = store
        .exercises()
        .unwrap_or_default()
        .into_iter()
        .map(|e| (e.id, e))
        .collect();
    let lines: Vec<PlanLine> = routine
        .exercises
        .iter()
        .take(4)
        .map(|item| {
            let exercise = exercises.get(&item.exercise_id);
            let t = item.target;
            let target = if t.reps_min == t.reps_max {
                format!("{} × {}", t.sets, t.reps_min)
            } else {
                format!("{} × {}-{}", t.sets, t.reps_min, t.reps_max)
            };
            let last = store
                .last_work_set(item.exercise_id)
                .ok()
                .flatten()
                .and_then(|set| match (set.weight_kg, set.reps) {
                    (Some(kg), Some(reps)) if kg > 0.0 => Some(format!("{} × {reps}", kg_text(kg))),
                    (_, Some(reps)) => Some(format!("× {reps}")),
                    _ => None,
                })
                .unwrap_or_default();
            PlanLine {
                code: exercise
                    .and_then(|e| e.code.clone())
                    .unwrap_or_default()
                    .into(),
                name: exercise
                    .and_then(|e| e.name.clone())
                    .unwrap_or_default()
                    .into(),
                target: target.into(),
                last: last.into(),
            }
        })
        .collect();
    TodayTraining {
        routine_id: routine.id as i32,
        name: routine.name.into(),
        start_min: routine.start_min.map_or(-1, i32::from),
        duration_min: i32::from(routine.duration_min),
        exercise_count: routine.exercises.len() as i32,
        lines: ModelRc::new(VecModel::from(lines)),
    }
}

fn kg_text(kg: f64) -> String {
    if kg.fract() == 0.0 {
        format!("{kg:.0}")
    } else {
        format!("{kg:.1}")
    }
}

fn day_of(ts: Timestamp) -> DayNumber {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("fecha válida");
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map_or(0, |d| (d.date_naive() - epoch).num_days())
}
