use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::transform::ApplyTransform;
use crate::uuid::NodeId;
use crate::{AlphaBlending, math::quad::Quad};
use dyn_any::{StaticType, StaticTypeSized};
use glam::DAffine2;
use std::collections::HashMap;
use std::hash::Hash;

// TODO: Temporal solution for storing an additional custom column in a table
#[derive(Clone, Debug, PartialEq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum CustomColumnValue {
	#[default]
	None,
	U32(u32),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Table<T> {
	#[serde(alias = "instances", alias = "instance")]
	element: Vec<T>,
	transform: Vec<DAffine2>,
	alpha_blending: Vec<AlphaBlending>,
	source_node_id: Vec<Option<NodeId>>,
	#[serde(default)]
	additional: HashMap<String, Vec<CustomColumnValue>>,
}

impl<T> Table<T> {
	pub fn new() -> Self {
		Self {
			element: Vec::new(),
			transform: Vec::new(),
			alpha_blending: Vec::new(),
			source_node_id: Vec::new(),
			additional: HashMap::new(),
		}
	}

	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			element: Vec::with_capacity(capacity),
			transform: Vec::with_capacity(capacity),
			alpha_blending: Vec::with_capacity(capacity),
			source_node_id: Vec::with_capacity(capacity),
			additional: HashMap::new(),
		}
	}

	pub fn new_from_element(element: T) -> Self {
		Self {
			element: vec![element],
			transform: vec![DAffine2::IDENTITY],
			alpha_blending: vec![AlphaBlending::default()],
			source_node_id: vec![None],
			additional: HashMap::new(),
		}
	}

	pub fn new_from_row(row: TableRow<T>) -> Self {
		Self {
			element: vec![row.element],
			transform: vec![row.transform],
			alpha_blending: vec![row.alpha_blending],
			source_node_id: vec![row.source_node_id],
			additional: row.additional.into_iter().map(|(key, value)| (key, vec![value])).collect(),
		}
	}

	pub fn push(&mut self, row: TableRow<T>) {
		self.element.push(row.element);
		self.transform.push(row.transform);
		self.alpha_blending.push(row.alpha_blending);
		self.source_node_id.push(row.source_node_id);

		let target_len = self.element.len();

		// Ensure all additional columns have the same length by padding with None
		for (key, value) in row.additional {
			let col = self.additional.entry(key).or_default();
			col.resize(target_len - 1, CustomColumnValue::None);
			col.push(value);
		}

		for values in self.additional.values_mut() {
			values.resize(target_len, CustomColumnValue::None);
		}
	}

	pub fn extend(&mut self, table: Table<T>) {
		let original_len = self.element.len();

		self.element.extend(table.element);
		self.transform.extend(table.transform);
		self.alpha_blending.extend(table.alpha_blending);
		self.source_node_id.extend(table.source_node_id);

		// Ensure all additional columns remain the same length after extending by padding with None
		let target_len = self.element.len();

		for (key, values) in table.additional {
			let col = self.additional.entry(key).or_default();
			col.resize(original_len, CustomColumnValue::None);
			col.extend(values);
		}

		for values in self.additional.values_mut() {
			values.resize(target_len, CustomColumnValue::None);
		}
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
			additional: self.additional.iter().map(|(key, values)| (key.as_str(), &values[index])).collect(),
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
			additional: self.additional.iter_mut().map(|(key, value)| (key.as_str(), &mut value[index])).collect(),
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
		(0..self.element.len()).map(move |i| TableRowRef {
			element: &self.element[i],
			transform: &self.transform[i],
			alpha_blending: &self.alpha_blending[i],
			source_node_id: &self.source_node_id[i],
			additional: self.additional.iter().map(|(key, values)| (key.as_str(), &values[i])).collect(),
		})
	}

	/// Mutably borrows a [`Table`] and returns an iterator of [`TableRowMut`]s, each containing mutable references to the data of the respective row from the table.
	pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = TableRowMut<'_, T>> {
		let len = self.element.len();

		let mut additional_rows: Vec<HashMap<&str, &mut CustomColumnValue>> = (0..len).map(|_| HashMap::new()).collect();
		for (key, values) in self.additional.iter_mut() {
			for (i, value) in values.iter_mut().enumerate() {
				additional_rows[i].insert(key.as_str(), value);
			}
		}

		self.element
			.iter_mut()
			.zip(self.transform.iter_mut())
			.zip(self.alpha_blending.iter_mut())
			.zip(self.source_node_id.iter_mut())
			.zip(additional_rows)
			.map(|((((element, transform), alpha_blending), source_node_id), additional)| TableRowMut {
				element,
				transform,
				alpha_blending,
				source_node_id,
				additional,
			})
	}

	pub fn additional_column_keys(&self) -> Vec<&str> {
		let mut keys: Vec<_> = self.additional.keys().map(|key| key.as_str()).collect();
		keys.sort();
		keys
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

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let mut combined_bounds = None;

		for row in self.iter() {
			match row.element.thumbnail_bounding_box(transform * *row.transform, include_stroke) {
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
			additional: self.additional.into_iter().map(|(key, value)| (key, value.into_iter())).collect(),
		}
	}
}

pub struct TableRowIter<T> {
	element: std::vec::IntoIter<T>,
	transform: std::vec::IntoIter<DAffine2>,
	alpha_blending: std::vec::IntoIter<AlphaBlending>,
	source_node_id: std::vec::IntoIter<Option<NodeId>>,
	additional: HashMap<String, std::vec::IntoIter<CustomColumnValue>>,
}
impl<T> Iterator for TableRowIter<T> {
	type Item = TableRow<T>;

	fn next(&mut self) -> Option<Self::Item> {
		let element = self.element.next()?;
		let transform = self.transform.next()?;
		let alpha_blending = self.alpha_blending.next()?;
		let source_node_id = self.source_node_id.next()?;
		let additional = self
			.additional
			.iter_mut()
			.map(|(key, value)| (key.clone(), value.next().expect("additional column length mismatch")))
			.collect();

		Some(TableRow {
			element,
			transform,
			alpha_blending,
			source_node_id,
			additional,
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
			additional: HashMap::new(),
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
		// Sort by key to have the same result
		let mut entries: Vec<_> = self.additional.iter().collect();
		entries.sort_by_key(|(key, _)| *key);
		for (key, values) in entries {
			key.hash(state);
			for value in values {
				value.hash(state);
			}
		}
	}
}

impl<T: PartialEq> PartialEq for Table<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element == other.element && self.transform == other.transform && self.alpha_blending == other.alpha_blending && self.additional == other.additional
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

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TableRow<T> {
	#[serde(alias = "instance")]
	pub element: T,
	pub transform: DAffine2,
	pub alpha_blending: AlphaBlending,
	pub source_node_id: Option<NodeId>,
	#[serde(default)]
	pub additional: HashMap<String, CustomColumnValue>,
}

impl<T> TableRow<T> {
	pub fn new_from_element(element: T) -> Self {
		Self {
			element,
			transform: DAffine2::IDENTITY,
			alpha_blending: AlphaBlending::default(),
			source_node_id: None,
			additional: HashMap::new(),
		}
	}

	pub fn as_ref(&self) -> TableRowRef<'_, T> {
		TableRowRef {
			element: &self.element,
			transform: &self.transform,
			alpha_blending: &self.alpha_blending,
			source_node_id: &self.source_node_id,
			additional: self.additional.iter().map(|(key, value)| (key.as_str(), value)).collect(),
		}
	}

	pub fn as_mut(&mut self) -> TableRowMut<'_, T> {
		TableRowMut {
			element: &mut self.element,
			transform: &mut self.transform,
			alpha_blending: &mut self.alpha_blending,
			source_node_id: &mut self.source_node_id,
			additional: self.additional.iter_mut().map(|(key, value)| (key.as_str(), value)).collect(),
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRowRef<'a, T> {
	pub element: &'a T,
	pub transform: &'a DAffine2,
	pub alpha_blending: &'a AlphaBlending,
	pub source_node_id: &'a Option<NodeId>,
	pub additional: HashMap<&'a str, &'a CustomColumnValue>,
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
			additional: self.additional.into_iter().map(|(key, value)| (key.to_string(), value.clone())).collect(),
		}
	}
}

#[derive(Debug)]
pub struct TableRowMut<'a, T> {
	pub element: &'a mut T,
	pub transform: &'a mut DAffine2,
	pub alpha_blending: &'a mut AlphaBlending,
	pub source_node_id: &'a mut Option<NodeId>,
	pub additional: HashMap<&'a str, &'a mut CustomColumnValue>,
}

// Conversion from Table<Color> to Option<Color> - extracts first element
impl From<Table<crate::Color>> for Option<crate::Color> {
	fn from(table: Table<crate::Color>) -> Self {
		table.iter().nth(0).map(|row| row.element).copied()
	}
}
