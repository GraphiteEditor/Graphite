pub mod operation;

use std::collections::{hash_map::Keys, HashMap};

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
			Self::Folder(f) => f.elements.values().map(|e| e.render()).fold(String::with_capacity(f.elements.len() * 30), |s, e| s + &e),
			Self::Circle(c) => {
				format!(r#"<circle cx="{}" cy="{}" r="{}" style="fill: #fff;" />"#, c.center.x, c.center.y, c.radius)
			}
			Self::Rect(r) => {
				format!(r#"<rect x="{}" y="{}" width="{}" height="{}" style="fill: #fff;" />"#, r.min_x(), r.min_y(), r.width(), r.height())
			}
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DocumentError {
	ElementNotFound,
	NotAShape,
	ElementAlreadyExists,
	InvalidPath,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
	elements: HashMap<String, SvgElement>,
}

impl Folder {
	fn add_element(&mut self, name: String, svg: SvgElement) -> Result<(), DocumentError> {
		#[allow(clippy::map_entry)]
		if self.elements.contains_key(&name) {
			Err(DocumentError::ElementAlreadyExists)
		} else {
			self.elements.insert(name, svg);
			Ok(())
		}
	}

	fn remove_element(&mut self, name: &str) -> Result<(), DocumentError> {
		self.elements.remove(name).map_or(Err(DocumentError::ElementNotFound), |_| Ok(()))
	}

	/// Returns a list of elements in the folder
	pub fn list(&self) -> Keys<String, SvgElement> {
		self.elements.keys()
	}

	fn element(&self, name: &str) -> Option<&SvgElement> {
		self.elements.get(name)
	}

	fn mut_element(&mut self, name: &str) -> Option<&mut SvgElement> {
		self.elements.get_mut(name)
	}
}

impl Default for Folder {
	fn default() -> Self {
		Self { elements: HashMap::new() }
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
	pub svg: SvgElement,
}

impl Default for Document {
	fn default() -> Self {
		Self {
			svg: SvgElement::Folder(Folder::default()),
		}
	}
}

impl Document {
	pub fn render(&self) -> String {
		self.svg.render()
	}

	pub fn open(&self, path: &str) -> Result<&SvgElement, DocumentError> {
		assert!(matches!(self.svg, SvgElement::Folder(_)), "SVG root has to be of type Folder");
		let mut root = &self.svg;
		for s in path.split('/') {
			if s.is_empty() {
				continue;
			}
			if let SvgElement::Folder(f) = root {
				match f.element(s).ok_or(DocumentError::ElementNotFound)? {
					f if matches!(f, &SvgElement::Folder(_)) => root = f,
					e => return Ok(e),
				}
			}
		}
		Ok(root)
	}

	pub fn open_mut(&mut self, path: &str) -> Result<&mut SvgElement, DocumentError> {
		assert!(matches!(self.svg, SvgElement::Folder(_)), "SVG root has to be of type Folder");
		let mut root = &mut self.svg;
		for s in path.split('/') {
			if s.is_empty() {
				continue;
			}
			if let SvgElement::Folder(f) = root {
				match f.mut_element(s).ok_or(DocumentError::ElementNotFound)? {
					f if matches!(f, &mut SvgElement::Folder(_)) => root = f,
					e => return Ok(e),
				}
			}
		}
		Ok(root)
	}

	fn resolve_path<'a>(&mut self, path: &'a str) -> Result<(&mut Folder, &'a str), DocumentError> {
		let name = path.split('/').last().ok_or(DocumentError::InvalidPath)?;
		let len = path.len() - name.len();
		if let SvgElement::Folder(folder) = self.open_mut(&path[..len])? {
			Ok((folder, name))
		} else {
			Err(DocumentError::InvalidPath)
		}
	}

	pub fn write(&mut self, path: &str, element: SvgElement) -> Result<(), DocumentError> {
		let (folder, name) = self.resolve_path(path)?;
		if let Some(e) = folder.mut_element(&name) {
			// TODO: We should decide on whether we should just silently overwrite old elements
			*e = element;
		} else {
			folder.add_element(name.to_string(), element)?;
		}
		Ok(())
	}

	pub fn delete(&mut self, path: &str) -> Result<(), DocumentError> {
		let (folder, name) = self.resolve_path(path)?;
		log::debug!("removing {} from folder: {:?}", name, folder);
		folder.remove_element(name)?;
		Ok(())
	}

	pub fn handle_operation<F: Fn(String)>(&mut self, operation: Operation, update_frontend: F) -> Result<(), DocumentError> {
		match operation {
			Operation::AddCircle { path, cx, cy, r } => {
				self.write(&path, SvgElement::Circle(Circle::new(Point::new(cx, cy), r)))?;

				update_frontend(self.render());
			}
			Operation::AddRect { path, x0, y0, x1, y1 } => {
				self.write(&path, SvgElement::Rect(Rect::from_points(Point::new(x0, y0), Point::new(x1, y1))))?;

				update_frontend(self.render());
			}
			Operation::DeleteElement { path } => {
				self.delete(&path)?;
				update_frontend(self.render());
			}
			Operation::AddFolder { path } => self.write(&path, SvgElement::Folder(Folder::default()))?,
		}
		Ok(())
	}
}
