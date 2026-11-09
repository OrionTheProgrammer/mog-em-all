//! Catálogo de ejercicios incluidos, leído de `data/exercises.csv`.
//!
//! Cada ejercicio sabe qué músculos trabaja, con qué equipo se hace, cómo se mide,
//! qué patrón de movimiento es, si cuenta para una tabla de clase de fuerza y qué
//! meta se le pone por defecto al agregarlo a una rutina.

use std::sync::OnceLock;

use crate::ranks::Lift;
use crate::training::ExerciseKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Muscle {
    Chest,
    Back,
    Shoulders,
    Biceps,
    Triceps,
    Forearms,
    Quads,
    Hamstrings,
    Glutes,
    Calves,
    Core,
    FullBody,
    Cardio,
}

impl Muscle {
    pub const ALL: [Muscle; 13] = [
        Muscle::Chest,
        Muscle::Back,
        Muscle::Shoulders,
        Muscle::Biceps,
        Muscle::Triceps,
        Muscle::Forearms,
        Muscle::Quads,
        Muscle::Hamstrings,
        Muscle::Glutes,
        Muscle::Calves,
        Muscle::Core,
        Muscle::FullBody,
        Muscle::Cardio,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0) as u8
    }

    pub fn from_index(index: i64) -> Option<Muscle> {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    fn parse(code: &str) -> Option<Muscle> {
        Some(match code {
            "chest" => Muscle::Chest,
            "back" => Muscle::Back,
            "shoulders" => Muscle::Shoulders,
            "biceps" => Muscle::Biceps,
            "triceps" => Muscle::Triceps,
            "forearms" => Muscle::Forearms,
            "quads" => Muscle::Quads,
            "hamstrings" => Muscle::Hamstrings,
            "glutes" => Muscle::Glutes,
            "calves" => Muscle::Calves,
            "core" => Muscle::Core,
            "full_body" => Muscle::FullBody,
            "cardio" => Muscle::Cardio,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Equipment {
    Barbell,
    Dumbbell,
    Machine,
    Cable,
    Bodyweight,
    Kettlebell,
    Band,
    Smith,
    EzBar,
    None,
}

impl Equipment {
    pub const ALL: [Equipment; 10] = [
        Equipment::Barbell,
        Equipment::Dumbbell,
        Equipment::Machine,
        Equipment::Cable,
        Equipment::Bodyweight,
        Equipment::Kettlebell,
        Equipment::Band,
        Equipment::Smith,
        Equipment::EzBar,
        Equipment::None,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|e| *e == self).unwrap_or(0) as u8
    }

    fn parse(code: &str) -> Option<Equipment> {
        Some(match code {
            "barbell" => Equipment::Barbell,
            "dumbbell" => Equipment::Dumbbell,
            "machine" => Equipment::Machine,
            "cable" => Equipment::Cable,
            "bodyweight" => Equipment::Bodyweight,
            "kettlebell" => Equipment::Kettlebell,
            "band" => Equipment::Band,
            "smith" => Equipment::Smith,
            "ez_bar" => Equipment::EzBar,
            "none" => Equipment::None,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    Squat,
    Hinge,
    Lunge,
    HorizontalPush,
    VerticalPush,
    HorizontalPull,
    VerticalPull,
    Isolation,
    Carry,
    Core,
    Cardio,
    Olympic,
}

impl Pattern {
    pub const ALL: [Pattern; 12] = [
        Pattern::Squat,
        Pattern::Hinge,
        Pattern::Lunge,
        Pattern::HorizontalPush,
        Pattern::VerticalPush,
        Pattern::HorizontalPull,
        Pattern::VerticalPull,
        Pattern::Isolation,
        Pattern::Carry,
        Pattern::Core,
        Pattern::Cardio,
        Pattern::Olympic,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0) as u8
    }

    /// Ejercicios que mueven varias articulaciones a la vez.
    pub fn is_compound(self) -> bool {
        !matches!(self, Pattern::Isolation | Pattern::Core | Pattern::Cardio)
    }

    fn parse(code: &str) -> Option<Pattern> {
        Some(match code {
            "squat" => Pattern::Squat,
            "hinge" => Pattern::Hinge,
            "lunge" => Pattern::Lunge,
            "horizontal_push" => Pattern::HorizontalPush,
            "vertical_push" => Pattern::VerticalPush,
            "horizontal_pull" => Pattern::HorizontalPull,
            "vertical_pull" => Pattern::VerticalPull,
            "isolation" => Pattern::Isolation,
            "carry" => Pattern::Carry,
            "core" => Pattern::Core,
            "cardio" => Pattern::Cardio,
            "olympic" => Pattern::Olympic,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogExercise {
    pub code: &'static str,
    pub name_en: &'static str,
    pub name_es: &'static str,
    pub muscle: Muscle,
    pub secondary: Vec<Muscle>,
    pub equipment: Equipment,
    pub kind: ExerciseKind,
    pub pattern: Pattern,
    pub lift: Option<Lift>,
    pub reps_min: u32,
    pub reps_max: u32,
    pub rest_s: u32,
}

const DATA: &str = include_str!("../data/exercises.csv");

/// Todos los ejercicios del catálogo, en el orden del archivo.
pub fn all() -> &'static [CatalogExercise] {
    static CATALOG: OnceLock<Vec<CatalogExercise>> = OnceLock::new();
    CATALOG.get_or_init(|| parse(DATA).expect("data/exercises.csv tiene un error"))
}

pub fn find(code: &str) -> Option<&'static CatalogExercise> {
    all().iter().find(|e| e.code == code)
}

/// Busca por nombre en inglés o español, sin importar mayúsculas ni tildes.
/// Cada palabra de la búsqueda tiene que aparecer en alguno de los dos nombres.
pub fn matches(exercise: &CatalogExercise, query: &str) -> bool {
    text_matches(
        &format!(
            "{} {} {}",
            exercise.name_en, exercise.name_es, exercise.code
        ),
        query,
    )
}

/// La misma búsqueda, para un texto cualquiera (por ejemplo, un ejercicio propio).
pub fn text_matches(haystack: &str, query: &str) -> bool {
    let haystack = fold(haystack);
    fold(query)
        .split_whitespace()
        .all(|word| haystack.contains(word))
}

fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            '_' | '-' => ' ',
            other => other,
        })
        .collect()
}

fn parse_lift(code: &str) -> Option<Option<Lift>> {
    Some(match code {
        "" => None,
        "squat" => Some(Lift::Squat),
        "bench_press" => Some(Lift::BenchPress),
        "deadlift" => Some(Lift::Deadlift),
        "overhead_press" => Some(Lift::OverheadPress),
        "barbell_row" => Some(Lift::BarbellRow),
        "pull_up" => Some(Lift::PullUp),
        _ => return None,
    })
}

fn parse_kind(code: &str) -> Option<ExerciseKind> {
    Some(match code {
        "weighted" => ExerciseKind::Weighted,
        "bodyweight" => ExerciseKind::Bodyweight,
        "timed" => ExerciseKind::Timed,
        _ => return None,
    })
}

fn parse(data: &'static str) -> Result<Vec<CatalogExercise>, String> {
    let mut rows = data
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header = rows.next().ok_or("falta el encabezado")?;
    if header
        != "code,name_en,name_es,muscle,secondary,equipment,kind,pattern,lift,reps_min,reps_max,rest_s"
    {
        return Err(format!("encabezado inesperado: {header}"));
    }
    rows.map(|line| {
        let f: Vec<&'static str> = line.split(',').collect();
        let err = |what: &str| format!("{what} en la fila: {line}");
        if f.len() != 12 {
            return Err(err("cantidad de columnas"));
        }
        let number = |s: &str, what: &str| s.parse::<u32>().map_err(|_| err(what));
        Ok(CatalogExercise {
            code: f[0],
            name_en: f[1],
            name_es: f[2],
            muscle: Muscle::parse(f[3]).ok_or_else(|| err("músculo"))?,
            secondary: f[4]
                .split('|')
                .filter(|s| !s.is_empty())
                .map(|s| Muscle::parse(s).ok_or_else(|| err("músculo secundario")))
                .collect::<Result<_, _>>()?,
            equipment: Equipment::parse(f[5]).ok_or_else(|| err("equipo"))?,
            kind: parse_kind(f[6]).ok_or_else(|| err("tipo"))?,
            pattern: Pattern::parse(f[7]).ok_or_else(|| err("patrón"))?,
            lift: parse_lift(f[8]).ok_or_else(|| err("tabla de fuerza"))?,
            reps_min: number(f[9], "reps mínimas")?,
            reps_max: number(f[10], "reps máximas")?,
            rest_s: number(f[11], "descanso")?,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_is_valid() {
        let catalog = all();
        assert!(
            catalog.len() >= 100,
            "el catálogo debería tener al menos 100 ejercicios"
        );
        let mut codes = HashSet::new();
        for e in catalog {
            assert!(codes.insert(e.code), "código repetido: {}", e.code);
            assert!(e.reps_min <= e.reps_max, "{}", e.code);
            assert!(
                !e.secondary.contains(&e.muscle),
                "{} repite el músculo principal",
                e.code
            );
            assert!(e.rest_s <= 600, "{}", e.code);
            assert!(!e.name_en.is_empty() && !e.name_es.is_empty(), "{}", e.code);
        }
    }

    #[test]
    fn original_codes_still_exist() {
        // Las bases de datos antiguas guardan estos códigos.
        for code in [
            "squat",
            "bench_press",
            "deadlift",
            "overhead_press",
            "barbell_row",
            "pull_up",
            "dip",
            "push_up",
            "plank",
            "run",
        ] {
            assert!(find(code).is_some(), "falta {code}");
        }
    }

    #[test]
    fn each_strength_table_has_one_exercise() {
        for lift in [
            Lift::Squat,
            Lift::BenchPress,
            Lift::Deadlift,
            Lift::OverheadPress,
            Lift::BarbellRow,
            Lift::PullUp,
        ] {
            assert_eq!(
                all().iter().filter(|e| e.lift == Some(lift)).count(),
                1,
                "{lift:?}"
            );
        }
    }

    #[test]
    fn search_in_both_languages() {
        let bench = find("bench_press").unwrap();
        assert!(matches(bench, "banca"));
        assert!(matches(bench, "BENCH"));
        assert!(matches(bench, "press banca"));
        assert!(!matches(bench, "sentadilla"));
        let squat = find("bulgarian_split_squat").unwrap();
        assert!(matches(squat, "bulgara"), "sin tilde también");
        assert!(text_matches("Curl martillo con cuerda", "martillo"));
    }

    #[test]
    fn bad_rows_are_reported() {
        assert!(parse("code,name_en,name_es,muscle,secondary,equipment,kind,pattern,lift,reps_min,reps_max,rest_s\nx,X,X,pecho,,barbell,weighted,squat,,1,1,1").is_err());
        assert!(parse("otro encabezado").is_err());
    }
}
