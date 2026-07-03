#![allow(
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::cast_possible_wrap
)]

use super::device::HumanizedDevice;
use super::failures::{FailureType, KeyCombinationFailure, KeyboardFailure};
use crate::{HumioError, InputDevice, Keyboard};
use enigo::{Direction, Key};

// RAII guard to release keys if any step fails or panics
struct ModifierGuard<'a, D: InputDevice> {
	device: &'a mut D,
	pressed: Vec<Key>,
}

impl<D: InputDevice> Drop for ModifierGuard<'_, D> {
	fn drop(&mut self) {
		for &key in self.pressed.iter().rev() {
			let _ = self.device.key(key, Direction::Release);
		}
	}
}

impl<D: InputDevice> HumanizedDevice<D> {
	/// Types text with natural keystroke delays and optional simulated typos (replacements,
	/// transpositions, or double-taps) with backspace correction, using probabilities from `self.config`.
	pub fn text_humanized(&mut self, text: &str, allow_error: bool) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();
		log::debug!("Typing humanized text: {text:?}");

		let is_failure_allowed = allow_error && !self.should_bypass_failures();

		let chars: Vec<char> = text.chars().collect();
		let mut i = 0;
		while i < chars.len() {
			let c = chars[i];

			// 1. Check for Transposition Typo (requires at least one lookahead character)
			let transposition_base = self
				.config
				.typing_failures
				.iter()
				.find(|(f, _)| matches!(f, KeyboardFailure::Transposition))
				.map_or(0.0, |(_, p)| *p);
			let transposition_prob = if is_failure_allowed {
				if let Some(ref calc) = self.chance_calculator {
					calc.calculate_chance(
						&FailureType::Keyboard(KeyboardFailure::Transposition),
						transposition_base,
					)
				} else {
					transposition_base
				}
			} else {
				0.0
			};

			if i + 1 < chars.len() && rng.random_bool(transposition_prob) {
				let next_c = chars[i + 1];
				log::debug!("Transposition error triggered! Swapping {c:?} and {next_c:?}");

				// Type next_c then c
				let mut s = String::new();
				s.push(next_c);
				self.inner.text(&s)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(50..=140)));

				s.clear();
				s.push(c);
				self.inner.text(&s)?;

				// Realization pause (200-400ms)
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(200..400)));

				log::trace!(
					"Realized transposition error. Backspacing twice to delete {next_c}{c}"
				);
				// Backspace twice to wipe the swapped chars
				for _ in 0..2 {
					self.inner.key(Key::Backspace, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..50)));
					self.inner.key(Key::Backspace, Direction::Release)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(30..80)));
				}

				// Delay before typing correct characters (150-300ms)
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(150..300)));

				// Type c then next_c correctly
				s.clear();
				s.push(c);
				self.inner.text(&s)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(50..=140)));

				s.clear();
				s.push(next_c);
				self.inner.text(&s)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(50..=140)));

				// Advance past both characters
				i += 2;
				continue;
			}

			// 2. Check for Double-Tap Key Jitter (simulates double-pressing a single key)
			let double_tap_base = self
				.config
				.typing_failures
				.iter()
				.find(|(f, _)| matches!(f, KeyboardFailure::DoubleTap))
				.map_or(0.0, |(_, p)| *p);
			let double_tap_prob = if is_failure_allowed {
				if let Some(ref calc) = self.chance_calculator {
					calc.calculate_chance(
						&FailureType::Keyboard(KeyboardFailure::DoubleTap),
						double_tap_base,
					)
				} else {
					double_tap_base
				}
			} else {
				0.0
			};

			if rng.random_bool(double_tap_prob) {
				log::debug!("Double-tap jitter triggered for character {c:?}");

				// Type character twice with a very short bounce delay
				let mut s = String::new();
				s.push(c);
				self.inner.text(&s)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(15..45)));
				self.inner.text(&s)?;

				// Realization pause (150-300ms)
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(150..300)));

				log::trace!("Realized double-tap. Backspacing once to delete extra {c:?}");
				// Backspace once
				self.inner.key(Key::Backspace, Direction::Press)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..50)));
				self.inner.key(Key::Backspace, Direction::Release)?;

				// Delay before resuming typing (100-200ms)
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(100..200)));

				i += 1;
				continue;
			}

			// 3. Normal Standard typo (single character replacement)
			let typo_base = self
				.config
				.typing_failures
				.iter()
				.find(|(f, _)| matches!(f, KeyboardFailure::Typo))
				.map_or(0.0, |(_, p)| *p);
			let typo_prob = if is_failure_allowed {
				if let Some(ref calc) = self.chance_calculator {
					calc.calculate_chance(&FailureType::Keyboard(KeyboardFailure::Typo), typo_base)
				} else {
					typo_base
				}
			} else {
				0.0
			};

			if rng.random_bool(typo_prob) {
				let typo_char = if c.is_ascii_lowercase() {
					rng.random_range(b'a'..=b'z') as char
				} else {
					rng.random_range(b'A'..=b'Z') as char
				};
				log::debug!("Typo generated: typed {typo_char:?} instead of expected {c:?}");

				let mut typo_str = String::new();
				typo_str.push(typo_char);
				self.inner.text(&typo_str)?;

				// Realization pause (150-300ms)
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(150..300)));

				log::trace!("Realized typo. Tapping Backspace to delete {typo_char:?}");
				// Press Backspace
				self.inner.key(Key::Backspace, Direction::Press)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..50)));
				self.inner.key(Key::Backspace, Direction::Release)?;

				// Delay before typing correct character (100-200ms)
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(100..200)));
			}

			let mut s = String::new();
			s.push(c);
			self.inner.text(&s)?;

			let delay = rng.random_range(50..=140);
			std::thread::sleep(std::time::Duration::from_millis(delay));
			i += 1;
		}
		Ok(())
	}

	/// Press modifiers and target key with natural keyboard modifiers, using the configured
	/// `KeyCombinationFailure` parameters and default recovery routines.
	pub fn key_combination_humanized(
		&mut self,
		modifiers: &[Key],
		key: Key,
		allow_error: bool,
	) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		let mut triggered_failure = None;

		if allow_error && !self.should_bypass_failures() {
			let roll = rng.random_range(0.0..1.0);
			let mut cumulative_prob = 0.0;
			for (failure, base_prob) in &self.config.key_combo_failures {
				let prob = if let Some(ref calc) = self.chance_calculator {
					calc.calculate_chance(&FailureType::KeyCombination(*failure), *base_prob)
				} else {
					*base_prob
				};

				cumulative_prob += prob;
				if roll < cumulative_prob {
					triggered_failure = Some(*failure);
					break;
				}
			}
		}

		if let Some(failure) = triggered_failure {
			log::debug!("Simulated key combination failure triggered: {failure:?}");

			// 1. Perform simulated incorrect combination
			match failure {
				KeyCombinationFailure::MissedModifier(missed_mod) => {
					log::trace!("Executing key combination error: missing modifier {missed_mod:?}");
					for &mod_key in modifiers {
						if mod_key == missed_mod {
							continue;
						}
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					self.inner.key(key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						if mod_key == missed_mod {
							continue;
						}
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
				KeyCombinationFailure::WrongKeyTap(wrong_key) => {
					log::trace!(
						"Executing key combination error: tapped wrong target key {wrong_key:?} instead of expected {key:?}"
					);
					for &mod_key in modifiers {
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					self.inner.key(wrong_key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(wrong_key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
				KeyCombinationFailure::ReleasedModifierEarly(early_mod) => {
					log::trace!(
						"Executing key combination error: releasing modifier {early_mod:?} early"
					);
					for &mod_key in modifiers {
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					// Release the early modifier before target key is pressed
					self.inner.key(early_mod, Direction::Release)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(10..30)));

					self.inner.key(key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						if mod_key == early_mod {
							continue;
						}
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
				KeyCombinationFailure::ModifierStuck(stuck_mod) => {
					log::trace!(
						"Executing key combination error: modifier {stuck_mod:?} got stuck (not released)"
					);
					for &mod_key in modifiers {
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					self.inner.key(key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						if mod_key == stuck_mod {
							continue; // Leave it stuck in OS keyboard state!
						}
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
			}

			// 2. Perform default built-in recovery inside recursive context guard
			self.execute_recovery_context(|d| {
				d.run_default_key_combo_recovery(&failure, modifiers, key)
			})?;
		} else {
			// Normal key combination
			self.key_combination_normal(modifiers, key)?;
		}
		Ok(())
	}

	/// Press modifiers and target key, allowing custom modifier/key tap failures with individual
	/// probabilities and associated recovery callbacks.
	pub fn key_combination_flexible(
		&mut self,
		modifiers: &[Key],
		key: Key,
		failures: &mut [(
			KeyCombinationFailure,
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
					calc.calculate_chance(&FailureType::KeyCombination(*failure), *base_prob)
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
			log::debug!("Simulated key combination failure triggered: {failure:?}");

			// 1. Perform simulated incorrect combination
			match failure {
				KeyCombinationFailure::MissedModifier(missed_mod) => {
					log::trace!("Executing key combination error: missing modifier {missed_mod:?}");
					for &mod_key in modifiers {
						if mod_key == *missed_mod {
							continue;
						}
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					self.inner.key(key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						if mod_key == *missed_mod {
							continue;
						}
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
				KeyCombinationFailure::WrongKeyTap(wrong_key) => {
					log::trace!(
						"Executing key combination error: tapped wrong target key {wrong_key:?} instead of expected {key:?}"
					);
					for &mod_key in modifiers {
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					self.inner.key(*wrong_key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(*wrong_key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
				KeyCombinationFailure::ReleasedModifierEarly(early_mod) => {
					log::trace!(
						"Executing key combination error: releasing modifier {early_mod:?} early"
					);
					for &mod_key in modifiers {
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					// Release the early modifier before target key is pressed
					self.inner.key(*early_mod, Direction::Release)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(10..30)));

					self.inner.key(key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						if mod_key == *early_mod {
							continue;
						}
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
				KeyCombinationFailure::ModifierStuck(stuck_mod) => {
					log::trace!(
						"Executing key combination error: modifier {stuck_mod:?} got stuck (not released)"
					);
					for &mod_key in modifiers {
						self.inner.key(mod_key, Direction::Press)?;
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
					}
					self.inner.key(key, Direction::Press)?;
					std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..60)));
					self.inner.key(key, Direction::Release)?;

					for &mod_key in modifiers.iter().rev() {
						if mod_key == *stuck_mod {
							continue; // Leave it stuck in OS keyboard state!
						}
						std::thread::sleep(std::time::Duration::from_millis(
							rng.random_range(15..45),
						));
						self.inner.key(mod_key, Direction::Release)?;
					}
				}
			}

			// 2. Realization delay (200-350ms)
			let reaction_delay = rng.random_range(200..=350);
			log::trace!(
				"Simulated failure completed. Realizing error (waiting {reaction_delay}ms)..."
			);
			std::thread::sleep(std::time::Duration::from_millis(reaction_delay));

			// 3. Execute recovery callback inside recursive context guard
			log::debug!("Executing recovery closure for KeyCombinationFailure...");
			self.execute_recovery_context(|d| recovery(d))?;

			// 4. Re-perform correct combination
			log::debug!("Recovery complete. Retrying correct modifier key combination.");
			self.key_combination_normal(modifiers, key)?;
		} else {
			// Normal combination
			log::trace!("Executing normal key combination: modifiers={modifiers:?}, key={key:?}");
			self.key_combination_normal(modifiers, key)?;
		}

		Ok(())
	}

	/// Executes a built-in default recovery routine for standard key combination failures.
	fn run_default_key_combo_recovery(
		&mut self,
		failure: &KeyCombinationFailure,
		modifiers: &[Key],
		key: Key,
	) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		let reaction_delay = rng.random_range(200..=350);
		log::trace!(
			"Built-in recovery: realizing key combo failure {failure:?} (waiting {reaction_delay}ms)..."
		);
		std::thread::sleep(std::time::Duration::from_millis(reaction_delay));

		match failure {
			KeyCombinationFailure::MissedModifier(_)
			| KeyCombinationFailure::ReleasedModifierEarly(_) => {
				log::debug!("Built-in recovery: retrying correct combination.");
				self.key_combination_normal(modifiers, key)?;
			}
			KeyCombinationFailure::WrongKeyTap(_) => {
				log::debug!("Built-in recovery: tapping Backspace once and retrying combination.");
				self.inner.key(Key::Backspace, Direction::Press)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(20..50)));
				self.inner.key(Key::Backspace, Direction::Release)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(100..200)));

				self.key_combination_normal(modifiers, key)?;
			}
			KeyCombinationFailure::ModifierStuck(stuck_mod) => {
				log::debug!(
					"Built-in recovery: modifier stuck. Releasing stuck modifier {stuck_mod:?} and retrying combination."
				);
				self.inner.key(*stuck_mod, Direction::Release)?;
				std::thread::sleep(std::time::Duration::from_millis(rng.random_range(100..200)));

				self.key_combination_normal(modifiers, key)?;
			}
		}
		Ok(())
	}

	/// Performs a standard humanized modifier combination (staggered press, tap target, staggered release).
	pub fn key_combination_normal(
		&mut self,
		modifiers: &[Key],
		key: Key,
	) -> Result<(), HumioError> {
		use rand::RngExt;
		let mut rng = rand::rng();

		let mut guard = ModifierGuard {
			device: &mut self.inner,
			pressed: Vec::new(),
		};

		// 1. Press modifiers one-by-one with realistic staggered entry delays (e.g. 15-45ms)
		for &mod_key in modifiers {
			guard.device.key(mod_key, Direction::Press)?;
			guard.pressed.push(mod_key);
			let delay = rng.random_range(15..=45);
			std::thread::sleep(std::time::Duration::from_millis(delay));
		}

		// 2. Click the target key (Press, hold for 20-60ms, then Release)
		guard.device.key(key, Direction::Press)?;
		let hold_delay = rng.random_range(20..=60);
		std::thread::sleep(std::time::Duration::from_millis(hold_delay));
		guard.device.key(key, Direction::Release)?;

		// 3. Release modifiers in reverse order with staggered delays.
		while let Some(mod_key) = guard.pressed.pop() {
			let delay = rng.random_range(15..=45);
			std::thread::sleep(std::time::Duration::from_millis(delay));
			guard.device.key(mod_key, Direction::Release)?;
		}

		Ok(())
	}
}

impl<D: InputDevice> Keyboard for HumanizedDevice<D> {
	fn key(&mut self, key: Key, action: Direction) -> Result<(), HumioError> {
		self.inner.key(key, action)
	}

	fn text(&mut self, text: &str) -> Result<(), HumioError> {
		self.text_humanized(text, false)
	}

	fn key_combination(&mut self, modifiers: &[Key], key: Key) -> Result<(), HumioError> {
		self.key_combination_normal(modifiers, key)
	}
}
