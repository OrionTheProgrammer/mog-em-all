//! Avisos: decide qué recordatorios toca mostrar ahora.
//!
//! Cada aviso tiene una clave única (por ejemplo, "event:3:20766") para no
//! mostrarlo dos veces; quien llama guarda las claves ya avisadas.

use crate::schedule::{DAY_MINUTES, DayNumber, Minute};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderKind {
    Event,
    Task,
    Training,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub key: String,
    pub kind: ReminderKind,
    pub title: String,
    /// Cuándo empieza (evento, entrenamiento) o vence (tarea).
    pub day: DayNumber,
    pub minute: Minute,
    /// Cuántos minutos antes avisar.
    pub lead: u32,
}

fn absolute(day: DayNumber, minute: Minute) -> i64 {
    day * i64::from(DAY_MINUTES) + i64::from(minute)
}

/// Avisos que corresponden ahora: ya llegó su hora de aviso y todavía no pasó lo
/// que anuncian. Si la app estuvo cerrada, no se acumulan avisos viejos.
pub fn due<'a>(
    candidates: &'a [Candidate],
    today: DayNumber,
    now: Minute,
    already: &dyn Fn(&str) -> bool,
) -> Vec<&'a Candidate> {
    let now = absolute(today, now);
    candidates
        .iter()
        .filter(|c| {
            let at = absolute(c.day, c.minute);
            let fire = at - i64::from(c.lead);
            fire <= now && now < at.max(fire + 1) && !already(&c.key)
        })
        .collect()
}

/// Minutos que faltan, para el texto del aviso ("en 15 min").
pub fn minutes_until(candidate: &Candidate, today: DayNumber, now: Minute) -> i64 {
    (absolute(candidate.day, candidate.minute) - absolute(today, now)).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(key: &str, day: DayNumber, minute: Minute, lead: u32) -> Candidate {
        Candidate {
            key: key.into(),
            kind: ReminderKind::Event,
            title: key.into(),
            day,
            minute,
            lead,
        }
    }

    #[test]
    fn fires_inside_the_window_only() {
        let list = [c("clase", 10, 600, 15)];
        let none = |_: &str| false;
        assert!(
            due(&list, 10, 584, &none).is_empty(),
            "16 min antes: todavía no"
        );
        assert_eq!(due(&list, 10, 585, &none).len(), 1, "15 min antes");
        assert_eq!(due(&list, 10, 599, &none).len(), 1);
        assert!(due(&list, 10, 600, &none).is_empty(), "ya empezó");
    }

    #[test]
    fn not_twice() {
        let list = [c("clase", 10, 600, 15)];
        assert!(due(&list, 10, 590, &|k| k == "clase").is_empty());
    }

    #[test]
    fn crosses_midnight() {
        // Algo a las 00:10 con aviso de 30 min: se avisa a las 23:40 del día anterior.
        let list = [c("tarde", 11, 10, 30)];
        assert_eq!(due(&list, 10, 23 * 60 + 45, &|_| false).len(), 1);
    }

    #[test]
    fn zero_lead_fires_at_the_minute() {
        let list = [c("ya", 10, 600, 0)];
        assert_eq!(due(&list, 10, 600, &|_| false).len(), 1);
        assert!(due(&list, 10, 601, &|_| false).is_empty());
        assert_eq!(minutes_until(&list[0], 10, 590), 10);
    }
}
