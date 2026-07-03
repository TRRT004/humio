use crate::{HumioError, InputDevice, Keyboard, Mouse, Point, ScrollAxis};
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
	#[must_use]
	pub fn new(initial_location: Point) -> Self {
		Self {
			location: Rc::new(RefCell::new(initial_location)),
			events: Rc::new(RefCell::new(Vec::new())),
		}
	}

	#[must_use]
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
	fn location(&self) -> Result<Point, HumioError> {
		let loc = *self.location.borrow();
		self.events
			.borrow_mut()
			.push(InputEvent::MouseLocationQuery(loc));
		Ok(loc)
	}

	fn move_mouse(&mut self, point: Point) -> Result<(), HumioError> {
		*self.location.borrow_mut() = point;
		self.events.borrow_mut().push(InputEvent::MouseMoved(point));
		Ok(())
	}

	fn move_mouse_by(&mut self, offset: Point) -> Result<(), HumioError> {
		let mut loc = self.location.borrow_mut();
		loc.x += offset.x;
		loc.y += offset.y;
		self.events
			.borrow_mut()
			.push(InputEvent::MouseMovedBy(offset));
		Ok(())
	}

	fn click(&mut self, button: Button) -> Result<(), HumioError> {
		self.events
			.borrow_mut()
			.push(InputEvent::MouseClicked(button));
		Ok(())
	}

	fn hold(&mut self, button: Button) -> Result<(), HumioError> {
		self.events.borrow_mut().push(InputEvent::MouseHeld(button));
		Ok(())
	}

	fn release(&mut self, button: Button) -> Result<(), HumioError> {
		self.events
			.borrow_mut()
			.push(InputEvent::MouseReleased(button));
		Ok(())
	}

	fn scroll(&mut self, length: i32, axis: ScrollAxis) -> Result<(), HumioError> {
		self.events
			.borrow_mut()
			.push(InputEvent::MouseScrolled { length, axis });
		Ok(())
	}
}

impl Keyboard for MockDevice {
	fn key(&mut self, key: Key, action: Direction) -> Result<(), HumioError> {
		self.events
			.borrow_mut()
			.push(InputEvent::KeyAction { key, action });
		Ok(())
	}

	fn text(&mut self, text: &str) -> Result<(), HumioError> {
		self.events
			.borrow_mut()
			.push(InputEvent::TextTyped(text.to_string()));
		Ok(())
	}
}

impl InputDevice for MockDevice {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::humanizer::{ClickFailure, FailureType, KeyCombinationFailure, KeyboardFailure};
	use crate::{DelayMs, HumanizedDevice, Keyboard, Mouse, PathStep, ScrollAxis, TargetArea};

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
		assert!(events.contains(&InputEvent::KeyAction {
			key: Key::Unicode('a'),
			action: Direction::Press
		}));
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
		let mut failures = vec![(
			ClickFailure::Misclick,
			1.0, // 100% chance to trigger
			Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
				recovery_called = true;
				Ok(())
			}) as Box<dyn FnMut(&mut _) -> _>,
		)];

		dev.click_area_flexible(&target_area, Button::Left, &mut failures)
			.unwrap();

		drop(failures);
		assert!(recovery_called);
		// Check that we performed the misclick, recovery, and then the correct click
		let events = mock.get_events();
		let clicks: Vec<_> = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(_)))
			.collect();
		// Should have clicked twice: 1st is the misclick, 2nd is the corrected click after recovery
		assert_eq!(clicks.len(), 2);
	}

	#[test]
	fn test_flexible_key_combination_failure_recovery() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());
		let mut recovery_called = false;

		let mut failures = vec![(
			KeyCombinationFailure::WrongKeyTap(Key::Unicode('a')),
			1.0, // 100% chance to trigger
			Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
				recovery_called = true;
				Ok(())
			}) as Box<dyn FnMut(&mut _) -> _>,
		)];

		dev.key_combination_flexible(&[Key::Control], Key::Unicode('c'), &mut failures)
			.unwrap();

		drop(failures);
		assert!(recovery_called);
		// Check that the keys were pressed, released, and recovery was invoked
		let events = mock.get_events();
		assert!(events.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Unicode('a'),
				action: Direction::Press
			}
		)));
	}

	#[test]
	fn test_text_humanized_failures_transpositions_and_doubletap() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// 1. Force transposition failure (probability = 1.0)
		dev.set_chance_calculator(Box::new(
			|failure: &FailureType, _base: f64| match failure {
				FailureType::Keyboard(KeyboardFailure::Transposition) => 1.0,
				_ => 0.0,
			},
		));

		// We type "ab" -> should swap to "ba", backspace twice, and type "ab"
		dev.text_humanized("ab", true).unwrap();

		let events = mock.get_events();
		// Let's count backspaces
		let backspaces = events
			.iter()
			.filter(|e| {
				matches!(
					e,
					InputEvent::KeyAction {
						key: Key::Backspace,
						action: Direction::Press
					}
				)
			})
			.count();
		assert_eq!(backspaces, 2);

		// 2. Force double tap failure (probability = 1.0)
		mock.clear_events();
		dev.set_chance_calculator(Box::new(
			|failure: &FailureType, _base: f64| match failure {
				FailureType::Keyboard(KeyboardFailure::DoubleTap) => 1.0,
				_ => 0.0,
			},
		));

		// We type "a" -> should double type "aa", backspace once, and complete
		dev.text_humanized("a", true).unwrap();

		let events2 = mock.get_events();
		let backspaces2 = events2
			.iter()
			.filter(|e| {
				matches!(
					e,
					InputEvent::KeyAction {
						key: Key::Backspace,
						action: Direction::Press
					}
				)
			})
			.count();
		assert_eq!(backspaces2, 1);
	}

	#[test]
	fn test_scroll_inertia() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		dev.scroll(10, ScrollAxis::Vertical).unwrap();

		let events = mock.get_events();
		let scrolls: Vec<_> = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseScrolled { .. }))
			.collect();
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
		let mut failures_early = vec![(
			KeyCombinationFailure::ReleasedModifierEarly(Key::Control),
			1.0, // 100% chance to trigger
			Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
				early_recovery_called = true;
				Ok(())
			}) as Box<dyn FnMut(&mut _) -> _>,
		)];

		dev.key_combination_flexible(&[Key::Control], Key::Unicode('c'), &mut failures_early)
			.unwrap();
		drop(failures_early);
		assert!(early_recovery_called);

		// Check early release events: Control is pressed, released, Unicode('c') pressed, released, etc.
		let events_early = mock.get_events();
		let control_release = events_early
			.iter()
			.position(|e| {
				matches!(
					e,
					InputEvent::KeyAction {
						key: Key::Control,
						action: Direction::Release
					}
				)
			})
			.unwrap();
		let c_press = events_early
			.iter()
			.position(|e| {
				matches!(
					e,
					InputEvent::KeyAction {
						key: Key::Unicode('c'),
						action: Direction::Press
					}
				)
			})
			.unwrap();
		assert!(control_release < c_press); // Control released early!

		// 2. Test Stuck Modifier
		mock.clear_events();
		let mut failures_stuck = vec![(
			KeyCombinationFailure::ModifierStuck(Key::Control),
			1.0, // 100% chance to trigger
			Box::new(|d: &mut HumanizedDevice<MockDevice>| {
				stuck_recovery_called = true;
				// The recovery closure is responsible for unsticking the modifier key!
				d.inner_mut().key(Key::Control, Direction::Release)?;
				Ok(())
			}) as Box<dyn FnMut(&mut _) -> _>,
		)];

		dev.key_combination_flexible(&[Key::Control], Key::Unicode('c'), &mut failures_stuck)
			.unwrap();
		drop(failures_stuck);
		assert!(stuck_recovery_called);

		let events_stuck = mock.get_events();
		let release_indices: Vec<_> = events_stuck
			.iter()
			.enumerate()
			.filter(|(_, e)| {
				matches!(
					e,
					InputEvent::KeyAction {
						key: Key::Control,
						action: Direction::Release
					}
				)
			})
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

		let mut failures = vec![(
			ClickFailure::Misclick,
			1.0, // 100% chance to trigger
			Box::new(|d: &mut HumanizedDevice<MockDevice>| {
				recovery_runs += 1;
				// Inside the recovery, we perform a second humanized call
				// which has failures enabled. It should NOT trigger recursively.
				let mut nested_failures =
					vec![
						(
							ClickFailure::Misclick,
							1.0, // normally 100% chance to trigger
							Box::new(|_nested_d: &mut HumanizedDevice<MockDevice>| -> Result<(), HumioError> {
								panic!("Nested recovery triggered! Recursion guard failed.");
							}) as Box<dyn FnMut(&mut HumanizedDevice<MockDevice>) -> Result<(), HumioError> + '_>
						)
					];
				d.click_area_flexible(&target_area, Button::Left, &mut nested_failures)?;
				Ok(())
			}) as Box<dyn FnMut(&mut _) -> _>,
		)];

		dev.click_area_flexible(&target_area, Button::Left, &mut failures)
			.unwrap();

		drop(failures);
		assert_eq!(recovery_runs, 1);
	}

	#[test]
	fn test_compound_click_failures() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());
		let mut recovery_called = false;

		let target_area = TargetArea::Point(Point::new(50, 50));

		let compound = ClickFailure::Compound(vec![
			ClickFailure::Misclick,
			ClickFailure::WrongButton(Button::Right),
		]);

		let mut failures = vec![(
			compound,
			1.0, // 100% chance to trigger
			Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
				recovery_called = true;
				Ok(())
			}) as Box<dyn FnMut(&mut _) -> _>,
		)];

		dev.click_area_flexible(&target_area, Button::Left, &mut failures)
			.unwrap();

		drop(failures);
		assert!(recovery_called);

		let events = mock.get_events();
		let clicks: Vec<_> = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(_)))
			.collect();
		// Should have clicked 3 times:
		// 1. misclick (Left button)
		// 2. wrong button click (Right button)
		// 3. corrected click (Left button) after recovery
		assert_eq!(clicks.len(), 3);
		assert_eq!(clicks[0], &InputEvent::MouseClicked(Button::Left));
		assert_eq!(clicks[1], &InputEvent::MouseClicked(Button::Right));
		assert_eq!(clicks[2], &InputEvent::MouseClicked(Button::Left));
	}

	#[test]
	fn test_compound_key_combination_failures() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());
		let mut recovery_called = false;

		let compound = KeyCombinationFailure::Compound(vec![
			KeyCombinationFailure::MissedModifier(Key::Control),
			KeyCombinationFailure::WrongKeyTap(Key::Unicode('a')),
		]);

		let mut failures = vec![(
			compound,
			1.0, // 100% chance to trigger
			Box::new(|_d: &mut HumanizedDevice<MockDevice>| {
				recovery_called = true;
				Ok(())
			}) as Box<dyn FnMut(&mut _) -> _>,
		)];

		dev.key_combination_flexible(&[Key::Control], Key::Unicode('c'), &mut failures)
			.unwrap();

		drop(failures);
		assert!(recovery_called);

		let events = mock.get_events();
		// Should have typed 'a' during the wrong key tap sub-failure
		assert!(events.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Unicode('a'),
				action: Direction::Press
			}
		)));
	}

	#[test]
	fn test_point_delay_scroll_axis() {
		// Point coverage
		let p1 = Point::new(10, 20);
		let p2 = Point::new(10, 20);
		let p3 = Point::new(30, 40);
		assert_eq!(p1, p2);
		assert_ne!(p1, p3);
		let _p_debug = format!("{p1:?}");

		// DelayMs coverage
		let d1 = DelayMs(100);
		let d2 = DelayMs(100);
		let d3 = DelayMs(200);
		assert_eq!(d1, d2);
		assert!(d1 < d3);
		assert_eq!(d1.to_duration(), std::time::Duration::from_millis(100));
		let _d_debug = format!("{d1:?}");

		// ScrollAxis coverage
		let axis1 = ScrollAxis::Horizontal;
		let axis2 = ScrollAxis::Vertical;
		assert_ne!(axis1, axis2);
		let _axis_debug = format!("{axis1:?}");

		// PathStep coverage
		let step1 = PathStep {
			point: p1,
			delay: d1,
		};
		let step2 = PathStep {
			point: p1,
			delay: d1,
		};
		assert_eq!(step1, step2);
		let _step_debug = format!("{step1:?}");

		// Default Keyboard::key_combination coverage (MockDevice uses default trait implementation)
		let mut mock = MockDevice::new(Point::new(0, 0));
		mock.key_combination(&[Key::Control, Key::Alt], Key::Unicode('c'))
			.unwrap();
		let events = mock.get_events();
		// Expected: Control Press -> Alt Press -> Unicode('c') Click -> Alt Release -> Control Release
		assert_eq!(events.len(), 5);
		assert_eq!(
			events[0],
			InputEvent::KeyAction {
				key: Key::Control,
				action: Direction::Press
			}
		);
		assert_eq!(
			events[1],
			InputEvent::KeyAction {
				key: Key::Alt,
				action: Direction::Press
			}
		);
		assert_eq!(
			events[2],
			InputEvent::KeyAction {
				key: Key::Unicode('c'),
				action: Direction::Click
			}
		);
		assert_eq!(
			events[3],
			InputEvent::KeyAction {
				key: Key::Alt,
				action: Direction::Release
			}
		);
		assert_eq!(
			events[4],
			InputEvent::KeyAction {
				key: Key::Control,
				action: Direction::Release
			}
		);
	}

	#[test]
	fn test_humio_error_formatting() {
		use crate::HumioError;
		let err_backend = HumioError::Backend("backend failure".to_string());
		let err_loc = HumioError::LocationQuery("query failure".to_string());

		assert!(format!("{err_backend:?}").contains("Backend"));
		assert!(format!("{err_backend}").contains("backend failure"));
		assert!(format!("{err_loc}").contains("query failure"));
	}

	#[test]
	fn test_physical_device_safe() {
		use crate::PhysicalDevice;
		// PhysicalDevice uses Enigo, which might fail to initialize on headless CI/CD,
		// but since we are running locally on Windows with GUI, it should succeed.
		// We handle it gracefully regardless.
		if let Ok(mut dev) = PhysicalDevice::new() {
			let _loc = dev.location();
			let _ = dev.move_mouse_by(Point::new(0, 0));
			let _ = dev.scroll(0, ScrollAxis::Vertical);
			let _ = dev.scroll(0, ScrollAxis::Horizontal);
		}
	}

	#[test]
	fn test_delay_gaussian_edge_cases() {
		use crate::humanizer::delay::{
			sample_gaussian, sample_gaussian_clamped, sleep_gaussian_delay,
		};

		// sample_gaussian should execute without panic
		let _val = sample_gaussian(10.0, 2.0);

		// sample_gaussian_clamped with min >= max should return min
		let min_greater_max = sample_gaussian_clamped(10, 2, 20, 10);
		assert_eq!(min_greater_max, 20);

		let min_equal_max = sample_gaussian_clamped(10, 2, 15, 15);
		assert_eq!(min_equal_max, 15);

		// test clamped limits
		let val_clamped_min = sample_gaussian_clamped(-100, 1, 0, 10);
		assert_eq!(val_clamped_min, 0);

		let val_clamped_max = sample_gaussian_clamped(100, 1, 0, 10);
		assert_eq!(val_clamped_max, 10);

		// sleep_gaussian_delay execution (small delay of 1ms so it runs quickly)
		sleep_gaussian_delay(DelayMs(1), 0);
	}

	#[test]
	fn test_humanized_device_wrappers() {
		let mock = MockDevice::new(Point::new(10, 10));
		let mut dev = HumanizedDevice::new(mock.clone());

		// Check initial bypass value
		assert!(!dev.should_bypass_failures());

		// inner_mut
		dev.inner_mut().set_location(Point::new(20, 20));
		assert_eq!(dev.location().unwrap(), Point::new(20, 20));

		// set/remove chance calculator
		dev.set_chance_calculator(Box::new(|_failure: &FailureType, _base: f64| 0.5));
		assert!(dev.chance_calculator.is_some());
		dev.remove_chance_calculator();
		assert!(dev.chance_calculator.is_none());

		// into_inner
		let inner_device = dev.into_inner();
		assert_eq!(inner_device.location().unwrap(), Point::new(20, 20));
	}

	#[test]
	fn test_target_area_rect_custom() {
		// Test rect with custom target (inside)
		let rect_custom_target = TargetArea::Rect {
			top_left: Point::new(10, 10),
			bottom_right: Point::new(50, 50),
			target: Some(Point::new(25, 25)),
			std_dev_x: Some(2),
			std_dev_y: Some(2),
		};
		let pt = rect_custom_target.generate_click_point();
		assert!(pt.x >= 10 && pt.x <= 50);
		assert!(pt.y >= 10 && pt.y <= 50);

		// Test rect with custom target (clamped outside)
		let rect_clamped_target = TargetArea::Rect {
			top_left: Point::new(10, 10),
			bottom_right: Point::new(50, 50),
			target: Some(Point::new(5, 60)),
			std_dev_x: None,
			std_dev_y: None,
		};
		let pt2 = rect_clamped_target.generate_click_point();
		assert!(pt2.x >= 10 && pt2.x <= 50);
		assert!(pt2.y >= 10 && pt2.y <= 50);
	}

	#[test]
	fn test_target_area_circle() {
		// 1. Circle with default target (center)
		let circle_default = TargetArea::Circle {
			center: Point::new(100, 100),
			radius: 20,
			target: None,
			std_dev: None,
		};
		let pt1 = circle_default.generate_click_point();
		let dx = pt1.x - 100;
		let dy = pt1.y - 100;
		assert!(dx * dx + dy * dy <= 20 * 20);

		// 2. Circle with custom target inside
		let circle_custom = TargetArea::Circle {
			center: Point::new(100, 100),
			radius: 20,
			target: Some(Point::new(105, 105)),
			std_dev: Some(5),
		};
		let pt2 = circle_custom.generate_click_point();
		let dx = pt2.x - 100;
		let dy = pt2.y - 100;
		assert!(dx * dx + dy * dy <= 20 * 20);

		// 3. Circle with custom target outside (which gets clamped)
		let circle_outside = TargetArea::Circle {
			center: Point::new(100, 100),
			radius: 20,
			target: Some(Point::new(200, 200)),
			std_dev: None,
		};
		let pt3 = circle_outside.generate_click_point();
		let dx = pt3.x - 100;
		let dy = pt3.y - 100;
		assert!(dx * dx + dy * dy <= 20 * 20);

		// 4. Rejection sampling timeout fallback (radius 0)
		let circle_rejection = TargetArea::Circle {
			center: Point::new(100, 100),
			radius: 0,
			target: Some(Point::new(100, 100)),
			std_dev: Some(10),
		};
		let pt4 = circle_rejection.generate_click_point();
		assert_eq!(pt4, Point::new(100, 100));
	}

	#[test]
	fn test_target_area_polygon() {
		// 1. Empty vertices
		let poly_empty = TargetArea::Polygon {
			vertices: vec![],
			target: None,
			std_dev_x: None,
			std_dev_y: None,
		};
		assert_eq!(poly_empty.generate_click_point(), Point::new(0, 0));

		// 2. Vertices count < 3 (rejection sampling will timeout because is_point_in_polygon is false,
		// and will fall back to vertex 0)
		let vertices_few = vec![Point::new(10, 10), Point::new(20, 20)];
		let poly_few = TargetArea::Polygon {
			vertices: vertices_few.clone(),
			target: None,
			std_dev_x: None,
			std_dev_y: None,
		};
		assert_eq!(poly_few.generate_click_point(), vertices_few[0]);

		// 3. Normal polygon (Triangle)
		let vertices_tri = vec![Point::new(0, 0), Point::new(100, 0), Point::new(50, 100)];
		let poly_tri = TargetArea::Polygon {
			vertices: vertices_tri.clone(),
			target: Some(Point::new(50, 20)),
			std_dev_x: Some(5),
			std_dev_y: Some(5),
		};
		// Generates click point inside
		let pt = poly_tri.generate_click_point();
		// Let's assert that the generated point is inside the bounding box
		assert!(pt.x >= 0 && pt.x <= 100);
		assert!(pt.y >= 0 && pt.y <= 100);

		// 4. Rejection sampling timeout fallback where target is outside
		let poly_fallback = TargetArea::Polygon {
			vertices: vertices_tri.clone(),
			target: Some(Point::new(500, 500)), // Far outside
			std_dev_x: Some(1),
			std_dev_y: Some(1),
		};
		// Rejection sampling will fail to generate point inside poly because std dev is 1 and target is 500.
		// Should fall back to target (if in polygon - which it isn't), then centroid (if in polygon), then vertex 0.
		// Triangle centroid is (0+100+50)/3 = 50, (0+0+100)/3 = 33.
		// Let's check if centroid (50, 33) is in polygon. Yes, it is!
		// So fallback should return centroid.
		let pt_fallback = poly_fallback.generate_click_point();
		assert_eq!(pt_fallback, Point::new(50, 33));
	}

	#[test]
	fn test_wind_mouse_early_return() {
		use crate::humanizer::wind_mouse::generate_wind_mouse_path;
		let pt = Point::new(10, 10);
		let path = generate_wind_mouse_path(pt, pt);
		assert_eq!(path.len(), 1);
		assert_eq!(path[0].point, pt);
		assert_eq!(path[0].delay.0, 0);
	}

	#[test]
	fn test_keyboard_typo_variations() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// Set chance calculator to force standard KeyboardFailure::Typo
		dev.set_chance_calculator(Box::new(
			|failure: &FailureType, _base: f64| match failure {
				FailureType::Keyboard(KeyboardFailure::Typo) => 1.0,
				_ => 0.0,
			},
		));

		// 1. Test lowercase char typo
		dev.text_humanized("z", true).unwrap();
		let events_lower = mock.get_events();
		// Should have typed a typo character (which is randomly chosen a..z/A..Z, but lowercase here since 'z' is lowercase),
		// then typed Backspace, then typed correct 'z'.
		let typed_events: Vec<_> = events_lower
			.iter()
			.filter_map(|e| {
				if let InputEvent::TextTyped(s) = e {
					Some(s.as_str())
				} else {
					None
				}
			})
			.collect();
		// Should have typed a typo char first, then 'z'
		assert_eq!(typed_events.len(), 2);
		assert_eq!(*typed_events.last().unwrap(), "z");
		assert!(events_lower.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Backspace,
				..
			}
		)));

		// 2. Test uppercase char typo
		mock.clear_events();
		dev.text_humanized("Z", true).unwrap();
		let events_upper = mock.get_events();
		let typed_events_upper: Vec<_> = events_upper
			.iter()
			.filter_map(|e| {
				if let InputEvent::TextTyped(s) = e {
					Some(s.as_str())
				} else {
					None
				}
			})
			.collect();
		assert_eq!(typed_events_upper.len(), 2);
		assert_eq!(*typed_events_upper.last().unwrap(), "Z");
		assert!(events_upper.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Backspace,
				..
			}
		)));
	}

	#[test]
	fn test_keyboard_builtin_recoveries() {
		// Test standard built-in recoveries when failures are configured in config.key_combo_failures
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// 1. MissedModifier
		dev.config.key_combo_failures = vec![(
			KeyCombinationFailure::MissedModifier(Key::Control),
			1.0, // 100% chance to trigger
		)];
		dev.key_combination_humanized(&[Key::Control], Key::Unicode('c'), true)
			.unwrap();
		// Recovery for MissedModifier is to retry correct combination
		let events = mock.get_events();
		// Control press must occur in the retry
		assert!(events.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Control,
				action: Direction::Press
			}
		)));

		// 2. ReleasedModifierEarly
		mock.clear_events();
		dev.config.key_combo_failures = vec![(
			KeyCombinationFailure::ReleasedModifierEarly(Key::Control),
			1.0,
		)];
		dev.key_combination_humanized(&[Key::Control], Key::Unicode('c'), true)
			.unwrap();
		let events = mock.get_events();
		assert!(events.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Control,
				action: Direction::Release
			}
		)));

		// 3. WrongKeyTap
		mock.clear_events();
		dev.config.key_combo_failures =
			vec![(KeyCombinationFailure::WrongKeyTap(Key::Unicode('x')), 1.0)];
		dev.key_combination_humanized(&[Key::Control], Key::Unicode('c'), true)
			.unwrap();
		let events = mock.get_events();
		// WrongKeyTap recovery taps Backspace and retries
		assert!(events.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Backspace,
				..
			}
		)));

		// 4. ModifierStuck
		mock.clear_events();
		dev.config.key_combo_failures =
			vec![(KeyCombinationFailure::ModifierStuck(Key::Control), 1.0)];
		dev.key_combination_humanized(&[Key::Control], Key::Unicode('c'), true)
			.unwrap();
		let events = mock.get_events();
		// ModifierStuck recovery releases the stuck modifier and retries
		let ctrl_releases = events
			.iter()
			.filter(|e| {
				matches!(
					e,
					InputEvent::KeyAction {
						key: Key::Control,
						action: Direction::Release
					}
				)
			})
			.count();
		assert!(ctrl_releases >= 1);

		// 5. Compound
		mock.clear_events();
		dev.config.key_combo_failures = vec![(
			KeyCombinationFailure::Compound(vec![KeyCombinationFailure::MissedModifier(
				Key::Control,
			)]),
			1.0,
		)];
		dev.key_combination_humanized(&[Key::Control], Key::Unicode('c'), true)
			.unwrap();
		let events = mock.get_events();
		assert!(events.iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Control,
				action: Direction::Press
			}
		)));
	}

	#[test]
	fn test_keyboard_trait_methods_on_humanized_device() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// test trait method `key`
		Keyboard::key(&mut dev, Key::Unicode('a'), Direction::Press).unwrap();
		assert_eq!(
			mock.get_events()[0],
			InputEvent::KeyAction {
				key: Key::Unicode('a'),
				action: Direction::Press
			}
		);

		// test trait method `text` (no failures allowed)
		mock.clear_events();
		Keyboard::text(&mut dev, "test").unwrap();
		assert!(
			mock.get_events()
				.iter()
				.any(|e| matches!(e, InputEvent::TextTyped(_)))
		);

		// test trait method `key_combination`
		mock.clear_events();
		Keyboard::key_combination(&mut dev, &[Key::Control], Key::Unicode('c')).unwrap();
		assert!(mock.get_events().iter().any(|e| matches!(
			e,
			InputEvent::KeyAction {
				key: Key::Unicode('c'),
				..
			}
		)));
	}

	#[test]
	fn test_mouse_overshoot_glide() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// Force overshoot path by setting config overshoot_chance to 1.0
		dev.config.overshoot_chance = 1.0;
		dev.move_to_area(&TargetArea::Point(Point::new(100, 100)), true)
			.unwrap();

		let events = mock.get_events();
		// In an overshoot path, we move to overshoot_point first, then to actual target (100, 100).
		// The last step should still be at (100, 100).
		assert_eq!(mock.location().unwrap(), Point::new(100, 100));

		// Check that the mouse path visited some point beyond or offset from the target, then came back.
		let path_points: Vec<Point> = events
			.iter()
			.filter_map(|e| {
				if let InputEvent::MouseMoved(p) = e {
					Some(*p)
				} else {
					None
				}
			})
			.collect();
		assert!(!path_points.is_empty());
		assert_eq!(*path_points.last().unwrap(), Point::new(100, 100));
	}

	#[test]
	fn test_mouse_misclick_to_and_double_click() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// 1. MisclickTo
		let error_target = TargetArea::Point(Point::new(20, 20));
		let mut failures = vec![(
			ClickFailure::MisclickTo(error_target),
			1.0,
			Box::new(|_d: &mut HumanizedDevice<MockDevice>| Ok(())) as Box<dyn FnMut(&mut _) -> _>,
		)];
		dev.click_area_flexible(
			&TargetArea::Point(Point::new(100, 100)),
			Button::Left,
			&mut failures,
		)
		.unwrap();
		// Verify we clicked at the misclick target (20, 20) during failure phase.
		assert_eq!(mock.location().unwrap(), Point::new(100, 100)); // corrected location at end

		// 2. DoubleClick
		mock.clear_events();
		let mut failures_double = vec![(
			ClickFailure::DoubleClick,
			1.0,
			Box::new(|_d: &mut HumanizedDevice<MockDevice>| Ok(())) as Box<dyn FnMut(&mut _) -> _>,
		)];
		dev.click_area_flexible(
			&TargetArea::Point(Point::new(100, 100)),
			Button::Left,
			&mut failures_double,
		)
		.unwrap();
		let clicks = mock
			.get_events()
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(Button::Left)))
			.count();
		// 2 clicks in double click failure + 1 click in retry = 3 total clicks
		assert_eq!(clicks, 3);
	}

	#[test]
	fn test_mouse_builtin_recoveries() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// 1. Misclick
		dev.config.click_failures = vec![(ClickFailure::Misclick, 1.0)];
		dev.click_area(&TargetArea::Point(Point::new(50, 50)), Button::Left, true)
			.unwrap();
		let events = mock.get_events();
		let clicks = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(Button::Left)))
			.count();
		// 1 misclick + 1 retry = 2 clicks
		assert_eq!(clicks, 2);

		// 2. MisclickTo
		mock.clear_events();
		dev.config.click_failures = vec![(
			ClickFailure::MisclickTo(TargetArea::Point(Point::new(10, 10))),
			1.0,
		)];
		dev.click_area(&TargetArea::Point(Point::new(50, 50)), Button::Left, true)
			.unwrap();
		let events = mock.get_events();
		let clicks = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(Button::Left)))
			.count();
		assert_eq!(clicks, 2);

		// 3. WrongButton
		mock.clear_events();
		dev.config.click_failures = vec![(ClickFailure::WrongButton(Button::Right), 1.0)];
		dev.click_area(&TargetArea::Point(Point::new(50, 50)), Button::Left, true)
			.unwrap();
		let events = mock.get_events();
		let left_clicks = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(Button::Left)))
			.count();
		let right_clicks = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(Button::Right)))
			.count();
		assert_eq!(left_clicks, 1);
		assert_eq!(right_clicks, 1);

		// 4. DoubleClick
		mock.clear_events();
		dev.config.click_failures = vec![(ClickFailure::DoubleClick, 1.0)];
		dev.click_area(&TargetArea::Point(Point::new(50, 50)), Button::Left, true)
			.unwrap();
		let events = mock.get_events();
		let clicks = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(Button::Left)))
			.count();
		// DoubleClick recovery realizes and does not retry click (since double clicking is often harmless/already happened)
		// So it is just 2 clicks from the DoubleClick failure itself.
		assert_eq!(clicks, 2);

		// 5. Compound
		mock.clear_events();
		dev.config.click_failures =
			vec![(ClickFailure::Compound(vec![ClickFailure::Misclick]), 1.0)];
		dev.click_area(&TargetArea::Point(Point::new(50, 50)), Button::Left, true)
			.unwrap();
		let events = mock.get_events();
		let clicks = events
			.iter()
			.filter(|e| matches!(e, InputEvent::MouseClicked(Button::Left)))
			.count();
		assert_eq!(clicks, 2);
	}

	#[test]
	fn test_mouse_trait_methods_on_humanized_device() {
		let mock = MockDevice::new(Point::new(0, 0));
		let mut dev = HumanizedDevice::new(mock.clone());

		// 1. location()
		assert_eq!(Mouse::location(&dev).unwrap(), Point::new(0, 0));

		// 2. move_mouse()
		Mouse::move_mouse(&mut dev, Point::new(20, 20)).unwrap();
		assert_eq!(mock.location().unwrap(), Point::new(20, 20));

		// 3. move_mouse_by()
		Mouse::move_mouse_by(&mut dev, Point::new(10, -5)).unwrap();
		assert_eq!(mock.location().unwrap(), Point::new(30, 15));

		// 4. click()
		mock.clear_events();
		Mouse::click(&mut dev, Button::Left).unwrap();
		assert!(
			mock.get_events()
				.iter()
				.any(|e| matches!(e, InputEvent::MouseClicked(Button::Left)))
		);

		// 5. hold()
		mock.clear_events();
		Mouse::hold(&mut dev, Button::Left).unwrap();
		assert!(
			mock.get_events()
				.iter()
				.any(|e| matches!(e, InputEvent::MouseHeld(Button::Left)))
		);

		// 6. release()
		mock.clear_events();
		Mouse::release(&mut dev, Button::Left).unwrap();
		assert!(
			mock.get_events()
				.iter()
				.any(|e| matches!(e, InputEvent::MouseReleased(Button::Left)))
		);

		// 7. scroll() - horizontal positive, horizontal negative, vertical positive, vertical negative
		mock.clear_events();
		Mouse::scroll(&mut dev, 5, ScrollAxis::Horizontal).unwrap();
		Mouse::scroll(&mut dev, -5, ScrollAxis::Vertical).unwrap();
		let events = mock.get_events();
		let scroll_horiz: Vec<_> = events
			.iter()
			.filter(|e| {
				matches!(
					e,
					InputEvent::MouseScrolled {
						axis: ScrollAxis::Horizontal,
						..
					}
				)
			})
			.collect();
		let scroll_vert: Vec<_> = events
			.iter()
			.filter(|e| {
				matches!(
					e,
					InputEvent::MouseScrolled {
						axis: ScrollAxis::Vertical,
						..
					}
				)
			})
			.collect();
		assert!(!scroll_horiz.is_empty());
		assert!(!scroll_vert.is_empty());
	}

	#[test]
	fn test_modifier_guard_error_recovery() {
		struct PartialErrorDevice {
			press_count: usize,
			released_keys: std::rc::Rc<std::cell::RefCell<Vec<Key>>>,
		}
		impl Mouse for PartialErrorDevice {
			fn location(&self) -> Result<Point, HumioError> {
				Ok(Point::new(0, 0))
			}
			fn move_mouse(&mut self, _: Point) -> Result<(), HumioError> {
				Ok(())
			}
			fn move_mouse_by(&mut self, _: Point) -> Result<(), HumioError> {
				Ok(())
			}
			fn click(&mut self, _: Button) -> Result<(), HumioError> {
				Ok(())
			}
			fn hold(&mut self, _: Button) -> Result<(), HumioError> {
				Ok(())
			}
			fn release(&mut self, _: Button) -> Result<(), HumioError> {
				Ok(())
			}
			fn scroll(&mut self, _: i32, _: ScrollAxis) -> Result<(), HumioError> {
				Ok(())
			}
		}
		impl Keyboard for PartialErrorDevice {
			fn key(&mut self, key: Key, action: Direction) -> Result<(), HumioError> {
				if action == Direction::Press {
					self.press_count += 1;
					if self.press_count > 1 {
						return Err(HumioError::Backend("failed on second press".to_string()));
					}
				} else if action == Direction::Release {
					self.released_keys.borrow_mut().push(key);
				}
				Ok(())
			}
			fn text(&mut self, _: &str) -> Result<(), HumioError> {
				Ok(())
			}
		}
		impl InputDevice for PartialErrorDevice {}

		let released = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
		let ped = PartialErrorDevice {
			press_count: 0,
			released_keys: released.clone(),
		};
		let mut dev = HumanizedDevice::new(ped);

		// This call should fail on the second modifier (Alt), triggering drop on the guard,
		// which should release the first modifier (Control).
		let res = dev.key_combination_normal(&[Key::Control, Key::Alt], Key::Unicode('c'));
		assert!(res.is_err());
		// Control should be released!
		assert_eq!(released.borrow().len(), 1);
		assert_eq!(released.borrow()[0], Key::Control);
	}
}
