#![allow(
	clippy::missing_errors_doc,
	clippy::similar_names,
	clippy::too_many_lines,
	clippy::type_complexity
)]
#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

//! # humio
//!
//! A humanized input simulation library for Rust scripting.
//!
//! `humio` provides realistic mouse movement, keyboard typing, and configurable failure
//! injection with automatic recovery — designed to mimic human imperfection rather than
//! robotic precision.
//!
//! ## Core Features
//!
//! - **WindMouse path generation** — human-like curved mouse paths with inertia and wind forces.
//! - **Gaussian delay distributions** — natural timing variance for clicks and keystrokes using a Box-Muller transform.
//! - **Hover overshoots** — configurable probability of slightly overshooting a target before correcting.
//! - **Typing error simulation** — realistic typos, transpositions, and double-taps with backspace correction.
//! - **Key combination failure injection** — simulates missing modifier keys, early release, or stuck keys.
//! - **Flexible failure API** — per-failure probabilities with custom recovery closures.
//! - **Testing Utilities** — includes [`MockDevice`] to capture and verify input sequences.
//!
//! ## Architecture
//!
//! ```text
//! humio
//!  ├── HumanizedDevice<D> (orchestrates delay, pathgen, errors, and recovery)
//!  │    ├── WindMouse Path Algorithm
//!  │    ├── Gaussian timing delay generators
//!  │    └── Configurable failure models & recovery routines
//!  └── InputDevice Trait (Mouse + Keyboard)
//!       ├── PhysicalDevice (Enigo hardware driver)
//!       └── MockDevice (In-memory testing buffer)
//! ```
//!
//! ## Quick Start
//!
//! ```rust
//! use humio::{HumanizedDevice, PhysicalDevice, TargetArea, Point};
//! use enigo::Button;
//!
//! # fn run() -> Result<(), humio::error::HumioError> {
//! // Initialize a real hardware device wrapper
//! let physical = PhysicalDevice::new()?;
//! let mut dev = HumanizedDevice::new(physical);
//!
//! // Target a rectangular region on screen
//! let target = TargetArea::Rect {
//!     top_left: Point::new(100, 200),
//!     bottom_right: Point::new(300, 250),
//!     target: None,
//!     std_dev_x: None,
//!     std_dev_y: None,
//! };
//!
//! // Perform a click inside the target area (with simulated delays and humanizer config)
//! dev.click_area(&target, Button::Left, true)?;
//!
//! // Type text with natural delays and simulated typos
//! dev.text_humanized("Hello, humio!", true)?;
//! # Ok(())
//! # }
//! ```

/// Error types and error handling support.
pub mod error;
/// Core humanization wrapper, timing configurations, and error injection engines.
pub mod humanizer;
/// Memory-buffered mock input recorder for unit testing.
pub mod mock;
/// OS-level keyboard/mouse hardware simulator backend driver.
pub mod physical_device;

pub use error::HumioError;
pub use humanizer::{
	ClickFailure, FailureChanceCalculator, FailureType, HumanizedDevice, HumanizerConfig,
	KeyCombinationFailure, KeyboardFailure, TargetArea,
};
pub use mock::{InputEvent, MockDevice};
pub use physical_device::PhysicalDevice;

use enigo::{Button, Direction, Key};

/// Represents a 2D screen coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
	/// The X coordinate (horizontal axis).
	pub x: i32,
	/// The Y coordinate (vertical axis).
	pub y: i32,
}

impl Point {
	/// Creates a new `Point` with the given coordinates.
	#[must_use]
	pub const fn new(x: i32, y: i32) -> Self {
		Self { x, y }
	}
}

/// A wrapper around a millisecond duration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DelayMs(pub u32);

impl DelayMs {
	/// Converts the millisecond value into a standard [`std::time::Duration`].
	#[must_use]
	pub const fn to_duration(self) -> std::time::Duration {
		std::time::Duration::from_millis(self.0 as u64)
	}
}

/// A single step along a generated mouse movement path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathStep {
	/// The screen coordinate for this step.
	pub point: Point,
	/// The timing delay associated with this step before moving to the next.
	pub delay: DelayMs,
}

/// The scrolling directions/axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
	/// Horizontal scrolling (left/right).
	Horizontal,
	/// Vertical scrolling (up/down).
	Vertical,
}

/// Lower-level mouse actions trait.
pub trait Mouse {
	/// Queries the current absolute mouse position.
	fn location(&self) -> Result<Point, HumioError>;
	/// Instantly moves the mouse cursor to the given coordinate.
	fn move_mouse(&mut self, point: Point) -> Result<(), HumioError>;
	/// Instantly offsets the mouse cursor position by the given relative offset.
	fn move_mouse_by(&mut self, offset: Point) -> Result<(), HumioError>;
	/// Simulates a complete click (press + release) of the specified button.
	fn click(&mut self, button: Button) -> Result<(), HumioError>;
	/// Holds down the specified mouse button.
	fn hold(&mut self, button: Button) -> Result<(), HumioError>;
	/// Releases a currently held mouse button.
	fn release(&mut self, button: Button) -> Result<(), HumioError>;
	/// Scrolls the wheel along the specified axis by the given step length.
	fn scroll(&mut self, length: i32, axis: ScrollAxis) -> Result<(), HumioError>;
}

/// Lower-level keyboard actions trait.
pub trait Keyboard {
	/// Performs a raw key action (press, release, or click).
	fn key(&mut self, key: Key, action: Direction) -> Result<(), HumioError>;
	/// Types the given string literal verbatim.
	fn text(&mut self, text: &str) -> Result<(), HumioError>;

	/// Simulates a key combination by pressing modifier keys in order, clicking the target key,
	/// and releasing modifier keys in reverse order.
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

/// Represents any hardware-like driver supporting both Mouse and Keyboard operations.
pub trait InputDevice: Mouse + Keyboard {}
