#![allow(
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::cast_possible_wrap
)]

use crate::{DelayMs, PathStep, Point};
use rand::RngExt;

const SQRT3: f64 = 1.732_050_807_568_877_2;

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

	let mut velocity_x = 0.0;
	let mut velocity_y = 0.0;

	let gravity = 9.0;
	let wind = 3.0;
	let max_step = 12.0;
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

		let current_wind = if dist < target_area { wind / 3.0 } else { wind };

		let wind_x = rng.random_range(-1.0..=1.0) * current_wind / SQRT3;
		let wind_y = rng.random_range(-1.0..=1.0) * current_wind / SQRT3;

		let mut acc_x = gravity * dx / dist + wind_x;
		let mut acc_y = gravity * dy / dist + wind_y;

		// As cursor approaches target, decelerate
		if dist < 10.0 {
			acc_x *= 0.5;
			acc_y *= 0.5;
		}

		velocity_x += acc_x;
		velocity_y += acc_y;

		let vel_mag = (velocity_x * velocity_x + velocity_y * velocity_y).sqrt();
		if vel_mag > max_step {
			let scale = max_step / vel_mag;
			velocity_x *= scale;
			velocity_y *= scale;
		}

		current_x += velocity_x;
		current_y += velocity_y;

		let mut step_wait = rng.random_range(8..=15);
		if dist < target_area {
			step_wait = (step_wait / 2).max(1);
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
