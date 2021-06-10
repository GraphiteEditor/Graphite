use kurbo::Point;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct PolyLine {
	points: Vec<Point>,
	style: style::PathStyle,
}

impl PolyLine {
	pub fn new(points: Vec<impl Into<Point>>, style: style::PathStyle) -> PolyLine {
		PolyLine {
			points: points.into_iter().map(|it| it.into()).collect(),
			style,
		}
	}
}

impl LayerData for PolyLine {
	fn render(&mut self, svg: &mut String) {
		if self.points.is_empty() {
			return;
		}
		let _ = write!(svg, r#"<polyline points=""#);
		let mut points = self.points.iter();
		let first = points.next().unwrap();
		let _ = write!(svg, "{:.3} {:.3}", first.x, first.y);
		for point in points {
			let _ = write!(svg, " {:.3} {:.3}", point.x, point.y);
		}
		let _ = write!(svg, r#""{} />"#, self.style.render());
	}

	fn contains(&self, point: Point) -> bool {
		false
	}

	fn intersects_quad(&self, quad: [Point; 4]) -> bool {
		false
	}
}

#[cfg(test)]
#[test]
fn polyline_should_render() {
	let mut polyline = PolyLine {
		points: vec![Point::new(3.0, 4.12354), Point::new(1.0, 5.54)],
		style: style::PathStyle::new(Some(style::Stroke::new(crate::color::Color::GREEN, 0.4)), None),
	};

	let mut svg = String::new();
	polyline.render(&mut svg);
	assert_eq!(r##"<polyline points="3.000 4.124 1.000 5.540" stroke="#00FF00" stroke-width="0.4" />"##, svg);
}
