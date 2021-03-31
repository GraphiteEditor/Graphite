pub mod operation;

pub use kurbo::{Circle, Point, Rect};
pub use operation::Operation;

#[derive(Debug, Clone, PartialEq)]
pub enum SvgElement {
    Folder(Folder),
	Circle(Circle),
	Rect(Rect),
}

impl SvgElement {
	pub fn render(&self) -> String {
		match self {
			Self::Folder(f) => {
                f.elements.iter().map(|e| e.render()).fold(String::with_capacity(f.elements.len() * 30), |s, e| s + &e)
			}
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
pub struct Folder {
    elements: Vec<SvgElement>,
    names: Vec<String>,
}

impl Folder {
    pub fn add_element(&mut self, svg: SvgElement, name: String) -> usize {
        self.elements.push(svg);
        self.names.push(name);
        self.elements.len() - 1
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
	pub svg: SvgElement,
}

impl Default for Document {
    fn default() -> Self {
        Self{svg: SvgElement::Folder(Folder::default())}
    }
}

impl Document {
	pub fn render(&self) -> String {
        self.svg.render()
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
