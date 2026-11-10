//! Pestaña TAREAS: entrada rápida, lista agrupada por fecha, completar y editar.

use std::cell::RefCell;
use std::rc::Rc;

use chrono::{Datelike, Local, NaiveDate, Timelike};
use mog_core::schedule::{self, DayNumber, Minute};
use mog_core::tasks::{self, Bucket, Priority, Repeat};
use mog_storage::{Store, Task};
use mog_ui::{AppWindow, TaskGroup, TaskRow, TasksState};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

type SharedStore = Rc<RefCell<Store>>;

/// Opciones de aviso, en el mismo orden que los botones del editor.
const REMINDERS: [Option<u32>; 6] = [None, Some(5), Some(15), Some(30), Some(60), Some(1440)];

struct Ctx {
    ui: slint::Weak<AppWindow>,
    store: SharedStore,
}

impl Ctx {
    fn run(&self, action: impl FnOnce(&AppWindow, &mut Store)) {
        if let Some(ui) = self.ui.upgrade() {
            let mut store = self.store.borrow_mut();
            action(&ui, &mut store);
            refresh(&ui, &store);
        }
    }
}

/// Conecta la pestaña y devuelve una función para refrescarla desde otros módulos.
pub fn connect(ui: &AppWindow, store: &SharedStore) -> impl Fn() + 'static {
    let ctx = Rc::new(Ctx {
        ui: ui.as_weak(),
        store: store.clone(),
    });
    let state = ui.global::<TasksState>();

    state.on_quick_changed({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                update_preview(&ui);
            }
        }
    });
    state.on_quick_add({
        let ctx = ctx.clone();
        move || {
            let mut added = false;
            ctx.run(|ui, store| {
                let state = ui.global::<TasksState>();
                if let Some((id, title)) = add_from_text(store, &state.get_quick_text()) {
                    crate::quick::notify(
                        ui,
                        crate::quick::TASK_ADDED,
                        &title,
                        crate::quick::Undo::TaskAdded(id),
                    );
                    added = true;
                }
                if added {
                    state.set_quick_text(SharedString::new());
                    update_preview(ui);
                }
            });
            added
        }
    });
    state.on_complete({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|ui, store| {
                let (today, _) = today_and_minute();
                let id = i64::from(id);
                let title = store
                    .task(id)
                    .ok()
                    .flatten()
                    .map(|t| t.title)
                    .unwrap_or_default();
                if let Some(next) = report(store.complete_task(id, crate::now(), today)) {
                    crate::quick::notify(
                        ui,
                        crate::quick::TASK_DONE,
                        &title,
                        crate::quick::Undo::TaskDone { id, next },
                    );
                }
                let id = id as i32;
                let state = ui.global::<TasksState>();
                if state.get_edit_id() == id {
                    state.set_edit_id(0);
                }
            })
        }
    });
    state.on_reopen({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|_, store| {
                report(store.reopen_task(i64::from(id)));
            })
        }
    });
    state.on_open_task({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|ui, store| {
                let Some(task) = report(store.task(i64::from(id))).flatten() else {
                    return;
                };
                let state = ui.global::<TasksState>();
                state.set_edit_id(task.id as i32);
                state.set_edit_title(task.title.into());
                state.set_edit_date(task.due_day.map(date_text).unwrap_or_default().into());
                state.set_edit_time(task.due_min.map(time_text).unwrap_or_default().into());
                state.set_edit_priority(i32::from(task.priority.index()));
                state.set_edit_repeat(i32::from(task.repeat.index()));
                state.set_edit_reminder(
                    REMINDERS
                        .iter()
                        .position(|r| *r == task.reminder_min)
                        .unwrap_or(0) as i32,
                );
                state.set_edit_tag(task.tag.unwrap_or_default().into());
                state.set_edit_error(0);
            })
        }
    });
    state.on_save_task({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store| {
                let state = ui.global::<TasksState>();
                match read_editor(&state) {
                    Ok(task) => {
                        if report(store.save_task(&task, crate::now())).is_some() {
                            state.set_edit_id(0);
                        }
                    }
                    Err(code) => state.set_edit_error(code),
                }
            })
        }
    });
    state.on_delete_task({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store| {
                let state = ui.global::<TasksState>();
                report(store.delete_task(i64::from(state.get_edit_id())));
                state.set_edit_id(0);
            })
        }
    });
    state.on_close_edit({
        let ctx = ctx.clone();
        move || ctx.run(|ui, _| ui.global::<TasksState>().set_edit_id(0))
    });

    refresh(ui, &store.borrow());
    move || ctx.run(|_, _| {})
}

fn read_editor(state: &TasksState) -> Result<Task, i32> {
    let title = state.get_edit_title().trim().to_string();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(1);
    }
    let date = state.get_edit_date();
    let due_day = if date.trim().is_empty() {
        None
    } else {
        Some(parse_date(&date).ok_or(2)?)
    };
    let time = state.get_edit_time();
    let due_min = if time.trim().is_empty() {
        None
    } else {
        Some(
            schedule::parse_time(&time)
                .filter(|m| *m < schedule::DAY_MINUTES)
                .ok_or(3)?,
        )
    };
    // Con hora y sin fecha, la fecha es hoy.
    let due_day = due_day.or(due_min.map(|_| today_and_minute().0));
    let tag = state
        .get_edit_tag()
        .trim()
        .trim_start_matches('#')
        .to_string();
    Ok(Task {
        id: i64::from(state.get_edit_id()),
        title,
        due_day,
        due_min,
        priority: Priority::from_index(i64::from(state.get_edit_priority())),
        repeat: Repeat::from_index(i64::from(state.get_edit_repeat())),
        reminder_min: REMINDERS
            .get(state.get_edit_reminder() as usize)
            .copied()
            .flatten(),
        tag: (!tag.is_empty()).then_some(tag),
        done_at: None,
    })
}

fn draft_task(text: &str) -> Option<Task> {
    let (today, now_min) = today_and_minute();
    let draft = tasks::parse_quick(text, today, now_min);
    if draft.title.trim().is_empty() {
        return None;
    }
    Some(Task {
        title: draft.title,
        due_day: draft.due_day,
        due_min: draft.due_min,
        priority: draft.priority,
        repeat: draft.repeat,
        tag: draft.tag,
        ..Default::default()
    })
}

/// Fila de la tarea que crearía este texto, para la vista previa de la entrada rápida.
pub fn preview_row(text: &str) -> Option<TaskRow> {
    let (today, now_min) = today_and_minute();
    draft_task(text).map(|t| row(&t, today, now_min))
}

/// Crea la tarea a partir de la entrada rápida. Devuelve su id y su título.
pub fn add_from_text(store: &Store, text: &str) -> Option<(i64, String)> {
    let task = draft_task(text)?;
    let id = report(store.save_task(&task, crate::now()))?;
    Some((id, task.title))
}

/// Pendientes para la tarjeta del panel: vencidas y de hoy primero, como mucho cinco.
pub fn top_rows(store: &Store) -> Vec<TaskRow> {
    let (today, now_min) = today_and_minute();
    let mut pending = store.pending_tasks().unwrap_or_default();
    pending.sort_by_key(|t| {
        (
            tasks::bucket(t.due_day, t.due_min, today, now_min).index(),
            std::cmp::Reverse(t.priority.index()),
            t.due_min.unwrap_or(u16::MAX),
        )
    });
    pending
        .iter()
        .take(5)
        .map(|t| row(t, today, now_min))
        .collect()
}

fn update_preview(ui: &AppWindow) {
    let state = ui.global::<TasksState>();
    let (today, now_min) = today_and_minute();
    let draft = tasks::parse_quick(&state.get_quick_text(), today, now_min);
    state.set_preview_valid(!draft.title.trim().is_empty());
    let preview = Task {
        title: draft.title,
        due_day: draft.due_day,
        due_min: draft.due_min,
        priority: draft.priority,
        repeat: draft.repeat,
        tag: draft.tag,
        ..Default::default()
    };
    state.set_preview(row(&preview, today, now_min));
}

pub fn refresh(ui: &AppWindow, store: &Store) {
    let state = ui.global::<TasksState>();
    let (today, now_min) = today_and_minute();
    let pending = store.pending_tasks().unwrap_or_default();

    let mut groups: Vec<TaskGroup> = Vec::new();
    for bucket in Bucket::ALL {
        let rows: Vec<TaskRow> = pending
            .iter()
            .filter(|t| tasks::bucket(t.due_day, t.due_min, today, now_min) == bucket)
            .map(|t| row(t, today, now_min))
            .collect();
        if !rows.is_empty() {
            groups.push(TaskGroup {
                bucket: i32::from(bucket.index()),
                tasks: model(rows),
            });
        }
    }
    let count = |b: Bucket| {
        pending
            .iter()
            .filter(|t| tasks::bucket(t.due_day, t.due_min, today, now_min) == b)
            .count() as i32
    };
    state.set_pending_count(pending.len() as i32);
    state.set_overdue_count(count(Bucket::Overdue));
    state.set_today_count(count(Bucket::Today));
    state.set_groups(model(groups));

    let midnight = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|d| d.and_local_timezone(Local).single())
        .map_or(0, |d| d.timestamp());
    let done = store.done_tasks_since(midnight).unwrap_or_default();
    state.set_done_today(model(done.iter().map(|t| row(t, today, now_min)).collect()));
    ui.global::<mog_ui::ShellState>()
        .set_top_tasks(model(top_rows(store)));
}

fn row(task: &Task, today: DayNumber, now_min: Minute) -> TaskRow {
    let date = task.due_day.map(date_of);
    TaskRow {
        id: task.id as i32,
        title: task.title.clone().into(),
        has_date: task.due_day.is_some(),
        day: date.map_or(0, |d| d.day() as i32),
        month: date.map_or(0, |d| d.month() as i32),
        weekday: task
            .due_day
            .map_or(0, |d| i32::from(schedule::weekday_of(d))),
        due_min: task.due_min.map_or(-1, i32::from),
        priority: i32::from(task.priority.index()),
        repeat: i32::from(task.repeat.index()),
        tag: task.tag.clone().unwrap_or_default().into(),
        bucket: i32::from(tasks::bucket(task.due_day, task.due_min, today, now_min).index()),
    }
}

fn today_and_minute() -> (DayNumber, Minute) {
    let now = Local::now();
    (
        day_number(now.date_naive()),
        (now.hour() * 60 + now.minute()) as Minute,
    )
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

/// Fecha del editor: la misma lógica que la entrada rápida ("24/11", "hoy", "viernes"...).
fn parse_date(text: &str) -> Option<DayNumber> {
    let (today, now_min) = today_and_minute();
    let draft = tasks::parse_quick(&format!("x {}", text.trim()), today, now_min);
    (draft.title == "x").then_some(draft.due_day).flatten()
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn report<T>(result: mog_storage::Result<T>) -> Option<T> {
    result.map_err(|err| eprintln!("base de datos: {err}")).ok()
}
