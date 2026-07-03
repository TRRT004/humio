use super::target_area::TargetArea;
use enigo::{Button, Key};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickFailure {
	/// Clicks outside the target area.
	Misclick,
	/// Clicks inside a specific designated failure area.
	MisclickTo(TargetArea),
	/// Clicks the wrong button instead of the expected one.
	WrongButton(Button),
	/// Clicks twice instead of once.
	DoubleClick,
	/// Compound click failure combining multiple mouse errors in sequence.
	Compound(Vec<ClickFailure>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCombinationFailure {
	/// Missed pressing one of the modifier keys.
	MissedModifier(Key),
	/// Tapped a wrong key instead of the target key.
	WrongKeyTap(Key),
	/// Released a modifier key before the target key is clicked/tapped.
	ReleasedModifierEarly(Key),
	/// Failed to release a modifier key after the combination completes (stuck key).
	ModifierStuck(Key),
	/// Compound key failure combining multiple modifier/key errors in sequence.
	Compound(Vec<KeyCombinationFailure>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardFailure {
	/// Typo on a character.
	Typo,
	/// Swapping two adjacent characters.
	Transposition,
	/// Pressing a key twice.
	DoubleTap,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FailureType {
	Click(ClickFailure),
	KeyCombination(KeyCombinationFailure),
	Keyboard(KeyboardFailure),
}

pub trait FailureChanceCalculator {
	fn calculate_chance(&self, failure: &FailureType, base_chance: f64) -> f64;
}

impl<F> FailureChanceCalculator for F
where
	F: Fn(&FailureType, f64) -> f64,
{
	fn calculate_chance(&self, failure: &FailureType, base_chance: f64) -> f64 {
		self(failure, base_chance)
	}
}

#[derive(Debug, Clone)]
pub struct HumanizerConfig {
	pub click_failures: Vec<(ClickFailure, f64)>,
	pub typing_failures: Vec<(KeyboardFailure, f64)>,
	pub key_combo_failures: Vec<(KeyCombinationFailure, f64)>,
	pub overshoot_chance: f64,
}

impl Default for HumanizerConfig {
	fn default() -> Self {
		Self {
			click_failures: vec![(ClickFailure::Misclick, 0.05)],
			typing_failures: vec![
				(KeyboardFailure::Typo, 0.02),
				(KeyboardFailure::Transposition, 0.015),
				(KeyboardFailure::DoubleTap, 0.01),
			],
			key_combo_failures: vec![],
			overshoot_chance: 0.04,
		}
	}
}
