pub mod operation;

pub use kurbo::{Circle, Point, Rect};
pub use operation::Operation;

#[derive(Debug, Clone, PartialEq)]
pub enum SvgElement {
	Circle(Circle),
	Rect(Rect),
}

impl SvgElement {
	pub fn render(&self) -> String {
		match self {
			Self::Circle(c) => {
				format!(r#"<circle cx="{}" cy="{}" r="{}" style="fill: #fff;" />"#, c.center.x, c.center.y, c.radius)
			}
			Self::Rect(r) => {
				format!(r#"<rect x="{}" y="{}" width="{}" height="{}" style="fill: #fff;" />"#, r.min_x(), r.min_y(), r.width(), r.height())
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
			Operation::AddRect { x0, y0, x1, y1 } => {
				self.svg.push(SvgElement::Rect(Rect::from_points(Point::new(x0, y0), Point::new(x1, y1))));

				update_frontend(self.render());
			}
		}
	}
}
