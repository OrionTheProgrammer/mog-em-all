//! Pestaña HORARIOS: arma la semana con eventos y rutinas, y maneja el formulario.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, Timelike};
use mog_core::schedule::{self, DayNumber, Minute, Recurrence};
use mog_core::training::Weekday;
use mog_storage::{Event, EventKind, Store};
use mog_ui::{AppWindow, Block, DayColumn, ScheduleState};
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};

type SharedStore = Rc<RefCell<Store>>;

/// Opciones de aviso, en el mismo orden que los botones de la interfaz.
const REMINDERS: [Option<u32>; 6] = [None, Some(5), Some(10), Some(15), Some(30), Some(60)];
const TRAINING_KIND: i32 = 5;

#[derive(Default)]
struct View {
    week_offset: Cell<i64>,
    form_days: RefCell<[bool; 7]>,
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
            refresh_with(&ui, &store, &self.view);
        }
    }
}

/// Conecta la pestaña. Devuelve el temporizador que la actualiza cada minuto
/// (hay que mantenerlo vivo) y una función para refrescarla desde otros módulos.
pub fn connect(ui: &AppWindow, store: &SharedStore) -> (Timer, impl Fn() + 'static) {
    let view = Rc::new(View::default());
    let ctx = Rc::new(Ctx {
        ui: ui.as_weak(),
        store: store.clone(),
        view: view.clone(),
    });
    let state = ui.global::<ScheduleState>();

    state.on_prev_week({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, view| view.week_offset.set(view.week_offset.get() - 1))
    });
    state.on_next_week({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, view| view.week_offset.set(view.week_offset.get() + 1))
    });
    state.on_this_week({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, view| view.week_offset.set(0))
    });
    state.on_new_event_at({
        let ctx = ctx.clone();
        move |index, minute| {
            ctx.run(|ui, _, view| {
                let state = ui.global::<ScheduleState>();
                let day = week_monday(view) + i64::from(index.clamp(0, 6));
                let start = (minute.clamp(0, 1410)) as Minute;
                let mut days = [false; 7];
                days[schedule::weekday_of(day) as usize] = true;
                *view.form_days.borrow_mut() = days;
                state.set_form_id(0);
                state.set_form_title("".into());
                state.set_form_kind(0);
                state.set_form_weekly(true);
                state.set_form_date(date_text(day).into());
                state.set_form_start(time_text(start).into());
                state.set_form_end(time_text((start + 60).min(schedule::DAY_MINUTES)).into());
                state.set_form_reminder(2);
                state.set_form_error(0);
                state.set_routine_id(0);
                state.set_form_open(true);
            })
        }
    });
    state.on_edit_event({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|ui, store, view| {
                let Some(event) = report(store.events())
                    .and_then(|e| e.into_iter().find(|e| e.id == i64::from(id)))
                else {
                    return;
                };
                let state = ui.global::<ScheduleState>();
                let mut days = [false; 7];
                let mut date = week_monday(view);
                match &event.recurrence {
                    Recurrence::Weekly(list) => list.iter().for_each(|d| days[*d as usize] = true),
                    Recurrence::Once(day) => date = *day,
                }
                *view.form_days.borrow_mut() = days;
                state.set_form_id(event.id as i32);
                state.set_form_title(event.title.into());
                state.set_form_kind(i32::from(event.kind.index()));
                state.set_form_weekly(matches!(event.recurrence, Recurrence::Weekly(_)));
                state.set_form_date(date_text(date).into());
                state.set_form_start(time_text(event.start_min).into());
                state.set_form_end(time_text(event.end_min).into());
                state.set_form_reminder(
                    REMINDERS
                        .iter()
                        .position(|r| *r == event.reminder_min)
                        .unwrap_or(0) as i32,
                );
                state.set_form_error(0);
                state.set_routine_id(0);
                state.set_form_open(true);
            })
        }
    });
    state.on_toggle_form_day({
        let ctx = ctx.clone();
        move |day| {
            ctx.run(|_, _, view| {
                if let Some(slot) = view.form_days.borrow_mut().get_mut(day as usize) {
                    *slot = !*slot;
                }
            })
        }
    });
    state.on_save_event({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, view| {
                let state = ui.global::<ScheduleState>();
                match read_form(&state, view) {
                    Ok(event) => {
                        if report(store.save_event(&event, crate::now())).is_some() {
                            state.set_form_open(false);
                        }
                    }
                    Err(code) => state.set_form_error(code),
                }
            })
        }
    });
    state.on_delete_event({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|ui, store, _| {
                report(store.delete_event(i64::from(id)));
                ui.global::<ScheduleState>().set_form_open(false);
            })
        }
    });
    state.on_close_form({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, _, _| {
                let state = ui.global::<ScheduleState>();
                state.set_form_open(false);
                state.set_routine_id(0);
            })
        }
    });
    state.on_open_routine({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|ui, store, _| {
                let Some(routine) = report(store.routine(i64::from(id))).flatten() else {
                    return;
                };
                let state = ui.global::<ScheduleState>();
                state.set_form_open(false);
                state.set_routine_id(routine.id as i32);
                state.set_routine_name(routine.name.into());
                state
                    .set_routine_start(routine.start_min.map(time_text).unwrap_or_default().into());
                state.set_routine_duration(routine.duration_min.to_string().into());
                state.set_routine_error(false);
            })
        }
    });
    state.on_save_routine_time({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, _| {
                let state = ui.global::<ScheduleState>();
                let start = schedule::parse_time(&state.get_routine_start())
                    .filter(|m| *m < schedule::DAY_MINUTES);
                let duration = state
                    .get_routine_duration()
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .filter(|d| (10..=600).contains(d));
                match (start, duration) {
                    (Some(start), Some(duration)) => {
                        report(store.set_routine_time(
                            i64::from(state.get_routine_id()),
                            Some(start),
                            duration,
                        ));
                        state.set_routine_id(0);
                    }
                    _ => state.set_routine_error(true),
                }
            })
        }
    });
    state.on_clear_routine_time({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, _| {
                let state = ui.global::<ScheduleState>();
                let id = i64::from(state.get_routine_id());
                let duration = report(store.routine(id))
                    .flatten()
                    .map_or(60, |r| r.duration_min);
                report(store.set_routine_time(id, None, duration));
                state.set_routine_id(0);
            })
        }
    });

    refresh_with(ui, &store.borrow(), &view);

    // Cada minuto: mover la línea de "ahora" y lo que queda del día.
    let timer = Timer::default();
    let tick = {
        let ctx = ctx.clone();
        move || ctx.run(|_, _, _| {})
    };
    timer.start(TimerMode::Repeated, Duration::from_secs(60), tick);
    let refresh = move || ctx.run(|_, _, _| {});
    (timer, refresh)
}

/// Lee el formulario. El error es el código que muestra la interfaz.
fn read_form(state: &ScheduleState, view: &View) -> Result<Event, i32> {
    let title = state.get_form_title().trim().to_string();
    if title.is_empty() || title.chars().count() > 80 {
        return Err(1);
    }
    let start =
        schedule::parse_time(&state.get_form_start()).filter(|m| *m < schedule::DAY_MINUTES);
    let end = schedule::parse_time(&state.get_form_end());
    let (Some(start), Some(end)) = (start, end) else {
        return Err(2);
    };
    if end <= start {
        return Err(2);
    }
    let recurrence = if state.get_form_weekly() {
        let days: Vec<Weekday> = view
            .form_days
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .map(|(d, _)| d as Weekday)
            .collect();
        if days.is_empty() {
            return Err(3);
        }
        Recurrence::Weekly(days)
    } else {
        Recurrence::Once(parse_date(&state.get_form_date(), week_monday(view)).ok_or(4)?)
    };
    Ok(Event {
        id: i64::from(state.get_form_id()),
        title,
        kind: EventKind::from_index(i64::from(state.get_form_kind())),
        start_min: start,
        end_min: end,
        recurrence,
        reminder_min: REMINDERS
            .get(state.get_form_reminder() as usize)
            .copied()
            .flatten(),
    })
}

fn refresh_with(ui: &AppWindow, store: &Store, view: &View) {
    let state = ui.global::<ScheduleState>();
    let now = Local::now();
    let today = day_number(now.date_naive());
    let monday = week_monday(view);
    let events = store.events().unwrap_or_default();
    let routines = store.routines().unwrap_or_default();

    let blocks_for = |day: DayNumber| -> (Vec<Block>, Vec<Block>) {
        let weekday = schedule::weekday_of(day);
        let mut timed: Vec<Block> = events
            .iter()
            .filter(|e| e.recurrence.occurs_on(day))
            .map(|e| {
                block(
                    e.id,
                    0,
                    &e.title,
                    i32::from(e.kind.index()),
                    e.start_min,
                    e.end_min,
                )
            })
            .collect();
        let mut untimed = Vec::new();
        for r in routines.iter().filter(|r| r.weekdays.contains(&weekday)) {
            match r.start_min {
                Some(start) => timed.push(block(
                    r.id,
                    1,
                    &r.name,
                    TRAINING_KIND,
                    start,
                    (start + r.duration_min).min(schedule::DAY_MINUTES),
                )),
                None => untimed.push(block(r.id, 1, &r.name, TRAINING_KIND, 0, 0)),
            }
        }
        let spans: Vec<(Minute, Minute)> = timed
            .iter()
            .map(|b| (b.start_min as Minute, b.end_min as Minute))
            .collect();
        for (b, place) in timed.iter_mut().zip(schedule::lay_out(&spans)) {
            b.lane = i32::from(place.lane);
            b.lanes = i32::from(place.lanes);
            b.conflict = place.conflict;
        }
        (timed, untimed)
    };

    let mut all_spans = Vec::new();
    let mut conflicts = 0;
    let days: Vec<DayColumn> = (0..7)
        .map(|offset| {
            let day = monday + offset;
            let date = date_of(day);
            let (timed, untimed) = blocks_for(day);
            conflicts += timed.iter().filter(|b| b.conflict).count();
            all_spans.extend(
                timed
                    .iter()
                    .map(|b| (b.start_min as Minute, b.end_min as Minute)),
            );
            DayColumn {
                day: date.day() as i32,
                month: date.month() as i32,
                weekday: offset as i32,
                today: day == today,
                blocks: model(timed),
                untimed: model(untimed),
            }
        })
        .collect();
    let (from, to) = schedule::visible_range(&all_spans);
    state.set_from_min(i32::from(from));
    state.set_to_min(i32::from(to));
    state.set_days(model(days));
    state.set_conflicts(conflicts as i32);
    state.set_week_offset(view.week_offset.get() as i32);
    state.set_form_days(model(view.form_days.borrow().to_vec()));

    let now_min = (now.hour() * 60 + now.minute()) as i32;
    state.set_now_min(now_min);
    let (mut today_timed, today_untimed) = blocks_for(today);
    today_timed.retain(|b| b.end_min > now_min);
    today_timed.sort_by_key(|b| b.start_min);
    today_timed.extend(today_untimed);
    state.set_today_left(model(today_timed));
}

fn block(id: i64, source: i32, title: &str, kind: i32, start: Minute, end: Minute) -> Block {
    Block {
        id: id as i32,
        source,
        title: title.into(),
        kind,
        start_min: i32::from(start),
        end_min: i32::from(end),
        lane: 0,
        lanes: 1,
        conflict: false,
    }
}

fn week_monday(view: &View) -> DayNumber {
    let today = day_number(Local::now().date_naive());
    schedule::monday_of(today) + 7 * view.week_offset.get()
}

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).expect("fecha válida")
}

fn day_number(date: NaiveDate) -> DayNumber {
    (date - epoch()).num_days()
}

fn date_of(day: DayNumber) -> NaiveDate {
    epoch() + chrono::Duration::days(day)
}

fn date_text(day: DayNumber) -> String {
    let d = date_of(day);
    format!("{:02}/{:02}", d.day(), d.month())
}

fn time_text(minute: Minute) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// "24/11" o "24/11/2026". Sin año, usa el de la semana mostrada.
fn parse_date(text: &str, reference: DayNumber) -> Option<DayNumber> {
    let parts: Vec<&str> = text.trim().split(['/', '-', '.']).collect();
    let day: u32 = parts.first()?.trim().parse().ok()?;
    let month: u32 = parts.get(1)?.trim().parse().ok()?;
    let year: i32 = match parts.get(2) {
        Some(y) => {
            let y: i32 = y.trim().parse().ok()?;
            if y < 100 { 2000 + y } else { y }
        }
        None => date_of(reference).year(),
    };
    NaiveDate::from_ymd_opt(year, month, day).map(day_number)
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn report<T>(result: mog_storage::Result<T>) -> Option<T> {
    result.map_err(|err| eprintln!("base de datos: {err}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        let monday = day_number(NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
        assert_eq!(monday, 20_766);
        assert_eq!(schedule::weekday_of(monday), 0);
        assert_eq!(date_text(monday), "09/11");
        assert_eq!(parse_date("24/11", monday), Some(monday + 15));
        assert_eq!(
            parse_date("1/1/27", monday),
            Some(day_number(NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()))
        );
        assert_eq!(parse_date("31/02", monday), None);
        assert_eq!(parse_date("mañana", monday), None);
    }

    #[test]
    fn times_format() {
        assert_eq!(time_text(510), "08:30");
        assert_eq!(time_text(1440), "24:00");
    }
}
