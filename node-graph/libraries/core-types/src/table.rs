use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::transform::ApplyTransform;
use crate::uuid::NodeId;
use crate::{AlphaBlending, math::quad::Quad};
use dyn_any::{StaticType, StaticTypeSized};
use glam::DAffine2;

// ATTRIBUTE VALUE TRAIT
// Enables type-erased storage that supports Clone, Send, Sync, and downcasting.

trait AttributeValue: std::any::Any + Send + Sync {
	fn clone_box(&self) -> Box<dyn AttributeValue>;
	fn as_any(&self) -> &dyn std::any::Any;
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
	fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

// The `Sized` bound ensures this blanket impl does not apply to `dyn AttributeValue` itself,
// which would cause infinite recursion in the `Clone for Box<dyn AttributeValue>` impl.
impl<T: Clone + Send + Sync + Sized + 'static> AttributeValue for T {
	fn clone_box(&self) -> Box<dyn AttributeValue> {
		Box::new(self.clone())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
		self
	}

	fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
		self
	}
}

impl Clone for Box<dyn AttributeValue> {
	fn clone(&self) -> Self {
		(**self).clone_box()
	}
}

// ATTRIBUTES

/// A small ordered map of type-erased attribute columns, keyed by string name.
/// Linear search preserves insertion order and is likely faster than a HashMap for small attribute counts.
#[derive(Clone, Default)]
pub struct Attributes {
	entries: Vec<(String, Box<dyn AttributeValue>)>,
}

impl std::fmt::Debug for Attributes {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.entries.iter().map(|(k, _)| k.as_str()).collect();
		f.debug_struct("Attributes").field("keys", &keys).finish()
	}
}

impl Attributes {
	pub fn new() -> Self {
		Self::default()
	}

	/// Inserts an attribute with the given key and value, replacing any existing entry with the same key.
	pub fn insert<T: Clone + Send + Sync + 'static>(&mut self, key: String, value: T) {
		for (k, v) in &mut self.entries {
			if *k == key {
				*v = Box::new(value);
				return;
			}
		}
		self.entries.push((key, Box::new(value)));
	}

	/// Gets a reference to the value of the attribute with the given key, if it exists and can be downcast to the requested type.
	pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
		// Explicit deref `(**v)` reaches `dyn AttributeValue` (which is !Sized and thus dispatches
		// through the vtable to the concrete type) rather than resolving to the blanket
		// `impl AttributeValue for Box<dyn AttributeValue>` which would return the wrong TypeId.
		self.entries.iter().find_map(|(k, v)| if k == key { (**v).as_any().downcast_ref::<T>() } else { None })
	}

	/// Gets a mutable reference to the value of the attribute with the given key, if it exists and can be downcast to the requested type.
	pub fn get_mut<T: 'static>(&mut self, key: &str) -> Option<&mut T> {
		self.entries.iter_mut().find_map(|(k, v)| if k == key { (**v).as_any_mut().downcast_mut::<T>() } else { None })
	}

	/// Gets a mutable reference to the value, inserting a default if it doesn't exist or has the wrong type.
	pub fn get_or_insert_default_mut<T: Clone + Send + Sync + Default + 'static>(&mut self, key: &str) -> &mut T {
		// Remove any existing entry with the wrong type, then insert a correctly-typed default
		let needs_insert = match self.entries.iter().position(|(k, _)| k == key) {
			Some(index) => {
				if (*self.entries[index].1).as_any().downcast_ref::<T>().is_some() {
					false
				} else {
					self.entries.remove(index);
					true
				}
			}
			None => true,
		};

		if needs_insert {
			self.entries.push((key.to_string(), Box::new(T::default())));
		}

		self.get_mut::<T>(key).expect("attribute was just ensured to exist with correct type")
	}

	/// Removes and returns the value for the given key, if it exists and can be downcast to the requested type.
	pub fn remove<T: 'static>(&mut self, key: &str) -> Option<T> {
		let index = self.entries.iter().position(|(k, _)| k == key)?;
		let (_, value) = self.entries.remove(index);
		value.into_any().downcast::<T>().ok().map(|b| *b)
	}
}

// TABLE

#[derive(Clone, Debug)]
pub struct Table<T> {
	element: Vec<T>,
	attributes: Attributes,
}

impl<T> Table<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_capacity(capacity: usize) -> Self {
		let mut attributes = Attributes::new();
		attributes.insert("transform".to_string(), Vec::<DAffine2>::with_capacity(capacity));
		attributes.insert("alpha_blending".to_string(), Vec::<AlphaBlending>::with_capacity(capacity));
		attributes.insert("source_node_id".to_string(), Vec::<Option<NodeId>>::with_capacity(capacity));

		Self {
			element: Vec::with_capacity(capacity),
			attributes,
		}
	}

	pub fn new_from_element(element: T) -> Self {
		let mut attributes = Attributes::new();
		attributes.insert("transform".to_string(), vec![DAffine2::IDENTITY]);
		attributes.insert("alpha_blending".to_string(), vec![AlphaBlending::default()]);
		attributes.insert("source_node_id".to_string(), vec![Option::<NodeId>::None]);

		Self { element: vec![element], attributes }
	}

	pub fn new_from_row(row: TableRow<T>) -> Self {
		let mut row_attributes = row.attributes;
		let transform = row_attributes.remove::<DAffine2>("transform").unwrap_or(DAffine2::IDENTITY);
		let alpha_blending = row_attributes.remove::<AlphaBlending>("alpha_blending").unwrap_or_default();
		let source_node_id = row_attributes.remove::<Option<NodeId>>("source_node_id").unwrap_or(None);

		let mut attributes = Attributes::new();
		attributes.insert("transform".to_string(), vec![transform]);
		attributes.insert("alpha_blending".to_string(), vec![alpha_blending]);
		attributes.insert("source_node_id".to_string(), vec![source_node_id]);

		Self {
			element: vec![row.element],
			attributes,
		}
	}

	pub fn push(&mut self, row: TableRow<T>) {
		let mut attributes = row.attributes;
		self.element.push(row.element);
		self.transforms_mut().push(attributes.remove::<DAffine2>("transform").unwrap_or(DAffine2::IDENTITY));
		self.alpha_blendings_mut().push(attributes.remove::<AlphaBlending>("alpha_blending").unwrap_or_default());
		self.source_node_ids_mut().push(attributes.remove::<Option<NodeId>>("source_node_id").unwrap_or(None));
	}

	pub fn extend(&mut self, table: Table<T>) {
		let mut other_attributes = table.attributes;

		self.element.extend(table.element);
		self.transforms_mut().extend(other_attributes.remove::<Vec<DAffine2>>("transform").unwrap_or_default());
		self.alpha_blendings_mut().extend(other_attributes.remove::<Vec<AlphaBlending>>("alpha_blending").unwrap_or_default());
		self.source_node_ids_mut().extend(other_attributes.remove::<Vec<Option<NodeId>>>("source_node_id").unwrap_or_default());
	}

	pub fn get(&self, index: usize) -> Option<TableRowRef<'_, T>> {
		if index >= self.element.len() {
			return None;
		}

		Some(TableRowRef {
			element: &self.element[index],
			transform: &self.transforms()[index],
			alpha_blending: &self.alpha_blendings()[index],
			source_node_id: &self.source_node_ids()[index],
		})
	}

	pub fn get_mut(&mut self, index: usize) -> Option<TableRowMut<'_, T>> {
		if index >= self.element.len() {
			return None;
		}

		// Split borrows: element from the vec, attributes from the Attributes map
		let element = &mut self.element[index] as *mut T;
		let transforms = self.transforms_mut();
		let transform = &mut transforms[index] as *mut DAffine2;
		let alpha_blendings = self.alpha_blendings_mut();
		let alpha_blending = &mut alpha_blendings[index] as *mut AlphaBlending;
		let source_node_ids = self.source_node_ids_mut();
		let source_node_id = &mut source_node_ids[index] as *mut Option<NodeId>;

		// SAFETY: All pointers come from distinct Vecs in self, so they don't alias
		Some(TableRowMut {
			element: unsafe { &mut *element },
			transform: unsafe { &mut *transform },
			alpha_blending: unsafe { &mut *alpha_blending },
			source_node_id: unsafe { &mut *source_node_id },
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
			.zip(self.transforms().iter())
			.zip(self.alpha_blendings().iter())
			.zip(self.source_node_ids().iter())
			.map(|(((element, transform), alpha_blending), source_node_id)| TableRowRef {
				element,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	/// Mutably borrows a [`Table`] and returns an iterator of [`TableRowMut`]s, each containing mutable references to the data of the respective row from the table.
	pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = TableRowMut<'_, T>> {
		let transforms = self.transforms_mut() as *mut Vec<DAffine2>;
		let alpha_blendings = self.alpha_blendings_mut() as *mut Vec<AlphaBlending>;
		let source_node_ids = self.source_node_ids_mut() as *mut Vec<Option<NodeId>>;

		// SAFETY: Each Vec is a distinct allocation within Attributes, so mutable references to their elements don't alias
		self.element
			.iter_mut()
			.zip(unsafe { &mut *transforms }.iter_mut())
			.zip(unsafe { &mut *alpha_blendings }.iter_mut())
			.zip(unsafe { &mut *source_node_ids }.iter_mut())
			.map(|(((element, transform), alpha_blending), source_node_id)| TableRowMut {
				element,
				transform,
				alpha_blending,
				source_node_id,
			})
	}

	// Convenience accessors for the well-known attribute columns

	pub fn transforms(&self) -> &[DAffine2] {
		self.attributes.get::<Vec<DAffine2>>("transform").map(Vec::as_slice).unwrap_or(&[])
	}

	pub fn transforms_mut(&mut self) -> &mut Vec<DAffine2> {
		self.attributes.get_or_insert_default_mut::<Vec<DAffine2>>("transform")
	}

	pub fn alpha_blendings(&self) -> &[AlphaBlending] {
		self.attributes.get::<Vec<AlphaBlending>>("alpha_blending").map(Vec::as_slice).unwrap_or(&[])
	}

	pub fn alpha_blendings_mut(&mut self) -> &mut Vec<AlphaBlending> {
		self.attributes.get_or_insert_default_mut::<Vec<AlphaBlending>>("alpha_blending")
	}

	pub fn source_node_ids(&self) -> &[Option<NodeId>] {
		self.attributes.get::<Vec<Option<NodeId>>>("source_node_id").map(Vec::as_slice).unwrap_or(&[])
	}

	pub fn source_node_ids_mut(&mut self) -> &mut Vec<Option<NodeId>> {
		self.attributes.get_or_insert_default_mut::<Vec<Option<NodeId>>>("source_node_id")
	}
}

// CUSTOM SERDE

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for Table<T> {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		#[derive(serde::Serialize)]
		struct TableHelper<'a, T: serde::Serialize> {
			element: &'a Vec<T>,
			transform: &'a [DAffine2],
			alpha_blending: &'a [AlphaBlending],
			source_node_id: &'a [Option<NodeId>],
		}

		TableHelper {
			element: &self.element,
			transform: self.transforms(),
			alpha_blending: self.alpha_blendings(),
			source_node_id: self.source_node_ids(),
		}
		.serialize(serializer)
	}
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Table<T> {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(serde::Deserialize)]
		struct TableHelper<T> {
			#[serde(alias = "instances", alias = "instance")]
			element: Vec<T>,
			#[serde(default)]
			transform: Vec<DAffine2>,
			#[serde(default)]
			alpha_blending: Vec<AlphaBlending>,
			#[serde(default)]
			source_node_id: Vec<Option<NodeId>>,
		}

		let helper = TableHelper::deserialize(deserializer)?;
		let length = helper.element.len();

		// Pad attribute vecs to match element length if they're shorter (e.g., from older save formats)
		let mut transform = helper.transform;
		transform.resize(length, DAffine2::IDENTITY);

		let mut alpha_blending = helper.alpha_blending;
		alpha_blending.resize(length, AlphaBlending::default());

		let mut source_node_id = helper.source_node_id;
		source_node_id.resize(length, None);

		let mut attributes = Attributes::new();
		attributes.insert("transform".to_string(), transform);
		attributes.insert("alpha_blending".to_string(), alpha_blending);
		attributes.insert("source_node_id".to_string(), source_node_id);

		Ok(Table { element: helper.element, attributes })
	}
}

// TRAIT IMPLS

impl<T: BoundingBox> BoundingBox for Table<T> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let mut combined_bounds = None;

		for row in self.iter() {
			match row.element.bounding_box(transform * *row.transform(), include_stroke) {
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
		let mut attributes = self.attributes;

		TableRowIter {
			element: self.element.into_iter(),
			transform: attributes.remove::<Vec<DAffine2>>("transform").unwrap_or_default().into_iter(),
			alpha_blending: attributes.remove::<Vec<AlphaBlending>>("alpha_blending").unwrap_or_default().into_iter(),
			source_node_id: attributes.remove::<Vec<Option<NodeId>>>("source_node_id").unwrap_or_default().into_iter(),
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

		Some(TableRow::new(element, transform, alpha_blending, source_node_id))
	}
}

impl<T> DoubleEndedIterator for TableRowIter<T> {
	fn next_back(&mut self) -> Option<Self::Item> {
		let element = self.element.next_back()?;
		let transform = self.transform.next_back()?;
		let alpha_blending = self.alpha_blending.next_back()?;
		let source_node_id = self.source_node_id.next_back()?;

		Some(TableRow::new(element, transform, alpha_blending, source_node_id))
	}
}

impl<T> Default for Table<T> {
	fn default() -> Self {
		let mut attributes = Attributes::new();
		attributes.insert("transform".to_string(), Vec::<DAffine2>::new());
		attributes.insert("alpha_blending".to_string(), Vec::<AlphaBlending>::new());
		attributes.insert("source_node_id".to_string(), Vec::<Option<NodeId>>::new());

		Self { element: Vec::new(), attributes }
	}
}

impl<T: graphene_hash::CacheHash> graphene_hash::CacheHash for Table<T> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		for element in &self.element {
			element.cache_hash(state);
		}
		for transform in self.transforms() {
			graphene_hash::CacheHash::cache_hash(transform, state);
		}
		for alpha_blending in self.alpha_blendings() {
			alpha_blending.cache_hash(state);
		}
	}
}

impl<T: PartialEq> PartialEq for Table<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element == other.element && self.transforms() == other.transforms() && self.alpha_blendings() == other.alpha_blendings()
	}
}

impl<T> ApplyTransform for Table<T> {
	fn apply_transform(&mut self, modification: &DAffine2) {
		for transform in self.transforms_mut() {
			*transform *= *modification;
		}
	}

	fn left_apply_transform(&mut self, modification: &DAffine2) {
		for transform in self.transforms_mut() {
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

// TABLE ROW TYPES

#[derive(Clone, Debug)]
pub struct TableRow<T> {
	pub element: T,
	attributes: Attributes,
}

impl<T: Default> Default for TableRow<T> {
	fn default() -> Self {
		Self::new_from_element(T::default())
	}
}

impl<T: PartialEq> PartialEq for TableRow<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element == other.element && self.transform() == other.transform() && self.alpha_blending() == other.alpha_blending() && self.source_node_id() == other.source_node_id()
	}
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for TableRow<T> {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		#[derive(serde::Serialize)]
		struct TableRowHelper<'a, T: serde::Serialize> {
			element: &'a T,
			transform: &'a DAffine2,
			alpha_blending: &'a AlphaBlending,
			source_node_id: &'a Option<NodeId>,
		}

		TableRowHelper {
			element: &self.element,
			transform: self.transform(),
			alpha_blending: self.alpha_blending(),
			source_node_id: self.source_node_id(),
		}
		.serialize(serializer)
	}
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for TableRow<T> {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(serde::Deserialize)]
		struct TableRowHelper<T> {
			#[serde(alias = "instance")]
			element: T,
			#[serde(default = "default_transform")]
			transform: DAffine2,
			#[serde(default)]
			alpha_blending: AlphaBlending,
			#[serde(default)]
			source_node_id: Option<NodeId>,
		}

		fn default_transform() -> DAffine2 {
			DAffine2::IDENTITY
		}

		let helper = TableRowHelper::deserialize(deserializer)?;
		Ok(TableRow::new(helper.element, helper.transform, helper.alpha_blending, helper.source_node_id))
	}
}

impl<T> TableRow<T> {
	pub fn new(element: T, transform: DAffine2, alpha_blending: AlphaBlending, source_node_id: Option<NodeId>) -> Self {
		let mut attributes = Attributes::new();
		attributes.insert("transform".to_string(), transform);
		attributes.insert("alpha_blending".to_string(), alpha_blending);
		attributes.insert("source_node_id".to_string(), source_node_id);
		Self { element, attributes }
	}

	pub fn new_from_element(element: T) -> Self {
		Self::new(element, DAffine2::IDENTITY, AlphaBlending::default(), None)
	}

	pub fn transform(&self) -> &DAffine2 {
		static DEFAULT: DAffine2 = DAffine2::IDENTITY;
		self.attributes.get::<DAffine2>("transform").unwrap_or(&DEFAULT)
	}

	pub fn transform_mut(&mut self) -> &mut DAffine2 {
		self.attributes.get_or_insert_default_mut::<DAffine2>("transform")
	}

	pub fn alpha_blending(&self) -> &AlphaBlending {
		static DEFAULT: AlphaBlending = AlphaBlending::new();
		self.attributes.get::<AlphaBlending>("alpha_blending").unwrap_or(&DEFAULT)
	}

	pub fn alpha_blending_mut(&mut self) -> &mut AlphaBlending {
		self.attributes.get_or_insert_default_mut::<AlphaBlending>("alpha_blending")
	}

	pub fn source_node_id(&self) -> &Option<NodeId> {
		static DEFAULT: Option<NodeId> = None;
		self.attributes.get::<Option<NodeId>>("source_node_id").unwrap_or(&DEFAULT)
	}

	pub fn source_node_id_mut(&mut self) -> &mut Option<NodeId> {
		self.attributes.get_or_insert_default_mut::<Option<NodeId>>("source_node_id")
	}

	pub fn as_ref(&self) -> TableRowRef<'_, T> {
		TableRowRef {
			element: &self.element,
			transform: self.transform(),
			alpha_blending: self.alpha_blending(),
			source_node_id: self.source_node_id(),
		}
	}
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TableRowRef<'a, T> {
	pub element: &'a T,
	transform: &'a DAffine2,
	alpha_blending: &'a AlphaBlending,
	source_node_id: &'a Option<NodeId>,
}

impl<T> TableRowRef<'_, T> {
	pub fn transform(&self) -> &DAffine2 {
		self.transform
	}

	pub fn alpha_blending(&self) -> &AlphaBlending {
		self.alpha_blending
	}

	pub fn source_node_id(&self) -> &Option<NodeId> {
		self.source_node_id
	}

	pub fn into_cloned(self) -> TableRow<T>
	where
		T: Clone,
	{
		TableRow::new(self.element.clone(), *self.transform, *self.alpha_blending, *self.source_node_id)
	}
}

#[derive(Debug)]
pub struct TableRowMut<'a, T> {
	pub element: &'a mut T,
	transform: &'a mut DAffine2,
	alpha_blending: &'a mut AlphaBlending,
	source_node_id: &'a mut Option<NodeId>,
}

impl<T> TableRowMut<'_, T> {
	pub fn transform(&self) -> &DAffine2 {
		self.transform
	}

	pub fn transform_mut(&mut self) -> &mut DAffine2 {
		self.transform
	}

	pub fn alpha_blending(&self) -> &AlphaBlending {
		self.alpha_blending
	}

	pub fn alpha_blending_mut(&mut self) -> &mut AlphaBlending {
		self.alpha_blending
	}

	pub fn source_node_id(&self) -> &Option<NodeId> {
		self.source_node_id
	}

	pub fn source_node_id_mut(&mut self) -> &mut Option<NodeId> {
		self.source_node_id
	}
}

// Conversion from Table<Color> to Option<Color> - extracts first element
impl From<Table<crate::Color>> for Option<crate::Color> {
	fn from(table: Table<crate::Color>) -> Self {
		table.iter().nth(0).map(|row| row.element).copied()
	}
}
