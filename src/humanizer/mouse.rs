#![allow(
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss
)]

use super::delay::sleep_gaussian_delay;
use super::device::HumanizedDevice;
use super::failures::{ClickFailure, FailureType};
use super::target_area::TargetArea;
use super::wind_mouse::generate_wind_mouse_path;
use crate::{DelayMs, HumioError, InputDevice, Mouse, Point, ScrollAxis};
use enigo::Button;

impl<D: InputDevice> HumanizedDevice<D> {
	fn execute_click_failure(
		&mut self,
		failure: &ClickFailure,
		area: &TargetArea,
		button: Button,
	) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		match failure {
			ClickFailure::Compound(sub_failures) => {
				log::trace!(
					"Executing compound click failure with {} sub-errors",
					sub_failures.len()
				);
				for sub in sub_failures {
					self.execute_click_failure(sub, area, button)?;
					// Stagger delay between cascading sub-failures
					std::thread::sleep(std::time::Duration::from_millis(50));
				}
			}
			ClickFailure::Misclick => {
				let correct_point = area.generate_click_point();
				let angle = rng.random_range(0.0..(2.0 * std::f64::consts::PI));
				let dist = rng.random_range(12.0..25.0);
				let mis_point = Point::new(
					correct_point.x + (dist * angle.cos()).round() as i32,
					correct_point.y + (dist * angle.sin()).round() as i32,
				);
				log::trace!("Executing misclick: clicking outside target area at {mis_point:?}");

				let start = self.inner.location()?;
				let path = generate_wind_mouse_path(start, mis_point);
				for step in &path {
					self.inner.move_mouse(step.point)?;
					if step.delay.0 > 0 {
						std::thread::sleep(step.delay.to_duration());
					}
				}
				sleep_gaussian_delay(DelayMs(80), 20);
				self.inner.click(button)?;
			}
			ClickFailure::MisclickTo(error_area) => {
				let mis_point = error_area.generate_click_point();
				log::trace!("Executing targeted misclick: clicking failure area at {mis_point:?}");

				let start = self.inner.location()?;
				let path = generate_wind_mouse_path(start, mis_point);
				for step in &path {
					self.inner.move_mouse(step.point)?;
					if step.delay.0 > 0 {
						std::thread::sleep(step.delay.to_duration());
					}
				}
				sleep_gaussian_delay(DelayMs(80), 20);
				self.inner.click(button)?;
			}
			ClickFailure::WrongButton(wrong_btn) => {
				log::trace!(
					"Executing wrong button click: clicking {wrong_btn:?} instead of expected {button:?}"
				);
				self.move_to_area(area, false)?;
				sleep_gaussian_delay(DelayMs(80), 20);
				self.inner.click(*wrong_btn)?;
			}
			ClickFailure::DoubleClick => {
				log::trace!("Executing double click: clicking target twice with button {button:?}");
				self.move_to_area(area, false)?;
				sleep_gaussian_delay(DelayMs(80), 20);
				self.inner.click(button)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(80..150)));
				self.inner.click(button)?;
			}
		}
		Ok(())
	}
	/// Moves the mouse from its current location to a point in the target area
	/// using a human-like `WindMouse` path, with optional simulated hover overshoots and adjustments.
	pub fn move_to_area(&mut self, area: &TargetArea, allow_error: bool) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		let target = area.generate_click_point();
		let start = self.inner.location()?;
		log::trace!("Moving mouse: start={start:?}, target={target:?}");

		if allow_error
			&& !self.should_bypass_failures()
			&& rng.random_bool(self.config.overshoot_chance)
		{
			// Generate an overshoot point slightly past/offset from the target
			let angle = rng.random_range(0.0..(2.0 * std::f64::consts::PI));
			let dist = rng.random_range(10.0..22.0);
			let overshoot_point = Point::new(
				target.x + (dist * angle.cos()).round() as i32,
				target.y + (dist * angle.sin()).round() as i32,
			);
			log::debug!("Hover overshoot triggered! Overshooting to {overshoot_point:?}");

			// Move to overshoot point
			let path = generate_wind_mouse_path(start, overshoot_point);
			for step in &path {
				self.inner.move_mouse(step.point)?;
				if step.delay.0 > 0 {
					std::thread::sleep(step.delay.to_duration());
				}
			}

			// Realization pause (120-220ms)
			std::thread::sleep(std::time::Duration::from_millis(rng.random_range(120..220)));

			log::trace!(
				"Overshoot realization complete. Adjusting mouse to actual target {target:?}"
			);
			// Corrective micro-glide to actual target
			let correction_path = generate_wind_mouse_path(overshoot_point, target);
			for step in &correction_path {
				self.inner.move_mouse(step.point)?;
				if step.delay.0 > 0 {
					std::thread::sleep(step.delay.to_duration());
				}
			}
		} else {
			// Normal direct move
			let path = generate_wind_mouse_path(start, target);
			for step in &path {
				self.inner.move_mouse(step.point)?;
				if step.delay.0 > 0 {
					std::thread::sleep(step.delay.to_duration());
				}
			}
		}
		Ok(())
	}

	/// Moves the mouse to the target area and performs a click with natural Gaussian delays,
	/// with optional simulated click errors (misclicks, wrong button, double clicks) and automatic recovery.
	pub fn click_area(
		&mut self,
		area: &TargetArea,
		button: Button,
		allow_error: bool,
	) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		let mut triggered_failure = None;

		if allow_error && !self.should_bypass_failures() {
			let roll = rng.random_range(0.0..1.0);
			let mut cumulative_prob = 0.0;
			for (failure, base_prob) in &self.config.click_failures {
				let prob = if let Some(ref calc) = self.chance_calculator {
					calc.calculate_chance(&FailureType::Click(failure.clone()), *base_prob)
				} else {
					*base_prob
				};

				cumulative_prob += prob;
				if roll < cumulative_prob {
					triggered_failure = Some(failure.clone());
					break;
				}
			}
		}

		if let Some(failure) = triggered_failure {
			log::debug!("Simulated click failure triggered: {failure:?}");
			// 1. Perform simulated incorrect action
			self.execute_click_failure(&failure, area, button)?;

			// 2. Run built-in default recovery routine inside recursive context guard
			self.execute_recovery_context(|d| {
				d.run_default_click_recovery(&failure, area, button)
			})?;
		} else {
			// Normal direct move and click (move_to_area might still simulate hover error)
			self.move_to_area(area, allow_error)?;
			sleep_gaussian_delay(DelayMs(80), 20);
			self.inner.click(button)?;
			sleep_gaussian_delay(DelayMs(80), 20);
		}
		Ok(())
	}

	/// Moves the mouse to the target area and clicks, allowing custom click failures with individual
	/// probabilities and associated recovery callbacks.
	pub fn click_area_flexible(
		&mut self,
		area: &TargetArea,
		button: Button,
		failures: &mut [(
			ClickFailure,
			f64,
			Box<dyn FnMut(&mut Self) -> Result<(), HumioError> + '_>,
		)],
	) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		let triggered_index = if self.should_bypass_failures() {
			None
		} else {
			let roll = rng.random_range(0.0..1.0);
			let mut cumulative_prob = 0.0;
			let mut idx = None;

			for (i, (failure, base_prob, _)) in failures.iter().enumerate() {
				let prob = if let Some(ref calc) = self.chance_calculator {
					calc.calculate_chance(&FailureType::Click(failure.clone()), *base_prob)
				} else {
					*base_prob
				};

				cumulative_prob += prob;
				if roll < cumulative_prob {
					idx = Some(i);
					break;
				}
			}
			idx
		};

		if let Some(idx) = triggered_index {
			let (failure, _, recovery) = &mut failures[idx];
			log::debug!("Simulated click failure triggered: {failure:?}");

			// 1. Perform the simulated error action
			self.execute_click_failure(failure, area, button)?;

			// 2. Realization delay (250-450ms)
			let reaction_delay = rng.random_range(250..=450);
			log::trace!(
				"Simulated failure completed. Realizing error (waiting {reaction_delay}ms)..."
			);
			std::thread::sleep(std::time::Duration::from_millis(reaction_delay));

			// 3. Execute custom recovery callback inside recursive context guard
			log::debug!("Executing recovery closure for ClickFailure...");
			self.execute_recovery_context(|d| recovery(d))?;

			// 4. Tap the correct button
			log::debug!("Recovery closure complete. Retrying correct click at target area.");
			self.move_to_area(area, false)?;
			sleep_gaussian_delay(DelayMs(80), 20);
			self.inner.click(button)?;
			sleep_gaussian_delay(DelayMs(80), 20);
		} else {
			// Normal click
			log::trace!("Executing normal click: target area, button {button:?}");
			self.move_to_area(area, false)?;
			sleep_gaussian_delay(DelayMs(80), 20);
			self.inner.click(button)?;
			sleep_gaussian_delay(DelayMs(80), 20);
		}

		Ok(())
	}

	/// Executes a built-in default recovery routine for standard mouse click failures.
	fn run_default_click_recovery(
		&mut self,
		failure: &ClickFailure,
		area: &TargetArea,
		button: Button,
	) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		let reaction_delay = rng.random_range(250..=450);
		log::trace!(
			"Built-in recovery: realizing click failure {failure:?} (waiting {reaction_delay}ms)..."
		);
		std::thread::sleep(std::time::Duration::from_millis(reaction_delay));

		match failure {
			ClickFailure::Misclick | ClickFailure::MisclickTo(_) => {
				log::debug!(
					"Built-in recovery: moving back to target area and retrying correct click."
				);
				self.move_to_area(area, false)?;
				sleep_gaussian_delay(DelayMs(80), 20);
				self.inner.click(button)?;
				sleep_gaussian_delay(DelayMs(80), 20);
			}
			ClickFailure::WrongButton(_) => {
				log::debug!(
					"Built-in recovery: clicked wrong button. Tapping correct button {button:?}."
				);
				sleep_gaussian_delay(DelayMs(80), 20);
				self.inner.click(button)?;
				sleep_gaussian_delay(DelayMs(80), 20);
			}
			ClickFailure::DoubleClick => {
				log::debug!("Built-in recovery: double-click occurred. Realized and proceeding.");
			}
			ClickFailure::Compound(_) => {
				log::debug!(
					"Built-in recovery: compound failure occurred. Retrying correct click at target area."
				);
				self.move_to_area(area, false)?;
				sleep_gaussian_delay(DelayMs(80), 20);
				self.inner.click(button)?;
				sleep_gaussian_delay(DelayMs(80), 20);
			}
		}
		Ok(())
	}
}

impl<D: InputDevice> Mouse for HumanizedDevice<D> {
	fn location(&self) -> Result<Point, HumioError> {
		self.inner.location()
	}

	/// Overridden to glide the cursor to the target using a `WindMouse` path
	fn move_mouse(&mut self, point: Point) -> Result<(), HumioError> {
		let start = self.inner.location()?;
		let path = generate_wind_mouse_path(start, point);
		for step in &path {
			self.inner.move_mouse(step.point)?;
			if step.delay.0 > 0 {
				std::thread::sleep(step.delay.to_duration());
			}
		}
		Ok(())
	}

	fn move_mouse_by(&mut self, offset: Point) -> Result<(), HumioError> {
		let start = self.inner.location()?;
		self.move_mouse(Point::new(start.x + offset.x, start.y + offset.y))
	}

	fn click(&mut self, button: Button) -> Result<(), HumioError> {
		sleep_gaussian_delay(DelayMs(80), 20);
		self.inner.click(button)?;
		sleep_gaussian_delay(DelayMs(80), 20);
		Ok(())
	}

	fn hold(&mut self, button: Button) -> Result<(), HumioError> {
		self.inner.hold(button)
	}

	fn release(&mut self, button: Button) -> Result<(), HumioError> {
		self.inner.release(button)
	}

	/// Scrolls using a physics-inspired momentum decay (inertia).
	/// As the scroll nears its final distance, the step delays decay exponentially.
	fn scroll(&mut self, length: i32, axis: ScrollAxis) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();
		let mut remaining = length;
		let step_sign = length.signum();

		let total_steps = f64::from(length.abs());
		let mut steps_done = 0.0;

		while remaining != 0 {
			let step_size = rng.random_range(1..=3).min(remaining.abs());
			let signed_step = step_size * step_sign;
			self.inner.scroll(signed_step, axis)?;
			remaining -= signed_step;
			steps_done += f64::from(step_size);

			if remaining != 0 {
				let fraction = steps_done / total_steps;
				let base_delay = rng.random_range(4.0..8.0);
				let delay_ms = base_delay + (fraction * fraction * 32.0);
				std::thread::sleep(std::time::Duration::from_millis(delay_ms.round() as u64));
			}
		}
		Ok(())
	}
}
