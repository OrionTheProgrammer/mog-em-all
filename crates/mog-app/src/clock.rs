//! Mantiene el reloj de la barra superior al día.

use std::time::Duration;

use chrono::{Datelike, Local, Timelike};
use mog_ui::{AppWindow, Clock};
use slint::{ComponentHandle, Timer, TimerMode};

/// Devuelve el temporizador; hay que mantenerlo vivo mientras corra la app.
pub fn start(ui: &AppWindow) -> Timer {
    let update = {
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                let now = Local::now();
                ui.set_clock(Clock {
                    hour: now.hour() as i32,
                    minute: now.minute() as i32,
                    weekday: now.weekday().num_days_from_monday() as i32,
                    day: now.day() as i32,
                    month: now.month() as i32,
                });
            }
        }
    };
    update();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), update);
    timer
}
