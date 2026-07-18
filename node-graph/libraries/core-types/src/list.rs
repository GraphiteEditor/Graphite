use crate::attr::{self, Attr};
use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::math::quad::Quad;
use crate::transform::ApplyTransform;
use crate::uuid::NodeId;
use dyn_any::{DynAny, StaticType, StaticTypeSized};
use glam::DAffine2;
use graphene_hash::CacheHash;
use std::fmt::Debug;

// =====================
// TYPE: NodeIdPath
// =====================

/// A single path of `NodeId`s locating a node (or its owning layer) within the nested document graph.
/// Wraps a `List<NodeId>` so it flows as one rank-0 value (`Item<NodeIdPath>`) rather than a rank-1
/// `List<NodeId>` that the element-wise machinery would wrongly zip over per ID.
#[derive(Default, Debug, Clone, PartialEq, CacheHash, DynAny)]
pub struct NodeIdPath(pub List<NodeId>);

impl From<Vec<NodeId>> for NodeIdPath {
	fn from(ids: Vec<NodeId>) -> Self {
		Self(ids.into_iter().map(Item::new_from_element).collect())
	}
}

// ================
// TYPE: Bundle
// ================

/// A whole `List<T>` treated as one rank-0 value (`Item<Bundle<T>>`) rather than a rank-1 `List<T>`.
/// Bundling a collection lets it pass through a connector that selects or carries the entire collection as one opaque
/// cell (such as a Switch branch), instead of the element-wise machinery zipping over it per element.
#[derive(Clone, Debug, PartialEq)]
pub struct Bundle<T>(pub List<T>);

impl<T> Default for Bundle<T> {
	fn default() -> Self {
		Self(List::default())
	}
}

impl<T: CacheHash> CacheHash for Bundle<T> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.0.cache_hash(state);
	}
}

impl<T> From<List<T>> for Bundle<T> {
	fn from(list: List<T>) -> Self {
		Self(list)
	}
}

unsafe impl<T: StaticTypeSized> StaticType for Bundle<T> {
	type Static = Bundle<T::Static>;
}

// ===========================
// Implicit attribute defaults
// ===========================

// TODO: Remove this is not maintainable
/// Overrides the type's default value for certain attributes.
fn implicit_default_value(key: &str) -> Option<Box<dyn AnyAttributeValue>> {
	if key == attr::Opacity::name() || key == attr::OpacityFill::name() {
		Some(Box::new(1_f64))
	} else {
		None
	}
}

/// The value an item without attribute `A` is considered to have: the key's implicit default if it
/// has one, otherwise the value type's `Default`.
fn implicit_default<A: Attr>() -> A::Value {
	implicit_default_value(A::name())
		.and_then(|value| value.into_any().downcast::<A::Value>().ok())
		.map_or_else(Default::default, |value| *value)
}

/// Appends `count` copies of `key`'s implicit default to `attribute` (see [`implicit_default_value`]).
fn pad_with_implicit_default(key: &str, attribute: &mut Box<dyn AnyAttribute>, count: usize) {
	match implicit_default_value(key) {
		Some(default) => attribute.push_repeated(&*default, count),
		None => {
			for _ in 0..count {
				attribute.push_default();
			}
		}
	}
}

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

	/// Hashes this value into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher);

	/// Compares this value to another for value-by-value equality (object-safe wrapper around `PartialEq`).
	/// Returns `false` if the underlying types differ.
	fn eq_dyn(&self, other: &dyn AnyAttributeValue) -> bool;

	/// Wraps this scalar value into a new attribute, preceded by `preceding_defaults` implicit defaults for `key`.
	fn into_attribute(self: Box<Self>, key: &str, preceding_defaults: usize) -> Box<dyn AnyAttribute>;
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

	/// Hashes this value into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher) {
		self.cache_hash(&mut DynHasher(state));
	}

	/// Compares this value to another for value-by-value equality (object-safe wrapper around `PartialEq`).
	/// Returns `false` if the underlying types differ.
	fn eq_dyn(&self, other: &dyn AnyAttributeValue) -> bool {
		other.as_any().downcast_ref::<Self>().is_some_and(|other| self == other)
	}

	/// Wraps this scalar value into a new attribute, preceded by `preceding_defaults` implicit defaults for `key`.
	fn into_attribute(self: Box<Self>, key: &str, preceding_defaults: usize) -> Box<dyn AnyAttribute> {
		let mut attribute: Box<dyn AnyAttribute> = Box::new(Attribute::<T>(Vec::with_capacity(preceding_defaults + 1)));
		pad_with_implicit_default(key, &mut attribute, preceding_defaults);
		attribute.push(self);
		attribute
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

	/// Appends `count` copies of `value` (downcast to this attribute's type, or the type default if it
	/// doesn't match), filling in bulk to avoid per-element boxing and dispatch.
	fn push_repeated(&mut self, value: &dyn AnyAttributeValue, count: usize);

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

	/// Appends `count` copies of `value`, downcast to `T` (or `T::default()` if the type doesn't match).
	fn push_repeated(&mut self, value: &dyn AnyAttributeValue, count: usize) {
		let value = value.as_any().downcast_ref::<T>().cloned().unwrap_or_default();
		self.0.resize(self.0.len() + count, value);
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

// ==================
// AttributeValueDyn
// ==================

/// Type-erased single attribute value, used as a node graph parameter type.
/// Lets a node accept a value of any valid concrete type via the auto-inserted input adapter conversion without monomorphizing over the value type.
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

	/// Returns a reference to the attribute value at the given runtime key and item index, downcast to `U`, if present and matching.
	/// For keys known at compile time use [`Self::attr`]; this variant is for keys only known at runtime (e.g. the attribute nodes).
	pub fn attribute_dyn<U: 'static>(&self, key: &str, index: usize) -> Option<&U> {
		self.attributes
			.iter()
			.find_map(|(k, attribute)| if k == key { attribute.get_any(index)?.downcast_ref::<U>() } else { None })
	}

	/// Returns a reference to the value of the typed attribute at the given item index, if present.
	pub fn attr<A: Attr>(&self, index: usize) -> Option<&A::Value> {
		self.attribute_dyn(A::name(), index)
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

impl PartialEq for ItemAttributeValues {
	fn eq(&self, other: &Self) -> bool {
		self.0.len() == other.0.len()
			&& self
				.0
				.iter()
				.zip(&other.0)
				.all(|((self_key, self_value), (other_key, other_value))| self_key == other_key && self_value.eq_dyn(other_value.as_ref()))
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

	/// Gets a mutable reference to the value, inserting the provided default if it doesn't exist or has the wrong type.
	pub fn get_or_insert_with_mut<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str, default: impl FnOnce() -> T) -> &mut T {
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
			self.0.push((key.to_string(), Box::new(default())));
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

	/// Returns a type-erased reference to the value of the attribute with the given key, if it exists.
	pub fn get_any(&self, key: &str) -> Option<&dyn std::any::Any> {
		self.0.iter().find_map(|(existing_key, value)| if existing_key == key { Some((**value).as_any()) } else { None })
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

	/// Moves the attribute at `from_key` to `to_key`.
	/// Does nothing if `from_key` is absent, overwrites any existing `to_key`.
	pub fn rename(&mut self, from_key: &str, to_key: impl Into<String>) {
		let Some(pos) = self.0.iter().position(|(k, _)| k == from_key) else { return };
		let (_, value) = self.0.remove(pos);

		let to_key = to_key.into();
		for (existing_key, existing_value) in &mut self.0 {
			if *existing_key == to_key {
				*existing_value = value;
				return;
			}
		}
		self.0.push((to_key, value));
	}

	/// Clones the attribute with `key` from `source`, replacing any existing attribute with the same key.
	pub fn insert_cloned_from(&mut self, source: &Self, key: &str) {
		let Some((_, value)) = source.0.iter().find(|(existing_key, _)| existing_key == key) else {
			return;
		};

		let value = value.clone();

		if let Some((_, existing_value)) = self.0.iter_mut().find(|(existing_key, _)| existing_key == key) {
			*existing_value = value;
		} else {
			self.0.push((key.to_string(), value));
		}
	}

	// ==================
	// Typed key variants
	// ==================

	/// Gets a reference to the value of the typed attribute, if present.
	pub fn attr<A: Attr>(&self) -> Option<&A::Value> {
		self.get(A::name())
	}

	/// Gets a mutable reference to the value of the typed attribute, if present.
	pub fn attr_mut<A: Attr>(&mut self) -> Option<&mut A::Value> {
		self.get_mut(A::name())
	}

	/// Gets a mutable reference to the value of the typed attribute, inserting the key's default value if absent.
	pub fn attr_mut_or_insert_default<A: Attr>(&mut self) -> &mut A::Value {
		self.get_or_insert_with_mut(A::name(), implicit_default::<A>)
	}

	/// Inserts the typed attribute's value, replacing any existing entry.
	pub fn set_attr<A: Attr>(&mut self, value: A::Value) {
		self.insert(A::name(), value);
	}

	/// Removes and returns the value of the typed attribute, if present.
	pub fn remove_attr<A: Attr>(&mut self) -> Option<A::Value> {
		self.remove(A::name())
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

		// Push values into existing attributes, or the implicit default if the item lacks that attribute
		for (attribute_key, attribute) in &mut self.attributes {
			if let Some(position) = item_entries.iter().position(|(k, _)| k == attribute_key) {
				let (_, value) = item_entries.swap_remove(position);
				attribute.push(value);
			} else {
				pad_with_implicit_default(attribute_key, attribute, 1);
			}
		}

		// Create new attributes for any remaining item values, padded with implicit defaults for prior items
		for (key, value) in item_entries {
			self.attributes.push((key.clone(), value.into_attribute(&key, self.len)));
		}

		self.len += 1;
	}

	/// Appends all attribute data from another attribute store into this one.
	/// Attributes present in only one side are padded with defaults for the other side's items.
	fn extend(&mut self, other: Attributes) {
		let other_len = other.len;
		let mut other_entries = other.attributes;

		// Extend matching attributes, or pad self's attributes with implicit defaults for the other's item count
		for (key, self_attribute) in &mut self.attributes {
			if let Some(position) = other_entries.iter().position(|(k, _)| k == key) {
				let (_, other_attribute) = other_entries.swap_remove(position);
				self_attribute.extend(other_attribute);
			} else {
				pad_with_implicit_default(key, self_attribute, other_len);
			}
		}

		// Remaining other attributes are new, so we pad with implicit defaults for self's existing items
		for (key, other_attribute) in other_entries {
			let mut combined = other_attribute.new_with_defaults(0);
			pad_with_implicit_default(&key, &mut combined, self.len);
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

	/// Creates a new attribute of type `T` filled with `key`'s implicit default for all existing items.
	fn new_attribute_padded<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&self, key: &str) -> Box<dyn AnyAttribute> {
		let mut attribute: Box<dyn AnyAttribute> = Box::new(Attribute::<T>(Vec::with_capacity(self.len)));
		pad_with_implicit_default(key, &mut attribute, self.len);
		attribute
	}

	/// Finds or creates an attribute for the given key and type, returning its position.
	/// If an attribute with the key exists but has the wrong type, it is removed and replaced with a new attribute of the correct type, padded with implicit defaults.
	/// A newly created attribute is filled with `key`'s implicit default for all existing items.
	fn find_or_create_attribute<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> usize {
		match self.attributes.iter().position(|(k, _)| k == key) {
			Some(position) => {
				if (*self.attributes[position].1).as_any().downcast_ref::<Attribute<T>>().is_some() {
					position
				} else {
					self.attributes.remove(position);
					let attribute = self.new_attribute_padded::<T>(key);
					self.attributes.push((key.to_string(), attribute));
					self.attributes.len() - 1
				}
			}
			None => {
				let attribute = self.new_attribute_padded::<T>(key);
				self.attributes.push((key.to_string(), attribute));
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

	// =================
	// Element iteration
	// =================

	/// Returns an iterator over shared references to all element values.
	pub fn iter_element_values(&self) -> std::slice::Iter<'_, T> {
		self.element.iter()
	}

	/// Returns an iterator over mutable references to all element values.
	pub fn iter_element_values_mut(&mut self) -> std::slice::IterMut<'_, T> {
		self.element.iter_mut()
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

	/// Returns a shared reference to the attribute value at the given item index and runtime key, if it exists and can be downcast to the requested type.
	/// For keys known at compile time use [`Self::attr`]; this variant is for keys only known at runtime (e.g. the attribute nodes).
	pub fn attribute_dyn<U: 'static>(&self, key: &str, index: usize) -> Option<&U> {
		self.attributes.get_value(key, index)
	}

	/// Sets a single type-erased attribute value at the given index, creating the attribute from the value's underlying type if it doesn't exist (padded with defaults to match the list's length).
	/// Falls back to default if the value's type doesn't match an existing attribute.
	pub fn set_attribute_value_dyn(&mut self, key: impl Into<String>, index: usize, value: AttributeValueDyn) {
		let key = key.into();
		if let Some(position) = self.attributes.attributes.iter().position(|(k, _)| k == &key) {
			self.attributes.attributes[position].1.set_at(index, value.0);
		} else {
			let mut new_attribute = value.0.into_attribute(&key, index);
			let trailing_defaults = self.element.len().saturating_sub(new_attribute.len());
			pad_with_implicit_default(&key, &mut new_attribute, trailing_defaults);
			self.attributes.attributes.push((key, new_attribute));
		}
	}

	/// Returns a debug-formatted display string for the attribute at the given item index and key.
	pub fn attribute_display_value(&self, key: &str, index: usize, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.attributes.display_value(key, index, overrides)
	}

	/// Returns a type-erased reference to the attribute value at the given item index and key, or `None` if absent.
	pub fn attribute_any(&self, key: &str, index: usize) -> Option<&dyn std::any::Any> {
		self.attributes.get_any_value(key, index)
	}

	// ==================
	// Typed key variants
	// ==================

	/// Returns a shared reference to the value of the typed attribute at the given item index, if present.
	pub fn attr<A: Attr>(&self, index: usize) -> Option<&A::Value> {
		self.attributes.get_value(A::name(), index)
	}

	/// Returns a clone of the value of the typed attribute at the given item index, or the key's default value if absent.
	pub fn attr_cloned_or_default<A: Attr>(&self, index: usize) -> A::Value {
		self.attr::<A>(index).cloned().unwrap_or_else(implicit_default::<A>)
	}

	/// Returns a clone of the value of the typed attribute at the given item index, or the provided default if absent.
	pub fn attr_cloned_or<A: Attr>(&self, index: usize, default: A::Value) -> A::Value {
		self.attr::<A>(index).cloned().unwrap_or(default)
	}

	/// Sets the value of the typed attribute at the given item index, creating the attribute with defaults if it doesn't exist.
	pub fn set_attr<A: Attr>(&mut self, index: usize, value: A::Value) {
		self.attributes.set_value(A::name(), index, value);
	}

	/// Removes the entire typed attribute, if present.
	pub fn remove_attr<A: Attr>(&mut self) {
		self.attributes.remove_attribute(A::name());
	}

	/// Runs the given closure on a mutable reference to the value of the typed attribute at the given item index,
	/// creating the attribute with defaults if it doesn't exist, and returns the closure's result.
	pub fn with_attr_mut_or_default<A: Attr, R, F: FnOnce(&mut A::Value) -> R>(&mut self, index: usize, f: F) -> R {
		f(self.attributes.get_or_insert_default_value::<A::Value>(A::name(), index))
	}

	/// Returns an iterator over shared references to the values of the typed attribute, or `None` if it doesn't exist.
	pub fn iter_attr_values<A: Attr>(&self) -> Option<std::slice::Iter<'_, A::Value>> {
		self.attributes.get_attribute_slice::<A::Value>(A::name()).map(|s| s.iter())
	}

	/// Returns an iterator over mutable references to the values of the typed attribute, or `None` if it doesn't exist.
	pub fn iter_attr_values_mut<A: Attr>(&mut self) -> Option<std::slice::IterMut<'_, A::Value>> {
		self.attributes.get_attribute_slice_mut::<A::Value>(A::name()).map(|s| s.iter_mut())
	}

	/// Returns an iterator that yields cloned values of the typed attribute, falling back to the key's default value for each item if the attribute is missing.
	pub fn iter_attr_values_or_default<A: Attr>(&self) -> impl Iterator<Item = A::Value> + '_ {
		let slice = self.attributes.get_attribute_slice::<A::Value>(A::name());
		let len = self.element.len();
		(0..len).map(move |i| slice.map_or_else(implicit_default::<A>, |s| s[i].clone()))
	}

	/// Returns a mutable iterator over the typed attribute, creating the attribute with defaults if it doesn't exist.
	pub fn iter_attr_values_mut_or_default<A: Attr>(&mut self) -> std::slice::IterMut<'_, A::Value> {
		self.attributes.get_or_create_attribute_slice_mut::<A::Value>(A::name()).iter_mut()
	}

	/// Returns disjoint mutable references to the element slice and the typed attribute's slice, creating the attribute with defaults if it doesn't exist.
	/// This enables simultaneous mutable access to elements and a single attribute without borrowing conflicts.
	pub fn element_and_attr_slices_mut<A: Attr>(&mut self) -> (&mut [T], &mut [A::Value]) {
		let Self { element, attributes } = self;
		let attribute_position = attributes.find_or_create_attribute::<A::Value>(A::name());
		let attribute = (*attributes.attributes[attribute_position].1).as_any_mut().downcast_mut::<Attribute<A::Value>>().unwrap();
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

		for (element, item_transform) in self.iter_element_values().zip(self.iter_attr_values_or_default::<crate::attr::Transform>()) {
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

		for (element, item_transform) in self.iter_element_values().zip(self.iter_attr_values_or_default::<crate::attr::Transform>()) {
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
		for transform in self.iter_attr_values_mut_or_default::<crate::attr::Transform>() {
			*transform *= *modification;
		}
	}

	/// Left-multiplies the modification into each item's transform attribute.
	fn left_apply_transform(&mut self, modification: &DAffine2) {
		for transform in self.iter_attr_values_mut_or_default::<crate::attr::Transform>() {
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
#[derive(Clone, Debug, PartialEq)]
pub struct Item<T> {
	element: T,
	attributes: ItemAttributeValues,
}

impl<T: Default> Default for Item<T> {
	fn default() -> Self {
		Self::new_from_element(T::default())
	}
}

impl<T: CacheHash> CacheHash for Item<T> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.element.cache_hash(state);

		// Hash every attribute (key + value) so attribute changes invalidate downstream caches, mirroring `List`
		for (key, attribute) in &self.attributes.0 {
			std::hash::Hash::hash(key.as_str(), state);
			attribute.cache_hash_dyn(state);
		}
	}
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

	// ======================
	// Typed attribute access
	// ======================

	/// Returns a reference to the value of the typed attribute, if present.
	pub fn attr<A: Attr>(&self) -> Option<&A::Value> {
		self.attributes.attr::<A>()
	}

	/// Returns a reference to the value of the typed attribute, or the provided default if absent.
	pub fn attr_or<'a, A: Attr>(&'a self, default: &'a A::Value) -> &'a A::Value {
		self.attr::<A>().unwrap_or(default)
	}

	/// Returns a clone of the value of the typed attribute, or the provided default if absent.
	pub fn attr_cloned_or<A: Attr>(&self, default: A::Value) -> A::Value {
		self.attr::<A>().cloned().unwrap_or(default)
	}

	/// Returns a clone of the value of the typed attribute, or the key's default value if absent.
	pub fn attr_cloned_or_default<A: Attr>(&self) -> A::Value {
		self.attr::<A>().cloned().unwrap_or_else(implicit_default::<A>)
	}

	/// Returns a mutable reference to the value of the typed attribute, if present.
	pub fn attr_mut<A: Attr>(&mut self) -> Option<&mut A::Value> {
		self.attributes.attr_mut::<A>()
	}

	/// Returns a mutable reference to the value of the typed attribute, inserting the key's default value if absent.
	pub fn attr_mut_or_insert_default<A: Attr>(&mut self) -> &mut A::Value {
		self.attributes.attr_mut_or_insert_default::<A>()
	}

	/// Sets the value of the typed attribute, replacing any existing entry.
	pub fn set_attr<A: Attr>(&mut self, value: A::Value) {
		self.attributes.set_attr::<A>(value);
	}

	/// Sets the value of the typed attribute and returns the item, enabling builder-style chaining.
	pub fn with_attr<A: Attr>(mut self, value: A::Value) -> Self {
		self.set_attr::<A>(value);
		self
	}

	/// Removes and returns the value of the typed attribute, if present.
	pub fn remove_attr<A: Attr>(&mut self) -> Option<A::Value> {
		self.attributes.remove_attr::<A>()
	}
}

impl<T> From<T> for Item<T> {
	fn from(element: T) -> Self {
		Self::new_from_element(element)
	}
}

impl<T> From<Item<T>> for List<T> {
	fn from(item: Item<T>) -> Self {
		Self::new_from_item(item)
	}
}

impl<T> From<T> for List<T> {
	fn from(element: T) -> Self {
		Self::new_from_element(element)
	}
}

impl<T> ApplyTransform for Item<T> {
	/// Right-multiplies the modification into the item's transform attribute.
	fn apply_transform(&mut self, modification: &DAffine2) {
		let transform = self.attr_mut_or_insert_default::<crate::attr::Transform>();
		*transform *= *modification;
	}

	/// Left-multiplies the modification into the item's transform attribute.
	fn left_apply_transform(&mut self, modification: &DAffine2) {
		let transform = self.attr_mut_or_insert_default::<crate::attr::Transform>();
		*transform = *modification * *transform;
	}
}

unsafe impl<T: StaticTypeSized> StaticType for Item<T> {
	type Static = Item<T::Static>;
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::attr;

	// An item that doesn't set opacity must read as fully opaque even once a sibling introduces the
	// opacity attribute, otherwise the dense store pads it with f64's `0.` default and it vanishes.
	#[test]
	fn implicit_opacity_default_is_opaque() {
		// Collecting items (the path Boolean Operation takes when merging operands)
		let mut collected = List::<()>::new();
		collected.push(Item::new_from_element(()));
		collected.push(Item::new_from_element(()).with_attr::<attr::Opacity>(1.));
		assert_eq!(collected.attr::<attr::Opacity>(0), Some(&1.));

		// Extending one list with another
		let mut base = List::<()>::new();
		base.push(Item::new_from_element(()));
		let mut tail = List::<()>::new();
		tail.push(Item::new_from_element(()).with_attr::<attr::OpacityFill>(1.));
		base.extend(tail);
		assert_eq!(base.attr::<attr::OpacityFill>(0), Some(&1.));

		// Setting one item's opacity leaves the others opaque, not transparent
		let mut indexed = List::<()>::new();
		indexed.push(Item::new_from_element(()));
		indexed.push(Item::new_from_element(()));
		indexed.set_attr::<attr::Opacity>(1, 0.5);
		assert_eq!(indexed.attr::<attr::Opacity>(0), Some(&1.));
		assert_eq!(indexed.attr::<attr::Opacity>(1), Some(&0.5));

		// A non-opacity numeric attribute still pads with its type default
		let mut other = List::<()>::new();
		other.push(Item::new_from_element(()));
		other.push(Item::new_from_element(()).with_attr::<attr::Start>(5));
		assert_eq!(other.attr::<attr::Start>(0), Some(&0));
	}

	// The typed keys must resolve to the same names as the string constants, and the typed
	// and string-keyed accessors must hit the same storage.
	#[test]
	fn typed_attribute_keys() {
		assert_eq!(attr::Transform::name(), "transform");
		assert_eq!(attr::BlendMode::name(), "blend_mode");
		assert_eq!(attr::Opacity::name(), "opacity");
		assert_eq!(attr::OpacityFill::name(), "opacity_fill");
		assert_eq!(attr::ClippingMask::name(), "clipping_mask");
		assert_eq!(attr::editor::LayerPath::name(), "editor:layer_path");
		assert_eq!(attr::editor::TextFrame::name(), "editor:text_frame");
		assert_eq!(attr::Start::name(), "start");
		assert_eq!(attr::End::name(), "end");
		assert_eq!(attr::Name::name(), "name");
		assert_eq!(attr::Type::name(), "type");
		assert_eq!(attr::Location::name(), "location");
		assert_eq!(attr::Dimensions::name(), "dimensions");
		assert_eq!(attr::Background::name(), "background");
		assert_eq!(attr::Clip::name(), "clip");
		assert_eq!(attr::FontSize::name(), "font_size");
		assert_eq!(attr::LineHeight::name(), "line_height");
		assert_eq!(attr::LetterSpacing::name(), "letter_spacing");
		assert_eq!(attr::MaxWidth::name(), "max_width");
		assert_eq!(attr::MaxHeight::name(), "max_height");
		assert_eq!(attr::LetterTilt::name(), "letter_tilt");

		// Typed writes are visible through dynamic string reads and vice versa
		let mut item = Item::new_from_element(());
		item.set_attr::<attr::Opacity>(0.5);
		assert_eq!(item.attributes().get::<f64>("opacity"), Some(&0.5));
		item.attributes_mut().insert("start", 5_u64);
		assert_eq!(item.attr::<attr::Start>(), Some(&5));

		// A missing attribute reads as the key's declared default
		let empty = Item::new_from_element(());
		assert_eq!(empty.attr_cloned_or_default::<attr::Opacity>(), 1.);
		assert_eq!(empty.attr_cloned_or_default::<attr::Start>(), 0);

		// The generated implicit-default lookup drives dense-store padding
		let mut list = List::<()>::new();
		list.push(Item::new_from_element(()));
		list.push(Item::new_from_element(()));
		list.set_attr::<attr::Opacity>(1, 0.5);
		assert_eq!(list.attr_cloned_or_default::<attr::Opacity>(0), 1.);
		assert_eq!(list.attr_cloned_or_default::<attr::Opacity>(1), 0.5);
	}
}
