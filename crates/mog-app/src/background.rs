//! Segundo plano: ícono en la bandeja, cerrar sin salir, avisos y abrir al iniciar sesión.
//!
//! Los textos de los avisos viven aquí (y no en los .po) porque los muestra el
//! sistema operativo, fuera de la interfaz de Slint.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use chrono::{Local, NaiveDate, Timelike};
use mog_core::reminders::{self, Candidate, ReminderKind};
use mog_core::schedule::{self, DayNumber, Minute};
use mog_storage::Store;
use mog_ui::{AppWindow, SettingsState};
use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};

type SharedStore = Rc<RefCell<Store>>;

/// Minutos antes de un entrenamiento con hora para avisar.
const TRAINING_LEAD: u32 = 30;

pub struct Background {
    _timer: Timer,
    #[cfg(target_os = "linux")]
    _tray: Option<ksni::blocking::Handle<tray::MogTray>>,
}

/// Prepara el segundo plano. Devuelve si la ventana debe quedar oculta al arrancar
/// (cuando se abrió con `--background` y hay bandeja para volver a abrirla).
pub fn start(ui: &AppWindow, store: &SharedStore) -> (Background, bool) {
    let state = ui.global::<SettingsState>();
    let setting = |key: &str, default: bool| {
        store
            .borrow()
            .setting(key)
            .ok()
            .flatten()
            .map_or(default, |v| v == "1")
    };

    #[cfg(target_os = "linux")]
    let tray = tray::spawn(ui.as_weak(), language(ui));
    #[cfg(target_os = "linux")]
    let tray_available = tray.is_some();
    #[cfg(not(target_os = "linux"))]
    let tray_available = false;

    state.set_tray_available(tray_available);
    state.set_background_on_close(tray_available && setting("background_on_close", true));
    state.set_notifications(setting("notifications", true));
    state.set_autostart(
        autostart()
            .and_then(|a| a.is_enabled().ok())
            .unwrap_or(false),
    );

    // Cerrar la ventana: si hay bandeja y está activado, solo se oculta.
    ui.window().on_close_requested({
        let weak = ui.as_weak();
        move || {
            let keep = weak
                .upgrade()
                .is_some_and(|ui| ui.global::<SettingsState>().get_background_on_close());
            if keep {
                CloseRequestResponse::HideWindow
            } else {
                let _ = slint::quit_event_loop();
                CloseRequestResponse::HideWindow
            }
        }
    });

    state.on_set_background_on_close({
        let (weak, store) = (ui.as_weak(), store.clone());
        move |on| {
            let _ = store
                .borrow()
                .set_setting("background_on_close", if on { "1" } else { "0" });
            if let Some(ui) = weak.upgrade() {
                ui.global::<SettingsState>().set_background_on_close(on);
            }
        }
    });
    state.on_set_notifications({
        let (weak, store) = (ui.as_weak(), store.clone());
        move |on| {
            let _ = store
                .borrow()
                .set_setting("notifications", if on { "1" } else { "0" });
            if let Some(ui) = weak.upgrade() {
                ui.global::<SettingsState>().set_notifications(on);
            }
        }
    });
    state.on_set_autostart({
        let weak = ui.as_weak();
        move |on| {
            let result = autostart().map(|a| if on { a.enable() } else { a.disable() });
            match result {
                Some(Ok(())) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.global::<SettingsState>().set_autostart(on);
                    }
                }
                Some(Err(err)) => eprintln!("inicio automático: {err}"),
                None => {}
            }
        }
    });
    state.on_test_notification({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                let text = Texts::for_language(&language(&ui));
                notify(&ui, text.test_title.into(), text.test_body.into());
            }
        }
    });

    let timer = Timer::default();
    let check = {
        let (weak, store) = (ui.as_weak(), store.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                check_reminders(&ui, &store.borrow());
            }
        }
    };
    check();
    timer.start(TimerMode::Repeated, Duration::from_secs(30), check);

    let start_hidden = tray_available && std::env::args().any(|a| a == "--background");
    (
        Background {
            _timer: timer,
            #[cfg(target_os = "linux")]
            _tray: tray,
        },
        start_hidden,
    )
}

fn autostart() -> Option<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe().ok()?;
    auto_launch::AutoLaunchBuilder::new()
        .set_app_name("mog-em-all")
        .set_app_path(&exe.to_string_lossy())
        .set_args(&["--background"])
        .build()
        .map_err(|err| eprintln!("inicio automático: {err}"))
        .ok()
}

/// Busca lo que toca avisar ahora y lo muestra.
fn check_reminders(ui: &AppWindow, store: &Store) {
    if !ui.global::<SettingsState>().get_notifications() {
        return;
    }
    let now = Local::now();
    let today =
        (now.date_naive() - NaiveDate::from_ymd_opt(1970, 1, 1).expect("fecha válida")).num_days();
    let minute = (now.hour() * 60 + now.minute()) as Minute;
    let candidates = candidates(store, today);
    let due = reminders::due(&candidates, today, minute, &|key| {
        store.was_notified(key).unwrap_or(true)
    });
    let text = Texts::for_language(&language(ui));
    for c in due {
        let left = reminders::minutes_until(c, today, minute);
        let when = if left == 0 {
            text.now.to_string()
        } else {
            text.in_minutes.replace("{}", &left.to_string())
        };
        let title = match c.kind {
            ReminderKind::Event => c.title.clone(),
            ReminderKind::Task => text.task_due.replace("{}", &c.title),
            ReminderKind::Training => text.training.replace("{}", &c.title),
        };
        notify(ui, title, when);
        let _ = store.mark_notified(&c.key, crate::now());
    }
}

/// Todo lo que puede necesitar un aviso hoy o mañana temprano.
fn candidates(store: &Store, today: DayNumber) -> Vec<Candidate> {
    let mut list = Vec::new();
    for day in [today, today + 1] {
        for e in store.events().unwrap_or_default() {
            if let Some(lead) = e.reminder_min
                && e.recurrence.occurs_on(day)
            {
                list.push(Candidate {
                    key: format!("event:{}:{day}", e.id),
                    kind: ReminderKind::Event,
                    title: e.title.clone(),
                    day,
                    minute: e.start_min,
                    lead,
                });
            }
        }
        for r in store.routines().unwrap_or_default() {
            if let Some(start) = r.start_min
                && r.weekdays.contains(&schedule::weekday_of(day))
            {
                list.push(Candidate {
                    key: format!("training:{}:{day}", r.id),
                    kind: ReminderKind::Training,
                    title: r.name.clone(),
                    day,
                    minute: start,
                    lead: TRAINING_LEAD,
                });
            }
        }
    }
    for t in store.pending_tasks().unwrap_or_default() {
        if let (Some(lead), Some(day)) = (t.reminder_min, t.due_day) {
            // Sin hora, la tarea vence al final del día: se avisa contando desde las 23:59.
            let minute = t.due_min.unwrap_or(schedule::DAY_MINUTES - 1);
            list.push(Candidate {
                key: format!("task:{}:{day}:{minute}", t.id),
                kind: ReminderKind::Task,
                title: t.title.clone(),
                day,
                minute,
                lead,
            });
        }
    }
    list
}

/// Muestra un aviso del sistema. Al hacer clic, abre la app.
fn notify(ui: &AppWindow, title: String, body: String) {
    let weak = ui.as_weak();
    let open_label = Texts::for_language(&language(ui)).open;
    std::thread::spawn(move || {
        let mut notification = notify_rust::Notification::new();
        notification
            .appname("mog 'em all")
            .summary(&title)
            .body(&body);
        #[cfg(all(unix, not(target_os = "macos")))]
        notification
            .action("default", open_label)
            .icon("appointment-soon");
        let _ = open_label;
        match notification.show() {
            #[cfg(all(unix, not(target_os = "macos")))]
            Ok(handle) => handle.wait_for_action(|action| {
                if action == "default" {
                    show_window(weak.clone());
                }
            }),
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            Ok(_) => drop(weak),
            Err(err) => eprintln!("aviso: {err}"),
        }
    });
}

/// Muestra la ventana desde cualquier hilo.
pub fn show_window(weak: slint::Weak<AppWindow>) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.show();
        }
    });
}

fn language(ui: &AppWindow) -> String {
    ui.get_language().to_string()
}

struct Texts {
    open: &'static str,
    quit: &'static str,
    now: &'static str,
    in_minutes: &'static str,
    task_due: &'static str,
    training: &'static str,
    test_title: &'static str,
    test_body: &'static str,
}

impl Texts {
    fn for_language(lang: &str) -> Texts {
        if lang == "es" {
            Texts {
                open: "Abrir mog 'em all",
                quit: "Salir",
                now: "Ahora",
                in_minutes: "En {} min",
                task_due: "Vence: {}",
                training: "Entrenamiento: {}",
                test_title: "Así se ven los avisos",
                test_body: "Te avisaremos antes de tus eventos, tareas y entrenamientos.",
            }
        } else {
            Texts {
                open: "Open mog 'em all",
                quit: "Quit",
                now: "Now",
                in_minutes: "In {} min",
                task_due: "Due: {}",
                training: "Training: {}",
                test_title: "This is how reminders look",
                test_body: "You'll get a heads-up before your events, tasks and workouts.",
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod tray {
    use ksni::blocking::TrayMethods;
    use mog_ui::AppWindow;

    use super::Texts;

    pub struct MogTray {
        ui: slint::Weak<AppWindow>,
        open: &'static str,
        quit: &'static str,
    }

    impl ksni::Tray for MogTray {
        fn id(&self) -> String {
            "mog-em-all".into()
        }

        fn title(&self) -> String {
            "mog 'em all".into()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            vec![icon(32), icon(64)]
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            super::show_window(self.ui.clone());
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::StandardItem;
            vec![
                StandardItem {
                    label: self.open.into(),
                    activate: Box::new(|tray: &mut Self| super::show_window(tray.ui.clone())),
                    ..Default::default()
                }
                .into(),
                ksni::MenuItem::Separator,
                StandardItem {
                    label: self.quit.into(),
                    activate: Box::new(|_| {
                        let _ = slint::invoke_from_event_loop(|| {
                            let _ = slint::quit_event_loop();
                        });
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    /// Lanza el ícono. `None` si el escritorio no tiene bandeja.
    pub fn spawn(
        ui: slint::Weak<AppWindow>,
        lang: String,
    ) -> Option<ksni::blocking::Handle<MogTray>> {
        let text = Texts::for_language(&lang);
        MogTray {
            ui,
            open: text.open,
            quit: text.quit,
        }
        .spawn()
        .map_err(|err| eprintln!("bandeja del sistema no disponible: {err}"))
        .ok()
    }

    /// Ícono dibujado en código: la M roja con el corte cian sobre fondo oscuro
    /// (las mismas figuras que crates/mog-ui/ui/assets/icon.svg, en una grilla de 128).
    fn icon(size: i32) -> ksni::Icon {
        let m: [(f32, f32); 11] = [
            (10.0, 104.0),
            (10.0, 24.0),
            (32.0, 24.0),
            (52.0, 60.0),
            (72.0, 24.0),
            (94.0, 24.0),
            (94.0, 104.0),
            (74.0, 104.0),
            (74.0, 58.0),
            (52.0, 94.0),
            (30.0, 58.0),
        ];
        let m = [m.as_slice(), &[(30.0, 104.0)]].concat();
        let slash = [(100.0, 12.0), (120.0, 12.0), (100.0, 116.0), (80.0, 116.0)];
        let mut data = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                let p = (
                    (x as f32 + 0.5) * 128.0 / size as f32,
                    (y as f32 + 0.5) * 128.0 / size as f32,
                );
                let (r, g, b) = if inside(&slash, p) {
                    (0x2e, 0xf2, 0xff)
                } else if inside(&m, p) {
                    (0xff, 0x2a, 0x3d)
                } else {
                    (0x07, 0x01, 0x02)
                };
                data.extend_from_slice(&[0xff, r, g, b]); // ARGB
            }
        }
        ksni::Icon {
            width: size,
            height: size,
            data,
        }
    }

    fn inside(polygon: &[(f32, f32)], (x, y): (f32, f32)) -> bool {
        let mut inside = false;
        let mut j = polygon.len() - 1;
        for i in 0..polygon.len() {
            let (xi, yi) = polygon[i];
            let (xj, yj) = polygon[j];
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn icon_has_the_three_colors() {
            let icon = super::icon(32);
            assert_eq!(icon.data.len(), 32 * 32 * 4);
            let pixels: Vec<&[u8]> = icon.data.chunks(4).collect();
            assert!(pixels.iter().any(|p| p[1] == 0xff && p[2] == 0x2a), "rojo");
            assert!(pixels.iter().any(|p| p[1] == 0x2e), "cian");
            assert!(pixels.iter().any(|p| p[1] == 0x07), "fondo");
        }
    }
}
