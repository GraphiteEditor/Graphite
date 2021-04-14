pub mod operation;

pub use kurbo::{Circle, Line, Point, Rect};
pub use operation::Operation;

#[derive(Debug, Clone, PartialEq)]
pub enum LayerType {
	Folder(Folder),
	Circle(Circle),
	Rect(Rect),
	Line(Line),
}

impl LayerType {
	pub fn render(&self) -> String {
		match self {
			Self::Folder(f) => f.render(),
			Self::Circle(c) => {
				format!(r#"<circle cx="{}" cy="{}" r="{}" style="fill: #fff;" />"#, c.center.x, c.center.y, c.radius)
			}
			Self::Rect(r) => {
				format!(r#"<rect x="{}" y="{}" width="{}" height="{}" style="fill: #fff;" />"#, r.min_x(), r.min_y(), r.width(), r.height())
			}
			Self::Line(l) => {
				format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" style="stroke: #fff;" />"#, l.p0.x, l.p0.y, l.p1.x, l.p1.y)
			}
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DocumentError {
	LayerNotFound,
	InvalidPath,
	IndexOutOfBounds,
}

type LayerId = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
	visible: bool,
	name: Option<String>,
	data: LayerType,
}

impl Layer {
	pub fn new(data: LayerType) -> Self {
		Self { visible: true, name: None, data }
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
	next_assignment_id: LayerId,
	layer_ids: Vec<LayerId>,
	layers: Vec<Layer>,
}

impl Folder {
	pub fn render(&self) -> String {
		self.layers
			.iter()
			.filter(|layer| layer.visible)
			.map(|layer| layer.data.render())
			.fold(String::with_capacity(self.layers.len() * 30), |s, n| s + "\n" + &n)
	}

	fn add_layer(&mut self, layer: Layer, insert_index: isize) -> Option<LayerId> {
		let mut insert_index = insert_index as i128;
		if insert_index < 0 {
			insert_index = self.layers.len() as i128 + insert_index as i128 + 1;
		}

		if insert_index <= self.layers.len() as i128 && insert_index >= 0 {
			self.layers.insert(insert_index as usize, layer);
			self.layer_ids.insert(insert_index as usize, self.next_assignment_id);
			self.next_assignment_id += 1;
			Some(self.next_assignment_id - 1)
		} else {
			None
		}
	}

	fn remove_layer(&mut self, id: LayerId) -> Result<(), DocumentError> {
		let pos = self.layer_ids.iter().position(|x| *x == id).ok_or(DocumentError::LayerNotFound)?;
		self.layers.remove(pos);
		self.layer_ids.remove(pos);
		Ok(())
	}

	/// Returns a list of layers in the folder
	pub fn list_layers(&self) -> &[LayerId] {
		self.layer_ids.as_slice()
	}

	fn layer(&self, id: LayerId) -> Option<&Layer> {
		let pos = self.layer_ids.iter().position(|x| *x == id)?;
		Some(&self.layers[pos])
	}

	fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
		let pos = self.layer_ids.iter().position(|x| *x == id)?;
		Some(&mut self.layers[pos])
	}

	fn folder(&self, id: LayerId) -> Option<&Folder> {
		match self.layer(id) {
			Some(Layer { data: LayerType::Folder(folder), .. }) => Some(&folder),
			_ => None,
		}
	}

	fn folder_mut(&mut self, id: LayerId) -> Option<&mut Folder> {
		match self.layer_mut(id) {
			Some(Layer { data: LayerType::Folder(folder), .. }) => Some(folder),
			_ => None,
		}
	}
}

impl Default for Folder {
	fn default() -> Self {
		Self {
			layer_ids: vec![],
			layers: vec![],
			next_assignment_id: 0,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
	pub root: Folder,
}

impl Default for Document {
	fn default() -> Self {
		Self { root: Folder::default() }
	}
}

fn split_path(path: &[LayerId]) -> Result<(&[LayerId], LayerId), DocumentError> {
	let id = path.last().ok_or(DocumentError::InvalidPath)?;
	let folder_path = &path[0..path.len() - 1];
	Ok((folder_path, *id))
}

impl Document {
	pub fn render(&self) -> String {
		self.root.render()
	}

	pub fn folder(&self, path: &[LayerId]) -> Result<&Folder, DocumentError> {
		let mut root = &self.root;
		for id in path {
			root = root.folder(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	pub fn folder_mut(&mut self, path: &[LayerId]) -> Result<&mut Folder, DocumentError> {
		let mut root = &mut self.root;
		for id in path {
			root = root.folder_mut(*id).ok_or(DocumentError::LayerNotFound)?;
		}
		Ok(root)
	}

	pub fn layer(&self, path: &[LayerId]) -> Result<&Layer, DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder(path)?.layer(id).ok_or(DocumentError::LayerNotFound)
	}

	pub fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut Layer, DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or(DocumentError::LayerNotFound)
	}

	pub fn set_layer(&mut self, path: &[LayerId], layer: Layer) -> Result<(), DocumentError> {
		let mut folder = &mut self.root;
		if let Ok((path, id)) = split_path(path) {
			folder = self.folder_mut(path)?;
			if let Some(folder_layer) = folder.layer_mut(id) {
				*folder_layer = layer;
				return Ok(());
			}
		}
		folder.add_layer(layer, -1).ok_or(DocumentError::IndexOutOfBounds)?;
		Ok(())
	}

	/// Passing a negative `insert_index` indexes relative to the end
	/// -1 is equivalent to adding the layer to the top
	pub fn add_layer(&mut self, path: &[LayerId], layer: Layer, insert_index: isize) -> Result<LayerId, DocumentError> {
		let folder = self.folder_mut(path)?;
		folder.add_layer(layer, insert_index).ok_or(DocumentError::IndexOutOfBounds)
	}

	pub fn delete(&mut self, path: &[LayerId]) -> Result<(), DocumentError> {
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.remove_layer(id)?;
		Ok(())
	}

	pub fn handle_operation<F: Fn(String)>(&mut self, operation: Operation, update_frontend: F) -> Result<(), DocumentError> {
		match operation {
			Operation::AddCircle { path, insert_index, cx, cy, r } => {
				self.add_layer(&path, Layer::new(LayerType::Circle(Circle::new(Point::new(cx, cy), r))), insert_index)?;

				update_frontend(self.render());
			}
			Operation::AddRect { path, insert_index, x0, y0, x1, y1 } => {
				self.add_layer(&path, Layer::new(LayerType::Rect(Rect::from_points(Point::new(x0, y0), Point::new(x1, y1)))), insert_index)?;

				update_frontend(self.render());
			}
			Operation::AddLine { path, insert_index, x0, y0, x1, y1 } => {
				self.add_layer(&path, Layer::new(LayerType::Line(Line::new(Point::new(x0, y0), Point::new(x1, y1)))), insert_index)?;

				update_frontend(self.render());
			}
			Operation::DeleteLayer { path } => {
				self.delete(&path)?;

				update_frontend(self.render());
			}
			Operation::AddFolder { path } => self.set_layer(&path, Layer::new(LayerType::Folder(Folder::default())))?,
		}
		Ok(())
	}
}
