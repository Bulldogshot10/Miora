use std::f64::consts::{PI, SQRT_2};

pub(crate) fn normal_log_pdf(value: f64, mean: f64, standard_deviation: f64) -> f64 {
    let variance = standard_deviation * standard_deviation;
    let difference = value - mean;
    -0.5 * ((2.0 * PI * variance).ln() + difference * difference / variance)
}

pub(crate) fn normal_cdf(value: f64, mean: f64, standard_deviation: f64) -> f64 {
    if standard_deviation <= 0.0 {
        return if value < mean { 0.0 } else { 1.0 };
    }

    let z = (value - mean) / (standard_deviation * SQRT_2);
    0.5 * (1.0 + erf(z))
}

fn erf(value: f64) -> f64 {
    let sign = if value < 0.0 { -1.0 } else { 1.0 };
    let x = value.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let polynomial =
        (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t;
    sign * (1.0 - polynomial * (-x * x).exp())
}

pub(crate) fn log_gamma(value: f64) -> f64 {
    const COEFFICIENTS: [f64; 8] = [
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];

    if value <= 0.0 || !value.is_finite() {
        return f64::NAN;
    }
    if value < 0.5 {
        return PI.ln() - (PI * value).sin().ln() - log_gamma(1.0 - value);
    }

    let z = value - 1.0;
    let mut sum = 0.999_999_999_999_809_9;
    for (index, coefficient) in COEFFICIENTS.iter().enumerate() {
        sum += coefficient / (z + index as f64 + 1.0);
    }
    let t = z + 7.5;
    0.5 * (2.0 * PI).ln() + (z + 0.5) * t.ln() - t + sum.ln()
}

pub(crate) fn regularized_gamma_q(shape: f64, value: f64) -> f64 {
    const MAX_ITERATIONS: usize = 256;
    const EPSILON: f64 = 1e-12;
    const TINY: f64 = 1e-300;

    if shape <= 0.0 || value < 0.0 || shape.is_nan() || value.is_nan() {
        return f64::NAN;
    }
    if value == 0.0 {
        return 1.0;
    }
    if value.is_infinite() {
        return 0.0;
    }

    let log_factor = -value + shape * value.ln() - log_gamma(shape);

    if value < shape + 1.0 {
        let mut denominator = shape;
        let mut term = 1.0 / shape;
        let mut sum = term;
        for _ in 0..MAX_ITERATIONS {
            denominator += 1.0;
            term *= value / denominator;
            sum += term;
            if term.abs() <= sum.abs() * EPSILON {
                break;
            }
        }
        let lower = sum * log_factor.exp();
        (1.0 - lower).clamp(0.0, 1.0)
    } else {
        let mut b = value + 1.0 - shape;
        let mut c = 1.0 / TINY;
        let mut d = 1.0 / b.max(TINY);
        let mut fraction = d;

        for iteration in 1..=MAX_ITERATIONS {
            let index = iteration as f64;
            let coefficient = -index * (index - shape);
            b += 2.0;
            d = coefficient * d + b;
            if d.abs() < TINY {
                d = TINY;
            }
            c = b + coefficient / c;
            if c.abs() < TINY {
                c = TINY;
            }
            d = 1.0 / d;
            let delta = d * c;
            fraction *= delta;
            if (delta - 1.0).abs() <= EPSILON {
                break;
            }
        }

        (log_factor.exp() * fraction).clamp(0.0, 1.0)
    }
}
