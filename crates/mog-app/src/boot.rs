//! Secuencia de arranque que alimenta la pantalla de carga.
//!
//! Por ahora los pasos son instantáneos (todavía no hay base de datos), así que se
//! reparten en un tiempo mínimo para que la animación se lea. Cuando existan pasos
//! reales, cada uno avanzará la barra al terminar.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mog_ui::AppWindow;
use slint::{ComponentHandle, Timer, TimerMode};

const MIN_BOOT: Duration = Duration::from_millis(2400);
const HOLD_AT_FULL: Duration = Duration::from_millis(350);

/// Devuelve el temporizador; hay que mantenerlo vivo mientras dure el arranque.
pub fn start(ui: &AppWindow) -> Timer {
    let started = Instant::now();
    let finished = Rc::new(Cell::new(false));

    ui.on_skip_boot({
        let weak = ui.as_weak();
        let finished = finished.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                finished.set(true);
                ui.set_boot_progress(1.0);
                ui.set_booted(true);
            }
        }
    });

    let timer = Timer::default();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(ui) = weak.upgrade() else { return };
        if finished.get() {
            return;
        }
        let elapsed = started.elapsed();
        let progress = (elapsed.as_secs_f32() / MIN_BOOT.as_secs_f32()).min(1.0);
        ui.set_boot_progress(progress);
        if elapsed >= MIN_BOOT + HOLD_AT_FULL {
            finished.set(true);
            ui.set_booted(true);
        }
    });
    timer
}
