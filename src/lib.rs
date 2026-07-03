#![allow(
	clippy::missing_errors_doc,
	clippy::similar_names,
	clippy::too_many_lines,
	clippy::type_complexity
)]

pub mod error;
pub mod humanizer;
pub mod mock;
pub mod physical_device;

pub use error::HumioError;
pub use humanizer::{
	ClickFailure, FailureChanceCalculator, FailureType, HumanizedDevice, HumanizerConfig,
	KeyCombinationFailure, KeyboardFailure, TargetArea,
};
pub use mock::{InputEvent, MockDevice};
pub use physical_device::PhysicalDevice;

use enigo::{Button, Direction, Key};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
	pub x: i32,
	pub y: i32,
}

impl Point {
	#[must_use]
	pub const fn new(x: i32, y: i32) -> Self {
		Self { x, y }
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DelayMs(pub u32);

impl DelayMs {
	#[must_use]
	pub const fn to_duration(self) -> std::time::Duration {
		std::time::Duration::from_millis(self.0 as u64)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathStep {
	pub point: Point,
	pub delay: DelayMs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
	Horizontal,
	Vertical,
}

pub trait Mouse {
	fn location(&self) -> Result<Point, HumioError>;
	fn move_mouse(&mut self, point: Point) -> Result<(), HumioError>;
	fn move_mouse_by(&mut self, offset: Point) -> Result<(), HumioError>;
	fn click(&mut self, button: Button) -> Result<(), HumioError>;
	fn hold(&mut self, button: Button) -> Result<(), HumioError>;
	fn release(&mut self, button: Button) -> Result<(), HumioError>;
	fn scroll(&mut self, length: i32, axis: ScrollAxis) -> Result<(), HumioError>;
}

pub trait Keyboard {
	fn key(&mut self, key: Key, action: Direction) -> Result<(), HumioError>;
	fn text(&mut self, text: &str) -> Result<(), HumioError>;

	fn key_combination(&mut self, modifiers: &[Key], key: Key) -> Result<(), HumioError> {
		for &mod_key in modifiers {
			self.key(mod_key, Direction::Press)?;
		}
		self.key(key, Direction::Click)?;
		for &mod_key in modifiers.iter().rev() {
			self.key(mod_key, Direction::Release)?;
		}
		Ok(())
	}
}

pub trait InputDevice: Mouse + Keyboard {}
