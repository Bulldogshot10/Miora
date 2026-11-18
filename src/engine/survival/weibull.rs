#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weibull {
    pub shape: f64,
    pub scale: f64,
}

impl Weibull {
    pub fn cdf(self, value: f64) -> f64 {
        if value <= 0.0 {
            return 0.0;
        }
        let exponent = (value / self.scale).powf(self.shape);
        1.0 - (-exponent).exp()
    }

    pub fn log_pdf(self, value: f64) -> f64 {
        if value <= 0.0 || self.shape <= 0.0 || self.scale <= 0.0 {
            return f64::NEG_INFINITY;
        }
        self.shape.ln() - self.scale.ln()
            + (self.shape - 1.0) * (value.ln() - self.scale.ln())
            - (value / self.scale).powf(self.shape)
    }

    pub fn log_survival(self, value: f64) -> f64 {
        if value <= 0.0 {
            return 0.0;
        }
        -((value / self.scale).powf(self.shape))
    }
}
