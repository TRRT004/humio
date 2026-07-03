#![allow(
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_sign_loss
)]

use crate::DelayMs;

/// Sample a 1D normal (Gaussian) distribution using Box-Muller transform.
#[must_use]
pub fn sample_gaussian(mean: f64, std_dev: f64) -> f64 {
	use rand::RngExt;
	let mut rng = rand::rng();
	let u1: f64 = rng.random_range(0.0001..=1.0);
	let u2: f64 = rng.random_range(0.0..=1.0);
	let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
	mean + z * std_dev
}

/// Helper that samples a normal distribution and clamps it strictly within the bounds.
#[must_use]
pub fn sample_gaussian_clamped(mean: i32, std_dev: i32, min: i32, max: i32) -> i32 {
	if min >= max {
		return min;
	}
	let val = sample_gaussian(f64::from(mean), f64::from(std_dev));
	(val.round() as i32).clamp(min, max)
}

/// Sleeps for a duration matching a normal distribution.
pub fn sleep_gaussian_delay(mean: DelayMs, std_dev_ms: u64) {
	let delay = sample_gaussian_clamped(
		mean.0 as i32,
		std_dev_ms as i32,
		(mean.0 / 3) as i32,
		(mean.0 * 3) as i32,
	);
	std::thread::sleep(std::time::Duration::from_millis(delay as u64));
}
