use super::failures::{FailureChanceCalculator, HumanizerConfig};
use crate::InputDevice;

/// A wrapper around any [`InputDevice`] that injects realistic human timing, movement patterns,
/// and imperfection (mistakes and automatic recoveries).
///
/// # Type Parameters
///
/// * `D`: The underlying input driver. This is typically a [`crate::PhysicalDevice`] in production,
///   or a [`crate::MockDevice`] in automated test environments.
///
/// # Examples
///
/// ```rust
/// use humio::{HumanizedDevice, MockDevice, Point};
///
/// let mock = MockDevice::new(Point::new(0, 0));
/// let mut dev = HumanizedDevice::new(mock);
///
/// // Configure custom overshoot chance
/// dev.config.overshoot_chance = 0.15; // 15% chance to overshoot targets
/// ```
pub struct HumanizedDevice<D: InputDevice> {
	pub(crate) inner: D,
	pub(crate) chance_calculator: Option<Box<dyn FailureChanceCalculator>>,
	/// Configuration parameters governing failure rates and timing models.
	pub config: HumanizerConfig,
	/// The current recursion depth of nested recovery actions (used to prevent endless loops).
	pub recovery_depth: usize,
	/// The maximum allowed nesting depth for failure recovery routines.
	pub max_recovery_depth: usize,
}

impl<D: InputDevice> HumanizedDevice<D> {
	/// Wraps the provided input device in a humanized simulation layer.
	pub fn new(inner: D) -> Self {
		Self {
			inner,
			chance_calculator: None,
			config: HumanizerConfig::default(),
			recovery_depth: 0,
			max_recovery_depth: 1, // Default to a maximum of 1 recovery nesting layer
		}
	}

	/// Sets a custom dynamically evaluated failure probability calculator.
	pub fn set_chance_calculator(&mut self, calculator: Box<dyn FailureChanceCalculator>) {
		self.chance_calculator = Some(calculator);
	}

	/// Removes the custom dynamically evaluated failure probability calculator, reverting to static configuration percentages.
	pub fn remove_chance_calculator(&mut self) {
		self.chance_calculator = None;
	}

	/// Consumes this wrapper, returning the underlying input device.
	pub fn into_inner(self) -> D {
		self.inner
	}

	/// Borrows the underlying input device mutably.
	pub fn inner_mut(&mut self) -> &mut D {
		&mut self.inner
	}

	/// Returns true if failure simulations should be bypassed due to current recovery nesting depth.
	pub(crate) fn should_bypass_failures(&self) -> bool {
		self.recovery_depth >= self.max_recovery_depth
	}

	/// Executes a block of code within an active recovery context, incrementing/decrementing depth.
	pub(crate) fn execute_recovery_context<F, R>(&mut self, mut f: F) -> R
	where
		F: FnMut(&mut Self) -> R,
	{
		self.recovery_depth += 1;
		let result = f(self);
		self.recovery_depth -= 1;
		result
	}
}

impl<D: InputDevice> InputDevice for HumanizedDevice<D> {}
