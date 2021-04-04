pub mod operation;

pub use kurbo::{Circle, Point};
pub use operation::Operation;

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

	pub fn handle_operation<F: Fn(String)>(&mut self, operation: &Operation, update_frontend: F) {
		match *operation {
			Operation::AddCircle { cx, cy, r } => {
				self.svg.push(SvgElement::Circle(Circle {
					center: Point { x: cx, y: cy },
					radius: r,
				}));

				update_frontend(self.render());
			}
		}
	}
}
