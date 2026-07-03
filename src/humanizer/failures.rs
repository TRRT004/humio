use super::target_area::TargetArea;
use enigo::{Button, Key};

/// Types of simulated mouse click errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickFailure {
	/// Click slightly outside the bounds of the expected target area.
	Misclick,
	/// Click in a specific designated decoy/error area (e.g. hitting an adjacent button).
	MisclickTo(TargetArea),
	/// Pressing the incorrect mouse button (e.g. right-clicking instead of left-clicking).
	WrongButton(Button),
	/// Accidentally clicking twice in rapid succession.
	DoubleClick,
	/// A combination of multiple mouse errors in sequence.
	Compound(Vec<ClickFailure>),
}

/// Types of simulated key combination errors (shortcuts/chords).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCombinationFailure {
	/// Missed pressing one of the required modifier keys (e.g. pressing `C` instead of `Ctrl+C`).
	MissedModifier(Key),
	/// Tapped the wrong key instead of the target key while modifiers are held.
	WrongKeyTap(Key),
	/// Released a modifier key too early before the target key is clicked/tapped.
	ReleasedModifierEarly(Key),
	/// Failed to release a modifier key after the combination completes, leaving it stuck in the OS.
	ModifierStuck(Key),
	/// A sequence combining multiple modifier/key combo errors.
	Compound(Vec<KeyCombinationFailure>),
}

/// Types of simulated typing errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardFailure {
	/// Accidentally replacing a character with an adjacent key, followed by backspace and correction.
	Typo,
	/// Swapping two adjacent characters in a word (e.g., typing "teh" instead of "the"), followed by corrections.
	Transposition,
	/// Key bounce causing a double-press/double-tap of a single character, followed by corrections.
	DoubleTap,
}

/// A wrapper enum unifying all failure categories.
#[derive(Debug, Clone, PartialEq)]
pub enum FailureType {
	/// A mouse click failure.
	Click(ClickFailure),
	/// A keyboard modifier combination failure.
	KeyCombination(KeyCombinationFailure),
	/// A standard keyboard typing failure.
	Keyboard(KeyboardFailure),
}

/// Custom callback trait to dynamically adjust the probability of specific failures.
///
/// This trait is useful when failure chances are stateful (e.g., fatigue increases
/// error rate over time).
///
/// # Examples
///
/// ```rust
/// use humio::{FailureChanceCalculator, FailureType, ClickFailure};
///
/// struct FatigueCalculator {
///     fatigue_multiplier: f64,
/// }
///
/// impl FailureChanceCalculator for FatigueCalculator {
///     fn calculate_chance(&self, failure: &FailureType, base_chance: f64) -> f64 {
///         match failure {
///             FailureType::Click(ClickFailure::Misclick) => base_chance * self.fatigue_multiplier,
///             _ => base_chance,
///         }
///     }
/// }
/// ```
pub trait FailureChanceCalculator {
	/// Computes the final probability (0.0 to 1.0) of a failure occurring.
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

/// Configuration parameters governing failure rates and timing models.
#[derive(Debug, Clone)]
pub struct HumanizerConfig {
	/// List of active click failures and their corresponding base probabilities.
	pub click_failures: Vec<(ClickFailure, f64)>,
	/// List of active typing failures and their corresponding base probabilities.
	pub typing_failures: Vec<(KeyboardFailure, f64)>,
	/// List of active key combination failures and their corresponding base probabilities.
	pub key_combo_failures: Vec<(KeyCombinationFailure, f64)>,
	/// The base probability (0.0 to 1.0) that the mouse cursor will overshoot a target before correcting.
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
