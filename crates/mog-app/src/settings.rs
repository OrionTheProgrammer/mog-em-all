//! Pestaña AJUSTES y registro inicial: perfil, moneda y datos.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use chrono::{Local, TimeZone};
use mog_core::finance;
use mog_core::ranks::BodyProfile;
use mog_storage::{Profile, Store};
use mog_ui::{AppWindow, SettingsState};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

type SharedStore = Rc<RefCell<Store>>;

pub fn connect(
    ui: &AppWindow,
    store: &SharedStore,
    db_path: Option<PathBuf>,
    refresh_all: Rc<dyn Fn()>,
) {
    let state = ui.global::<SettingsState>();
    state.set_version(env!("CARGO_PKG_VERSION").into());
    state.set_currencies(ModelRc::new(VecModel::from(
        finance::CURRENCIES
            .iter()
            .map(|c| SharedString::from(c.code))
            .collect::<Vec<_>>(),
    )));
    state.set_db_path(
        db_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
            .into(),
    );
    load(ui, &store.borrow());
    {
        let store = store.borrow();
        let skipped = store.setting("onboarding_skipped").ok().flatten().is_some();
        let has_profile = store.profile().ok().flatten().is_some();
        state.set_onboarding(!has_profile && !skipped);
    }

    state.on_save_profile({
        let (weak, store, refresh_all) = (ui.as_weak(), store.clone(), refresh_all.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            if save_profile(&ui, &store.borrow()) {
                ui.global::<SettingsState>().set_profile_saved(true);
                crate::refresh_progress(&ui, &store.borrow());
                refresh_all();
            }
        }
    });
    state.on_finish_onboarding({
        let (weak, store, refresh_all) = (ui.as_weak(), store.clone(), refresh_all.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            if save_profile(&ui, &store.borrow()) {
                ui.global::<SettingsState>().set_onboarding(false);
                crate::refresh_progress(&ui, &store.borrow());
                refresh_all();
            }
        }
    });
    state.on_skip_onboarding({
        let (weak, store) = (ui.as_weak(), store.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                let _ = store.borrow().set_setting("onboarding_skipped", "1");
                ui.global::<SettingsState>().set_onboarding(false);
            }
        }
    });
    state.on_set_currency({
        let (weak, store, refresh_all) = (ui.as_weak(), store.clone(), refresh_all.clone());
        move |code| {
            let Some(ui) = weak.upgrade() else { return };
            if finance::CURRENCIES.iter().any(|c| c.code == code.as_str()) {
                let _ = store.borrow().set_setting("currency", &code);
                ui.global::<SettingsState>().set_currency(code);
                refresh_all();
            }
        }
    });
    state.on_backup_now({
        let (weak, store, path) = (ui.as_weak(), store.clone(), db_path.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let Some(dir) = path.as_ref().and_then(|p| p.parent()) else {
                return;
            };
            let target = dir
                .join("copias")
                .join(format!("mog-{}.db", Local::now().format("%Y%m%d-%H%M%S")));
            match store.borrow().backup_to(&target) {
                Ok(()) => ui
                    .global::<SettingsState>()
                    .set_backup_message(target.display().to_string().into()),
                Err(err) => eprintln!("copia de seguridad: {err}"),
            }
        }
    });
    state.on_preview_rank({
        let weak = ui.as_weak();
        move |tier| {
            if let Some(ui) = weak.upgrade() {
                ui.set_rank_up_tier(tier.clamp(0, 6));
                ui.set_rank_up_division(1);
                ui.set_rank_up_full(true);
                ui.set_rank_up_open(true);
            }
        }
    });
    state.on_open_folder({
        let path = db_path.clone();
        move || {
            if let Some(dir) = path.as_ref().and_then(|p| p.parent()) {
                open_folder(dir);
            }
        }
    });
}

/// Llena los campos con lo guardado.
pub fn load(ui: &AppWindow, store: &Store) {
    let state = ui.global::<SettingsState>();
    let profile = store.profile().ok().flatten();
    let comma = ui.get_language() == "es";
    let number = |v: Option<f64>| {
        v.map(|n| {
            let text = format!("{n}");
            if comma { text.replace('.', ",") } else { text }
        })
        .unwrap_or_default()
    };
    state.set_name(
        profile
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default()
            .into(),
    );
    state.set_weight(number(profile.as_ref().and_then(|p| p.bodyweight_kg)).into());
    state.set_height(number(profile.as_ref().and_then(|p| p.height_cm)).into());
    state.set_body(match profile.as_ref().map(|p| p.body_profile) {
        Some(BodyProfile::Male) => 0,
        Some(BodyProfile::Female) => 1,
        _ => 2,
    });
    state.set_weekly_goal(profile.as_ref().map_or(3, |p| p.weekly_goal as i32));
    state.set_currency(crate::finance::currency_of(store).code.into());
    state.set_schema_version(store.schema_version().unwrap_or(0) as i32);
    let history: Vec<SharedString> = store
        .migration_history()
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let date = Local
                .timestamp_opt(m.applied_at, 0)
                .single()
                .map(|d| d.format("%d/%m/%Y").to_string())
                .unwrap_or_default();
            format!("v{} · {} · {date}", m.version, m.name).into()
        })
        .collect();
    state.set_migrations(ModelRc::new(VecModel::from(history)));
}

/// Valida y guarda el perfil. Devuelve si se guardó.
fn save_profile(ui: &AppWindow, store: &Store) -> bool {
    let state = ui.global::<SettingsState>();
    state.set_profile_saved(false);
    let name = state.get_name().trim().to_string();
    if name.is_empty() || name.chars().count() > 40 {
        state.set_profile_error(1);
        return false;
    }
    let number = |text: &str, range: std::ops::RangeInclusive<f64>| -> Result<Option<f64>, ()> {
        let text = text.trim().replace(',', ".");
        if text.is_empty() {
            return Ok(None);
        }
        text.parse::<f64>()
            .ok()
            .filter(|n| range.contains(n))
            .map(Some)
            .ok_or(())
    };
    let Ok(weight) = number(&state.get_weight(), 20.0..=400.0) else {
        state.set_profile_error(2);
        return false;
    };
    let Ok(height) = number(&state.get_height(), 100.0..=250.0) else {
        state.set_profile_error(3);
        return false;
    };
    let profile = Profile {
        name,
        bodyweight_kg: weight,
        height_cm: height,
        body_profile: match state.get_body() {
            0 => BodyProfile::Male,
            1 => BodyProfile::Female,
            _ => BodyProfile::Unspecified,
        },
        weekly_goal: state.get_weekly_goal().clamp(1, 7) as u32,
    };
    state.set_profile_error(0);
    store.save_profile(&profile, crate::now()).is_ok()
}

fn open_folder(dir: &std::path::Path) {
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(err) = std::process::Command::new(opener).arg(dir).spawn() {
        eprintln!("no se pudo abrir {}: {err}", dir.display());
    }
}
