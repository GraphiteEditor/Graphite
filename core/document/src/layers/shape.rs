use kurbo::Point;
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
	pub fn new(center: Point, far: Point, sides: u8, style: style::PathStyle) -> Shape {
		fn rotate(v: &Vec2, theta: f64) -> Vec2 {
			let cosine = theta.cos();
			let sine = theta.sin();
			Vec2::new(v.x * cosine - v.y * sine, v.x * sine + v.y * cosine)
		}
		let extent = far - center;
		let mut path = kurbo::BezPath::new();
		let apothem_offset_angle = std::f64::consts::PI / (sides as f64);
		for i in 0..sides {
			let radians = apothem_offset_angle * ((i * 2 + (sides % 2)) as f64);
			let offset = rotate(&extent, radians);
			let point: (f64, f64) = (center + offset).into();
			if i == 0 {
				path.move_to(point);
			} else {
				path.line_to(point);
			}
		}
		path.close_path();
		Shape { shape: path, style }
	}
}

impl LayerData for Shape {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.shape.to_svg(), self.style.render());
	}
}
