//! Finanzas: montos en la moneda elegida, categorías de gasto, presupuestos y el
//! intérprete de la entrada rápida ("12.500 almuerzo", "+20000 venta polera #ventas").
//!
//! Los montos se guardan como enteros en la unidad mínima de la moneda
//! (pesos chilenos tal cual, centavos en dólares).

use crate::schedule::DayNumber;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Currency {
    pub code: &'static str,
    pub symbol: &'static str,
    /// Decimales que usa: 0 en CLP, 2 en USD.
    pub decimals: u8,
}

pub const CURRENCIES: [Currency; 8] = [
    Currency {
        code: "CLP",
        symbol: "$",
        decimals: 0,
    },
    Currency {
        code: "USD",
        symbol: "US$",
        decimals: 2,
    },
    Currency {
        code: "EUR",
        symbol: "€",
        decimals: 2,
    },
    Currency {
        code: "MXN",
        symbol: "MX$",
        decimals: 2,
    },
    Currency {
        code: "ARS",
        symbol: "AR$",
        decimals: 2,
    },
    Currency {
        code: "PEN",
        symbol: "S/",
        decimals: 2,
    },
    Currency {
        code: "COP",
        symbol: "COL$",
        decimals: 0,
    },
    Currency {
        code: "UYU",
        symbol: "UY$",
        decimals: 2,
    },
];

pub fn currency(code: &str) -> Currency {
    CURRENCIES
        .iter()
        .copied()
        .find(|c| c.code == code)
        .unwrap_or(CURRENCIES[0])
}

/// Escribe un monto con separador de miles y de decimales del idioma:
/// `format_amount(1250000, CLP, '.', ',')` → "$1.250.000".
pub fn format_amount(minor: i64, currency: Currency, thousands: char, decimal: char) -> String {
    let negative = minor < 0;
    let minor = minor.unsigned_abs();
    let factor = 10u64.pow(u32::from(currency.decimals));
    let (whole, cents) = (minor / factor, minor % factor);
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(thousands);
        }
        grouped.push(c);
    }
    let mut text = format!(
        "{}{}{grouped}",
        if negative { "-" } else { "" },
        currency.symbol
    );
    if currency.decimals > 0 {
        text.push(decimal);
        text.push_str(&format!(
            "{cents:0width$}",
            width = usize::from(currency.decimals)
        ));
    }
    text
}

/// Lee un monto escrito por el usuario: "12.500", "12500", "$12.500", "12,50",
/// "1.234,56", "15k", "1,5k", "2m" (millones). Devuelve unidades mínimas.
pub fn parse_amount(text: &str, currency: Currency) -> Option<i64> {
    let mut t = text
        .trim()
        .trim_start_matches(currency.symbol)
        .trim_start_matches('$')
        .trim()
        .to_lowercase();
    let multiplier = if t.ends_with('k') {
        t.pop();
        1_000.0
    } else if t.ends_with("mm") || t.ends_with('m') {
        t = t.trim_end_matches('m').to_string();
        1_000_000.0
    } else {
        1.0
    };
    if t.is_empty()
        || !t
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return None;
    }
    // El último separador es decimal si le siguen 1 o 2 dígitos y la moneda usa
    // decimales (o si hay multiplicador, como en "1,5k"). Los demás son de miles.
    let last_sep = t.rfind(['.', ',']);
    let decimal_sep = last_sep.filter(|&i| {
        let after = t.len() - i - 1;
        (1..=2).contains(&after) && (currency.decimals > 0 || multiplier > 1.0)
    });
    let (int_part, frac_part) = match decimal_sep {
        Some(i) => (&t[..i], &t[i + 1..]),
        None => (t.as_str(), ""),
    };
    let int_digits: String = int_part.chars().filter(|c| c.is_ascii_digit()).collect();
    let value: f64 = format!(
        "{}.{}",
        if int_digits.is_empty() {
            "0"
        } else {
            &int_digits
        },
        if frac_part.is_empty() { "0" } else { frac_part }
    )
    .parse()
    .ok()?;
    let minor = (value * multiplier * 10f64.powi(i32::from(currency.decimals))).round();
    (minor > 0.0 && minor < 1e15).then_some(minor as i64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryKind {
    #[default]
    Expense,
    Income,
}

/// Ingresos: fijos (sueldo, se esperan cada mes) o informales / no planificados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IncomeType {
    Fixed,
    #[default]
    Informal,
}

/// Categorías de gasto que vienen con la app, por código (la interfaz las traduce).
pub const DEFAULT_CATEGORIES: [&str; 10] = [
    "food",
    "transport",
    "home",
    "bills",
    "health",
    "fun",
    "education",
    "subscriptions",
    "gym",
    "other",
];

/// Palabras que delatan una categoría, en español e inglés (sin tildes).
const KEYWORDS: &[(&str, &[&str])] = &[
    (
        "food",
        &[
            "almuerzo",
            "comida",
            "cena",
            "desayuno",
            "super",
            "supermercado",
            "pan",
            "cafe",
            "completo",
            "sushi",
            "pizza",
            "lunch",
            "dinner",
            "food",
            "groceries",
            "coffee",
            "once",
            "feria",
            "jumbo",
            "lider",
        ],
    ),
    (
        "transport",
        &[
            "uber",
            "didi",
            "micro",
            "metro",
            "bip",
            "bencina",
            "taxi",
            "bus",
            "pasaje",
            "estacionamiento",
            "gas",
            "fuel",
            "parking",
            "cabify",
            "peaje",
        ],
    ),
    (
        "home",
        &[
            "arriendo",
            "casa",
            "dividendo",
            "mueble",
            "rent",
            "home",
            "limpieza",
            "ferreteria",
        ],
    ),
    (
        "bills",
        &[
            "luz",
            "agua",
            "internet",
            "celular",
            "telefono",
            "plan",
            "cuenta",
            "electricity",
            "water",
            "phone",
            "bill",
        ],
    ),
    (
        "health",
        &[
            "farmacia", "remedio", "doctor", "medico", "dentista", "isapre", "fonasa", "pharmacy",
            "medicine",
        ],
    ),
    (
        "fun",
        &[
            "cine",
            "salida",
            "carrete",
            "juego",
            "steam",
            "bar",
            "concierto",
            "movie",
            "game",
            "party",
            "entrada",
        ],
    ),
    (
        "education",
        &[
            "libro",
            "curso",
            "universidad",
            "duoc",
            "matricula",
            "arancel",
            "book",
            "course",
            "tuition",
            "fotocopia",
        ],
    ),
    (
        "subscriptions",
        &[
            "netflix",
            "spotify",
            "youtube",
            "disney",
            "hbo",
            "prime",
            "suscripcion",
            "subscription",
            "icloud",
            "chatgpt",
        ],
    ),
    (
        "gym",
        &[
            "gimnasio",
            "gym",
            "proteina",
            "creatina",
            "suplemento",
            "protein",
            "smartfit",
        ],
    ),
];

/// Adivina la categoría de un gasto por sus palabras. `None` si no hay pistas.
pub fn guess_category(note: &str) -> Option<&'static str> {
    let words: Vec<String> = fold(note)
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_string)
        .collect();
    KEYWORDS
        .iter()
        .find(|(_, keys)| words.iter().any(|w| keys.contains(&w.as_str())))
        .map(|(code, _)| *code)
}

/// Lo que se entendió de la entrada rápida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickEntry {
    pub kind: EntryKind,
    pub amount: i64,
    pub note: String,
    /// Texto después de `#`, para buscar la categoría por nombre.
    pub tag: Option<String>,
    /// Categoría deducida de las palabras, si no hubo `#`.
    pub guessed: Option<&'static str>,
    pub day: DayNumber,
}

/// "12.500 almuerzo", "-12500 uber ayer", "+20000 venta polera #ventas".
///
/// El monto es el número que parece un monto (con `$`, `k`, `m` o separador de
/// miles); si ninguno lo parece, el más grande. `+` delante lo vuelve ingreso. Los
/// demás números quedan en la nota ("3 poleras"). Sin monto, no hay entrada.
pub fn parse_quick(input: &str, currency: Currency, today: DayNumber) -> Option<QuickEntry> {
    let words: Vec<&str> = input.split_whitespace().collect();
    let candidates: Vec<(usize, EntryKind, i64, bool)> = words
        .iter()
        .enumerate()
        .filter_map(|(i, word)| {
            let (sign, rest) = match word.strip_prefix('+') {
                Some(rest) => (EntryKind::Income, rest),
                None => (EntryKind::Expense, word.strip_prefix('-').unwrap_or(word)),
            };
            let value = parse_amount(rest, currency)?;
            let looks_like_money = rest.starts_with('$')
                || rest.ends_with(['k', 'K', 'm', 'M'])
                || rest.contains(['.', ','])
                || word.starts_with('+');
            Some((i, sign, value, looks_like_money))
        })
        .collect();
    let chosen = candidates
        .iter()
        .rev()
        .find(|c| c.3)
        .or_else(|| candidates.iter().max_by_key(|c| c.2))
        .copied();
    let (amount_index, kind, amount, _) = chosen?;

    let mut tag = None;
    let mut day = today;
    let mut note: Vec<&str> = Vec::new();
    for (i, word) in words.iter().enumerate() {
        if i == amount_index {
            continue;
        }
        let folded = fold(word);
        if let Some(t) = word.strip_prefix('#').filter(|t| !t.is_empty()) {
            tag = Some(t.to_string());
        } else if folded == "ayer" || folded == "yesterday" {
            day = today - 1;
        } else if folded == "anteayer" {
            day = today - 2;
        } else if folded == "hoy" || folded == "today" {
            day = today;
        } else {
            note.push(word);
        }
    }
    let note = note.join(" ");
    let guessed = if tag.is_none() && kind == EntryKind::Expense {
        guess_category(&note)
    } else {
        None
    };
    Some(QuickEntry {
        kind,
        amount,
        note,
        tag,
        guessed,
        day,
    })
}

/// Cómo va un presupuesto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetState {
    NoBudget,
    Fine,
    /// Pasó el 80 %.
    Warning,
    Over,
}

pub fn budget_state(spent: i64, budget: Option<i64>) -> BudgetState {
    match budget {
        None | Some(0) => BudgetState::NoBudget,
        Some(b) if spent > b => BudgetState::Over,
        Some(b) if spent * 10 >= b * 8 => BudgetState::Warning,
        Some(_) => BudgetState::Fine,
    }
}

fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLP: Currency = CURRENCIES[0];
    const USD: Currency = CURRENCIES[1];

    #[test]
    fn formatting() {
        assert_eq!(format_amount(1_250_000, CLP, '.', ','), "$1.250.000");
        assert_eq!(format_amount(990, CLP, '.', ','), "$990");
        assert_eq!(format_amount(-12_500, CLP, '.', ','), "-$12.500");
        assert_eq!(format_amount(123_456, USD, ',', '.'), "US$1,234.56");
        assert_eq!(format_amount(5, USD, ',', '.'), "US$0.05");
    }

    #[test]
    fn parsing_amounts() {
        assert_eq!(parse_amount("12.500", CLP), Some(12_500));
        assert_eq!(parse_amount("12500", CLP), Some(12_500));
        assert_eq!(parse_amount("$1.250.000", CLP), Some(1_250_000));
        assert_eq!(parse_amount("15k", CLP), Some(15_000));
        assert_eq!(parse_amount("1,5k", CLP), Some(1_500));
        assert_eq!(parse_amount("2m", CLP), Some(2_000_000));
        assert_eq!(parse_amount("12,50", USD), Some(1_250));
        assert_eq!(parse_amount("1.234,56", USD), Some(123_456));
        assert_eq!(parse_amount("1,234.56", USD), Some(123_456));
        assert_eq!(
            parse_amount("1.000", USD),
            Some(100_000),
            "tres dígitos: separador de miles"
        );
        assert_eq!(parse_amount("0", CLP), None);
        assert_eq!(parse_amount("almuerzo", CLP), None);
        assert_eq!(parse_amount("12a", CLP), None);
    }

    #[test]
    fn quick_entries() {
        let e = parse_quick("12.500 almuerzo", CLP, 100).unwrap();
        assert_eq!(
            (e.kind, e.amount, e.note.as_str(), e.guessed, e.day),
            (EntryKind::Expense, 12_500, "almuerzo", Some("food"), 100)
        );

        let e = parse_quick("+20000 venta polera #ventas", CLP, 100).unwrap();
        assert_eq!(
            (e.kind, e.amount, e.tag.as_deref(), e.guessed),
            (EntryKind::Income, 20_000, Some("ventas"), None)
        );

        let e = parse_quick("uber al centro 4.300 ayer", CLP, 100).unwrap();
        assert_eq!(
            (e.amount, e.note.as_str(), e.guessed, e.day),
            (4_300, "uber al centro", Some("transport"), 99)
        );

        let e = parse_quick("Netflix 7990", CLP, 100).unwrap();
        assert_eq!(e.guessed, Some("subscriptions"));

        assert!(
            parse_quick("almuerzo con Nico", CLP, 100).is_none(),
            "sin monto no hay entrada"
        );
        let e = parse_quick("compré 3 poleras 15k", CLP, 100).unwrap();
        assert_eq!(
            (e.amount, e.note.as_str()),
            (15_000, "compré 3 poleras"),
            "el que parece monto, no el primero"
        );
        assert_eq!(
            parse_quick("2 completos 3500", CLP, 100).unwrap().amount,
            3_500,
            "si ninguno lo parece, el más grande"
        );
    }

    #[test]
    fn budgets() {
        assert_eq!(budget_state(500, None), BudgetState::NoBudget);
        assert_eq!(budget_state(700, Some(1000)), BudgetState::Fine);
        assert_eq!(budget_state(800, Some(1000)), BudgetState::Warning);
        assert_eq!(budget_state(1000, Some(1000)), BudgetState::Warning);
        assert_eq!(budget_state(1001, Some(1000)), BudgetState::Over);
    }

    #[test]
    fn every_keyword_category_is_a_default_category() {
        for (code, _) in KEYWORDS {
            assert!(DEFAULT_CATEGORIES.contains(code), "{code}");
        }
    }
}
