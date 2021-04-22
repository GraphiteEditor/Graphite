use super::style;
use super::LayerData;

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
		let points = self.points.iter().map(|p| format!("{},{}", p.x, p.y)).collect::<Vec<_>>().join(" ");
		format!(r#"<polyline points="{}" {}" />"#, points, self.style.render())
	}
}
