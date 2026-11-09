//! Horario semanal: horas del día, repetición de eventos y cómo acomodar los
//! bloques que se pisan.
//!
//! Las fechas se manejan como "número de día" (días desde el 1-1-1970 en hora
//! local), así este módulo no depende de una biblioteca de fechas.

use crate::training::Weekday;

/// Minutos desde la medianoche, de 0 a 1440.
pub type Minute = u16;
pub const DAY_MINUTES: Minute = 24 * 60;

/// Número de día: días desde el 1 de enero de 1970.
pub type DayNumber = i64;

/// Día de la semana de un número de día (0 lunes ... 6 domingo).
/// El 1-1-1970 fue jueves.
pub fn weekday_of(day: DayNumber) -> Weekday {
    (day + 3).rem_euclid(7) as Weekday
}

/// Lunes de la semana que contiene `day`.
pub fn monday_of(day: DayNumber) -> DayNumber {
    day - DayNumber::from(weekday_of(day))
}

/// Lee una hora "18:30", "8:05" o "18". Acepta "24:00" como fin del día.
pub fn parse_time(text: &str) -> Option<Minute> {
    let text = text.trim();
    let (h, m) = match text.split_once([':', '.', 'h']) {
        Some((h, m)) => (h.trim(), if m.trim().is_empty() { "0" } else { m.trim() }),
        None => (text, "0"),
    };
    let h: u16 = h.parse().ok()?;
    let m: u16 = m.parse().ok()?;
    if m >= 60 || h > 24 || (h == 24 && m > 0) {
        return None;
    }
    Some(h * 60 + m)
}

/// Cuándo ocurre un evento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recurrence {
    /// Todas las semanas, en estos días.
    Weekly(Vec<Weekday>),
    /// Una sola vez, en este día.
    Once(DayNumber),
}

impl Recurrence {
    pub fn occurs_on(&self, day: DayNumber) -> bool {
        match self {
            Recurrence::Weekly(days) => days.contains(&weekday_of(day)),
            Recurrence::Once(on) => *on == day,
        }
    }
}

/// Lugar de un bloque dentro de su día: columna (`lane`) de cuántas (`lanes`)
/// y si choca con otro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub lane: u8,
    pub lanes: u8,
    pub conflict: bool,
}

/// Acomoda los bloques de un día. Los que se pisan se reparten en columnas lado
/// a lado; los que solo se tocan (uno termina 10:00 y el otro empieza 10:00) no
/// chocan. Devuelve un lugar por bloque, en el mismo orden que la entrada.
pub fn lay_out(blocks: &[(Minute, Minute)]) -> Vec<Placement> {
    let mut order: Vec<usize> = (0..blocks.len()).collect();
    order.sort_by_key(|&i| (blocks[i].0, blocks[i].1));

    let mut result = vec![
        Placement {
            lane: 0,
            lanes: 1,
            conflict: false
        };
        blocks.len()
    ];
    let mut cluster: Vec<usize> = Vec::new();
    let mut lane_ends: Vec<Minute> = Vec::new();
    let mut cluster_end: Minute = 0;

    let close = |cluster: &mut Vec<usize>, lanes: usize, result: &mut Vec<Placement>| {
        for &i in cluster.iter() {
            result[i].lanes = lanes as u8;
            result[i].conflict = lanes > 1;
        }
        cluster.clear();
    };

    for i in order {
        let (start, end) = blocks[i];
        if !cluster.is_empty() && start >= cluster_end {
            close(&mut cluster, lane_ends.len(), &mut result);
            lane_ends.clear();
        }
        let lane = match lane_ends.iter().position(|&e| e <= start) {
            Some(free) => {
                lane_ends[free] = end;
                free
            }
            None => {
                lane_ends.push(end);
                lane_ends.len() - 1
            }
        };
        result[i].lane = lane as u8;
        cluster.push(i);
        cluster_end = if cluster.len() == 1 {
            end
        } else {
            cluster_end.max(end)
        };
    }
    close(&mut cluster, lane_ends.len(), &mut result);
    result
}

/// Rango de horas a mostrar: de 07:00 a 22:00, ampliado para que quepa todo.
pub fn visible_range(blocks: &[(Minute, Minute)]) -> (Minute, Minute) {
    let from = blocks
        .iter()
        .map(|b| b.0)
        .min()
        .map_or(7 * 60, |m| m.min(7 * 60));
    let to = blocks
        .iter()
        .map(|b| b.1)
        .max()
        .map_or(22 * 60, |m| m.max(22 * 60));
    (from / 60 * 60, (to.div_ceil(60) * 60).min(DAY_MINUTES))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays_from_day_numbers() {
        // 9-11-2026 es lunes: 20_766 días desde 1970.
        assert_eq!(weekday_of(20_766), 0);
        assert_eq!(weekday_of(20_772), 6);
        assert_eq!(weekday_of(0), 3, "1970 empezó en jueves");
        assert_eq!(monday_of(20_770), 20_766);
        assert_eq!(monday_of(20_766), 20_766);
    }

    #[test]
    fn times() {
        assert_eq!(parse_time("18:30"), Some(18 * 60 + 30));
        assert_eq!(parse_time("8:05"), Some(8 * 60 + 5));
        assert_eq!(parse_time("9"), Some(540));
        assert_eq!(parse_time("9h"), Some(540));
        assert_eq!(parse_time("24:00"), Some(DAY_MINUTES));
        assert_eq!(parse_time("24:30"), None);
        assert_eq!(parse_time("10:75"), None);
        assert_eq!(parse_time("mañana"), None);
    }

    #[test]
    fn recurrence() {
        let weekly = Recurrence::Weekly(vec![0, 2]);
        assert!(weekly.occurs_on(20_766));
        assert!(!weekly.occurs_on(20_767));
        assert!(weekly.occurs_on(20_768));
        assert!(Recurrence::Once(20_770).occurs_on(20_770));
        assert!(!Recurrence::Once(20_770).occurs_on(20_777));
    }

    #[test]
    fn touching_blocks_do_not_conflict() {
        let p = lay_out(&[(540, 600), (600, 660)]);
        assert!(p.iter().all(|x| !x.conflict && x.lanes == 1 && x.lane == 0));
    }

    #[test]
    fn overlapping_blocks_share_the_day() {
        // 9-11 y 10-12 chocan; 13-14 queda solo.
        let p = lay_out(&[(600, 720), (540, 660), (780, 840)]);
        assert_eq!((p[1].lane, p[0].lane), (0, 1));
        assert!(p[0].conflict && p[1].conflict);
        assert_eq!((p[0].lanes, p[1].lanes), (2, 2));
        assert_eq!(
            p[2],
            Placement {
                lane: 0,
                lanes: 1,
                conflict: false
            }
        );
    }

    #[test]
    fn lanes_are_reused_inside_a_cluster() {
        // A 9-12 largo; B 9-10 y C 10-11 caben en la misma segunda columna.
        let p = lay_out(&[(540, 720), (540, 600), (600, 660)]);
        assert_eq!(p[1].lane, p[2].lane);
        assert!(p.iter().all(|x| x.lanes == 2));
    }

    #[test]
    fn visible_range_grows_to_fit() {
        assert_eq!(visible_range(&[]), (420, 1320));
        assert_eq!(visible_range(&[(330, 400), (1350, 1410)]), (300, 1440));
    }
}
