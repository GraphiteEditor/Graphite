use kurbo::Vec2;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
	shape: kurbo::BezPath,
	style: style::PathStyle,
}

impl Shape {
	pub fn new(cols: [f64; 6], equal_sides: bool, sides: u8, style: style::PathStyle) -> Shape {
		fn unit_rotation(theta: f64) -> Vec2 {
			Vec2::new(-theta.sin(), theta.cos())
		}
		let extent = Vec2::new((cols[0] + cols[1]) / 2., (cols[2] + cols[3]) / 2.);
		let translation = Vec2::new(cols[4], cols[5]);
		let mut path = kurbo::BezPath::new();
		let apothem_offset_angle = std::f64::consts::PI / (sides as f64);
		if !equal_sides {}

		let relative_points = (0..sides).map(|i| apothem_offset_angle * ((i * 2 + ((sides + 1) % 2)) as f64)).map(|radians| unit_rotation(radians));

		let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
relative_points.clone().for_each(|p| {
					min_x = min_x.min(p.x);
					min_y = min_y.min(p.y);
					max_x = max_x.max(p.x);
					max_y = max_y.max(p.y);
				});

		relative_points.map(|p|if equal_sides{p}else{Vec2::new((p.x - min_x) / (max_x - min_x) * 2. - 1., (p.y - min_y) / (max_y - min_y) * 2. - 1.)})
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
		Shape { shape: path, style }
	}
}

impl LayerData for Shape {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.shape.to_svg(), self.style.render());
	}
}
