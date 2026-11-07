//! Punto de entrada: conecta el motor de lógica (`mog-core`) con la interfaz (`mog-ui`).
//! No contiene reglas de negocio ni dibujo.

mod boot;
mod clock;
mod commands;

use mog_core::{health, ranks};
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

    // Hasta que exista la base de datos, el perfil arranca vacío.
    ui.set_operator(operator_info(0));
    ui.set_week(WeekStats {
        sessions: 0,
        streak_weeks: 0,
    });

    let _clock = clock::start(&ui);
    let _boot = boot::start(&ui);
    ui.run()
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
    }
}
