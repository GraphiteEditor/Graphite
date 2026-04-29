use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::math::quad::Quad;
use crate::transform::ApplyTransform;
use dyn_any::{StaticType, StaticTypeSized};
use glam::DAffine2;
use std::fmt::Debug;

/// Attribute key under which each row of an editor-aware layer stores a `Table<NodeId>` describing the
/// path (from the root document network) to the layer node that owns the row. Editor tools use this to
/// route clicks/selection back to the originating layer at any nesting depth.
pub const EDITOR_LAYER_PATH: &str = "editor:layer_path";

/// Attribute key under which a row stores a `Table<Graphic>` snapshot of the upstream content that fed
/// into a destructive merge (Boolean Operation, Flatten Path, Morph, Rasterize, etc.). The renderer
/// recurses into this snapshot during metadata collection so the editor can still surface click targets
/// for the original child layers after their content has been collapsed into a single output.
pub const EDITOR_MERGED_LAYERS: &str = "editor:merged_layers";

// =====================
// TRAIT: AttributeValue
// =====================

/// Enables type-erased scalar storage that supports Clone, Send, Sync, and downcasting.
/// Used for individual attribute values in a TableRow.
trait AttributeValue: std::any::Any + Send + Sync {
	/// Clones this value into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AttributeValue>;

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any;

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

	/// Consumes the box and returns the underlying concrete type for downcasting.
	fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;

	/// Returns a debug-formatted string representation of this value.
	fn display_string(&self) -> String;

	/// Wraps this scalar value into a new column for columnar storage,
	/// with `preceding_defaults` default values before this value.
	fn into_column(self: Box<Self>, preceding_defaults: usize) -> Box<dyn AttributeColumn>;
}

impl<T: Clone + Send + Sync + Default + Sized + Debug + 'static> AttributeValue for T {
	/// Clones this value into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AttributeValue> {
		Box::new(self.clone())
	}

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
		self
	}

	/// Consumes the box and returns the underlying concrete type for downcasting.
	fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
		self
	}

	/// Returns a debug-formatted string representation of this value.
	fn display_string(&self) -> String {
		format!("{:?}", self)
	}

	/// Wraps this scalar value into a new column, padded with `preceding_defaults` default values before it.
	fn into_column(self: Box<Self>, preceding_defaults: usize) -> Box<dyn AttributeColumn> {
		let mut data = vec![T::default(); preceding_defaults];
		data.push(*self);
		Box::new(Column(data))
	}
}

impl Clone for Box<dyn AttributeValue> {
	fn clone(&self) -> Self {
		(**self).clone_box()
	}
}

// ======================
// TRAIT: AttributeColumn
// ======================

/// Enables type-erased columnar storage for parallel attribute lists in a Table.
trait AttributeColumn: std::any::Any + Send + Sync {
	/// Clones this column into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AttributeColumn>;

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any;

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

	/// Pushes a scalar attribute value onto the end of this column.
	fn push(&mut self, value: Box<dyn AttributeValue>);

	/// Pushes a default value onto the end of this column.
	fn push_default(&mut self);

	/// Creates a new column of the same type filled with `count` number of default values.
	fn new_with_defaults(&self, count: usize) -> Box<dyn AttributeColumn>;

	/// Returns the number of elements in this column.
	fn len(&self) -> usize;

	/// Appends all values from another column of the same type.
	fn extend(&mut self, other: Box<dyn AttributeColumn>);

	/// Returns a shared reference to the value at the requested index.
	fn get_any(&self, index: usize) -> Option<&dyn std::any::Any>;

	/// Returns a debug-formatted display string for the value at the requested index.
	fn display_at(&self, index: usize) -> Option<String>;

	/// Clones a single value from this column into a boxed scalar attribute value.
	fn clone_value(&self, index: usize) -> Option<Box<dyn AttributeValue>>;

	/// Drains all values out of this column into a Vec of scalar attribute values.
	fn drain(self: Box<Self>) -> Vec<Box<dyn AttributeValue>>;
}

impl Clone for Box<dyn AttributeColumn> {
	fn clone(&self) -> Self {
		(**self).clone_box()
	}
}

// =========
// Column<T>
// =========

/// Wraps a Vec<T> for column-major attribute storage in a Table.
struct Column<T>(Vec<T>);

impl<T: Clone + Send + Sync + Default + Debug + 'static> AttributeColumn for Column<T> {
	/// Clones this column into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AttributeColumn> {
		Box::new(Column(self.0.clone()))
	}

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
		self
	}

	/// Pushes a scalar attribute value onto the end of this column, downcasting it to `T`.
	/// Falls back to a default value if the type doesn't match, to maintain the column-length invariant.
	fn push(&mut self, value: Box<dyn AttributeValue>) {
		if let Ok(value) = value.into_any().downcast::<T>() {
			self.0.push(*value);
		} else {
			self.0.push(T::default());
		}
	}

	/// Pushes a default `T` value onto the end of this column.
	fn push_default(&mut self) {
		self.0.push(T::default());
	}

	/// Creates a new column filled with `count` default `T` values.
	fn new_with_defaults(&self, count: usize) -> Box<dyn AttributeColumn> {
		Box::new(Column(vec![T::default(); count]))
	}

	/// Returns the number of elements in this column.
	fn len(&self) -> usize {
		self.0.len()
	}

	/// Appends all values from another column, downcasting it to the same `Column<T>` type.
	/// Falls back to padding with defaults if the type doesn't match, to maintain the column-length invariant.
	fn extend(&mut self, other: Box<dyn AttributeColumn>) {
		let other_len = other.len();
		if let Ok(other) = (other as Box<dyn std::any::Any>).downcast::<Self>() {
			self.0.extend(other.0);
		} else {
			self.0.extend(std::iter::repeat_with(T::default).take(other_len));
		}
	}

	/// Returns a shared reference to the value at the given index as a type-erased `Any`.
	fn get_any(&self, index: usize) -> Option<&dyn std::any::Any> {
		self.0.get(index).map(|v| v as &dyn std::any::Any)
	}

	/// Returns a debug-formatted string for the value at the given index.
	fn display_at(&self, index: usize) -> Option<String> {
		self.0.get(index).map(|v| format!("{v:?}"))
	}

	/// Clones the value at the given index into a boxed scalar attribute value.
	fn clone_value(&self, index: usize) -> Option<Box<dyn AttributeValue>> {
		self.0.get(index).map(|v| Box::new(v.clone()) as Box<dyn AttributeValue>)
	}

	/// Consumes this column and returns all values as a Vec of boxed scalar attribute values.
	fn drain(self: Box<Self>) -> Vec<Box<dyn AttributeValue>> {
		self.0.into_iter().map(|v| Box::new(v) as Box<dyn AttributeValue>).collect()
	}
}

// ===============
// AttributeValues
// ===============

/// Scalar attribute storage.
///
/// A small ordered map of type-erased scalar attribute values, keyed by string name.
/// Used for individual attribute values in a TableRow.
/// Linear search preserves insertion order and is likely faster than a HashMap for small attribute counts.
#[derive(Clone, Default)]
pub struct AttributeValues(Vec<(String, Box<dyn AttributeValue>)>);

impl Debug for AttributeValues {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.0.iter().map(|(k, _)| k.as_str()).collect();
		f.debug_struct("Attributes").field("keys", &keys).finish()
	}
}

impl AttributeValues {
	/// Creates an empty set of attributes.
	pub fn new() -> Self {
		Self::default()
	}

	/// Inserts an attribute with the given key and value, replacing any existing entry with the same key.
	pub fn insert<T: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: impl Into<String>, value: T) {
		let key = key.into();

		for (existing_key, existing_value) in &mut self.0 {
			if *existing_key == key {
				*existing_value = Box::new(value);
				return;
			}
		}

		self.0.push((key, Box::new(value)));
	}

	/// Gets a reference to the value of the attribute with the given key, if it exists and can be downcast to the requested type.
	pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
		// Explicit deref `(**value)` reaches `dyn AttributeValue` (which is !Sized and thus dispatches
		// through the vtable to the concrete type) rather than resolving to the blanket
		// `impl AttributeValue for Box<dyn AttributeValue>` which would return the wrong TypeId.
		self.0
			.iter()
			.find_map(|(existing_key, value)| if existing_key == key { (**value).as_any().downcast_ref::<T>() } else { None })
	}

	/// Gets a mutable reference to the value of the attribute with the given key, if it exists and can be downcast to the requested type.
	pub fn get_mut<T: 'static>(&mut self, key: &str) -> Option<&mut T> {
		self.0
			.iter_mut()
			.find_map(|(existing_key, value)| if existing_key == key { (**value).as_any_mut().downcast_mut::<T>() } else { None })
	}

	/// Gets a mutable reference to the value, inserting a default if it doesn't exist or has the wrong type.
	pub fn get_or_insert_default_mut<T: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: &str) -> &mut T {
		let needs_insert = match self.0.iter().position(|(existing_key, _)| existing_key == key) {
			Some(index) => {
				if (*self.0[index].1).as_any().downcast_ref::<T>().is_some() {
					false
				} else {
					self.0.remove(index);
					true
				}
			}
			None => true,
		};

		if needs_insert {
			self.0.push((key.to_string(), Box::new(T::default())));
		}

		self.get_mut::<T>(key).expect("Attribute was just ensured to exist with correct type")
	}

	/// Removes and returns the value for the given key, if it exists and can be downcast to the requested type.
	pub fn remove<T: 'static>(&mut self, key: &str) -> Option<T> {
		let index = self.0.iter().position(|(existing_key, _)| existing_key == key)?;
		let (_, value) = self.0.remove(index);
		value.into_any().downcast::<T>().ok().map(|boxed| *boxed)
	}

	/// Returns an iterator over the keys of all stored attributes, in insertion order.
	pub fn keys(&self) -> impl Iterator<Item = &str> {
		self.0.iter().map(|(key, _)| key.as_str())
	}

	/// Returns a debug-formatted string representation of the attribute value for the given key, if it exists.
	/// The `overrides` function can provide custom formatting for specific type.
	pub fn display_value(&self, key: &str, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.0.iter().find_map(|(k, value)| {
			if k == key {
				if let Some(text) = overrides(value.as_any()) { Some(text) } else { Some(value.display_string()) }
			} else {
				None
			}
		})
	}
}

// ================
// AttributeColumns
// ================

/// Columnar attribute storage.
///
/// A collection of type-erased parallel attribute columns, keyed by string name.
/// Used for columnar attribute storage in a Table.
/// Not public. All access goes through Table and TableRow.
/// Invariant: every column in `columns` has exactly `len` elements.
#[derive(Clone, Default)]
struct AttributeColumns {
	columns: Vec<(String, Box<dyn AttributeColumn>)>,
	len: usize,
}

impl Debug for AttributeColumns {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.columns.iter().map(|(k, _)| k.as_str()).collect();
		f.debug_struct("AttributeColumns").field("keys", &keys).field("len", &self.len).finish()
	}
}

impl AttributeColumns {
	/// Creates an empty column store with no columns and zero length.
	fn new() -> Self {
		Self::default()
	}

	/// Creates an empty column store with no columns but a pre-set row count.
	fn with_len(len: usize) -> Self {
		Self { columns: Vec::new(), len }
	}

	/// Pushes a row's scalar attributes into this column store.
	/// Existing columns that the row lacks receive a default value.
	/// New attribute keys create a new column padded with defaults for all prior rows.
	fn push_row(&mut self, row: AttributeValues) {
		let mut row_entries = row.0;

		// Push values into existing columns, or a default if the row lacks that attribute
		for (column_key, column) in &mut self.columns {
			if let Some(position) = row_entries.iter().position(|(k, _)| k == column_key) {
				let (_, value) = row_entries.swap_remove(position);
				column.push(value);
			} else {
				column.push_default();
			}
		}

		// Create new columns for any remaining row entries, padded with defaults for prior rows
		for (key, value) in row_entries {
			self.columns.push((key, value.into_column(self.len)));
		}

		self.len += 1;
	}

	/// Appends all column data from another column store into this one.
	/// Columns present in only one side are padded with defaults for the other side's rows.
	fn extend(&mut self, other: AttributeColumns) {
		let other_len = other.len;
		let mut other_entries = other.columns;

		// Extend matching columns, or pad self's columns with defaults for the other's row count
		for (key, self_column) in &mut self.columns {
			if let Some(position) = other_entries.iter().position(|(k, _)| k == key) {
				let (_, other_column) = other_entries.swap_remove(position);
				self_column.extend(other_column);
			} else {
				for _ in 0..other_len {
					self_column.push_default();
				}
			}
		}

		// Remaining other columns are new, pad with defaults for self's existing rows
		for (key, other_column) in other_entries {
			let mut combined = other_column.new_with_defaults(self.len);
			combined.extend(other_column);
			self.columns.push((key, combined));
		}

		self.len += other_len;
	}

	/// Gets a reference to the value at the given index from the column for the given key.
	fn get_value<T: 'static>(&self, key: &str, index: usize) -> Option<&T> {
		self.columns.iter().find_map(|(k, column)| if k == key { column.get_any(index)?.downcast_ref::<T>() } else { None })
	}

	/// Finds or creates a column for the given key and type, returning its position.
	/// If a column with the key exists but has the wrong type, it is removed and replaced with a new column of the correct type, padded with defaults.
	/// A newly created column is filled with `T::default()` for all existing rows.
	fn find_or_create_column<T: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: &str) -> usize {
		match self.columns.iter().position(|(k, _)| k == key) {
			Some(position) => {
				if (*self.columns[position].1).as_any().downcast_ref::<Column<T>>().is_some() {
					position
				} else {
					self.columns.remove(position);
					self.columns.push((key.to_string(), Box::new(Column::<T>(vec![T::default(); self.len]))));
					self.columns.len() - 1
				}
			}
			None => {
				self.columns.push((key.to_string(), Box::new(Column::<T>(vec![T::default(); self.len]))));
				self.columns.len() - 1
			}
		}
	}

	/// Gets a mutable reference to the value at the given index, creating the column if it doesn't exist or has the wrong type.
	fn get_or_insert_default_value<T: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: &str, index: usize) -> &mut T {
		let column_position = self.find_or_create_column::<T>(key);
		let column = (*self.columns[column_position].1).as_any_mut().downcast_mut::<Column<T>>().unwrap();
		&mut column.0[index]
	}

	/// Sets the value at the given index in the column for the given key.
	/// Creates the column with defaults if it doesn't exist.
	fn set_value<T: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: impl Into<String>, index: usize, value: T) {
		let key = key.into();
		let column_position = self.find_or_create_column::<T>(&key);
		let column = (*self.columns[column_position].1).as_any_mut().downcast_mut::<Column<T>>().unwrap();
		column.0[index] = value;
	}

	/// Returns a debug-formatted string for the value at the given index in the column for the given key.
	fn display_value(&self, key: &str, index: usize, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.columns.iter().find_map(|(k, column)| {
			if k == key {
				if let Some(value) = column.get_any(index)
					&& let Some(text) = overrides(value)
				{
					return Some(text);
				}
				column.display_at(index)
			} else {
				None
			}
		})
	}

	/// Returns a type-erased reference to the value at the given index in the column for the given key.
	fn get_any_value(&self, key: &str, index: usize) -> Option<&dyn std::any::Any> {
		self.columns.iter().find_map(|(k, column)| if k == key { column.get_any(index) } else { None })
	}

	/// Returns an iterator over the keys of all stored attribute columns, in insertion order.
	fn keys(&self) -> impl Iterator<Item = &str> {
		self.columns.iter().map(|(key, _)| key.as_str())
	}

	/// Returns a typed slice of the column for the given key, if it exists and can be downcast to `Column<T>`.
	fn get_column_slice<T: 'static>(&self, key: &str) -> Option<&[T]> {
		self.columns
			.iter()
			.find_map(|(k, column)| if k == key { column.as_any().downcast_ref::<Column<T>>().map(|c| c.0.as_slice()) } else { None })
	}

	/// Returns a mutable typed slice of the column for the given key, if it exists and can be downcast to `Column<T>`.
	fn get_column_slice_mut<T: 'static>(&mut self, key: &str) -> Option<&mut [T]> {
		self.columns.iter_mut().find_map(|(k, column)| {
			if k == key {
				column.as_any_mut().downcast_mut::<Column<T>>().map(|c| c.0.as_mut_slice())
			} else {
				None
			}
		})
	}

	/// Returns a mutable typed slice of the column for the given key, creating a new column filled with defaults if it doesn't exist.
	fn get_or_create_column_slice_mut<T: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: &str) -> &mut [T] {
		let position = self.find_or_create_column::<T>(key);
		let column = (*self.columns[position].1).as_any_mut().downcast_mut::<Column<T>>().unwrap();
		&mut column.0
	}

	/// Clones all attribute values at the given row index into a new scalar Attributes.
	fn clone_row(&self, index: usize) -> AttributeValues {
		let mut attributes = AttributeValues::new();

		for (key, column) in &self.columns {
			if let Some(value) = column.clone_value(index) {
				attributes.0.push((key.clone(), value));
			}
		}

		attributes
	}

	/// Drains all column data into a Vec of per-row scalar Attributes.
	fn into_row_vec(self) -> Vec<AttributeValues> {
		let mut rows: Vec<AttributeValues> = (0..self.len).map(|_| AttributeValues::new()).collect();

		for (key, column) in self.columns {
			for (i, value) in column.drain().into_iter().enumerate() {
				rows[i].0.push((key.clone(), value));
			}
		}

		rows
	}
}

// ========
// Table<T>
// ========

/// A struct-of-arrays collection where each row holds an element of type `T` alongside
/// a set of type-erased, dynamically-typed attributes stored in parallel columns.
///
/// Elements are stored contiguously in a `Vec<T>`, while attributes live in an internal
/// [`AttributeColumns`] store that keeps one column per attribute key. Rows are accessed
/// by index through element/attribute accessor methods, or consumed as owned [`TableRow`]s via iteration.
#[derive(Clone, Debug)]
pub struct Table<T> {
	element: Vec<T>,
	attributes: AttributeColumns,
}

impl<T> Table<T> {
	/// Creates an empty table with no rows.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates an empty table with pre-allocated capacity for the given number of rows.
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			element: Vec::with_capacity(capacity),
			attributes: AttributeColumns::new(),
		}
	}

	/// Creates a table containing a single row with the given element and no attributes.
	pub fn new_from_element(element: T) -> Self {
		Self {
			element: vec![element],
			attributes: AttributeColumns::with_len(1),
		}
	}

	/// Creates a table containing a single row from the given [`TableRow`], preserving its attributes.
	pub fn new_from_row(row: TableRow<T>) -> Self {
		let mut attributes = AttributeColumns::new();
		attributes.push_row(row.attributes);
		Self {
			element: vec![row.element],
			attributes,
		}
	}

	/// Appends a row to the end of this table.
	pub fn push(&mut self, row: TableRow<T>) {
		self.element.push(row.element);
		self.attributes.push_row(row.attributes);
	}

	/// Appends all rows from another table into this one.
	pub fn extend(&mut self, table: Table<T>) {
		self.element.extend(table.element);
		self.attributes.extend(table.attributes);
	}

	/// Returns the number of rows in this table.
	pub fn len(&self) -> usize {
		self.element.len()
	}

	/// Returns `true` if this table contains no rows.
	pub fn is_empty(&self) -> bool {
		self.element.is_empty()
	}

	/// Returns an iterator over all attribute keys in this table, in insertion order.
	pub fn attribute_keys(&self) -> impl Iterator<Item = &str> {
		self.attributes.keys()
	}

	// ===========================
	// Column-oriented iteration
	// ===========================

	/// Returns an iterator over shared references to all element values.
	pub fn iter_element_values(&self) -> std::slice::Iter<'_, T> {
		self.element.iter()
	}

	/// Returns an iterator over mutable references to all element values.
	pub fn iter_element_values_mut(&mut self) -> std::slice::IterMut<'_, T> {
		self.element.iter_mut()
	}

	/// Returns an iterator over shared references to the values of a typed attribute column, or `None` if the column doesn't exist or has the wrong type.
	pub fn iter_attribute_values<U: 'static>(&self, key: &str) -> Option<std::slice::Iter<'_, U>> {
		self.attributes.get_column_slice::<U>(key).map(|s| s.iter())
	}

	/// Returns an iterator over mutable references to the values of a typed attribute column, or `None` if the column doesn't exist or has the wrong type.
	pub fn iter_attribute_values_mut<U: 'static>(&mut self, key: &str) -> Option<std::slice::IterMut<'_, U>> {
		self.attributes.get_column_slice_mut::<U>(key).map(|s| s.iter_mut())
	}

	/// Returns an iterator that yields cloned attribute values for the given key, falling back to `U::default()` for each row if the column is missing or has the wrong type.
	pub fn iter_attribute_values_or_default<U: Clone + Default + 'static>(&self, key: &str) -> impl Iterator<Item = U> + '_ {
		let slice = self.attributes.get_column_slice::<U>(key);
		let len = self.element.len();
		(0..len).map(move |i| slice.map_or_else(U::default, |s| s[i].clone()))
	}

	/// Returns a mutable iterator over a typed attribute column, creating the column with default values if it doesn't exist or has the wrong type.
	pub fn iter_attribute_values_mut_or_default<U: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: &str) -> std::slice::IterMut<'_, U> {
		self.attributes.get_or_create_column_slice_mut::<U>(key).iter_mut()
	}

	// =======================
	// Indexed element access
	// =======================

	/// Returns a shared reference to the element at the given index, or `None` if out of bounds.
	pub fn element(&self, index: usize) -> Option<&T> {
		self.element.get(index)
	}

	/// Returns a mutable reference to the element at the given index, or `None` if out of bounds.
	pub fn element_mut(&mut self, index: usize) -> Option<&mut T> {
		self.element.get_mut(index)
	}

	// =========================
	// Indexed attribute access
	// =========================

	/// Returns a shared reference to the attribute value at the given row index and key, if it exists and can be downcast to the requested type.
	pub fn attribute<U: 'static>(&self, key: &str, index: usize) -> Option<&U> {
		self.attributes.get_value(key, index)
	}

	/// Returns a clone of the attribute value at the given row index and key, or `U::default()` if absent or of a different type.
	pub fn attribute_cloned_or_default<U: Clone + Default + 'static>(&self, key: &str, index: usize) -> U {
		self.attributes.get_value::<U>(key, index).cloned().unwrap_or_default()
	}

	/// Returns a clone of the attribute value at the given row index and key, or the provided default if absent or of a different type.
	pub fn attribute_cloned_or<U: Clone + 'static>(&self, key: &str, index: usize, default: U) -> U {
		self.attributes.get_value::<U>(key, index).cloned().unwrap_or(default)
	}

	/// Sets the attribute value at the given row index and key, creating the column with defaults if it doesn't exist.
	pub fn set_attribute<U: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: impl Into<String>, index: usize, value: U) {
		self.attributes.set_value(key, index, value);
	}

	/// Runs the given closure on a mutable reference to the attribute value at the given row index,
	/// creating the column with defaults if it doesn't exist, and returns the closure's result.
	pub fn with_attribute_mut_or_default<U: Clone + Send + Sync + Default + Debug + 'static, R, F: FnOnce(&mut U) -> R>(&mut self, key: &str, index: usize, f: F) -> R {
		f(self.attributes.get_or_insert_default_value::<U>(key, index))
	}

	/// Returns a debug-formatted display string for the attribute at the given row index and key.
	pub fn attribute_display_value(&self, key: &str, index: usize, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.attributes.display_value(key, index, overrides)
	}

	/// Returns a type-erased reference to the attribute value at the given row index and key, or `None` if absent.
	pub fn attribute_any(&self, key: &str, index: usize) -> Option<&dyn std::any::Any> {
		self.attributes.get_any_value(key, index)
	}

	// =====================
	// Split borrow helpers
	// =====================

	/// Returns disjoint mutable references to the element slice and a typed attribute column slice,
	/// creating the column with defaults if it doesn't exist. This enables simultaneous mutable
	/// access to elements and a single attribute column without borrowing conflicts.
	pub fn element_and_attribute_slices_mut<U: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: &str) -> (&mut [T], &mut [U]) {
		let Self { element, attributes } = self;
		let column_position = attributes.find_or_create_column::<U>(key);
		let column = (*attributes.columns[column_position].1).as_any_mut().downcast_mut::<Column<U>>().unwrap();
		(element.as_mut_slice(), &mut column.0)
	}

	// ==================
	// Row-level cloning
	// ==================

	/// Clones both the element and all attributes at the given row index into a new owned [`TableRow`], or `None` if out of bounds.
	pub fn clone_row(&self, index: usize) -> Option<TableRow<T>>
	where
		T: Clone,
	{
		Some(TableRow {
			element: self.element.get(index)?.clone(),
			attributes: self.attributes.clone_row(index),
		})
	}

	/// Clones all attribute values at the given row index into a new [`AttributeValues`], without cloning the element.
	pub fn clone_row_attributes(&self, index: usize) -> AttributeValues {
		self.attributes.clone_row(index)
	}
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for Table<T> {
	/// Serializes only the element vec, omitting type-erased attributes which are not serializable.
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		#[derive(serde::Serialize)]
		struct TableHelper<'a, T: serde::Serialize> {
			element: &'a Vec<T>,
		}

		TableHelper { element: &self.element }.serialize(serializer)
	}
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Table<T> {
	/// Deserializes the element vec and initializes an empty attribute column store with the matching row count.
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(serde::Deserialize)]
		struct TableHelper<T> {
			#[serde(alias = "instances", alias = "instance")]
			element: Vec<T>,
		}

		let helper = TableHelper::deserialize(deserializer)?;
		let len = helper.element.len();

		Ok(Table {
			element: helper.element,
			attributes: AttributeColumns::with_len(len),
		})
	}
}

impl<T: BoundingBox> BoundingBox for Table<T> {
	/// Computes the combined bounding box of all rows, composing each row's transform attribute with the given transform.
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let mut combined_bounds = None;

		for (element, row_transform) in self.iter_element_values().zip(self.iter_attribute_values_or_default::<DAffine2>("transform")) {
			match element.bounding_box(transform * row_transform, include_stroke) {
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
		let row_attributes = self.attributes.into_row_vec();
		TableRowIter {
			element: self.element.into_iter(),
			attributes: row_attributes.into_iter(),
		}
	}
}

impl<T> Default for Table<T> {
	fn default() -> Self {
		Self {
			element: Vec::new(),
			attributes: AttributeColumns::new(),
		}
	}
}

impl<T: graphene_hash::CacheHash> graphene_hash::CacheHash for Table<T> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		for element in self.iter_element_values() {
			element.cache_hash(state);
		}
		for transform in self.iter_attribute_values_or_default::<DAffine2>("transform") {
			graphene_hash::CacheHash::cache_hash(&transform, state);
		}
		for alpha_blending in self.iter_attribute_values_or_default::<crate::AlphaBlending>("alpha_blending") {
			alpha_blending.cache_hash(state);
		}
	}
}

impl<T: PartialEq> PartialEq for Table<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element == other.element
	}
}

impl<T> ApplyTransform for Table<T> {
	/// Right-multiplies the modification into each row's transform attribute.
	fn apply_transform(&mut self, modification: &DAffine2) {
		for transform in self.iter_attribute_values_mut_or_default::<DAffine2>("transform") {
			*transform *= *modification;
		}
	}

	/// Left-multiplies the modification into each row's transform attribute.
	fn left_apply_transform(&mut self, modification: &DAffine2) {
		for transform in self.iter_attribute_values_mut_or_default::<DAffine2>("transform") {
			*transform = *modification * *transform;
		}
	}
}

unsafe impl<T: StaticTypeSized> StaticType for Table<T> {
	type Static = Table<T::Static>;
}

impl<T> FromIterator<TableRow<T>> for Table<T> {
	/// Collects an iterator of [`TableRow`]s into a [`Table`], pre-allocating based on the iterator's size hint.
	fn from_iter<I: IntoIterator<Item = TableRow<T>>>(iter: I) -> Self {
		let iter = iter.into_iter();
		let (lower_bound, _) = iter.size_hint();
		let mut table = Self::with_capacity(lower_bound);

		for row in iter {
			table.push(row);
		}

		table
	}
}

// ===========
// TableRow<T>
// ===========

/// An owned row containing an element of type `T` and a set of type-erased scalar attributes.
///
/// Used to build rows before pushing them into a [`Table`], or when consuming rows out of a
/// table via [`IntoIterator`]. Attribute values use scalar [`AttributeValues`] storage rather
/// than the columnar layout inside a [`Table`].
#[derive(Clone, Debug)]
pub struct TableRow<T> {
	element: T,
	attributes: AttributeValues,
}

impl<T: Default> Default for TableRow<T> {
	fn default() -> Self {
		Self::new_from_element(T::default())
	}
}

impl<T: PartialEq> PartialEq for TableRow<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element == other.element
	}
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for TableRow<T> {
	/// Serializes only the element, omitting type-erased attributes which are not serializable.
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		#[derive(serde::Serialize)]
		struct TableRowHelper<'a, T: serde::Serialize> {
			element: &'a T,
		}

		TableRowHelper { element: &self.element }.serialize(serializer)
	}
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for TableRow<T> {
	/// Deserializes the element and initializes an empty set of attributes.
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(serde::Deserialize)]
		struct TableRowHelper<T> {
			#[serde(alias = "instance")]
			element: T,
		}

		let helper = TableRowHelper::deserialize(deserializer)?;
		Ok(TableRow::new_from_element(helper.element))
	}
}

impl<T> TableRow<T> {
	/// Constructs a row from a pre-built element and attributes pair.
	pub fn from_parts(element: T, attributes: AttributeValues) -> Self {
		Self { element, attributes }
	}

	/// Constructs a row with the given element and an empty set of attributes.
	pub fn new_from_element(element: T) -> Self {
		Self::from_parts(element, AttributeValues::new())
	}

	/// Returns a shared reference to this row's element.
	pub fn element(&self) -> &T {
		&self.element
	}

	/// Returns a mutable reference to this row's element.
	pub fn element_mut(&mut self) -> &mut T {
		&mut self.element
	}

	/// Consumes this row and returns the owned element, discarding attributes.
	pub fn into_element(self) -> T {
		self.element
	}

	/// Consumes this row and returns its element and attributes as separate owned values.
	pub fn into_parts(self) -> (T, AttributeValues) {
		(self.element, self.attributes)
	}

	/// Returns a shared reference to all attributes of this row.
	pub fn attributes(&self) -> &AttributeValues {
		&self.attributes
	}

	/// Returns a mutable reference to all attributes of this row.
	pub fn attributes_mut(&mut self) -> &mut AttributeValues {
		&mut self.attributes
	}

	/// Returns a reference to the attribute value for the given key, if it exists and is of the requested type.
	pub fn attribute<U: 'static>(&self, key: &str) -> Option<&U> {
		self.attributes.get(key)
	}

	/// Returns the attribute value for the given key, or the provided default if absent or of a different type.
	pub fn attribute_or<'a, U: 'static>(&'a self, key: &str, default: &'a U) -> &'a U {
		self.attribute(key).unwrap_or(default)
	}

	/// Returns a clone of the attribute value for the given key, or the provided default if absent or of a different type.
	pub fn attribute_cloned_or<U: Clone + 'static>(&self, key: &str, default: U) -> U {
		self.attribute(key).cloned().unwrap_or(default)
	}

	/// Returns a clone of the attribute value for the given key, or `U`'s default value if absent or of a different type.
	pub fn attribute_cloned_or_default<U: Clone + Default + 'static>(&self, key: &str) -> U {
		self.attribute(key).cloned().unwrap_or_default()
	}

	/// Returns a mutable reference to the attribute value for the given key, if it exists and is of the requested type.
	pub fn attribute_mut<U: 'static>(&mut self, key: &str) -> Option<&mut U> {
		self.attributes.get_mut(key)
	}

	/// Returns a mutable reference to the attribute value for the given key, inserting a default value if absent or of a different type.
	pub fn attribute_mut_or_insert_default<U: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: &str) -> &mut U {
		self.attributes.get_or_insert_default_mut(key)
	}

	/// Sets the attribute value for the given key, replacing any existing entry with the same key.
	pub fn set_attribute<U: Clone + Send + Sync + Default + Debug + 'static>(&mut self, key: impl Into<String>, value: U) {
		self.attributes.insert(key, value);
	}

	/// Sets the attribute value for the given key and returns the row, enabling builder-style chaining.
	pub fn with_attribute<U: Clone + Send + Sync + Default + Debug + 'static>(mut self, key: impl Into<String>, value: U) -> Self {
		self.set_attribute(key, value);
		self
	}

	/// Removes and returns the attribute value for the given key, if it exists and is of the requested type.
	pub fn remove_attribute<U: 'static>(&mut self, key: &str) -> Option<U> {
		self.attributes.remove(key)
	}
}

// ===============
// TableRowIter<T>
// ===============

/// Owning iterator over the rows of a consumed [`Table`], yielding [`TableRow`]s.
///
/// Created by [`Table::into_iter`]. The table's columnar attributes are converted into
/// per-row scalar [`AttributeValues`] during construction so each yielded row is self-contained.
pub struct TableRowIter<T> {
	element: std::vec::IntoIter<T>,
	attributes: std::vec::IntoIter<AttributeValues>,
}

impl<T> Iterator for TableRowIter<T> {
	type Item = TableRow<T>;

	fn next(&mut self) -> Option<Self::Item> {
		Some(TableRow {
			element: self.element.next()?,
			attributes: self.attributes.next()?,
		})
	}
}

impl<T> DoubleEndedIterator for TableRowIter<T> {
	fn next_back(&mut self) -> Option<Self::Item> {
		Some(TableRow {
			element: self.element.next_back()?,
			attributes: self.attributes.next_back()?,
		})
	}
}
