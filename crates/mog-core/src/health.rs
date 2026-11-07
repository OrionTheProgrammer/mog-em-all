//! Cálculos de salud corporal.

/// Categoría del IMC según los rangos de la OMS para adultos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmiCategory {
    Underweight,
    Normal,
    Overweight,
    Obese,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bmi {
    pub value: f64,
    pub category: BmiCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthError {
    InvalidWeight,
    InvalidHeight,
}

/// Calcula el índice de masa corporal (peso / altura²).
///
/// Es un dato orientativo: no distingue masa muscular de grasa.
pub fn bmi(weight_kg: f64, height_cm: f64) -> Result<Bmi, HealthError> {
    if !(20.0..=400.0).contains(&weight_kg) {
        return Err(HealthError::InvalidWeight);
    }
    if !(100.0..=250.0).contains(&height_cm) {
        return Err(HealthError::InvalidHeight);
    }
    let meters = height_cm / 100.0;
    let value = weight_kg / (meters * meters);
    let category = match value {
        v if v < 18.5 => BmiCategory::Underweight,
        v if v < 25.0 => BmiCategory::Normal,
        v if v < 30.0 => BmiCategory::Overweight,
        _ => BmiCategory::Obese,
    };
    Ok(Bmi { value, category })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmi_normal() {
        let r = bmi(70.0, 175.0).unwrap();
        assert!((r.value - 22.857).abs() < 0.01);
        assert_eq!(r.category, BmiCategory::Normal);
    }

    #[test]
    fn bmi_limits() {
        assert_eq!(bmi(56.6, 175.0).unwrap().category, BmiCategory::Underweight);
        assert_eq!(bmi(80.0, 175.0).unwrap().category, BmiCategory::Overweight);
        assert_eq!(bmi(100.0, 175.0).unwrap().category, BmiCategory::Obese);
    }

    #[test]
    fn bmi_rejects_bad_input() {
        assert_eq!(bmi(0.0, 175.0), Err(HealthError::InvalidWeight));
        assert_eq!(bmi(70.0, 17.5), Err(HealthError::InvalidHeight));
        assert_eq!(bmi(f64::NAN, 175.0), Err(HealthError::InvalidWeight));
    }
}
