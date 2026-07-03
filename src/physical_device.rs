use crate::{InputDevice, Keyboard, Mouse, Point, ScrollAxis};
use enigo::{
	Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard as EnigoKeyboard,
	Mouse as EnigoMouse, Settings,
};

pub struct PhysicalDevice {
	enigo: Enigo,
}

impl PhysicalDevice {
	pub fn new() -> Result<Self, String> {
		let enigo =
			Enigo::new(&Settings::default()).map_err(|e| format!("Failed to init Enigo: {e:?}"))?;
		Ok(Self { enigo })
	}
}

impl Mouse for PhysicalDevice {
	fn location(&self) -> Result<Point, String> {
		self.enigo
			.location()
			.map(|(x, y)| Point::new(x, y))
			.map_err(|e| format!("Failed to get mouse location: {e:?}"))
	}

	fn move_mouse(&mut self, point: Point) -> Result<(), String> {
		self.enigo
			.move_mouse(point.x, point.y, Coordinate::Abs)
			.map_err(|e| format!("Failed to move mouse: {e:?}"))
	}

	fn move_mouse_by(&mut self, offset: Point) -> Result<(), String> {
		self.enigo
			.move_mouse(offset.x, offset.y, Coordinate::Rel)
			.map_err(|e| format!("Failed to move mouse: {e:?}"))
	}

	fn click(&mut self, button: Button) -> Result<(), String> {
		self.enigo
			.button(button, Direction::Click)
			.map_err(|e| format!("Failed to click button: {e:?}"))
	}

	fn hold(&mut self, button: Button) -> Result<(), String> {
		self.enigo
			.button(button, Direction::Press)
			.map_err(|e| format!("Failed to press button: {e:?}"))
	}

	fn release(&mut self, button: Button) -> Result<(), String> {
		self.enigo
			.button(button, Direction::Release)
			.map_err(|e| format!("Failed to release button: {e:?}"))
	}

	fn scroll(&mut self, length: i32, axis: ScrollAxis) -> Result<(), String> {
		let enigo_axis = match axis {
			ScrollAxis::Horizontal => Axis::Horizontal,
			ScrollAxis::Vertical => Axis::Vertical,
		};
		self.enigo
			.scroll(length, enigo_axis)
			.map_err(|e| format!("Failed to scroll: {e:?}"))
	}
}

impl Keyboard for PhysicalDevice {
	fn key(&mut self, key: Key, action: Direction) -> Result<(), String> {
		self.enigo
			.key(key, action)
			.map_err(|e| format!("Failed to press/release/click key: {e:?}"))
	}

	fn text(&mut self, text: &str) -> Result<(), String> {
		self.enigo
			.text(text)
			.map_err(|e| format!("Failed to type text: {e:?}"))
	}
}

impl InputDevice for PhysicalDevice {}
