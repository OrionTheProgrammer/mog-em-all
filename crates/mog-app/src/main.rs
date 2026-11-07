//! Punto de entrada: conecta el motor de lógica (`mog-core`) con la interfaz (`mog-ui`).
//! Aquí no hay reglas de negocio ni dibujo, solo el cableado entre ambos.

mod boot;

use mog_core::{health, ranks};
use mog_ui::{AppWindow, BmiResult, OperatorInfo};
use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.set_version(env!("CARGO_PKG_VERSION").into());

    let language = system_language();
    apply_language(&ui, language);

    ui.on_select_language({
        let weak = ui.as_weak();
        move |lang| {
            if let Some(ui) = weak.upgrade() {
                apply_language(&ui, &lang);
            }
        }
    });

    ui.on_compute_bmi(|weight, height| compute_bmi(&weight, &height));
    ui.set_operator(operator_info(0));

    let _boot = boot::start(&ui);
    ui.run()
}

/// Idiomas incluidos. El primero es el de respaldo.
const LANGUAGES: [&str; 2] = ["en", "es"];

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

fn apply_language(ui: &AppWindow, lang: &str) {
    match slint::select_bundled_translation(lang) {
        Ok(()) => ui.set_language(lang.into()),
        Err(err) => eprintln!("idioma '{lang}' no disponible: {err}"),
    }
}

fn compute_bmi(weight: &str, height: &str) -> BmiResult {
    let parse = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok();
    match (parse(weight), parse(height)) {
        (Some(w), Some(h)) => match health::bmi(w, h) {
            Ok(bmi) => BmiResult {
                valid: true,
                value: bmi.value as f32,
                category: bmi.category as i32,
            },
            Err(_) => BmiResult::default(),
        },
        _ => BmiResult::default(),
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
    }
}
