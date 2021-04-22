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
	fn render(&self) -> String {
		if self.points.is_empty() {
			return String::new();
		}
		let points = self.points.iter().fold(String::new(), |mut acc, p| {
			let _ = write!(&mut acc, " {:.3} {:.3}", p.x, p.y);
			acc
		});
		format!(r#"<polyline points="{}" {}/>"#, &points[1..], self.style.render())
	}
}

#[test]
fn polyline_should_render() {
	let polyline = PolyLine {
		points: vec![kurbo::Point::new(3.0, 4.12354), kurbo::Point::new(1.0, 5.54)],
		style: style::PathStyle::new(Some(style::Stroke::new(crate::color::Color::GREEN, 0.4)), None),
	};

	assert_eq!(r#"<polyline points="3.000 4.124 1.000 5.540" style="stroke: #00FF00FF;stroke-width:0.4;"/>"#, polyline.render());
}
