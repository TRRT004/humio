/// Gaussian delay timing and distribution functions.
pub mod delay;
/// Geometric targeting structures (Points, Rectangles, Circles, Polygons) with skewed distribution sampling.
pub mod target_area;
/// Curved mouse path trajectory generator using the `WindMouse` physics model.
pub mod wind_mouse;

mod device;
mod failures;
mod keyboard;
mod mouse;

pub use delay::sleep_gaussian_delay;
pub use target_area::TargetArea;
pub use wind_mouse::generate_wind_mouse_path;

pub use device::HumanizedDevice;
pub use failures::{
	ClickFailure, FailureChanceCalculator, FailureType, HumanizerConfig, KeyCombinationFailure,
	KeyboardFailure,
};
