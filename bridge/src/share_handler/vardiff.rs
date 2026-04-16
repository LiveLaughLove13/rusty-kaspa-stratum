#[allow(dead_code)]
pub(crate) const VAR_DIFF_THREAD_SLEEP: u64 = 10;
#[allow(dead_code)]
const WORK_WINDOW: u64 = 80;

// VarDiff tunables
const VARDIFF_MIN_ELAPSED_SECS: f64 = 30.0;
const VARDIFF_MAX_ELAPSED_SECS_NO_SHARES: f64 = 90.0;
const VARDIFF_MIN_SHARES: f64 = 3.0;
const VARDIFF_LOWER_RATIO: f64 = 0.75; // below this => decrease diff
const VARDIFF_UPPER_RATIO: f64 = 1.25; // above this => increase diff
const VARDIFF_MAX_STEP_UP: f64 = 2.0; // max 2x per adjustment tick
const VARDIFF_MAX_STEP_DOWN: f64 = 0.5; // max -50% per adjustment tick

fn vardiff_pow2_clamp_towards(current: f64, next: f64) -> f64 {
    if !next.is_finite() || next <= 0.0 {
        return 1.0;
    }

    let exp = if next >= current {
        next.log2().ceil()
    } else {
        next.log2().floor()
    };
    let clamped = 2_f64.powi(exp as i32);
    if clamped < 1.0 { 1.0 } else { clamped }
}

pub(crate) fn vardiff_compute_next_diff(
    current: f64,
    shares: f64,
    elapsed_secs: f64,
    expected_spm: f64,
    clamp_pow2: bool,
) -> Option<f64> {
    if !current.is_finite() || current <= 0.0 {
        return None;
    }
    if !elapsed_secs.is_finite() || elapsed_secs <= 0.0 {
        return None;
    }

    if shares == 0.0 && elapsed_secs >= VARDIFF_MAX_ELAPSED_SECS_NO_SHARES {
        let mut next = current * VARDIFF_MAX_STEP_DOWN;
        if next < 1.0 {
            next = 1.0;
        }
        if clamp_pow2 {
            next = vardiff_pow2_clamp_towards(current, next);
        }
        return if (next - current).abs() > f64::EPSILON {
            Some(next)
        } else {
            None
        };
    }

    if elapsed_secs < VARDIFF_MIN_ELAPSED_SECS || shares < VARDIFF_MIN_SHARES {
        return None;
    }

    let observed_spm = (shares / elapsed_secs) * 60.0;
    let ratio = observed_spm / expected_spm.max(1.0);
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    if ratio > VARDIFF_LOWER_RATIO && ratio < VARDIFF_UPPER_RATIO {
        return None;
    }

    let step = ratio
        .sqrt()
        .clamp(VARDIFF_MAX_STEP_DOWN, VARDIFF_MAX_STEP_UP);
    let mut next = current * step;
    if next < 1.0 {
        next = 1.0;
    }
    if clamp_pow2 {
        next = vardiff_pow2_clamp_towards(current, next);
    }

    let rel_change = (next - current).abs() / current.max(1.0);
    if rel_change < 0.10 {
        return None;
    }
    if (next - current).abs() > f64::EPSILON {
        Some(next)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_shares_long_wait_lowers_diff() {
        let next = vardiff_compute_next_diff(100.0, 0.0, 95.0, 10.0, false).expect("should adjust");
        assert!(next < 100.0);
        assert!(next >= 1.0);
    }

    #[test]
    fn no_change_when_ratio_in_band() {
        assert!(vardiff_compute_next_diff(64.0, 5.0, 60.0, 5.0, false).is_none());
    }

    #[test]
    fn pow2_clamp_rounds_to_power_of_two() {
        let next = vardiff_compute_next_diff(8.0, 0.0, 95.0, 10.0, true).expect("adjust");
        assert!(next.is_finite() && next >= 1.0);
        let log2 = next.log2();
        assert!(
            (log2 - log2.round()).abs() < 1e-9,
            "expected power of 2, got {}",
            next
        );
    }

    #[test]
    fn invalid_current_returns_none() {
        assert!(vardiff_compute_next_diff(0.0, 1.0, 60.0, 5.0, false).is_none());
        assert!(vardiff_compute_next_diff(f64::NAN, 1.0, 60.0, 5.0, false).is_none());
    }
}
