//! Conceptos del entrenamiento que no dependen de la base de datos.

/// Cómo se mide un ejercicio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExerciseKind {
    /// Carga externa: peso y repeticiones.
    Weighted,
    /// Peso corporal, con lastre opcional.
    Bodyweight,
    /// Se mide en tiempo (plancha, correr).
    Timed,
}

/// Tipo de serie. Se anota al registrarla.
///
/// El orden de `ALL` es el número que guarda la base de datos: solo se agregan
/// tipos nuevos al final.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SetType {
    #[default]
    Normal,
    /// Serie de calentamiento: no cuenta para récords ni para el volumen.
    WarmUp,
    /// Dropset: se baja el peso y se sigue sin descanso.
    Drop,
    /// Superserie: se encadena con otro ejercicio sin descanso.
    Super,
    /// Serie llevada al fallo.
    Failure,
    /// Serie de aproximación: se sube el peso hacia la serie fuerte (por ejemplo,
    /// antes de una al fallo). No cuenta como serie de trabajo.
    Approach,
}

impl SetType {
    pub const ALL: [SetType; 6] = [
        SetType::Normal,
        SetType::WarmUp,
        SetType::Drop,
        SetType::Super,
        SetType::Failure,
        SetType::Approach,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as u8
    }

    pub fn from_index(index: i64) -> SetType {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
            .unwrap_or_default()
    }

    /// Si la serie cuenta para récords y volumen.
    pub fn counts_as_work(self) -> bool {
        !matches!(self, SetType::WarmUp | SetType::Approach)
    }

    /// Si después de esta serie corresponde descansar. En dropsets y superseries
    /// se pasa directo a la siguiente.
    pub fn rests_after(self) -> bool {
        !matches!(self, SetType::Drop | SetType::Super)
    }
}

/// Día de la semana, 0 = lunes ... 6 = domingo.
pub type Weekday = u8;

pub const WEEKDAYS: std::ops::RangeInclusive<Weekday> = 0..=6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_types_round_trip() {
        for t in SetType::ALL {
            assert_eq!(SetType::from_index(i64::from(t.index())), t);
        }
        assert_eq!(SetType::from_index(99), SetType::Normal);
        assert_eq!(SetType::from_index(-1), SetType::Normal);
    }

    #[test]
    fn warm_ups_do_not_count_and_drops_skip_rest() {
        assert!(!SetType::WarmUp.counts_as_work());
        assert!(!SetType::Approach.counts_as_work());
        assert!(SetType::Approach.rests_after());
        assert!(SetType::Failure.counts_as_work());
        assert!(!SetType::Drop.rests_after());
        assert!(!SetType::Super.rests_after());
        assert!(SetType::Normal.rests_after());
    }
}
