#![allow(
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::cast_possible_wrap
)]

use crate::{DelayMs, PathStep, Point};
use rand::RngExt;

const SQRT3: f64 = 1.732_050_807_568_877_2;
const SQRT5: f64 = 2.236_067_977_499_79;

/// Generates a human-like mouse path from start to target
/// using the `WindMouse` path algorithm.
#[must_use]
pub fn generate_wind_mouse_path(start: Point, target: Point) -> Vec<PathStep> {
	if start == target {
		return vec![PathStep {
			point: target,
			delay: DelayMs(0),
		}];
	}

	let mut path = Vec::new();
	let mut current_x = f64::from(start.x);
	let mut current_y = f64::from(start.y);
	let dest_x = f64::from(target.x);
	let dest_y = f64::from(target.y);

	let dx_total = dest_x - current_x;
	let dy_total = dest_y - current_y;
	let total_distance = (dx_total * dx_total + dy_total * dy_total).sqrt();

	let mut velocity_x = 0.0;
	let mut velocity_y = 0.0;
	let mut wind_x = 0.0;
	let mut wind_y = 0.0;

	let gravity = 9.0;
	let wind = 3.0;
	let target_area = 15.0;

	let mut rng = rand::rng();

	path.push(PathStep {
		point: start,
		delay: DelayMs(0),
	});

	loop {
		let dx = dest_x - current_x;
		let dy = dest_y - current_y;
		let dist = (dx * dx + dy * dy).sqrt();

		if dist < 1.5 {
			break;
		}

		let p = if total_distance > 0.0 {
			((total_distance - dist) / total_distance).clamp(0.0, 1.0)
		} else {
			1.0
		};

		// Smoothly scale down wind magnitude near the target
		let current_wind = wind * (1.0 - (2.0 / 3.0) * p);

		wind_x = (wind_x / SQRT3) + rng.random_range(-1.0..=1.0) * current_wind / SQRT5;
		wind_y = (wind_y / SQRT3) + rng.random_range(-1.0..=1.0) * current_wind / SQRT5;

		// Smoothly scale down gravity/acceleration scaling near the target
		let acc_scale = 1.0 - 0.5 * p;
		let acc_x = (gravity * dx / dist + wind_x) * acc_scale;
		let acc_y = (gravity * dy / dist + wind_y) * acc_scale;

		// Smoothly scale damping to allow slight underdamped overshoot and correction near the target
		let damping = 0.9 - 0.35 * p;
		velocity_x = (velocity_x * damping) + acc_x;
		velocity_y = (velocity_y * damping) + acc_y;

		// Introduce physiological micro-tremor noise to the velocity
		let tremor_x = rng.random_range(-0.35..=0.35);
		let tremor_y = rng.random_range(-0.35..=0.35);
		velocity_x += tremor_x;
		velocity_y += tremor_y;

		// Sublinear max step scaling based on total distance, with Fitts' Law progress shape (peak at ~33% progress)
		let dynamic_max_step = (1.0 * total_distance.sqrt()).clamp(12.0, 60.0);
		let shape = 6.75 * p * (1.0 - p) * (1.0 - p);

		// Add random noise (+/- 15%) to the speed limit to simulate organic velocity variations
		let speed_noise = rng.random_range(0.85..=1.15);
		let current_max_step = (3.0 + (dynamic_max_step - 3.0) * shape) * speed_noise;

		let vel_mag = (velocity_x * velocity_x + velocity_y * velocity_y).sqrt();
		if vel_mag > current_max_step {
			let scale = current_max_step / vel_mag;
			velocity_x *= scale;
			velocity_y *= scale;
		}

		current_x += velocity_x;
		current_y += velocity_y;

		let mut step_wait = rng.random_range(4..=8);
		if dist < target_area {
			step_wait = (step_wait / 2).max(2);
		}

		path.push(PathStep {
			point: Point::new(current_x.round() as i32, current_y.round() as i32),
			delay: DelayMs(step_wait),
		});
	}

	let last_x = target.x;
	let last_y = target.y;

	let already_at_target = if let Some(step) = path.last() {
		step.point.x == last_x && step.point.y == last_y
	} else {
		false
	};

	if !already_at_target {
		path.push(PathStep {
			point: target,
			delay: DelayMs(rng.random_range(5..=10)),
		});
	}

	path
}
