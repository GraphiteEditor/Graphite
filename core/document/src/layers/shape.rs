use glam::DVec2;
use kurbo::BezPath;

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
	fn to_kurbo_path(&mut self, transform: glam::DAffine2, _style: style::PathStyle) -> BezPath {
		fn unit_rotation(theta: f64) -> DVec2 {
			DVec2::new(-theta.sin(), theta.cos())
		}
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
					DVec2::new((p.x - min_x) / (max_x - min_x) * 2. - 1., (p.y - min_y) / (max_y - min_y) * 2. - 1.)
				}
			})
			.map(|p| DVec2::new(p.x / 2. + 0.5, p.y / 2. + 0.5))
			.map(|unit| transform.transform_point2(unit.into()))
			.map(|pos| kurbo::Point::new(pos.x, pos.y))
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
}
