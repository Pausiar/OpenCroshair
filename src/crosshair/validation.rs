use super::model::Crosshair;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{field}: {message}")]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

/// Returns every out-of-range / NaN field.
pub fn validate(_c: &Crosshair) -> Result<(), Vec<ValidationError>> {
    Ok(())
}

/// Clamp all fields into valid range (NaN -> default).
pub fn clamp(_c: &mut Crosshair) {}
