//! Tareas: prioridad, repetición, grupos por fecha y el intérprete de la entrada
//! rápida ("pagar arriendo mañana 18:00 !alta #casa").

use crate::schedule::{self, DayNumber, Minute};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

impl Priority {
    pub const ALL: [Priority; 3] = [Priority::Low, Priority::Normal, Priority::High];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(1) as u8
    }

    pub fn from_index(index: i64) -> Priority {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    Never,
    Daily,
    Weekly,
    Monthly,
}

impl Repeat {
    pub const ALL: [Repeat; 4] = [
        Repeat::Never,
        Repeat::Daily,
        Repeat::Weekly,
        Repeat::Monthly,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(0) as u8
    }

    pub fn from_index(index: i64) -> Repeat {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
            .unwrap_or_default()
    }

    /// Siguiente fecha después de `day`. Sin fecha, una tarea que se repite vuelve a `today`.
    pub fn next(self, day: DayNumber) -> Option<DayNumber> {
        match self {
            Repeat::Never => None,
            Repeat::Daily => Some(day + 1),
            Repeat::Weekly => Some(day + 7),
            Repeat::Monthly => {
                let (y, m, d) = civil_from_days(day);
                let (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                // 31 de enero → 28/29 de febrero.
                Some(days_from_civil(y, m, d.min(days_in_month(y, m))))
            }
        }
    }
}

/// Grupo en que se muestra una tarea pendiente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bucket {
    Overdue,
    Today,
    Tomorrow,
    ThisWeek,
    Later,
    NoDate,
}

impl Bucket {
    pub const ALL: [Bucket; 6] = [
        Bucket::Overdue,
        Bucket::Today,
        Bucket::Tomorrow,
        Bucket::ThisWeek,
        Bucket::Later,
        Bucket::NoDate,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|b| *b == self).unwrap_or(0) as u8
    }
}

/// Grupo de una tarea según su fecha. Hoy con hora ya pasada cuenta como vencida.
pub fn bucket(
    due_day: Option<DayNumber>,
    due_min: Option<Minute>,
    today: DayNumber,
    now_min: Minute,
) -> Bucket {
    let Some(day) = due_day else {
        return Bucket::NoDate;
    };
    if day < today || (day == today && due_min.is_some_and(|m| m <= now_min)) {
        return Bucket::Overdue;
    }
    if day == today {
        return Bucket::Today;
    }
    if day == today + 1 {
        return Bucket::Tomorrow;
    }
    // Hasta el domingo de esta semana.
    if day < schedule::monday_of(today) + 7 {
        return Bucket::ThisWeek;
    }
    Bucket::Later
}

/// Lo que se entendió de la entrada rápida.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Draft {
    pub title: String,
    pub due_day: Option<DayNumber>,
    pub due_min: Option<Minute>,
    pub priority: Priority,
    pub repeat: Repeat,
    pub tag: Option<String>,
}

/// Interpreta "título + palabras clave". Las palabras reconocidas se quitan del título.
///
/// - Fecha: `hoy`, `mañana`, `pasado`, un día de la semana (`lunes`, `vie`), `24/11`.
/// - Hora: `18:30`, `9h`, `9am`, `9pm`.
/// - Prioridad: `!alta`, `!baja`, `!!` (alta), `!` (normal).
/// - Repetición: `diario`, `semanal`, `mensual` (o `cada día`, `cada semana`, `cada mes`).
/// - Etiqueta: `#casa`.
///
/// Con hora y sin fecha, la tarea es para hoy (o mañana si esa hora ya pasó).
pub fn parse_quick(input: &str, today: DayNumber, now_min: Minute) -> Draft {
    let mut draft = Draft::default();
    let mut title: Vec<&str> = Vec::new();
    let words: Vec<&str> = input.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let word = words[i];
        let lower = fold(word);
        let next = words.get(i + 1).map(|w| fold(w));
        let mut consumed = 1;
        if let Some(tag) = word.strip_prefix('#').filter(|t| !t.is_empty()) {
            draft.tag = Some(tag.to_string());
        } else if let Some(priority) = priority_word(&lower) {
            draft.priority = priority;
        } else if let Some(repeat) = repeat_word(&lower) {
            draft.repeat = repeat;
        } else if lower == "cada" || lower == "every" {
            match next
                .as_deref()
                .and_then(|n| repeat_word(&format!("cada {n}")))
            {
                Some(repeat) => {
                    draft.repeat = repeat;
                    consumed = 2;
                }
                None => title.push(word),
            }
        } else if let Some(day) = day_word(&lower, today) {
            draft.due_day = Some(day);
        } else if let Some(minute) = time_word(&lower) {
            draft.due_min = Some(minute);
        } else {
            title.push(word);
        }
        i += consumed;
    }
    if draft.due_min.is_some() && draft.due_day.is_none() {
        let minute = draft.due_min.unwrap_or_default();
        draft.due_day = Some(if minute > now_min { today } else { today + 1 });
    }
    if draft.repeat != Repeat::Never && draft.due_day.is_none() {
        draft.due_day = Some(today);
    }
    draft.title = title.join(" ");
    draft
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
        .collect::<String>()
        .trim_matches(|c: char| c == ',' || c == '.')
        .to_string()
}

fn priority_word(word: &str) -> Option<Priority> {
    Some(match word {
        "!alta" | "!high" | "!!" | "!!!" | "!urgente" | "!urgent" => Priority::High,
        "!baja" | "!low" => Priority::Low,
        "!" | "!normal" | "!media" => Priority::Normal,
        _ => return None,
    })
}

fn repeat_word(word: &str) -> Option<Repeat> {
    Some(match word {
        "diario" | "diaria" | "daily" | "cada dia" | "every day" => Repeat::Daily,
        "semanal" | "weekly" | "cada semana" | "every week" => Repeat::Weekly,
        "mensual" | "monthly" | "cada mes" | "every month" => Repeat::Monthly,
        _ => return None,
    })
}

fn day_word(word: &str, today: DayNumber) -> Option<DayNumber> {
    let weekday = |target: u8| {
        let current = schedule::weekday_of(today);
        // El próximo día con ese nombre; si es hoy, la semana que viene.
        let ahead = (i64::from(target) - i64::from(current)).rem_euclid(7);
        today + if ahead == 0 { 7 } else { ahead }
    };
    Some(match word {
        "hoy" | "today" => today,
        "manana" | "tomorrow" => today + 1,
        "pasado" => today + 2,
        "lunes" | "lun" | "monday" | "mon" => weekday(0),
        "martes" | "mar" | "tuesday" | "tue" => weekday(1),
        "miercoles" | "mie" | "wednesday" | "wed" => weekday(2),
        "jueves" | "jue" | "thursday" | "thu" => weekday(3),
        "viernes" | "vie" | "friday" | "fri" => weekday(4),
        "sabado" | "sab" | "saturday" | "sat" => weekday(5),
        "domingo" | "dom" | "sunday" | "sun" => weekday(6),
        _ => return date_word(word, today),
    })
}

/// "24/11" (este año, o el siguiente si ya pasó) o "24/11/2027".
fn date_word(word: &str, today: DayNumber) -> Option<DayNumber> {
    let parts: Vec<&str> = word.split('/').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let d: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    if !(1..=12).contains(&m) {
        return None;
    }
    let (this_year, _, _) = civil_from_days(today);
    let explicit = parts.get(2).map(|y| {
        y.parse::<i64>()
            .ok()
            .map(|y| if y < 100 { 2000 + y } else { y })
    });
    let year = match explicit {
        Some(Some(y)) => y,
        Some(None) => return None,
        None => this_year,
    };
    if d == 0 || d > days_in_month(year, m) {
        return None;
    }
    let day = days_from_civil(year, m, d);
    if explicit.is_none() && day < today {
        return Some(days_from_civil(
            year + 1,
            m,
            d.min(days_in_month(year + 1, m)),
        ));
    }
    Some(day)
}

fn time_word(word: &str) -> Option<Minute> {
    if let Some(h) = word.strip_suffix("am").or_else(|| word.strip_suffix("pm")) {
        let pm = word.ends_with("pm");
        let h: u16 = h.parse().ok().filter(|h| (1..=12).contains(h))?;
        return Some(((h % 12) + if pm { 12 } else { 0 }) * 60);
    }
    if !word.contains(':') && !word.ends_with('h') {
        return None;
    }
    schedule::parse_time(word).filter(|m| *m < schedule::DAY_MINUTES)
}

// Calendario civil a partir del número de día (algoritmo de Howard Hinnant).

pub fn civil_from_days(day: DayNumber) -> (i64, u32, u32) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_from_civil(y: i64, m: u32, d: u32) -> DayNumber {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lunes 9 de noviembre de 2026.
    const MONDAY: DayNumber = 20_766;

    #[test]
    fn civil_calendar() {
        assert_eq!(civil_from_days(MONDAY), (2026, 11, 9));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2026, 11, 9), MONDAY);
        assert_eq!(
            days_from_civil(2028, 2, 29) + 1,
            days_from_civil(2028, 3, 1)
        );
        for day in [0, 365, 11_016, MONDAY, 30_000] {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(days_from_civil(y, m, d), day);
        }
    }

    #[test]
    fn repeats() {
        assert_eq!(Repeat::Daily.next(MONDAY), Some(MONDAY + 1));
        assert_eq!(Repeat::Weekly.next(MONDAY), Some(MONDAY + 7));
        assert_eq!(
            Repeat::Monthly.next(MONDAY),
            Some(days_from_civil(2026, 12, 9))
        );
        assert_eq!(
            Repeat::Monthly.next(days_from_civil(2027, 1, 31)),
            Some(days_from_civil(2027, 2, 28))
        );
        assert_eq!(
            Repeat::Monthly.next(days_from_civil(2026, 12, 15)),
            Some(days_from_civil(2027, 1, 15))
        );
        assert_eq!(Repeat::Never.next(MONDAY), None);
    }

    #[test]
    fn buckets() {
        let noon = 12 * 60;
        assert_eq!(bucket(None, None, MONDAY, noon), Bucket::NoDate);
        assert_eq!(
            bucket(Some(MONDAY - 1), None, MONDAY, noon),
            Bucket::Overdue
        );
        assert_eq!(
            bucket(Some(MONDAY), Some(9 * 60), MONDAY, noon),
            Bucket::Overdue,
            "hoy a una hora que ya pasó"
        );
        assert_eq!(
            bucket(Some(MONDAY), Some(18 * 60), MONDAY, noon),
            Bucket::Today
        );
        assert_eq!(bucket(Some(MONDAY), None, MONDAY, noon), Bucket::Today);
        assert_eq!(
            bucket(Some(MONDAY + 1), None, MONDAY, noon),
            Bucket::Tomorrow
        );
        assert_eq!(
            bucket(Some(MONDAY + 6), None, MONDAY, noon),
            Bucket::ThisWeek
        );
        assert_eq!(bucket(Some(MONDAY + 7), None, MONDAY, noon), Bucket::Later);
    }

    #[test]
    fn quick_add_in_spanish() {
        let d = parse_quick("Pagar arriendo mañana 18:00 !alta #casa", MONDAY, 600);
        assert_eq!(d.title, "Pagar arriendo");
        assert_eq!((d.due_day, d.due_min), (Some(MONDAY + 1), Some(1080)));
        assert_eq!(
            (d.priority, d.tag.as_deref()),
            (Priority::High, Some("casa"))
        );

        let d = parse_quick("Entregar informe viernes", MONDAY, 600);
        assert_eq!(
            (d.title.as_str(), d.due_day),
            ("Entregar informe", Some(MONDAY + 4))
        );

        let d = parse_quick("Clase de Cloud lunes", MONDAY, 600);
        assert_eq!(
            d.due_day,
            Some(MONDAY + 7),
            "hoy es lunes: el próximo lunes"
        );

        let d = parse_quick("Regar plantas cada semana", MONDAY, 600);
        assert_eq!(
            (d.title.as_str(), d.repeat, d.due_day),
            ("Regar plantas", Repeat::Weekly, Some(MONDAY))
        );

        let d = parse_quick("Prueba 24/11 9am", MONDAY, 600);
        assert_eq!(
            (d.due_day, d.due_min),
            (Some(days_from_civil(2026, 11, 24)), Some(540))
        );
    }

    #[test]
    fn quick_add_in_english_and_edge_cases() {
        let d = parse_quick("Call mom tomorrow 7pm !!", MONDAY, 600);
        assert_eq!(
            (d.title.as_str(), d.due_day, d.due_min, d.priority),
            ("Call mom", Some(MONDAY + 1), Some(1140), Priority::High)
        );

        let d = parse_quick("Comprar pan 9:00", MONDAY, 600);
        assert_eq!(d.due_day, Some(MONDAY + 1), "las 9 ya pasaron: mañana");

        let d = parse_quick("Leer 5 páginas", MONDAY, 600);
        assert_eq!(
            (d.title.as_str(), d.due_day),
            ("Leer 5 páginas", None),
            "un número suelto no es una hora"
        );

        let d = parse_quick("Comprar cada cosa", MONDAY, 600);
        assert_eq!(d.title, "Comprar cada cosa");

        let d = parse_quick("Fiesta 3/1", MONDAY, 600);
        assert_eq!(
            d.due_day,
            Some(days_from_civil(2027, 1, 3)),
            "una fecha pasada es del año siguiente"
        );
        assert_eq!(parse_quick("x 31/02", MONDAY, 600).title, "x 31/02");
    }
}
