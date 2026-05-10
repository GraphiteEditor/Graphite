use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::math::quad::Quad;
use crate::transform::ApplyTransform;
use dyn_any::{StaticType, StaticTypeSized};
use glam::DAffine2;
use graphene_hash::CacheHash;
use std::fmt::Debug;

// =================================================
// Standard attribute keys used across the data flow
// =================================================

/// Item's `DAffine2` transformation, composed multiplicatively through nested groups.
pub const ATTR_TRANSFORM: &str = "transform";

/// Item's `BlendMode`, controlling how it composites with content beneath it.
pub const ATTR_BLEND_MODE: &str = "blend_mode";

/// Item's opacity multiplier (`f64`, implicit default `1.`).
/// Composed multiplicatively through nested groups. Affects content clipped to the item.
pub const ATTR_OPACITY: &str = "opacity";

/// Item's fill opacity multiplier (`f64`, implicit default `1.`).
/// Like opacity but does not affect content clipped to the item.
pub const ATTR_OPACITY_FILL: &str = "opacity_fill";

/// `bool` for whether an item inherits the alpha of the content beneath it (clipping mask).
pub const ATTR_CLIPPING_MASK: &str = "clipping_mask";

/// `List<NodeId>` path from the root network to the layer node owning this item.
/// Used by editor tools to route clicks/selection back to the originating layer.
pub const ATTR_EDITOR_LAYER_PATH: &str = "editor:layer_path";

/// `List<Graphic>` snapshot of the upstream content that fed into a destructive merge
/// (Boolean Operation, Rasterize, etc.), so the editor can still surface click targets for
/// the original child layers after their content has been collapsed.
pub const ATTR_EDITOR_MERGED_LAYERS: &str = "editor:merged_layers";

/// Optional `Vector` that overrides the item's own geometry for click-target generation.
/// Used by the 'Text' node for per-glyph bounding-box rectangles so glyphs are selectable
/// by clicking anywhere within their bounds, not just the filled letterform.
pub const ATTR_EDITOR_CLICK_TARGET: &str = "editor:click_target";

/// `DAffine2` mapping the unit square `[(0, 0), (1, 1)]` (top-left convention) onto the 'Text'
/// node's text frame in this item's local space. Each item carries the frame relative to its own
/// glyph origin so it survives `Index Elements` filtering. The Text tool reads this to position
/// its drag cage. Stored as an affine to allow non-axis-aligned frames in the future.
pub const ATTR_EDITOR_TEXT_FRAME: &str = "editor:text_frame";

/// `u64` byte offset where a regex match begins ('Regex Find All', 'Regex Capture' text nodes).
pub const ATTR_START: &str = "start";

/// `u64` byte offset where a regex match ends ('Regex Find All', 'Regex Capture' text nodes).
pub const ATTR_END: &str = "end";

/// `String` for a regex named-capture-group's name, or empty for unnamed groups ('Regex Capture' text node).
pub const ATTR_NAME: &str = "name";

/// `String` for a JSON value's type (`"string"`, `"number"`, `"object"`, etc.) from 'JSON Query All'.
pub const ATTR_TYPE: &str = "type";

/// Artboard's `DVec2` top-left corner in document coordinates.
pub const ATTR_LOCATION: &str = "location";

/// Artboard's `DVec2` width and height.
pub const ATTR_DIMENSIONS: &str = "dimensions";

/// Artboard's `Color` background fill.
pub const ATTR_BACKGROUND: &str = "background";

/// `bool` for whether an artboard clips content to its bounds.
pub const ATTR_CLIP: &str = "clip";

/// Gradient's `GradientSpreadMethod` (`Pad`, `Reflect`, or `Repeat`).
pub const ATTR_SPREAD_METHOD: &str = "spread_method";

/// Gradient's `GradientType` (`Linear` or `Radial`).
pub const ATTR_GRADIENT_TYPE: &str = "gradient_type";

// ========================
// TRAIT: AnyAttributeValue
// ========================

/// Enables type-erased scalar storage that supports Clone, Send, Sync, and downcasting.
/// Used for individual attribute values in an [`Item`].
pub trait AnyAttributeValue: std::any::Any + Send + Sync {
	/// Clones this value into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AnyAttributeValue>;

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any;

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

	/// Consumes the box and returns the underlying concrete type for downcasting.
	fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;

	/// Returns a debug-formatted string representation of this value.
	fn display_string(&self) -> String;

	/// Wraps this scalar value into a new attribute with `preceding_defaults` default values before this value.
	fn into_attribute(self: Box<Self>, preceding_defaults: usize) -> Box<dyn AnyAttribute>;
}

impl<T: Clone + Send + Sync + Default + Sized + Debug + PartialEq + CacheHash + 'static> AnyAttributeValue for T {
	/// Clones this value into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AnyAttributeValue> {
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

	/// Wraps this scalar value into a new attribute, padded with `preceding_defaults` default values before it.
	fn into_attribute(self: Box<Self>, preceding_defaults: usize) -> Box<dyn AnyAttribute> {
		let mut data = vec![T::default(); preceding_defaults];
		data.push(*self);
		Box::new(Attribute(data))
	}
}

impl Clone for Box<dyn AnyAttributeValue> {
	fn clone(&self) -> Self {
		(**self).clone_box()
	}
}

// ===================
// TRAIT: AnyAttribute
// ===================

/// Enables type-erased storage for parallel attribute lists in a [`List`].
pub trait AnyAttribute: std::any::Any + Send + Sync {
	/// Clones this attribute into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AnyAttribute>;

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any;

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

	/// Pushes a scalar attribute value onto the end of this attribute.
	fn push(&mut self, value: Box<dyn AnyAttributeValue>);

	/// Pushes a default value onto the end of this attribute.
	fn push_default(&mut self);

	/// Sets the value at the given index, padding with defaults if the attribute is shorter than `index`.
	/// Falls back to a default if the value's type doesn't match.
	fn set_at(&mut self, index: usize, value: Box<dyn AnyAttributeValue>);

	/// Creates a new attribute of the same type filled with `count` number of default values.
	fn new_with_defaults(&self, count: usize) -> Box<dyn AnyAttribute>;

	/// Returns the number of elements in this attribute.
	fn len(&self) -> usize;

	/// Returns whether this attribute has any elements.
	fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Appends all values from another attribute of the same type.
	fn extend(&mut self, other: Box<dyn AnyAttribute>);

	/// Returns a shared reference to the value at the requested index.
	fn get_any(&self, index: usize) -> Option<&dyn std::any::Any>;

	/// Returns a debug-formatted display string for the value at the requested index.
	fn display_at(&self, index: usize) -> Option<String>;

	/// Clones a single value from this attribute into a boxed scalar attribute value.
	fn clone_value(&self, index: usize) -> Option<Box<dyn AnyAttributeValue>>;

	/// Drains all values out of this attribute into a Vec of scalar attribute values.
	fn drain(self: Box<Self>) -> Vec<Box<dyn AnyAttributeValue>>;

	/// Hashes every value in this attribute into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher);

	/// Compares this attribute to another for value-by-value equality (object-safe wrapper around `PartialEq`).
	/// Returns `false` if the underlying types differ.
	fn eq_dyn(&self, other: &dyn AnyAttribute) -> bool;
}

/// Adapts a `&mut dyn Hasher` so generic `CacheHash::cache_hash<H>` calls (which require `H: Sized + Hasher`) can
/// drive a trait-object hasher. Forwards both `Hasher` methods to the inner `dyn Hasher`.
struct DynHasher<'a>(&'a mut dyn core::hash::Hasher);
impl core::hash::Hasher for DynHasher<'_> {
	fn finish(&self) -> u64 {
		self.0.finish()
	}
	fn write(&mut self, bytes: &[u8]) {
		self.0.write(bytes)
	}
}

impl Clone for Box<dyn AnyAttribute> {
	fn clone(&self) -> Self {
		(**self).clone_box()
	}
}

// ============
// Attribute<T>
// ============

/// Wraps a Vec<T> for attribute storage in a [`List`].
pub struct Attribute<T>(pub Vec<T>);

impl<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static> AnyAttribute for Attribute<T> {
	/// Clones this attribute into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AnyAttribute> {
		Box::new(Attribute(self.0.clone()))
	}

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
		self
	}

	/// Pushes an attribute value onto the end of this attribute's list, downcasting it to `T`.
	/// Falls back to a default value if the type doesn't match, to maintain the attribute-length invariant.
	fn push(&mut self, value: Box<dyn AnyAttributeValue>) {
		if let Ok(value) = value.into_any().downcast::<T>() {
			self.0.push(*value);
		} else {
			self.0.push(T::default());
		}
	}

	/// Pushes a default `T` value onto the end of this attribute list.
	fn push_default(&mut self) {
		self.0.push(T::default());
	}

	/// Sets the value at the given index, padding with defaults if the attribute is shorter than `index`.
	/// Falls back to a default if the value's type doesn't match.
	fn set_at(&mut self, index: usize, value: Box<dyn AnyAttributeValue>) {
		while self.0.len() < index {
			self.0.push(T::default());
		}
		let value = value.into_any().downcast::<T>().map(|v| *v).unwrap_or_default();
		if self.0.len() == index {
			self.0.push(value);
		} else {
			self.0[index] = value;
		}
	}

	/// Creates a new attribute filled with `count` default `T` values.
	fn new_with_defaults(&self, count: usize) -> Box<dyn AnyAttribute> {
		Box::new(Attribute(vec![T::default(); count]))
	}

	/// Returns the number of elements in this attribute.
	fn len(&self) -> usize {
		self.0.len()
	}

	/// Appends all values from another attribute, downcasting it to the same `Attribute<T>` type.
	/// Falls back to padding with defaults if the type doesn't match, to maintain the attribute-length invariant.
	fn extend(&mut self, other: Box<dyn AnyAttribute>) {
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
	fn clone_value(&self, index: usize) -> Option<Box<dyn AnyAttributeValue>> {
		self.0.get(index).map(|v| Box::new(v.clone()) as Box<dyn AnyAttributeValue>)
	}

	/// Consumes this attribute and returns all values as a Vec of boxed scalar attribute values.
	fn drain(self: Box<Self>) -> Vec<Box<dyn AnyAttributeValue>> {
		self.0.into_iter().map(|v| Box::new(v) as Box<dyn AnyAttributeValue>).collect()
	}

	/// Hashes every value in this attribute into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher) {
		self.0.cache_hash(&mut DynHasher(state));
	}

	/// Compares this attribute to another for value-by-value equality (object-safe wrapper around `PartialEq`).
	fn eq_dyn(&self, other: &dyn AnyAttribute) -> bool {
		other.as_any().downcast_ref::<Self>().is_some_and(|other| self.0 == other.0)
	}
}

// ============
// AttributeDyn
// ============

/// Type-erased list of attribute values, used as a node graph parameter type.
/// Lets a node accept any `List<U>` source via the auto-inserted `Convert<AttributeDyn, ()>`
/// without monomorphizing over `U` (so the cartesian product of `(content T, source U)` collapses to just `T`).
pub struct AttributeDyn(pub Box<dyn AnyAttribute>);

impl AttributeDyn {
	/// Number of values in this attribute.
	pub fn len(&self) -> usize {
		self.0.len()
	}

	/// Whether this attribute has zero values.
	pub fn is_empty(&self) -> bool {
		self.0.len() == 0
	}

	/// Builds a new attribute matching `target_len` items, taking values from this attribute (wrapping if shorter, truncating if longer).
	pub fn cloned_to_length(&self, target_len: usize) -> Box<dyn AnyAttribute> {
		let mut result = self.0.new_with_defaults(0);
		let source_len = self.0.len();
		if source_len == 0 {
			for _ in 0..target_len {
				result.push_default();
			}
			return result;
		}
		for i in 0..target_len {
			let value = self.0.clone_value(i % source_len).expect("source_len > 0");
			result.push(value);
		}
		result
	}
}

impl Clone for AttributeDyn {
	fn clone(&self) -> Self {
		Self(self.0.clone_box())
	}
}

impl Default for AttributeDyn {
	fn default() -> Self {
		Self(Box::new(Attribute::<bool>(Vec::new())))
	}
}

impl Debug for AttributeDyn {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "AttributeDyn(len: {})", self.0.len())
	}
}

impl PartialEq for AttributeDyn {
	fn eq(&self, other: &Self) -> bool {
		self.0.eq_dyn(&*other.0)
	}
}

impl CacheHash for AttributeDyn {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.0.cache_hash_dyn(state);
	}
}

unsafe impl StaticType for AttributeDyn {
	type Static = Self;
}

// ==================
// AttributeValueDyn
// ==================

/// Type-erased single attribute value, used as a node graph parameter type.
/// Lets a node accept a value of any concrete type via the auto-inserted `Convert<AttributeValueDyn, ()>`
/// without monomorphizing over the value type.
pub struct AttributeValueDyn(pub Box<dyn AnyAttributeValue>);

impl Clone for AttributeValueDyn {
	fn clone(&self) -> Self {
		Self(self.0.clone_box())
	}
}

impl Default for AttributeValueDyn {
	fn default() -> Self {
		Self(Box::new(false))
	}
}

impl Debug for AttributeValueDyn {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "AttributeValueDyn({})", self.0.display_string())
	}
}

impl PartialEq for AttributeValueDyn {
	fn eq(&self, other: &Self) -> bool {
		self.0.display_string() == other.0.display_string()
	}
}

impl CacheHash for AttributeValueDyn {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.0.display_string().cache_hash(state);
	}
}

unsafe impl StaticType for AttributeValueDyn {
	type Static = Self;
}

// =======
// ListDyn
// =======

/// Type-erased view of a `List<T>` exposing only its attributes and item count, used as a node graph parameter type.
/// Lets a node accept any `List<U>` source via the auto-inserted `Convert<ListDyn, ()>` without monomorphizing over `U`,
/// for cases where the element type is irrelevant (such as nodes that read out a named attribute regardless of the carrier `List`).
#[derive(Default)]
pub struct ListDyn {
	attributes: Vec<(String, Box<dyn AnyAttribute>)>,
	len: usize,
}

impl ListDyn {
	/// Number of items in the underlying `List`.
	pub fn len(&self) -> usize {
		self.len
	}

	/// Whether the underlying `List` has zero items.
	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Returns a reference to the attribute value at the given key and item index, downcast to `U`, if present and matching.
	pub fn attribute<U: 'static>(&self, key: &str, index: usize) -> Option<&U> {
		self.attributes
			.iter()
			.find_map(|(k, attribute)| if k == key { attribute.get_any(index)?.downcast_ref::<U>() } else { None })
	}
}

impl<T> From<List<T>> for ListDyn {
	fn from(list: List<T>) -> Self {
		Self {
			attributes: list.attributes.attributes,
			len: list.attributes.len,
		}
	}
}

impl Clone for ListDyn {
	fn clone(&self) -> Self {
		Self {
			attributes: self.attributes.iter().map(|(key, attribute)| (key.clone(), attribute.clone_box())).collect(),
			len: self.len,
		}
	}
}

impl Debug for ListDyn {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.attributes.iter().map(|(k, _)| k.as_str()).collect();
		f.debug_struct("ListDyn").field("keys", &keys).field("len", &self.len).finish()
	}
}

impl PartialEq for ListDyn {
	fn eq(&self, other: &Self) -> bool {
		self.len == other.len
			&& self.attributes.len() == other.attributes.len()
			&& self
				.attributes
				.iter()
				.zip(&other.attributes)
				.all(|((key_a, attribute_a), (key_b, attribute_b))| key_a == key_b && attribute_a.eq_dyn(&**attribute_b))
	}
}

impl CacheHash for ListDyn {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.len.cache_hash(state);
		for (key, attribute) in &self.attributes {
			key.cache_hash(state);
			attribute.cache_hash_dyn(state);
		}
	}
}

unsafe impl StaticType for ListDyn {
	type Static = Self;
}

// ===================
// ItemAttributeValues
// ===================

/// Scalar attribute storage for a single item.
///
/// A small ordered map of type-erased scalar attribute values, keyed by string name.
/// Used for individual attribute values in an [`Item`].
/// Linear search preserves insertion order and is likely faster than a HashMap for small attribute counts.
#[derive(Clone, Default)]
pub struct ItemAttributeValues(Vec<(String, Box<dyn AnyAttributeValue>)>);

impl Debug for ItemAttributeValues {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.0.iter().map(|(k, _)| k.as_str()).collect();
		f.debug_struct("Attributes").field("keys", &keys).finish()
	}
}

impl ItemAttributeValues {
	/// Creates an empty set of attributes.
	pub fn new() -> Self {
		Self::default()
	}

	/// Inserts an attribute with the given key and value, replacing any existing entry with the same key.
	pub fn insert<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl Into<String>, value: T) {
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
	pub fn get_or_insert_default_mut<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> &mut T {
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

// ==========
// Attributes
// ==========

/// The storage data structure for attributes.
///
/// A collection of type-erased parallel attributes, keyed by string name.
/// All access goes through [`List`] and [`Item`] since internals are private.
/// Invariant: every attribute in `attributes` has exactly `len` elements.
#[derive(Clone, Default)]
struct Attributes {
	attributes: Vec<(String, Box<dyn AnyAttribute>)>,
	len: usize,
}

impl Debug for Attributes {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.attributes.iter().map(|(k, _)| k.as_str()).collect();
		f.debug_struct("Attributes").field("keys", &keys).field("len", &self.len).finish()
	}
}

impl Attributes {
	/// Creates an empty attribute store with no attributes and zero length.
	fn new() -> Self {
		Self::default()
	}

	/// Creates an empty attribute store with no attributes but a pre-set item count.
	fn with_len(len: usize) -> Self {
		Self { attributes: Vec::new(), len }
	}

	/// Pushes an item's scalar attributes into this attribute store.
	/// Existing attributes that the item lacks receive a default value.
	/// New attribute keys create a new attribute padded with defaults for all prior items.
	fn push_item(&mut self, item: ItemAttributeValues) {
		let mut item_entries = item.0;

		// Push values into existing attributes, or a default if the item lacks that attribute
		for (attribute_key, attribute) in &mut self.attributes {
			if let Some(position) = item_entries.iter().position(|(k, _)| k == attribute_key) {
				let (_, value) = item_entries.swap_remove(position);
				attribute.push(value);
			} else {
				attribute.push_default();
			}
		}

		// Create new attributes for any remaining item values, padded with defaults for prior items
		for (key, value) in item_entries {
			self.attributes.push((key, value.into_attribute(self.len)));
		}

		self.len += 1;
	}

	/// Appends all attribute data from another attribute store into this one.
	/// Attributes present in only one side are padded with defaults for the other side's items.
	fn extend(&mut self, other: Attributes) {
		let other_len = other.len;
		let mut other_entries = other.attributes;

		// Extend matching attributes, or pad self's attributes with defaults for the other's item count
		for (key, self_attribute) in &mut self.attributes {
			if let Some(position) = other_entries.iter().position(|(k, _)| k == key) {
				let (_, other_attribute) = other_entries.swap_remove(position);
				self_attribute.extend(other_attribute);
			} else {
				for _ in 0..other_len {
					self_attribute.push_default();
				}
			}
		}

		// Remaining other attributes are new, so we pad with defaults for self's existing items
		for (key, other_attribute) in other_entries {
			let mut combined = other_attribute.new_with_defaults(self.len);
			combined.extend(other_attribute);
			self.attributes.push((key, combined));
		}

		self.len += other_len;
	}

	/// Gets a reference to the value at the given index from the attribute for the given key.
	fn get_value<T: 'static>(&self, key: &str, index: usize) -> Option<&T> {
		self.attributes
			.iter()
			.find_map(|(k, attribute)| if k == key { attribute.get_any(index)?.downcast_ref::<T>() } else { None })
	}

	/// Removes the entire attribute for the given key, if present.
	fn remove_attribute(&mut self, key: &str) {
		if let Some(position) = self.attributes.iter().position(|(k, _)| k == key) {
			self.attributes.remove(position);
		}
	}

	/// Finds or creates an attribute for the given key and type, returning its position.
	/// If an attribute with the key exists but has the wrong type, it is removed and replaced with a new attribute of the correct type, padded with defaults.
	/// A newly created attribute is filled with `T::default()` for all existing items.
	fn find_or_create_attribute<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> usize {
		match self.attributes.iter().position(|(k, _)| k == key) {
			Some(position) => {
				if (*self.attributes[position].1).as_any().downcast_ref::<Attribute<T>>().is_some() {
					position
				} else {
					self.attributes.remove(position);
					self.attributes.push((key.to_string(), Box::new(Attribute::<T>(vec![T::default(); self.len]))));
					self.attributes.len() - 1
				}
			}
			None => {
				self.attributes.push((key.to_string(), Box::new(Attribute::<T>(vec![T::default(); self.len]))));
				self.attributes.len() - 1
			}
		}
	}

	/// Gets a mutable reference to the value at the given index, creating the attribute if it doesn't exist or has the wrong type.
	fn get_or_insert_default_value<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str, index: usize) -> &mut T {
		let attribute_position = self.find_or_create_attribute::<T>(key);
		let attribute = (*self.attributes[attribute_position].1).as_any_mut().downcast_mut::<Attribute<T>>().unwrap();
		&mut attribute.0[index]
	}

	/// Sets the value at the given index in the attribute for the given key.
	/// Creates the attribute with defaults if it doesn't exist.
	fn set_value<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl Into<String>, index: usize, value: T) {
		let key = key.into();
		let attribute_position = self.find_or_create_attribute::<T>(&key);
		let attribute = (*self.attributes[attribute_position].1).as_any_mut().downcast_mut::<Attribute<T>>().unwrap();
		attribute.0[index] = value;
	}

	/// Returns a debug-formatted string for the value at the given index in the attribute for the given key.
	fn display_value(&self, key: &str, index: usize, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.attributes.iter().find_map(|(k, attribute)| {
			if k == key {
				if let Some(value) = attribute.get_any(index)
					&& let Some(text) = overrides(value)
				{
					return Some(text);
				}
				attribute.display_at(index)
			} else {
				None
			}
		})
	}

	/// Returns a type-erased reference to the value at the given index in the attribute for the given key.
	fn get_any_value(&self, key: &str, index: usize) -> Option<&dyn std::any::Any> {
		self.attributes.iter().find_map(|(k, attribute)| if k == key { attribute.get_any(index) } else { None })
	}

	/// Returns an iterator over the keys of all stored attributes (in insertion order).
	fn keys(&self) -> impl Iterator<Item = &str> {
		self.attributes.iter().map(|(key, _)| key.as_str())
	}

	/// Returns a typed slice of the attribute for the given key, if it exists and can be downcast to `Attribute<T>`.
	fn get_attribute_slice<T: 'static>(&self, key: &str) -> Option<&[T]> {
		self.attributes.iter().find_map(|(k, attribute)| {
			if k == key {
				attribute.as_any().downcast_ref::<Attribute<T>>().map(|c| c.0.as_slice())
			} else {
				None
			}
		})
	}

	/// Returns a mutable typed slice of the attribute for the given key, if it exists and can be downcast to `Attribute<T>`.
	fn get_attribute_slice_mut<T: 'static>(&mut self, key: &str) -> Option<&mut [T]> {
		self.attributes.iter_mut().find_map(|(k, attribute)| {
			if k == key {
				attribute.as_any_mut().downcast_mut::<Attribute<T>>().map(|c| c.0.as_mut_slice())
			} else {
				None
			}
		})
	}

	/// Returns a mutable typed slice of the attribute for the given key, creating a new attribute filled with defaults if it doesn't exist.
	fn get_or_create_attribute_slice_mut<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> &mut [T] {
		let position = self.find_or_create_attribute::<T>(key);
		let attribute = (*self.attributes[position].1).as_any_mut().downcast_mut::<Attribute<T>>().unwrap();
		&mut attribute.0
	}

	/// Clones all attribute values at the given item index into a new scalar Attributes.
	fn clone_item(&self, index: usize) -> ItemAttributeValues {
		let mut attributes = ItemAttributeValues::new();

		for (key, attribute) in &self.attributes {
			if let Some(value) = attribute.clone_value(index) {
				attributes.0.push((key.clone(), value));
			}
		}

		attributes
	}

	/// Drains all attribute data into a Vec of per-item scalar Attributes.
	fn into_item_vec(self) -> Vec<ItemAttributeValues> {
		let mut items: Vec<ItemAttributeValues> = (0..self.len).map(|_| ItemAttributeValues::new()).collect();

		for (key, attribute) in self.attributes {
			for (i, value) in attribute.drain().into_iter().enumerate() {
				items[i].0.push((key.clone(), value));
			}
		}

		items
	}
}

// =======
// List<T>
// =======

/// A struct-of-arrays collection where each item holds an element of type `T` alongside
/// a set of type-erased, dynamically-typed attributes stored in parallel attributes.
///
/// Elements are stored contiguously in a `Vec<T>`, while attributes live in an internal
/// [`Attributes`] store that keeps one attribute per attribute key. Items are accessed by
/// index through element/attribute accessor methods, or consumed as owned [`Item`]s via iteration.
#[derive(Clone, Debug)]
pub struct List<T> {
	element: Vec<T>,
	attributes: Attributes,
}

impl<T> List<T> {
	/// Creates an empty list with no items.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates an empty list with pre-allocated capacity for the given number of items.
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			element: Vec::with_capacity(capacity),
			attributes: Attributes::new(),
		}
	}

	/// Creates a list containing a single item with the given element and no attributes.
	pub fn new_from_element(element: T) -> Self {
		Self {
			element: vec![element],
			attributes: Attributes::with_len(1),
		}
	}

	/// Creates a list containing a single item from the given [`Item`], preserving its attributes.
	pub fn new_from_item(item: Item<T>) -> Self {
		let mut attributes = Attributes::new();
		attributes.push_item(item.attributes);
		Self {
			element: vec![item.element],
			attributes,
		}
	}

	/// Appends an item to the end of this list.
	pub fn push(&mut self, item: Item<T>) {
		self.element.push(item.element);
		self.attributes.push_item(item.attributes);
	}

	/// Appends all items from another list into this one.
	pub fn extend(&mut self, list: List<T>) {
		self.element.extend(list.element);
		self.attributes.extend(list.attributes);
	}

	/// Returns the number of items in this list.
	pub fn len(&self) -> usize {
		self.element.len()
	}

	/// Returns `true` if this list contains no items.
	pub fn is_empty(&self) -> bool {
		self.element.is_empty()
	}

	/// Returns an iterator over all attribute keys in this list, in insertion order.
	pub fn attribute_keys(&self) -> impl Iterator<Item = &str> {
		self.attributes.keys()
	}

	// ============================
	// Attribute-oriented iteration
	// ============================

	/// Returns an iterator over shared references to all element values.
	pub fn iter_element_values(&self) -> std::slice::Iter<'_, T> {
		self.element.iter()
	}

	/// Returns an iterator over mutable references to all element values.
	pub fn iter_element_values_mut(&mut self) -> std::slice::IterMut<'_, T> {
		self.element.iter_mut()
	}

	/// Returns an iterator over shared references to the values of a typed attribute, or `None` if the attribute doesn't exist or has the wrong type.
	pub fn iter_attribute_values<U: 'static>(&self, key: &str) -> Option<std::slice::Iter<'_, U>> {
		self.attributes.get_attribute_slice::<U>(key).map(|s| s.iter())
	}

	/// Returns an iterator over mutable references to the values of a typed attribute attribute, or `None` if the attribute doesn't exist or has the wrong type.
	pub fn iter_attribute_values_mut<U: 'static>(&mut self, key: &str) -> Option<std::slice::IterMut<'_, U>> {
		self.attributes.get_attribute_slice_mut::<U>(key).map(|s| s.iter_mut())
	}

	/// Returns an iterator that yields cloned attribute values for the given key, falling back to `U::default()` for each item if the attribute is missing or has the wrong type.
	pub fn iter_attribute_values_or_default<U: Clone + Default + 'static>(&self, key: &str) -> impl Iterator<Item = U> + '_ {
		let slice = self.attributes.get_attribute_slice::<U>(key);
		let len = self.element.len();
		(0..len).map(move |i| slice.map_or_else(U::default, |s| s[i].clone()))
	}

	/// Returns a mutable iterator over a typed attribute, creating the attribute with default values if it doesn't exist or has the wrong type.
	pub fn iter_attribute_values_mut_or_default<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> std::slice::IterMut<'_, U> {
		self.attributes.get_or_create_attribute_slice_mut::<U>(key).iter_mut()
	}

	// ======================
	// Indexed element access
	// ======================

	/// Returns a shared reference to the element at the given index, or `None` if out of bounds.
	pub fn element(&self, index: usize) -> Option<&T> {
		self.element.get(index)
	}

	/// Returns a mutable reference to the element at the given index, or `None` if out of bounds.
	pub fn element_mut(&mut self, index: usize) -> Option<&mut T> {
		self.element.get_mut(index)
	}

	// ========================
	// Indexed attribute access
	// ========================

	/// Returns a shared reference to the attribute value at the given item index and key, if it exists and can be downcast to the requested type.
	pub fn attribute<U: 'static>(&self, key: &str, index: usize) -> Option<&U> {
		self.attributes.get_value(key, index)
	}

	/// Returns a clone of the attribute value at the given item index and key, or `U::default()` if absent or of a different type.
	pub fn attribute_cloned_or_default<U: Clone + Default + 'static>(&self, key: &str, index: usize) -> U {
		self.attributes.get_value::<U>(key, index).cloned().unwrap_or_default()
	}

	/// Returns a clone of the attribute value at the given item index and key, or the provided default if absent or of a different type.
	pub fn attribute_cloned_or<U: Clone + 'static>(&self, key: &str, index: usize, default: U) -> U {
		self.attributes.get_value::<U>(key, index).cloned().unwrap_or(default)
	}

	/// Sets the attribute value at the given item index and key, creating the attribute with defaults if it doesn't exist.
	pub fn set_attribute<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl Into<String>, index: usize, value: U) {
		self.attributes.set_value(key, index, value);
	}

	/// Replaces (or adds) an attribute from a type-erased source. The source is wrapped or truncated to match this list's item count.
	pub fn set_attribute_dyn(&mut self, key: impl Into<String>, source: AttributeDyn) {
		let key = key.into();
		self.attributes.attributes.retain(|(k, _)| k != &key);
		let new_attribute = source.cloned_to_length(self.element.len());
		self.attributes.attributes.push((key, new_attribute));
	}

	/// Sets a single type-erased attribute value at the given index, creating the attribute from the value's underlying type if it doesn't exist (padded with defaults to match the list's length).
	/// Falls back to default if the value's type doesn't match an existing attribute.
	pub fn set_attribute_value_dyn(&mut self, key: impl Into<String>, index: usize, value: AttributeValueDyn) {
		let key = key.into();
		if let Some(position) = self.attributes.attributes.iter().position(|(k, _)| k == &key) {
			self.attributes.attributes[position].1.set_at(index, value.0);
		} else {
			let mut new_attribute = value.0.into_attribute(index);
			while new_attribute.len() < self.element.len() {
				new_attribute.push_default();
			}
			self.attributes.attributes.push((key, new_attribute));
		}
	}

	/// Removes the entire attribute for the given key, if present.
	pub fn remove_attribute(&mut self, key: &str) {
		self.attributes.remove_attribute(key);
	}

	/// Runs the given closure on a mutable reference to the attribute value at the given item index,
	/// creating the attribute with defaults if it doesn't exist, and returns the closure's result.
	pub fn with_attribute_mut_or_default<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static, R, F: FnOnce(&mut U) -> R>(&mut self, key: &str, index: usize, f: F) -> R {
		f(self.attributes.get_or_insert_default_value::<U>(key, index))
	}

	/// Returns a debug-formatted display string for the attribute at the given item index and key.
	pub fn attribute_display_value(&self, key: &str, index: usize, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.attributes.display_value(key, index, overrides)
	}

	/// Returns a type-erased reference to the attribute value at the given item index and key, or `None` if absent.
	pub fn attribute_any(&self, key: &str, index: usize) -> Option<&dyn std::any::Any> {
		self.attributes.get_any_value(key, index)
	}

	// ====================
	// Split borrow helpers
	// ====================

	/// Returns disjoint mutable references to the element slice and a typed attribute slice, creating the attribute with defaults if it doesn't exist.
	/// This enables simultaneous mutable access to elements and a single attribute without borrowing conflicts.
	pub fn element_and_attribute_slices_mut<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> (&mut [T], &mut [U]) {
		let Self { element, attributes } = self;
		let attribute_position = attributes.find_or_create_attribute::<U>(key);
		let attribute = (*attributes.attributes[attribute_position].1).as_any_mut().downcast_mut::<Attribute<U>>().unwrap();
		(element.as_mut_slice(), &mut attribute.0)
	}

	// ==================
	// Item-level cloning
	// ==================

	/// Clones both the element and all attributes at the given item index into a new owned [`Item`], or [`None`] if out of bounds.
	pub fn clone_item(&self, index: usize) -> Option<Item<T>>
	where
		T: Clone,
	{
		Some(Item {
			element: self.element.get(index)?.clone(),
			attributes: self.attributes.clone_item(index),
		})
	}

	/// Clones all attribute values at the given item index into a new [`ItemAttributeValues`], without cloning the element.
	pub fn clone_item_attributes(&self, index: usize) -> ItemAttributeValues {
		self.attributes.clone_item(index)
	}
}

impl<T: BoundingBox> BoundingBox for List<T> {
	/// Computes the combined bounding box of all items, composing each item's transform attribute with the given transform.
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let mut combined_bounds = None;

		for (element, item_transform) in self.iter_element_values().zip(self.iter_attribute_values_or_default::<DAffine2>(ATTR_TRANSFORM)) {
			match element.bounding_box(transform * item_transform, include_stroke) {
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
		// `Infinite` items are skipped here (rather than propagating outward as in `bounding_box`) so a finite sibling in a mixed group dictates the framing
		let mut combined_bounds = None;
		let mut any_infinite = false;

		for (element, item_transform) in self.iter_element_values().zip(self.iter_attribute_values_or_default::<DAffine2>(ATTR_TRANSFORM)) {
			match element.thumbnail_bounding_box(transform * item_transform, include_stroke) {
				RenderBoundingBox::None => continue,
				RenderBoundingBox::Infinite => any_infinite = true,
				RenderBoundingBox::Rectangle(bounds) => match combined_bounds {
					Some(existing) => combined_bounds = Some(Quad::combine_bounds(existing, bounds)),
					None => combined_bounds = Some(bounds),
				},
			}
		}

		match (combined_bounds, any_infinite) {
			(Some(bounds), _) => RenderBoundingBox::Rectangle(bounds),
			(None, true) => RenderBoundingBox::Infinite,
			(None, false) => RenderBoundingBox::None,
		}
	}
}

impl<T> IntoIterator for List<T> {
	type Item = Item<T>;
	type IntoIter = ItemIter<T>;

	/// Consumes a [`List`] and returns an iterator of [`Item`]s, each containing the owned data of the respective item from the original list.
	fn into_iter(self) -> Self::IntoIter {
		let attributes = self.attributes.into_item_vec();
		ItemIter {
			element: self.element.into_iter(),
			attributes: attributes.into_iter(),
		}
	}
}

impl<T> Default for List<T> {
	fn default() -> Self {
		Self {
			element: Vec::new(),
			attributes: Attributes::new(),
		}
	}
}

impl<T: CacheHash> CacheHash for List<T> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.element.cache_hash(state);

		// Hash every attribute attribute (key + values) rather than just the well-known ones, so changes to user-defined keys
		// (e.g., gradient_type, spread_method) invalidate downstream graph caches as expected
		for (key, attribute) in &self.attributes.attributes {
			std::hash::Hash::hash(key.as_str(), state);
			attribute.cache_hash_dyn(state);
		}
	}
}

impl<T: PartialEq> PartialEq for List<T> {
	fn eq(&self, other: &Self) -> bool {
		// Attributes participate in equality so the `a == b` ⇒ `hash(a) == hash(b)` contract holds with `cache_hash`
		self.element == other.element
			&& self.attributes.attributes.len() == other.attributes.attributes.len()
			&& self
				.attributes
				.attributes
				.iter()
				.zip(&other.attributes.attributes)
				.all(|((self_key, self_attribute), (other_key, other_attribute))| self_key == other_key && self_attribute.eq_dyn(other_attribute.as_ref()))
	}
}

impl<T> ApplyTransform for List<T> {
	/// Right-multiplies the modification into each item's transform attribute.
	fn apply_transform(&mut self, modification: &DAffine2) {
		for transform in self.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*transform *= *modification;
		}
	}

	/// Left-multiplies the modification into each item's transform attribute.
	fn left_apply_transform(&mut self, modification: &DAffine2) {
		for transform in self.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*transform = *modification * *transform;
		}
	}
}

unsafe impl<T: StaticTypeSized> StaticType for List<T> {
	type Static = List<T::Static>;
}

impl<T> FromIterator<Item<T>> for List<T> {
	/// Collects an iterator of [`Item`]s into a [`List`], pre-allocating based on the iterator's size hint.
	fn from_iter<I: IntoIterator<Item = Item<T>>>(iter: I) -> Self {
		let iter = iter.into_iter();
		let (lower_bound, _) = iter.size_hint();
		let mut list = Self::with_capacity(lower_bound);

		for item in iter {
			list.push(item);
		}

		list
	}
}

// =======
// Item<T>
// =======

/// An owned item containing an element of type `T` and a set of type-erased scalar attributes.
///
/// Used to build individual items before pushing them into a [`List`], or when consuming items out of a list via [`IntoIterator`].
#[derive(Clone, Debug)]
pub struct Item<T> {
	element: T,
	attributes: ItemAttributeValues,
}

impl<T: Default> Default for Item<T> {
	fn default() -> Self {
		Self::new_from_element(T::default())
	}
}

impl<T: PartialEq> PartialEq for Item<T> {
	fn eq(&self, other: &Self) -> bool {
		self.element == other.element
	}
}

impl<T: CacheHash> CacheHash for Item<T> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.element.cache_hash(state);
	}
}

unsafe impl<T: StaticTypeSized> StaticType for Item<T> {
	type Static = Item<T::Static>;
}

impl<T> Item<T> {
	/// Constructs an item from a pre-built element and attributes pair.
	pub fn from_parts(element: T, attributes: ItemAttributeValues) -> Self {
		Self { element, attributes }
	}

	/// Constructs an item with the given element and an empty set of attributes.
	pub fn new_from_element(element: T) -> Self {
		Self::from_parts(element, ItemAttributeValues::new())
	}

	/// Returns a shared reference to this item's element.
	pub fn element(&self) -> &T {
		&self.element
	}

	/// Returns a mutable reference to this item's element.
	pub fn element_mut(&mut self) -> &mut T {
		&mut self.element
	}

	/// Consumes this item and returns the owned element, discarding attributes.
	pub fn into_element(self) -> T {
		self.element
	}

	/// Consumes this item and returns its element and attributes as separate owned values.
	pub fn into_parts(self) -> (T, ItemAttributeValues) {
		(self.element, self.attributes)
	}

	/// Returns a shared reference to all attributes of this item.
	pub fn attributes(&self) -> &ItemAttributeValues {
		&self.attributes
	}

	/// Returns a mutable reference to all attributes of this item.
	pub fn attributes_mut(&mut self) -> &mut ItemAttributeValues {
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
	pub fn attribute_mut_or_insert_default<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> &mut U {
		self.attributes.get_or_insert_default_mut(key)
	}

	/// Sets the attribute value for the given key, replacing any existing entry with the same key.
	pub fn set_attribute<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl Into<String>, value: U) {
		self.attributes.insert(key, value);
	}

	/// Sets the attribute value for the given key and returns the item, enabling builder-style chaining.
	pub fn with_attribute<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(mut self, key: impl Into<String>, value: U) -> Self {
		self.set_attribute(key, value);
		self
	}

	/// Removes and returns the attribute value for the given key, if it exists and is of the requested type.
	pub fn remove_attribute<U: 'static>(&mut self, key: &str) -> Option<U> {
		self.attributes.remove(key)
	}
}

// ===========
// ItemIter<T>
// ===========

/// Owning iterator over the items of a consumed [`List`], yielding [`Item`]s.
///
/// Created by [`List::into_iter`]. The list's attributes are converted into per-item
/// scalar [`ItemAttributeValues`] during construction so each yielded item is self-contained.
pub struct ItemIter<T> {
	element: std::vec::IntoIter<T>,
	attributes: std::vec::IntoIter<ItemAttributeValues>,
}

impl<T> Iterator for ItemIter<T> {
	type Item = Item<T>;

	fn next(&mut self) -> Option<Self::Item> {
		Some(Item {
			element: self.element.next()?,
			attributes: self.attributes.next()?,
		})
	}
}

impl<T> DoubleEndedIterator for ItemIter<T> {
	fn next_back(&mut self) -> Option<Self::Item> {
		Some(Item {
			element: self.element.next_back()?,
			attributes: self.attributes.next_back()?,
		})
	}
}
