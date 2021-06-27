use glam::DAffine2;
use glam::DVec2;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;
use kurbo::BezPath;
use kurbo::Vec2;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
	equal_sides: bool,
	sides: u8,
}

impl Shape {
	pub fn new(equal_sides: bool, sides: u8) -> Shape {
		Shape { equal_sides, sides }
	}
}

impl LayerData for Shape {
	fn to_kurbo_path(&self, transform: glam::DAffine2, _style: style::PathStyle) -> BezPath {
		fn unit_rotation(theta: f64) -> Vec2 {
			Vec2::new(-theta.sin(), theta.cos())
		}
		let extent = Vec2::new((transform.x_axis.x + transform.x_axis.y) / 2., (transform.y_axis.x + transform.y_axis.y) / 2.);
		let translation = transform.translation;
		let mut path = kurbo::BezPath::new();
		let apothem_offset_angle = std::f64::consts::PI / (self.sides as f64);

		let relative_points = (0..self.sides)
			.map(|i| apothem_offset_angle * ((i * 2 + ((self.sides + 1) % 2)) as f64))
			.map(|radians| unit_rotation(radians));

		let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
		relative_points.clone().for_each(|p| {
			min_x = min_x.min(p.x);
			min_y = min_y.min(p.y);
			max_x = max_x.max(p.x);
			max_y = max_y.max(p.y);
		});

		relative_points
			.map(|p| {
				if self.equal_sides {
					p
				} else {
					Vec2::new((p.x - min_x) / (max_x - min_x) * 2. - 1., (p.y - min_y) / (max_y - min_y) * 2. - 1.)
				}
			})
			.map(|unit| Vec2::new(-unit.x * extent.x + translation.x + extent.x, -unit.y * extent.y + translation.y + extent.y))
			.map(|pos| (pos).to_point())
			.enumerate()
			.for_each(|(i, p)| {
				if i == 0 {
					path.move_to(p);
				} else {
					path.line_to(p);
				}
			});

		path.close_path();
		path
	}
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.to_kurbo_path(transform, style).to_svg(), style.render());
	}

	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		if intersect_quad_bez_path(quad, &self.to_kurbo_path(DAffine2::IDENTITY, style)) {
			intersections.push(path.clone());
		}
	}

	fn intersects_point(&self, point: DVec2, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		if kurbo::Shape::contains(&self.to_kurbo_path(DAffine2::IDENTITY, style), kurbo::Point::new(point.x, point.y)) {
			intersections.push(path.clone());
		}
	}
}
