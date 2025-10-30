use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::transform::ApplyTransform;
use crate::uuid::NodeId;
use crate::{AlphaBlending, math::quad::Quad};
use dyn_any::{StaticType, StaticTypeSized};
use glam::DAffine2;
use std::hash::Hash;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Table<T> {
	#[serde(alias = "instances", alias = "instance")]
	element: Vec<T>,
	transform: Vec<DAffine2>,
	alpha_blending: Vec<AlphaBlending>,
	source_node_id: Vec<Option<NodeId>>,
}

impl<T> Table<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			element: Vec::with_capacity(capacity),
			transform: Vec::with_capacity(capacity),
			alpha_blending: Vec::with_capacity(capacity),
			source_node_id: Vec::with_capacity(capacity),
		}
	}

	pub fn new_from_element(element: T) -> Self {
		Self {
			element: vec![element],
			transform: vec![DAffine2::IDENTITY],
			alpha_blending: vec![AlphaBlending::default()],
			source_node_id: vec![None],
		}
	}

	pub fn new_from_row(row: TableRow<T>) -> Self {
		Self {
			element: vec![row.element],
			transform: vec![row.transform],
			alpha_blending: vec![row.alpha_blending],
			source_node_id: vec![row.source_node_id],
		}
	}

	pub fn push(&mut self, row: TableRow<T>) {
		self.element.push(row.element);
		self.transform.push(row.transform);
		self.alpha_blending.push(row.alpha_blending);
		self.source_node_id.push(row.source_node_id);
	}

	pub fn extend(&mut self, table: Table<T>) {
		self.element.extend(table.element);
		self.transform.extend(table.transform);
		self.alpha_blending.extend(table.alpha_blending);
		self.source_node_id.extend(table.source_node_id);
	}

	pub fn get(&self, index: usize) -> Option<TableRowRef<'_, T>> {
		if index >= self.element.len() {
			return None;
		}

		Some(TableRowRef {
			element: &self.element[index],
			transform: &self.transform[index],
			alpha_blending: &self.alpha_blending[index],
			source_node_id: &self.source_node_id[index],
		})
	}

	pub fn get_mut(&mut self, index: usize) -> Option<TableRowMut<'_, T>> {
		if index >= self.element.len() {
			return None;
		}

		Some(TableRowMut {
			element: &mut self.element[index],
			transform: &mut self.transform[index],
			alpha_blending: &mut self.alpha_blending[index],
			source_node_id: &mut self.source_node_id[index],
		})
	}

	pub fn len(&self) -> usize {
		self.element.len()
	}

	pub fn is_empty(&self) -> bool {
		self.element.is_empty()
	}

	/// Borrows a [`Table`] and returns an iterator of [`TableRowRef`]s, each containing references to the data of the respective row from the table.
	pub fn iter(&self) -> impl DoubleEndedIterator<Item = TableRowRef<'_, T>> + Clone {
		self.element
			.iter()
			.zip(self.transform.iter())
			.zip(self.alpha_blending.iter())
			.zip(self.source_node_id.iter())
			.map(|(((element, transform), alpha_blending), source_node_id)| TableRowRef {
				element,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	/// Mutably borrows a [`Table`] and returns an iterator of [`TableRowMut`]s, each containing mutable references to the data of the respective row from the table.
	pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = TableRowMut<'_, T>> {
		self.element
			.iter_mut()
			.zip(self.transform.iter_mut())
			.zip(self.alpha_blending.iter_mut())
			.zip(self.source_node_id.iter_mut())
			.map(|(((element, transform), alpha_blending), source_node_id)| TableRowMut {
				element,
				transform,
				alpha_blending,
				source_node_id,
			})
	}
}

impl<T: BoundingBox> BoundingBox for Table<T> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let mut combined_bounds = None;

		for row in self.iter() {
			match row.element.bounding_box(transform * *row.transform, include_stroke) {
				RenderBoundingBox::None => continue,
				RenderBoundingBox::Infinite => return RenderBoundingBox::Infinite,
				RenderBoundingBox::Rectangle(bounds) => match combined_bounds {
					Some(existing) => combined_bounds = Some(Quad::combine_bounds(existing, bounds)),
					None => combined_bounds = Some(bounds),
				},
			}
		}

		match combined_bounds {
			Some(bounds) => RenderBoundingBox::Rectangle(bounds),
			None => RenderBoundingBox::None,
		}
	}
}

impl<T> IntoIterator for Table<T> {
	type Item = TableRow<T>;
	type IntoIter = TableRowIter<T>;

	/// Consumes a [`Table`] and returns an iterator of [`TableRow`]s, each containing the owned data of the respective row from the original table.
	fn into_iter(self) -> Self::IntoIter {
		TableRowIter {
			element: self.element.into_iter(),
			transform: self.transform.into_iter(),
			alpha_blending: self.alpha_blending.into_iter(),
			source_node_id: self.source_node_id.into_iter(),
		}
	}
}

pub struct TableRowIter<T> {
	element: std::vec::IntoIter<T>,
	transform: std::vec::IntoIter<DAffine2>,
	alpha_blending: std::vec::IntoIter<AlphaBlending>,
	source_node_id: std::vec::IntoIter<Option<NodeId>>,
}
impl<T> Iterator for TableRowIter<T> {
	type Item = TableRow<T>;

	fn next(&mut self) -> Option<Self::Item> {
		let element = self.element.next()?;
		let transform = self.transform.next()?;
		let alpha_blending = self.alpha_blending.next()?;
		let source_node_id = self.source_node_id.next()?;

		Some(TableRow {
			element,
			transform,
			alpha_blending,
			source_node_id,
		})
	}
}

impl<T> Default for Table<T> {
	fn default() -> Self {
		Self {
			element: Vec::new(),
			transform: Vec::new(),
			alpha_blending: Vec::new(),
			source_node_id: Vec::new(),
		}
	}
}

impl<T: Hash> Hash for Table<T> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		for element in &self.element {
			element.hash(state);
		}
		for transform in &self.transform {
			transform.to_cols_array().map(|x| x.to_bits()).hash(state);
		}
		for alpha_blending in &self.alpha_blending {
			alpha_blending.hash(state);
		}
	}
}

impl<T: PartialEq> PartialEq for Table<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element == other.element && self.transform == other.transform && self.alpha_blending == other.alpha_blending
	}
}

impl<T> ApplyTransform for Table<T> {
	fn apply_transform(&mut self, modification: &DAffine2) {
		for transform in &mut self.transform {
			*transform *= *modification;
		}
	}

	fn left_apply_transform(&mut self, modification: &DAffine2) {
		for transform in &mut self.transform {
			*transform = *modification * *transform;
		}
	}
}

unsafe impl<T: StaticTypeSized> StaticType for Table<T> {
	type Static = Table<T::Static>;
}

impl<T> FromIterator<TableRow<T>> for Table<T> {
	fn from_iter<I: IntoIterator<Item = TableRow<T>>>(iter: I) -> Self {
		let iter = iter.into_iter();
		let (lower, _) = iter.size_hint();
		let mut table = Self::with_capacity(lower);
		for row in iter {
			table.push(row);
		}
		table
	}
}

#[derive(Copy, Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TableRow<T> {
	#[serde(alias = "instance")]
	pub element: T,
	pub transform: DAffine2,
	pub alpha_blending: AlphaBlending,
	pub source_node_id: Option<NodeId>,
}

impl<T> TableRow<T> {
	pub fn new_from_element(element: T) -> Self {
		Self {
			element,
			transform: DAffine2::IDENTITY,
			alpha_blending: AlphaBlending::default(),
			source_node_id: None,
		}
	}

	pub fn as_ref(&self) -> TableRowRef<'_, T> {
		TableRowRef {
			element: &self.element,
			transform: &self.transform,
			alpha_blending: &self.alpha_blending,
			source_node_id: &self.source_node_id,
		}
	}

	pub fn as_mut(&mut self) -> TableRowMut<'_, T> {
		TableRowMut {
			element: &mut self.element,
			transform: &mut self.transform,
			alpha_blending: &mut self.alpha_blending,
			source_node_id: &mut self.source_node_id,
		}
	}
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TableRowRef<'a, T> {
	pub element: &'a T,
	pub transform: &'a DAffine2,
	pub alpha_blending: &'a AlphaBlending,
	pub source_node_id: &'a Option<NodeId>,
}

impl<T> TableRowRef<'_, T> {
	pub fn into_cloned(self) -> TableRow<T>
	where
		T: Clone,
	{
		TableRow {
			element: self.element.clone(),
			transform: *self.transform,
			alpha_blending: *self.alpha_blending,
			source_node_id: *self.source_node_id,
		}
	}
}

#[derive(Debug)]
pub struct TableRowMut<'a, T> {
	pub element: &'a mut T,
	pub transform: &'a mut DAffine2,
	pub alpha_blending: &'a mut AlphaBlending,
	pub source_node_id: &'a mut Option<NodeId>,
}
