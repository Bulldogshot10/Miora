use crate::engine::math::{log_gamma, regularized_gamma_q};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gamma {
    pub shape: f64,
    pub scale: f64,
}

impl Gamma {
    pub fn cdf(self, value: f64) -> f64 {
        if value <= 0.0 {
            return 0.0;
        }
        (1.0 - regularized_gamma_q(self.shape, value / self.scale)).clamp(0.0, 1.0)
    }

    pub fn log_pdf(self, value: f64) -> f64 {
        if value <= 0.0 || self.shape <= 0.0 || self.scale <= 0.0 {
            return f64::NEG_INFINITY;
        }

        (self.shape - 1.0) * value.ln()
            - value / self.scale
            - log_gamma(self.shape)
            - self.shape * self.scale.ln()
    }

    pub fn log_survival(self, value: f64) -> f64 {
        if value <= 0.0 {
            return 0.0;
        }
        regularized_gamma_q(self.shape, value / self.scale)
            .max(f64::MIN_POSITIVE)
            .ln()
    }
}
