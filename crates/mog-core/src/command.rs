//! Intérprete de la línea de comandos del HUD.
//!
//! Acepta palabras en español e inglés, sin importar mayúsculas ni tildes.
//! Devuelve un `Command`; quien lo ejecuta es `mog-app`.

/// Módulos de la app, en el mismo orden que las pestañas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Dashboard,
    /// Entrenamiento: rutinas y sesiones en una sola pestaña.
    Training,
    Tasks,
    Schedule,
    Finances,
    Settings,
}

impl Module {
    pub const ALL: [Module; 6] = [
        Module::Dashboard,
        Module::Training,
        Module::Tasks,
        Module::Schedule,
        Module::Finances,
        Module::Settings,
    ];

    fn from_word(word: &str) -> Option<Module> {
        Some(match word {
            "panel" | "dashboard" | "inicio" | "home" => Module::Dashboard,
            "entrenamiento" | "entrenar" | "ejercicio" | "ejercicios" | "rutina" | "rutinas"
            | "gym" | "training" | "workout" | "workouts" | "routine" | "routines" => {
                Module::Training
            }
            "tarea" | "tareas" | "task" | "tasks" => Module::Tasks,
            "horario" | "horarios" | "calendario" | "schedule" | "calendar" => Module::Schedule,
            "finanza" | "finanzas" | "dinero" | "finance" | "finances" | "money" => {
                Module::Finances
            }
            "ajustes" | "configuracion" | "config" | "settings" => Module::Settings,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Empty,
    Go(Module),
    Bmi {
        weight_kg: f64,
        height_cm: f64,
    },
    Language(String),
    Help,
    /// Palabra conocida pero mal usada (por ejemplo `imc` sin números).
    Usage(UsageOf),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageOf {
    Go,
    Bmi,
    Language,
}

pub fn parse(input: &str) -> Command {
    let normalized = normalize(input);
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let Some((&first, rest)) = words.split_first() else {
        return Command::Empty;
    };

    match first {
        "ir" | "go" | "abrir" | "open" => match rest.first().and_then(|w| Module::from_word(w)) {
            Some(module) => Command::Go(module),
            None => Command::Usage(UsageOf::Go),
        },
        "imc" | "bmi" => match (
            rest.first().and_then(|w| number(w)),
            rest.get(1).and_then(|w| number(w)),
        ) {
            (Some(weight_kg), Some(height_cm)) => Command::Bmi {
                weight_kg,
                height_cm,
            },
            _ => Command::Usage(UsageOf::Bmi),
        },
        "idioma" | "lang" | "language" => match rest.first() {
            Some(code) if code.len() == 2 => Command::Language((*code).to_string()),
            _ => Command::Usage(UsageOf::Language),
        },
        "ayuda" | "help" | "?" => Command::Help,
        word => match Module::from_word(word) {
            Some(module) if rest.is_empty() => Command::Go(module),
            _ => Command::Unknown,
        },
    }
}

fn number(word: &str) -> Option<f64> {
    word.replace(',', ".").parse().ok()
}

fn normalize(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        assert_eq!(parse("   "), Command::Empty);
    }

    #[test]
    fn navigation_in_both_languages() {
        assert_eq!(parse("ir rutinas"), Command::Go(Module::Training));
        assert_eq!(parse("entrenamiento"), Command::Go(Module::Training));
        assert_eq!(parse("go finances"), Command::Go(Module::Finances));
        assert_eq!(parse("CONFIGURACIÓN"), Command::Go(Module::Settings));
        assert_eq!(parse("ir"), Command::Usage(UsageOf::Go));
        assert_eq!(parse("ir marte"), Command::Usage(UsageOf::Go));
    }

    #[test]
    fn bmi_accepts_comma() {
        assert_eq!(
            parse("imc 78,5 176"),
            Command::Bmi {
                weight_kg: 78.5,
                height_cm: 176.0
            }
        );
        assert_eq!(parse("bmi 80"), Command::Usage(UsageOf::Bmi));
    }

    #[test]
    fn language_and_help() {
        assert_eq!(parse("idioma EN"), Command::Language("en".into()));
        assert_eq!(parse("lang"), Command::Usage(UsageOf::Language));
        assert_eq!(parse("ayuda"), Command::Help);
        assert_eq!(parse("bailar"), Command::Unknown);
    }
}
