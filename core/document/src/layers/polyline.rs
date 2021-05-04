use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct PolyLine {
	points: Vec<kurbo::Point>,
	style: style::PathStyle,
}

impl PolyLine {
	pub fn new(points: Vec<impl Into<kurbo::Point>>, style: style::PathStyle) -> PolyLine {
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
		self.points.iter().for_each(|p| {
			let _ = write!(svg, " {:.3} {:.3}", p.x, p.y);
		});
		let _ = write!(svg, r#"" {}/>"#, self.style.render());
	}
}

#[cfg(test)]
#[test]
fn polyline_should_render() {
	let mut polyline = PolyLine {
		points: vec![kurbo::Point::new(3.0, 4.12354), kurbo::Point::new(1.0, 5.54)],
		style: style::PathStyle::new(Some(style::Stroke::new(crate::color::Color::GREEN, 0.4)), None),
	};

	let mut svg = String::new();
	polyline.render(&mut svg);
	assert_eq!(r#"<polyline points=" 3.000 4.124 1.000 5.540" style="stroke: #00FF00FF;stroke-width:0.4;"/>"#, svg);
}
