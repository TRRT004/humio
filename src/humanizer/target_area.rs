#![allow(
	clippy::cast_possible_truncation,
	clippy::cast_sign_loss,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss
)]

use super::delay::{sample_gaussian, sample_gaussian_clamped};
use crate::Point;

/// Represents a spatial target area on screen where user input is directed.
///
/// Unlike standard automated inputs that target exact pixels, `TargetArea`
/// leverages normal (Gaussian) distributions to sample coordinates,
/// realistically simulating how humans click.
///
/// # Examples
///
/// ```rust
/// use humio::{TargetArea, Point};
///
/// // Define a rectangular button target with high density in the center
/// let target = TargetArea::Rect {
///     top_left: Point::new(100, 100),
///     bottom_right: Point::new(200, 150),
///     target: None,       // defaults to centroid (150, 125)
///     std_dev_x: None,    // defaults to 1/6th of width
///     std_dev_y: None,    // defaults to 1/6th of height
/// };
///
/// let point = target.generate_click_point();
/// assert!(point.x >= 100 && point.x <= 200);
/// assert!(point.y >= 100 && point.y <= 150);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetArea {
	/// A single absolute pixel point on screen (no variance).
	Point(Point),
	/// A rectangular target bounds.
	Rect {
		/// The top-left corner of the rectangle.
		top_left: Point,
		/// The bottom-right corner of the rectangle.
		bottom_right: Point,
		/// Target point within the Rect where we skew the distribution (defaults to centroid).
		target: Option<Point>,
		/// Custom standard deviation along the X-axis (defaults to 1/6 of the width).
		std_dev_x: Option<i32>,
		/// Custom standard deviation along the Y-axis (defaults to 1/6 of the height).
		std_dev_y: Option<i32>,
	},
	/// A circular target bounds.
	Circle {
		/// The center coordinate of the circle.
		center: Point,
		/// The radius of the circle in pixels.
		radius: i32,
		/// Target point within the Circle where we skew the distribution (defaults to center).
		target: Option<Point>,
		/// Custom standard deviation for the distribution distance (defaults to 1/3 of the radius).
		std_dev: Option<i32>,
	},
	/// A polygonal target bounds.
	Polygon {
		/// List of vertices defining the polygon in order.
		vertices: Vec<Point>,
		/// Target point within the Polygon where we skew the distribution (defaults to centroid).
		target: Option<Point>,
		/// Custom standard deviation along the X-axis (defaults to 1/6 of the bounding box width).
		std_dev_x: Option<i32>,
		/// Custom standard deviation along the Y-axis (defaults to 1/6 of the bounding box height).
		std_dev_y: Option<i32>,
	},
}

impl TargetArea {
	/// Generates a coordinate within the target area, utilizing a skewed normal distribution
	/// centered on either the specified target/anchor point or the area's centroid.
	#[must_use]
	pub fn generate_click_point(&self) -> Point {
		match self {
			Self::Point(p) => *p,
			Self::Rect {
				top_left,
				bottom_right,
				target,
				std_dev_x,
				std_dev_y,
			} => {
				let x1 = top_left.x;
				let y1 = top_left.y;
				let x2 = bottom_right.x;
				let y2 = bottom_right.y;

				let default_tx = x1.midpoint(x2);
				let default_ty = y1.midpoint(y2);
				let (tx, ty) = target.map_or((default_tx, default_ty), |p| (p.x, p.y));

				// Clamp skew target inside the rectangle bounds
				let tx = tx.clamp(x1, x2);
				let ty = ty.clamp(y1, y2);

				// Standard deviation defaults to 1/6 of the dimension size to cover ~99% within the box
				let s_x = std_dev_x.unwrap_or_else(|| ((x2 - x1).abs() / 6).max(1));
				let s_y = std_dev_y.unwrap_or_else(|| ((y2 - y1).abs() / 6).max(1));

				let rx = sample_gaussian_clamped(tx, s_x, x1, x2);
				let ry = sample_gaussian_clamped(ty, s_y, y1, y2);
				Point::new(rx, ry)
			}
			Self::Circle {
				center,
				radius,
				target,
				std_dev,
			} => {
				let cx = center.x;
				let cy = center.y;
				let (tx, ty) = target.map_or((cx, cy), |p| (p.x, p.y));

				// Clamp skew target inside the circle bounds
				let dx = tx - cx;
				let dy = ty - cy;
				let (tx, ty) = if dx * dx + dy * dy <= radius * radius {
					(tx, ty)
				} else {
					let angle = f64::from(dy).atan2(f64::from(dx));
					let clamped_x = cx + (f64::from(*radius) * angle.cos()).round() as i32;
					let clamped_y = cy + (f64::from(*radius) * angle.sin()).round() as i32;
					(clamped_x, clamped_y)
				};

				let s = std_dev.unwrap_or_else(|| (radius.abs() / 3).max(1));

				// Rejection sampling (limit to 100 attempts, fallback to clamped target point)
				let mut attempts = 0;
				while attempts < 100 {
					let rx = sample_gaussian(f64::from(tx), f64::from(s)).round() as i32;
					let ry = sample_gaussian(f64::from(ty), f64::from(s)).round() as i32;
					let r_dx = rx - cx;
					let r_dy = ry - cy;
					if r_dx * r_dx + r_dy * r_dy <= radius * radius {
						return Point::new(rx, ry);
					}
					attempts += 1;
				}
				Point::new(tx, ty)
			}
			Self::Polygon {
				vertices,
				target,
				std_dev_x,
				std_dev_y,
			} => {
				if vertices.is_empty() {
					return Point::new(0, 0);
				}
				let (default_tx, default_ty) = polygon_centroid(vertices);
				let (tx, ty) = target.map_or((default_tx, default_ty), |p| (p.x, p.y));

				// Calculate bounding box for sampling and clamping
				let mut min_x = i32::MAX;
				let mut max_x = i32::MIN;
				let mut min_y = i32::MAX;
				let mut max_y = i32::MIN;
				for &Point { x: vx, y: vy } in vertices {
					if vx < min_x {
						min_x = vx;
					}
					if vx > max_x {
						max_x = vx;
					}
					if vy < min_y {
						min_y = vy;
					}
					if vy > max_y {
						max_y = vy;
					}
				}

				let s_x = std_dev_x.unwrap_or_else(|| ((max_x - min_x).abs() / 6).max(1));
				let s_y = std_dev_y.unwrap_or_else(|| ((max_y - min_y).abs() / 6).max(1));

				// Rejection sampling
				let mut attempts = 0;
				while attempts < 100 {
					let rx = sample_gaussian_clamped(tx, s_x, min_x, max_x);
					let ry = sample_gaussian_clamped(ty, s_y, min_y, max_y);
					if is_point_in_polygon(rx, ry, vertices) {
						return Point::new(rx, ry);
					}
					attempts += 1;
				}

				// Fallback strategy: check if target is valid, then default to centroid, then vertex 0
				if is_point_in_polygon(tx, ty, vertices) {
					Point::new(tx, ty)
				} else if is_point_in_polygon(default_tx, default_ty, vertices) {
					Point::new(default_tx, default_ty)
				} else {
					vertices[0]
				}
			}
		}
	}
}

/// Compute centroid (simple average of vertices) of a polygon.
fn polygon_centroid(vertices: &[Point]) -> (i32, i32) {
	if vertices.is_empty() {
		return (0, 0);
	}
	let mut sum_x = 0;
	let mut sum_y = 0;
	for p in vertices {
		sum_x += p.x;
		sum_y += p.y;
	}
	let len = vertices.len() as i32;
	(sum_x / len, sum_y / len)
}

/// Check if a point is inside a polygon using ray casting (PNPOLY).
fn is_point_in_polygon(x: i32, y: i32, vertices: &[Point]) -> bool {
	let mut inside = false;
	let n = vertices.len();
	if n < 3 {
		return false;
	}
	let mut j = n - 1;
	for i in 0..n {
		let vi = vertices[i];
		let vj = vertices[j];
		if ((vi.y > y) != (vj.y > y)) && (x < (vj.x - vi.x) * (y - vi.y) / (vj.y - vi.y) + vi.x) {
			inside = !inside;
		}
		j = i;
	}
	inside
}
