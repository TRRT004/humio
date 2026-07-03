use super::failures::{FailureChanceCalculator, HumanizerConfig};
use crate::InputDevice;

pub struct HumanizedDevice<D: InputDevice> {
	pub(crate) inner: D,
	pub(crate) chance_calculator: Option<Box<dyn FailureChanceCalculator>>,
	pub config: HumanizerConfig,
	pub recovery_depth: usize,
	pub max_recovery_depth: usize,
}

impl<D: InputDevice> HumanizedDevice<D> {
	pub fn new(inner: D) -> Self {
		Self {
			inner,
			chance_calculator: None,
			config: HumanizerConfig::default(),
			recovery_depth: 0,
			max_recovery_depth: 1, // Default to a maximum of 1 recovery nesting layer
		}
	}

	pub fn set_chance_calculator(&mut self, calculator: Box<dyn FailureChanceCalculator>) {
		self.chance_calculator = Some(calculator);
	}

	pub fn remove_chance_calculator(&mut self) {
		self.chance_calculator = None;
	}

	pub fn into_inner(self) -> D {
		self.inner
	}

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
