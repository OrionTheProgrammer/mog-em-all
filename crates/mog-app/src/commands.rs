//! Ejecuta los comandos que se escriben en la línea de comandos del HUD.

use mog_core::command::{self, Command, Module, UsageOf};
use mog_ui::{AppWindow, CommandFeedback};
use slint::ComponentHandle;

// Deben coincidir con Labels.feedback en mog-ui.
const OPENED: i32 = 1;
const BMI: i32 = 2;
const LANGUAGE_CHANGED: i32 = 3;
const HELP: i32 = 4;
const UNKNOWN: i32 = 5;
const USAGE_GO: i32 = 6;
const USAGE_BMI: i32 = 7;
const USAGE_LANGUAGE: i32 = 8;
const BMI_OUT_OF_RANGE: i32 = 9;
const LANGUAGE_UNAVAILABLE: i32 = 10;

pub fn connect(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_run_command(move |text| {
        if let Some(ui) = weak.upgrade()
            && let Some(feedback) = run(&ui, &text)
        {
            ui.set_feedback(feedback);
        }
    });
}

pub fn run(ui: &AppWindow, text: &str) -> Option<CommandFeedback> {
    let kind = |kind| CommandFeedback {
        kind,
        ..Default::default()
    };
    Some(match command::parse(text) {
        Command::Empty => return None,
        Command::Go(module) => {
            let index = module_index(module);
            ui.set_current_module(index);
            CommandFeedback {
                kind: OPENED,
                module: index,
                ..Default::default()
            }
        }
        Command::Bmi {
            weight_kg,
            height_cm,
        } => {
            let result = crate::bmi_result(weight_kg, height_cm);
            if !result.valid {
                return Some(kind(BMI_OUT_OF_RANGE));
            }
            ui.set_bmi(result.clone());
            CommandFeedback {
                kind: BMI,
                value: result.value,
                category: result.category,
                ..Default::default()
            }
        }
        Command::Language(code) => {
            if crate::apply_language(ui, &code) {
                kind(LANGUAGE_CHANGED)
            } else {
                kind(LANGUAGE_UNAVAILABLE)
            }
        }
        Command::Help => kind(HELP),
        Command::Usage(UsageOf::Go) => kind(USAGE_GO),
        Command::Usage(UsageOf::Bmi) => kind(USAGE_BMI),
        Command::Usage(UsageOf::Language) => kind(USAGE_LANGUAGE),
        Command::Unknown => kind(UNKNOWN),
    })
}

fn module_index(module: Module) -> i32 {
    Module::ALL.iter().position(|m| *m == module).unwrap_or(0) as i32
}
