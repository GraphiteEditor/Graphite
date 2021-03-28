pub use kurbo::{Circle, Point};

#[derive(Debug, Clone, PartialEq)]
pub enum SvgElement {
	Circle(Circle),
}

impl SvgElement {
	pub fn render(&self) -> String {
		match self {
			Self::Circle(c) => {
				format!(r#"<circle cx="{}" cy="{}" r="{}" style="fill: #fff;" />"#, c.center.x, c.center.y, c.radius)
			}
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Document {
	pub svg: Vec<SvgElement>,
}

impl Document {
	pub fn render(&self) -> String {
		self.svg.iter().map(|element| element.render()).collect::<Vec<_>>().join("\n")
	}
}
