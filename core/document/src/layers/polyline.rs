use std::fmt::Write;

use super::{style, LayerData};

#[derive(Debug, Clone)]
pub struct PolyLine {
	points: Vec<glam::DVec2>,
}

impl PolyLine {
	pub fn new(points: Vec<impl Into<glam::DVec2>>) -> PolyLine {
		PolyLine {
			points: points.into_iter().map(|it| it.into()).collect(),
		}
	}
}

impl LayerData for PolyLine {
	fn to_kurbo_path(&mut self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		let mut path = kurbo::BezPath::new();
		self.points
			.iter()
			.map(|v| transform.transform_point2(*v))
			.map(|v| kurbo::Point { x: v.x, y: v.y })
			.enumerate()
			.for_each(|(i, p)| if i == 0 { path.move_to(p) } else { path.line_to(p) });
		path
	}
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		if self.points.is_empty() {
			return;
		}
		let _ = write!(svg, r#"<polyline points=""#);
		let mut points = self.points.iter().map(|v| transform.transform_point2(*v));
		let first = points.next().unwrap();
		let _ = write!(svg, "{:.3} {:.3}", first.x, first.y);
		for point in points {
			let _ = write!(svg, " {:.3} {:.3}", point.x, point.y);
		}
		let _ = write!(svg, r#""{} />"#, style.render());
	}
}

#[cfg(test)]
#[test]
fn polyline_should_render() {
	use super::style::PathStyle;
	use glam::DVec2;
	let mut polyline = PolyLine {
		points: vec![DVec2::new(3.0, 4.12354), DVec2::new(1.0, 5.54)],
	};

	let mut svg = String::new();
	polyline.render(&mut svg, glam::DAffine2::IDENTITY, PathStyle::default());
	assert_eq!(r##"<polyline points="3.000 4.124 1.000 5.540" />"##, svg);
}
