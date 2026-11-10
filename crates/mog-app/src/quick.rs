//! Entrada rápida única (tareas, dinero y comandos) y el aviso con deshacer.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use mog_core::quick::{self, QuickKind};
use mog_storage::Store;
use mog_ui::{AppWindow, CommandFeedback, QuickPreview, ShellState, Toast};
use slint::ComponentHandle;

type SharedStore = Rc<RefCell<Store>>;

// Deben coincidir con ToastView en hud.slint.
pub const TASK_ADDED: i32 = 1;
pub const TASK_DONE: i32 = 2;
pub const EXPENSE_LOGGED: i32 = 3;
pub const INCOME_LOGGED: i32 = 4;
const UNDONE: i32 = 5;

/// Lo que se puede deshacer de la última acción.
pub enum Undo {
    None,
    TaskAdded(i64),
    /// `next` es la copia que se creó si la tarea se repite.
    TaskDone {
        id: i64,
        next: Option<i64>,
    },
    Entry(i64),
}

thread_local! {
    // La última acción y un contador, para que un aviso viejo no cierre uno nuevo.
    static LAST: RefCell<(Undo, u64)> = const { RefCell::new((Undo::None, 0)) };
}

/// Muestra el aviso de abajo a la derecha durante unos segundos.
pub fn notify(ui: &AppWindow, kind: i32, title: &str, undo: Undo) {
    let can_undo = !matches!(undo, Undo::None);
    let serial = LAST.with(|last| {
        let mut last = last.borrow_mut();
        last.0 = undo;
        last.1 += 1;
        last.1
    });
    ui.global::<ShellState>().set_toast(Toast {
        kind,
        title: title.into(),
        can_undo,
        visible: true,
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_secs(6), move || {
        if LAST.with(|last| last.borrow().1) != serial {
            return;
        }
        if let Some(ui) = weak.upgrade() {
            let shell = ui.global::<ShellState>();
            let mut toast = shell.get_toast();
            toast.visible = false;
            shell.set_toast(toast);
        }
    });
}

pub fn connect(ui: &AppWindow, store: &SharedStore, refresh_all: Rc<dyn Fn()>) {
    let shell = ui.global::<ShellState>();

    shell.on_quick_changed({
        let (weak, store) = (ui.as_weak(), store.clone());
        move |text| {
            let Some(ui) = weak.upgrade() else { return };
            let store = store.borrow();
            let currency = crate::finance::currency_of(&store);
            let mut preview = QuickPreview::default();
            match quick::classify(&text, currency) {
                QuickKind::Empty => {}
                QuickKind::Command => preview.kind = 1,
                QuickKind::Task => {
                    if let Some(row) = crate::tasks::preview_row(&text) {
                        preview.kind = 2;
                        preview.task = row;
                    }
                }
                QuickKind::Money => {
                    if let Some(row) = crate::finance::preview_row(&ui, &store, &text) {
                        preview.kind = 3;
                        preview.entry = row;
                    }
                }
            }
            ui.global::<ShellState>().set_preview(preview);
        }
    });

    shell.on_quick_submit({
        let (weak, store, refresh_all) = (ui.as_weak(), store.clone(), refresh_all.clone());
        move |text| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let kind = quick::classify(&text, crate::finance::currency_of(&store.borrow()));
            let done = match kind {
                QuickKind::Empty => false,
                QuickKind::Command => {
                    if let Some(feedback) = crate::commands::run(&ui, &text) {
                        ui.set_feedback(feedback);
                    }
                    true
                }
                QuickKind::Task => {
                    let added = crate::tasks::add_from_text(&store.borrow(), &text);
                    if let Some((id, title)) = &added {
                        notify(&ui, TASK_ADDED, title, Undo::TaskAdded(*id));
                    }
                    added.is_some()
                }
                QuickKind::Money => crate::finance::add_from_text(&ui, &store.borrow(), &text),
            };
            if done && kind != QuickKind::Command {
                ui.set_feedback(CommandFeedback::default());
                refresh_all();
            }
            done
        }
    });

    shell.on_undo_last({
        let (weak, store, refresh_all) = (ui.as_weak(), store.clone(), refresh_all);
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let undo = LAST.with(|last| std::mem::replace(&mut last.borrow_mut().0, Undo::None));
            {
                let store = store.borrow();
                let result = match undo {
                    Undo::None => return,
                    Undo::TaskAdded(id) => store.delete_task(id),
                    Undo::TaskDone { id, next } => store
                        .reopen_task(id)
                        .and_then(|()| next.map_or(Ok(()), |n| store.delete_task(n))),
                    Undo::Entry(id) => store.delete_entry(id),
                };
                if let Err(err) = result {
                    eprintln!("base de datos: {err}");
                }
            }
            notify(&ui, UNDONE, "", Undo::None);
            refresh_all();
        }
    });
}
