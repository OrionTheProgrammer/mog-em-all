//! Punto de entrada: conecta el motor de lógica (`mog-core`) con la interfaz (`mog-ui`).
//! No contiene reglas de negocio ni dibujo.

mod boot;
mod clock;
mod commands;
mod finance;
mod schedule;
mod tasks;
mod training;

use std::cell::RefCell;
use std::rc::Rc;

use mog_core::{health, ranks};
use mog_storage::Store;
use mog_ui::{AppWindow, BmiResult, OperatorInfo, WeekStats};
use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.set_version(env!("CARGO_PKG_VERSION").into());

    apply_language(&ui, system_language());
    ui.on_select_language({
        let weak = ui.as_weak();
        move |lang| {
            if let Some(ui) = weak.upgrade() {
                apply_language(&ui, &lang);
            }
        }
    });

    ui.on_compute_bmi(|weight, height| compute_bmi(&weight, &height));
    commands::connect(&ui);

    // Se comparte con los módulos que vayan a escribir datos (entrenamientos, perfil).
    let (store, storage_status) = open_store();
    ui.set_storage_status(storage_status);
    let store = Rc::new(RefCell::new(store));
    refresh_progress(&ui, &store.borrow());
    training::connect(&ui, &store);
    let (_schedule_timer, refresh_schedule) = schedule::connect(&ui, &store);
    let refresh_tasks = Rc::new(tasks::connect(&ui, &store));
    let refresh_finance = finance::connect(&ui, &store);
    ui.on_tab_changed({
        let refresh_tasks = refresh_tasks.clone();
        move |_| {
            refresh_schedule();
            refresh_tasks();
            refresh_finance();
        }
    });
    // Las tareas cambian de grupo con la hora (hoy → vencida): refrescar cada minuto.
    let _tasks_timer = slint::Timer::default();
    _tasks_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(60),
        move || refresh_tasks(),
    );

    let _clock = clock::start(&ui);
    let _boot = boot::start(&ui);
    ui.run()
}

/// Abre la base de datos del usuario. Si no se puede, la app sigue con una base
/// en memoria para no quedar inutilizable, y la barra de estado avisa que no se
/// está guardando nada. El archivo original no se toca.
///
/// Devuelve también el estado para la interfaz: 0 normal, 1 esquema más nuevo,
/// 2 otro error.
fn open_store() -> (Store, i32) {
    let status = match dirs::data_dir().map(|dir| Store::default_path(&dir)) {
        Some(path) => match Store::open(&path) {
            Ok(store) => return (store, 0),
            Err(err) => {
                eprintln!("no se pudo abrir {}: {err}", path.display());
                if matches!(err, mog_storage::Error::NewerSchema { .. }) {
                    1
                } else {
                    2
                }
            }
        },
        None => 2,
    };
    let memory = Store::open_in_memory().expect("SQLite en memoria siempre debería abrir");
    (memory, status)
}

pub fn now() -> mog_storage::Timestamp {
    chrono::Utc::now().timestamp()
}

/// Lee nivel, racha y nombre desde la base de datos y los pasa a la interfaz.
pub fn refresh_progress(ui: &AppWindow, store: &Store) {
    let total_xp = store.total_xp().unwrap_or(0);
    ui.set_operator(operator_info(total_xp));
    let week = store.week_stats(now()).unwrap_or_default();
    ui.set_week(WeekStats {
        sessions: week.sessions as i32,
        streak_weeks: week.streak_weeks as i32,
    });
    let profile_name = store.profile().ok().flatten().map(|p| p.name);
    ui.set_user_name(user_name(profile_name).into());
}

/// Nombre para el saludo de la pantalla de carga: el del perfil o, si todavía
/// no hay perfil, el de la cuenta del sistema.
fn user_name(profile_name: Option<String>) -> String {
    profile_name
        .filter(|name| !name.trim().is_empty())
        .or_else(|| std::env::var("USER").ok())
        .or_else(|| std::env::var("USERNAME").ok())
        .unwrap_or_default()
        .to_uppercase()
}

/// Idiomas incluidos. El primero es el de respaldo.
pub const LANGUAGES: [&str; 2] = ["en", "es"];

fn system_language() -> &'static str {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    LANGUAGES
        .iter()
        .find(|code| locale.starts_with(*code))
        .copied()
        .unwrap_or(LANGUAGES[0])
}

/// Cambia el idioma de la interfaz. Devuelve `false` si no está disponible.
pub fn apply_language(ui: &AppWindow, lang: &str) -> bool {
    if !LANGUAGES.contains(&lang) {
        return false;
    }
    match slint::select_bundled_translation(lang) {
        Ok(()) => {
            ui.set_language(lang.into());
            true
        }
        Err(err) => {
            eprintln!("idioma '{lang}' no disponible: {err}");
            false
        }
    }
}

fn compute_bmi(weight: &str, height: &str) -> BmiResult {
    let parse = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok();
    match (parse(weight), parse(height)) {
        (Some(w), Some(h)) => bmi_result(w, h),
        _ => BmiResult::default(),
    }
}

pub fn bmi_result(weight_kg: f64, height_cm: f64) -> BmiResult {
    match health::bmi(weight_kg, height_cm) {
        Ok(bmi) => BmiResult {
            valid: true,
            value: bmi.value as f32,
            category: bmi.category as i32,
        },
        Err(_) => BmiResult::default(),
    }
}

fn operator_info(total_xp: u64) -> OperatorInfo {
    let progress = ranks::level_from_xp(total_xp);
    let to_i32 = |v: u64| i32::try_from(v).unwrap_or(i32::MAX);
    OperatorInfo {
        level: progress.level as i32,
        tier: ranks::tier_for_level(progress.level) as i32,
        xp_in_level: to_i32(progress.xp_in_level),
        xp_for_next: to_i32(progress.xp_for_next),
        total_xp: to_i32(total_xp),
        tier_progress: ranks::tier_progress(progress.level) as f32,
        next_tier_level: ranks::next_tier_level(progress.level).map_or(0, |l| l as i32),
        division: ranks::division(progress.level) as i32,
        next_division_level: ranks::next_division_level(progress.level).map_or(0, |l| l as i32),
    }
}
