use crate::{InputDevice, Keyboard, Mouse, Point, ScrollAxis};
use enigo::{Button, Direction, Key};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
	MouseLocationQuery(Point),
	MouseMoved(Point),
	MouseMovedBy(Point),
	MouseClicked(Button),
	MouseHeld(Button),
	MouseReleased(Button),
	MouseScrolled { length: i32, axis: ScrollAxis },
	KeyAction { key: Key, action: Direction },
	TextTyped(String),
}

#[derive(Clone)]
pub struct MockDevice {
	location: Rc<RefCell<Point>>,
	events: Rc<RefCell<Vec<InputEvent>>>,
}

impl MockDevice {
	pub fn new(initial_location: Point) -> Self {
		Self {
			location: Rc::new(RefCell::new(initial_location)),
			events: Rc::new(RefCell::new(Vec::new())),
		}
	}

	pub fn get_events(&self) -> Vec<InputEvent> {
		self.events.borrow().clone()
	}

	pub fn clear_events(&self) {
		self.events.borrow_mut().clear();
	}

	pub fn set_location(&self, point: Point) {
		*self.location.borrow_mut() = point;
	}
}

impl Mouse for MockDevice {
	fn location(&self) -> Result<Point, String> {
		let loc = *self.location.borrow();
		self.events.borrow_mut().push(InputEvent::MouseLocationQuery(loc));
		Ok(loc)
	}

	fn move_mouse(&mut self, point: Point) -> Result<(), String> {
		*self.location.borrow_mut() = point;
		self.events.borrow_mut().push(InputEvent::MouseMoved(point));
		Ok(())
	}

	fn move_mouse_by(&mut self, offset: Point) -> Result<(), String> {
		let mut loc = self.location.borrow_mut();
		loc.x += offset.x;
		loc.y += offset.y;
		self.events.borrow_mut().push(InputEvent::MouseMovedBy(offset));
		Ok(())
	}

	fn click(&mut self, button: Button) -> Result<(), String> {
		self.events.borrow_mut().push(InputEvent::MouseClicked(button));
		Ok(())
	}

	fn hold(&mut self, button: Button) -> Result<(), String> {
		self.events.borrow_mut().push(InputEvent::MouseHeld(button));
		Ok(())
	}

	fn release(&mut self, button: Button) -> Result<(), String> {
		self.events.borrow_mut().push(InputEvent::MouseReleased(button));
		Ok(())
	}

	fn scroll(&mut self, length: i32, axis: ScrollAxis) -> Result<(), String> {
		self.events.borrow_mut().push(InputEvent::MouseScrolled { length, axis });
		Ok(())
	}
}

impl Keyboard for MockDevice {
	fn key(&mut self, key: Key, action: Direction) -> Result<(), String> {
		self.events.borrow_mut().push(InputEvent::KeyAction { key, action });
		Ok(())
	}

	fn text(&mut self, text: &str) -> Result<(), String> {
		self.events.borrow_mut().push(InputEvent::TextTyped(text.to_string()));
		Ok(())
	}
}

impl InputDevice for MockDevice {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::humanizer::{ClickFailure, FailureType, KeyboardFailure, KeyCombinationFailure};
	use crate::{HumanizedDevice, Keyboard, Mouse, ScrollAxis, TargetArea};

	#[test]
	fn test_mock_device_captures_events() {
		let mut mock = MockDevice::new(Point::new(100, 100));
		assert_eq!(mock.location().unwrap(), Point::new(100, 100));

		mock.move_mouse(Point::new(150, 200)).unwrap();
		mock.click(Button::Left).unwrap();
		mock.key(Key::Unicode('a'), Direction::Press).unwrap();

		let events = mock.get_events();
		assert!(events.contains(&InputEvent::MouseLocationQuery(Point::new(100, 100))));
		assert!(events.contains(&InputEvent::MouseMoved(Point::new(150, 200))));
		assert!(events.contains(&InputEvent::MouseClicked(Button::Left)));
		assert!(events.contains(&InputEvent::KeyAction { key: Key::Unicode('a'), action: Direction::Press }));
	}

	#[test]
	fn test_target_area_rect_bounds() {
		let rect = TargetArea::Rect {
			top_left: Point::new(10, 10),
			bottom_right: Point::new(50, 50),
			target: None,
			std_dev_x: None,
			std_dev_y: None,
		};
		for _ in 0..100 {
			let pt = rect.generate_click_point();
			assert!(pt.x >= 10 && pt.x <= 50);
			assert!(pt.y >= 10 && pt.y <= 50);
		}
	}

	#[test]
	fn test_wind_mouse_path_generation() {
		use crate::humanizer::wind_mouse::generate_wind_mouse_path;
		let start = Point::new(0, 0);
		let target = Point::new(100, 100);
		let path = generate_wind_mouse_path(start, target);
		assert!(!path.is_empty());
		assert_eq!(path.last().unwrap().point, target);
	}

	#[test]
	fn test_flexible_click_failure_recovery() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		let target_area = TargetArea::Point(Point::new(50, 50));
		let mut recovery_called = false;

		// Configure a failure that will ALWAYS trigger (probability = 1.0)
		let mut failures = vec![
			(
				ClickFailure::Misclick,
				1.0, // 100% chance to trigger
				Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
					recovery_called = true;
					Ok(())
				}) as Box<dyn FnMut(&mut _) -> _>
			)
		];

		dev.click_area_flexible(&target_area, Button::Left, &mut failures).unwrap();

		drop(failures);
		assert!(recovery_called);
		// Check that we performed the misclick, recovery, and then the correct click
		let events = mock.get_events();
		let clicks: Vec<_> = events.iter().filter(|e| matches!(e, InputEvent::MouseClicked(_))).collect();
		// Should have clicked twice: 1st is the misclick, 2nd is the corrected click after recovery
		assert_eq!(clicks.len(), 2);
	}

	#[test]
	fn test_flexible_key_combination_failure_recovery() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());
		let mut recovery_called = false;

		let mut failures = vec![
			(
				KeyCombinationFailure::WrongKeyTap(Key::Unicode('a')),
				1.0, // 100% chance to trigger
				Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
					recovery_called = true;
					Ok(())
				}) as Box<dyn FnMut(&mut _) -> _>
			)
		];

		dev.key_combination_flexible(&[Key::Control], Key::Unicode('c'), &mut failures).unwrap();

		drop(failures);
		assert!(recovery_called);
		// Check that the keys were pressed, released, and recovery was invoked
		let events = mock.get_events();
		assert!(events.iter().any(|e| matches!(e, InputEvent::KeyAction { key: Key::Unicode('a'), action: Direction::Press })));
	}

	#[test]
	fn test_text_humanized_failures_transpositions_and_doubletap() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// 1. Force transposition failure (probability = 1.0)
		dev.set_chance_calculator(Box::new(|failure: &FailureType, _base: f64| {
			match failure {
				FailureType::Keyboard(KeyboardFailure::Transposition) => 1.0,
				_ => 0.0,
			}
		}));

		// We type "ab" -> should swap to "ba", backspace twice, and type "ab"
		dev.text_humanized("ab", true).unwrap();

		let events = mock.get_events();
		// Let's count backspaces
		let backspaces = events.iter().filter(|e| matches!(e, InputEvent::KeyAction { key: Key::Backspace, action: Direction::Press })).count();
		assert_eq!(backspaces, 2);

		// 2. Force double tap failure (probability = 1.0)
		mock.clear_events();
		dev.set_chance_calculator(Box::new(|failure: &FailureType, _base: f64| {
			match failure {
				FailureType::Keyboard(KeyboardFailure::DoubleTap) => 1.0,
				_ => 0.0,
			}
		}));

		// We type "a" -> should double type "aa", backspace once, and complete
		dev.text_humanized("a", true).unwrap();

		let events2 = mock.get_events();
		let backspaces2 = events2.iter().filter(|e| matches!(e, InputEvent::KeyAction { key: Key::Backspace, action: Direction::Press })).count();
		assert_eq!(backspaces2, 1);
	}

	#[test]
	fn test_scroll_inertia() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		dev.scroll(10, ScrollAxis::Vertical).unwrap();

		let events = mock.get_events();
		let scrolls: Vec<_> = events.iter().filter(|e| matches!(e, InputEvent::MouseScrolled { .. })).collect();
		assert!(!scrolls.is_empty());
		// Total scroll length summed up should be 10
		let mut sum = 0;
		for s in scrolls {
			if let InputEvent::MouseScrolled { length, .. } = s {
				sum += length;
			}
		}
		assert_eq!(sum, 10);
	}

	#[test]
	fn test_flexible_key_combination_modifier_jitters() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());
		let mut early_recovery_called = false;
		let mut stuck_recovery_called = false;

		// 1. Test Early Modifier Release
		let mut failures_early = vec![
			(
				KeyCombinationFailure::ReleasedModifierEarly(Key::Control),
				1.0, // 100% chance to trigger
				Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
					early_recovery_called = true;
					Ok(())
				}) as Box<dyn FnMut(&mut _) -> _>
			)
		];

		dev.key_combination_flexible(&[Key::Control], Key::Unicode('c'), &mut failures_early).unwrap();
		drop(failures_early);
		assert!(early_recovery_called);

		// Check early release events: Control is pressed, released, Unicode('c') pressed, released, etc.
		let events_early = mock.get_events();
		let control_release = events_early.iter().position(|e| matches!(e, InputEvent::KeyAction { key: Key::Control, action: Direction::Release })).unwrap();
		let c_press = events_early.iter().position(|e| matches!(e, InputEvent::KeyAction { key: Key::Unicode('c'), action: Direction::Press })).unwrap();
		assert!(control_release < c_press); // Control released early!

		// 2. Test Stuck Modifier
		mock.clear_events();
		let mut failures_stuck = vec![
			(
				KeyCombinationFailure::ModifierStuck(Key::Control),
				1.0, // 100% chance to trigger
				Box::new(|d: &mut HumanizedDevice<MockDevice>| {
					stuck_recovery_called = true;
					// The recovery closure is responsible for unsticking the modifier key!
					d.inner_mut().key(Key::Control, Direction::Release)?;
					Ok(())
				}) as Box<dyn FnMut(&mut _) -> _>
			)
		];

		dev.key_combination_flexible(&[Key::Control], Key::Unicode('c'), &mut failures_stuck).unwrap();
		drop(failures_stuck);
		assert!(stuck_recovery_called);

		let events_stuck = mock.get_events();
		let release_indices: Vec<_> = events_stuck.iter().enumerate()
			.filter(|(_, e)| matches!(e, InputEvent::KeyAction { key: Key::Control, action: Direction::Release }))
			.map(|(idx, _)| idx)
			.collect();

		// The corrected repetition of Ctrl+c at the end will also release Control.
		// So Ctrl is released twice: once in recovery closure, once in the corrected replay.
		assert!(release_indices.len() >= 2);
	}

	#[test]
	fn test_recursive_failure_recovery_guard() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());
		let mut recovery_runs = 0;

		dev.max_recovery_depth = 1;

		let target_area = TargetArea::Point(Point::new(50, 50));

		let mut failures = vec![
			(
				ClickFailure::Misclick,
				1.0, // 100% chance to trigger
				Box::new(|d: &mut HumanizedDevice<MockDevice>| {
					recovery_runs += 1;
					// Inside the recovery, we perform a second humanized call
					// which has failures enabled. It should NOT trigger recursively.
					let mut nested_failures = vec![
						(
							ClickFailure::Misclick,
							1.0, // normally 100% chance to trigger
							Box::new(|_nested_d: &mut HumanizedDevice<MockDevice>| {
								if true {
									panic!("Nested recovery triggered! Recursion guard failed.");
								}
								Ok(())
							}) as Box<dyn FnMut(&mut HumanizedDevice<MockDevice>) -> Result<(), String> + '_>
						)
					];
					d.click_area_flexible(&target_area, Button::Left, &mut nested_failures)?;
					Ok(())
				}) as Box<dyn FnMut(&mut _) -> _>
			)
		];

		dev.click_area_flexible(&target_area, Button::Left, &mut failures).unwrap();

		drop(failures);
		assert_eq!(recovery_runs, 1);
	}
}
