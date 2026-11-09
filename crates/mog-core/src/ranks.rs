//! Sistema de rangos. El diseño completo está en `docs/RANKS.md`.
//!
//! Hay dos ejes separados:
//! - **Nivel y rango de operador**: se gana con XP por entrenar con constancia.
//!   Mide esfuerzo, así que es justo comparar a dos amigos con cuerpos distintos.
//! - **Clase de fuerza**: una letra por ejercicio (E a S) según la fuerza
//!   relativa al peso corporal.

/// Sesiones más cortas que esto no dan XP.
pub const MIN_SESSION_MINUTES: u32 = 10;
/// Minutos que cuentan para XP en una sesión.
pub const MAX_COUNTED_MINUTES: u32 = 60;
/// Récords personales que dan bonus en una sola sesión.
pub const MAX_PR_BONUSES: u32 = 3;
/// Sesiones por día que dan XP (se toman las de mayor XP).
pub const MAX_SESSIONS_PER_DAY: usize = 2;
/// Semanas seguidas cumpliendo la meta que suman bonus.
pub const MAX_STREAK_WEEKS: u32 = 5;
pub const MAX_LEVEL: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSummary {
    pub duration_min: u32,
    pub personal_records: u32,
}

/// Cada parte de la XP de una sesión, para mostrarla línea por línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct XpBreakdown {
    pub base: u32,
    pub minutes: u32,
    pub records: u32,
    /// Multiplicador por racha, en porcentaje (100 = sin bonus).
    pub streak_pct: u32,
    pub total: u32,
}

/// XP de una sesión, separada en sus partes. `streak_weeks` son las semanas
/// consecutivas en que el usuario cumplió su meta semanal.
pub fn xp_breakdown(session: SessionSummary, streak_weeks: u32) -> XpBreakdown {
    if session.duration_min < MIN_SESSION_MINUTES {
        return XpBreakdown {
            streak_pct: 100,
            ..Default::default()
        };
    }
    let base = 50;
    let minutes = session.duration_min.min(MAX_COUNTED_MINUTES);
    let records = 25 * session.personal_records.min(MAX_PR_BONUSES);
    let streak_pct = 100 + 10 * streak_weeks.min(MAX_STREAK_WEEKS);
    XpBreakdown {
        base,
        minutes,
        records,
        streak_pct,
        total: (base + minutes + records) * streak_pct / 100,
    }
}

/// XP de una sesión. Descansar no corta la racha porque se cuenta por semanas.
pub fn session_xp(session: SessionSummary, streak_weeks: u32) -> u32 {
    xp_breakdown(session, streak_weeks).total
}

/// XP de un día completo, aplicando el tope de sesiones diarias.
pub fn day_xp(sessions: &[SessionSummary], streak_weeks: u32) -> u32 {
    let mut xp: Vec<u32> = sessions
        .iter()
        .map(|s| session_xp(*s, streak_weeks))
        .collect();
    xp.sort_unstable_by(|a, b| b.cmp(a));
    xp.iter().take(MAX_SESSIONS_PER_DAY).sum()
}

/// Semanas seguidas que cumplieron la meta, contando hacia atrás desde la semana pasada.
///
/// `sessions_per_week` va de la semana más antigua a la actual (la última). La semana
/// actual suma solo si ya cumplió la meta; si todavía no, no corta la racha.
pub fn streak_weeks(sessions_per_week: &[u32], weekly_goal: u32) -> u32 {
    let goal = weekly_goal.max(1);
    let Some((&current, past)) = sessions_per_week.split_last() else {
        return 0;
    };
    let past_streak = past.iter().rev().take_while(|&&n| n >= goal).count() as u32;
    past_streak + u32::from(current >= goal)
}

/// XP total necesaria para llegar a `level` (el nivel 1 empieza en 0).
pub fn xp_to_reach(level: u32) -> u64 {
    let n = level.clamp(1, MAX_LEVEL) as f64 - 1.0;
    (100.0 * n.powf(1.5)).round() as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelProgress {
    pub level: u32,
    /// XP acumulada dentro del nivel actual.
    pub xp_in_level: u64,
    /// XP que pide el nivel actual para subir (0 en el nivel máximo).
    pub xp_for_next: u64,
}

pub fn level_from_xp(total_xp: u64) -> LevelProgress {
    let mut level = 1;
    while level < MAX_LEVEL && xp_to_reach(level + 1) <= total_xp {
        level += 1;
    }
    let start = xp_to_reach(level);
    let xp_for_next = if level == MAX_LEVEL {
        0
    } else {
        xp_to_reach(level + 1) - start
    };
    LevelProgress {
        level,
        xp_in_level: total_xp - start,
        xp_for_next,
    }
}

/// Rango de operador, agrupa tramos de niveles. Cada rango tiene tres divisiones (I, II, III).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    NoName,
    Beginner,
    Selfhater,
    Insane,
    Chad,
    Mogger,
    PsychoMogger,
}

pub fn tier_for_level(level: u32) -> Tier {
    match level {
        0..=9 => Tier::NoName,
        10..=19 => Tier::Beginner,
        20..=34 => Tier::Selfhater,
        35..=49 => Tier::Insane,
        50..=69 => Tier::Chad,
        70..=89 => Tier::Mogger,
        _ => Tier::PsychoMogger,
    }
}

/// Primer nivel de cada rango, en el orden de `Tier`.
const TIER_STARTS: [u32; 7] = [1, 10, 20, 35, 50, 70, 90];

/// Nivel en el que empieza el siguiente rango, o `None` si ya es Psycho Mogger.
pub fn next_tier_level(level: u32) -> Option<u32> {
    TIER_STARTS.iter().copied().find(|&start| start > level)
}

/// Primer y último nivel (inclusive) del rango que contiene `level`.
fn tier_band(level: u32) -> (u32, u32) {
    let level = level.clamp(1, MAX_LEVEL);
    let start = TIER_STARTS
        .iter()
        .copied()
        .rev()
        .find(|&s| s <= level)
        .unwrap_or(1);
    let end = next_tier_level(level).map_or(MAX_LEVEL, |next| next - 1);
    (start, end)
}

/// División dentro del rango: 1, 2 o 3. Cada rango se reparte en tres tramos de niveles.
pub fn division(level: u32) -> u8 {
    let (start, end) = tier_band(level);
    let span = end - start + 1;
    let offset = level.clamp(start, end) - start;
    (1 + offset * 3 / span) as u8
}

/// Nivel en el que empieza la división siguiente (o el rango siguiente).
/// `None` en la última división de Psycho Mogger.
pub fn next_division_level(level: u32) -> Option<u32> {
    let (start, end) = tier_band(level);
    let span = end - start + 1;
    let current = division(level) as u32;
    if current < 3 {
        // Primer nivel cuya división es current + 1: start + ceil(current * span / 3).
        Some(start + (current * span).div_ceil(3))
    } else {
        next_tier_level(level)
    }
}

/// Avance dentro del rango actual, de 0 a 1 (Psycho Mogger en el nivel máximo da 1).
pub fn tier_progress(level: u32) -> f64 {
    let (start, end) = tier_band(level);
    if end == start {
        return 1.0;
    }
    (level.clamp(start, end) - start) as f64 / (end - start) as f64
}

/// Qué tan grande fue un cambio de nivel, para decidir qué animación mostrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankChange {
    /// Subió de nivel sin cambiar de división.
    LevelOnly,
    /// Pasó a la división siguiente dentro del mismo rango.
    Division,
    /// Llegó a un rango nuevo.
    Tier,
}

pub fn rank_change(old_level: u32, new_level: u32) -> Option<RankChange> {
    if new_level <= old_level {
        return None;
    }
    Some(if tier_for_level(new_level) != tier_for_level(old_level) {
        RankChange::Tier
    } else if division(new_level) != division(old_level) {
        RankChange::Division
    } else {
        RankChange::LevelOnly
    })
}

/// Ejercicios con tabla de clase de fuerza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lift {
    Squat,
    BenchPress,
    Deadlift,
    OverheadPress,
    BarbellRow,
    /// Dominadas: la carga es peso corporal + lastre.
    PullUp,
}

impl Lift {
    fn is_lower_body(self) -> bool {
        matches!(self, Lift::Squat | Lift::Deadlift)
    }

    /// Ratios (1RM / peso corporal) para llegar a D, C, B, A y S.
    fn thresholds(self) -> [f64; 5] {
        match self {
            Lift::Squat => [0.75, 1.25, 1.5, 2.0, 2.5],
            Lift::BenchPress => [0.5, 0.75, 1.0, 1.5, 2.0],
            Lift::Deadlift => [1.0, 1.5, 2.0, 2.5, 3.0],
            Lift::OverheadPress => [0.35, 0.55, 0.75, 1.0, 1.25],
            Lift::BarbellRow => [0.5, 0.75, 1.0, 1.25, 1.5],
            Lift::PullUp => [1.0, 1.15, 1.3, 1.5, 1.75],
        }
    }
}

/// Perfil usado solo para escalar las tablas de fuerza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyProfile {
    Male,
    Female,
    Unspecified,
}

impl BodyProfile {
    fn scale(self, lower_body: bool) -> f64 {
        match (self, lower_body) {
            (BodyProfile::Male, _) => 1.0,
            (BodyProfile::Female, false) => 0.6,
            (BodyProfile::Female, true) => 0.7,
            (BodyProfile::Unspecified, false) => 0.8,
            (BodyProfile::Unspecified, true) => 0.85,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StrengthClass {
    E,
    D,
    C,
    B,
    A,
    S,
}

/// 1RM estimado con la fórmula de Epley. Solo es fiable hasta 12 repeticiones.
pub fn estimated_one_rep_max(load_kg: f64, reps: u32) -> Option<f64> {
    match reps {
        0 => None,
        1 => Some(load_kg),
        2..=12 => Some(load_kg * (1.0 + reps as f64 / 30.0)),
        _ => None,
    }
}

/// Clase de fuerza para una serie. Para dominadas, `load_kg` es solo el lastre.
pub fn strength_class(
    lift: Lift,
    load_kg: f64,
    reps: u32,
    bodyweight_kg: f64,
    profile: BodyProfile,
) -> Option<StrengthClass> {
    if bodyweight_kg <= 0.0 || load_kg < 0.0 {
        return None;
    }
    let load = if lift == Lift::PullUp {
        bodyweight_kg + load_kg
    } else {
        load_kg
    };
    let ratio = estimated_one_rep_max(load, reps)? / bodyweight_kg;
    let scale = profile.scale(lift.is_lower_body());
    let classes = [
        StrengthClass::D,
        StrengthClass::C,
        StrengthClass::B,
        StrengthClass::A,
        StrengthClass::S,
    ];
    let mut class = StrengthClass::E;
    for (threshold, c) in lift.thresholds().iter().zip(classes) {
        if ratio >= threshold * scale {
            class = c;
        }
    }
    Some(class)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(duration_min: u32, personal_records: u32) -> SessionSummary {
        SessionSummary {
            duration_min,
            personal_records,
        }
    }

    #[test]
    fn short_sessions_give_nothing() {
        assert_eq!(session_xp(s(9, 3), 5), 0);
    }

    #[test]
    fn session_xp_caps() {
        assert_eq!(session_xp(s(45, 0), 0), 95);
        assert_eq!(session_xp(s(200, 0), 0), 110);
        assert_eq!(session_xp(s(60, 10), 0), 110 + 75);
        assert_eq!(session_xp(s(60, 0), 50), 165);
    }

    #[test]
    fn breakdown_adds_up() {
        let b = xp_breakdown(s(52, 1), 6);
        assert_eq!(
            (b.base, b.minutes, b.records, b.streak_pct),
            (50, 52, 25, 150)
        );
        assert_eq!(b.total, (50 + 52 + 25) * 150 / 100);
        assert_eq!(b.total, session_xp(s(52, 1), 6));
        assert_eq!(xp_breakdown(s(5, 2), 3).total, 0);
    }

    #[test]
    fn day_takes_best_two() {
        let day = [s(20, 0), s(60, 1), s(30, 0)];
        assert_eq!(day_xp(&day, 0), 135 + 80);
    }

    #[test]
    fn streaks_count_weeks_not_days() {
        assert_eq!(streak_weeks(&[], 3), 0);
        // Semana actual a medias: no suma pero tampoco corta.
        assert_eq!(streak_weeks(&[3, 4, 3, 1], 3), 3);
        // Semana actual cumplida: suma.
        assert_eq!(streak_weeks(&[3, 4, 3, 3], 3), 4);
        // Una semana floja corta la racha.
        assert_eq!(streak_weeks(&[5, 5, 2, 3, 0], 3), 1);
        // Meta 0 se trata como 1.
        assert_eq!(streak_weeks(&[1, 0], 0), 1);
    }

    #[test]
    fn level_curve() {
        assert_eq!(level_from_xp(0).level, 1);
        assert_eq!(level_from_xp(99).level, 1);
        assert_eq!(level_from_xp(100).level, 2);
        assert_eq!(level_from_xp(xp_to_reach(10)).level, 10);
        let max = level_from_xp(u64::MAX / 2);
        assert_eq!((max.level, max.xp_for_next), (MAX_LEVEL, 0));
    }

    #[test]
    fn level_progress_is_consistent() {
        let p = level_from_xp(xp_to_reach(7) + 40);
        assert_eq!(p.level, 7);
        assert_eq!(p.xp_in_level, 40);
        assert_eq!(p.xp_for_next, xp_to_reach(8) - xp_to_reach(7));
    }

    #[test]
    fn tiers() {
        assert_eq!(tier_for_level(1), Tier::NoName);
        assert_eq!(tier_for_level(10), Tier::Beginner);
        assert_eq!(tier_for_level(49), Tier::Insane);
        assert_eq!(tier_for_level(100), Tier::PsychoMogger);
    }

    #[test]
    fn tier_steps() {
        assert_eq!(next_tier_level(1), Some(10));
        assert_eq!(next_tier_level(23), Some(35));
        assert_eq!(next_tier_level(95), None);
        assert_eq!(tier_progress(1), 0.0);
        assert_eq!(tier_progress(20), 0.0);
        assert!((tier_progress(30) - 10.0 / 14.0).abs() < 1e-9);
        assert_eq!(tier_progress(34), 1.0);
        assert_eq!(tier_progress(MAX_LEVEL), 1.0);
        for level in 1..=MAX_LEVEL {
            let p = tier_progress(level);
            assert!((0.0..=1.0).contains(&p), "nivel {level}: {p}");
        }
    }

    #[test]
    fn divisions_split_each_tier_in_three() {
        // Chad va del 50 al 69: I 50-56, II 57-63, III 64-69.
        assert_eq!(division(50), 1);
        assert_eq!(division(56), 1);
        assert_eq!(division(57), 2);
        assert_eq!(division(63), 2);
        assert_eq!(division(64), 3);
        assert_eq!(division(69), 3);
        // No-Name va del 1 al 9: tres niveles por división.
        assert_eq!((division(1), division(4), division(7)), (1, 2, 3));
        assert_eq!(division(MAX_LEVEL), 3);
    }

    #[test]
    fn next_division_matches_division() {
        for level in 1..MAX_LEVEL {
            if let Some(next) = next_division_level(level) {
                assert!(next > level, "nivel {level}");
                let before = (tier_for_level(next - 1), division(next - 1));
                let after = (tier_for_level(next), division(next));
                assert_ne!(before, after, "nivel {next} no empieza una división");
                assert_eq!(before, (tier_for_level(level), division(level)));
            }
        }
        assert_eq!(next_division_level(50), Some(57));
        assert_eq!(next_division_level(64), Some(70));
        assert_eq!(next_division_level(MAX_LEVEL), None);
    }

    #[test]
    fn rank_changes() {
        assert_eq!(rank_change(5, 5), None);
        assert_eq!(rank_change(51, 52), Some(RankChange::LevelOnly));
        assert_eq!(rank_change(56, 57), Some(RankChange::Division));
        assert_eq!(rank_change(69, 70), Some(RankChange::Tier));
        assert_eq!(rank_change(8, 23), Some(RankChange::Tier));
    }

    #[test]
    fn epley() {
        assert_eq!(estimated_one_rep_max(100.0, 1), Some(100.0));
        assert!((estimated_one_rep_max(100.0, 5).unwrap() - 116.67).abs() < 0.01);
        assert_eq!(estimated_one_rep_max(100.0, 0), None);
        assert_eq!(estimated_one_rep_max(100.0, 20), None);
    }

    #[test]
    fn strength_classes() {
        use BodyProfile::*;
        assert_eq!(
            strength_class(Lift::BenchPress, 30.0, 1, 80.0, Male),
            Some(StrengthClass::E)
        );
        assert_eq!(
            strength_class(Lift::BenchPress, 80.0, 1, 80.0, Male),
            Some(StrengthClass::B)
        );
        assert_eq!(
            strength_class(Lift::Deadlift, 240.0, 1, 80.0, Male),
            Some(StrengthClass::S)
        );
        // La misma marca pesa más en un perfil femenino.
        assert_eq!(
            strength_class(Lift::BenchPress, 48.0, 1, 60.0, Female),
            Some(StrengthClass::B)
        );
        // Una dominada sin lastre ya es D.
        assert_eq!(
            strength_class(Lift::PullUp, 0.0, 1, 80.0, Male),
            Some(StrengthClass::D)
        );
        assert_eq!(strength_class(Lift::Squat, 100.0, 1, 0.0, Male), None);
    }
}
