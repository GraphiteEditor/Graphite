use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::math::quad::Quad;
use crate::transform::ApplyTransform;
use crate::uuid::NodeId;
use dyn_any::{DynAny, StaticType, StaticTypeSized};
use glam::DAffine2;
use graphene_hash::CacheHash;
use std::fmt::Debug;
use std::sync::Arc;

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
/// `NodeIdPath` path from the root network to the layer node owning this item.
/// Used by editor tools to route clicks/selection back to the originating layer.
pub const ATTR_EDITOR_LAYER_PATH: &str = "editor:layer_path";
/// `List<Graphic>` snapshot of the upstream content that fed into a destructive merge
/// (Boolean Operation, Rasterize, etc.), so the editor can still surface click targets for
/// the original child layers after their content has been collapsed.
pub const ATTR_EDITOR_MERGED_LAYERS: &str = "editor:merged_layers";
/// Optional `BezPath` that overrides the item's own geometry for click-target generation.
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
// TODO: Consider adding "gradient_spread_left" and "gradient_spread_right" override attributes to allow setting different gradient spreads on each side of a gradient
/// Gradient's `GradientSpread` (`Pad`, `Reflect`, `Repeat`, or `Clear`).
pub const ATTR_GRADIENT_SPREAD: &str = "gradient_spread";
/// Gradient's `GradientForm` (`Linear` or `Radial`).
pub const ATTR_GRADIENT_FORM: &str = "gradient_form";
/// Gradient's `GradientSpace`, the color space its stops interpolate in.
pub const ATTR_GRADIENT_SPACE: &str = "gradient_space";
/// Gradient's `GradientHueDirection` (`Shorter`, `Longer`, `Increasing`, or `Decreasing`), which way around the
/// hue wheel the stops interpolate when the gradient space is polar.
pub const ATTR_GRADIENT_HUE_DIRECTION: &str = "gradient_hue_direction";
/// Gradient's `GradientInterpolation` (`Stepped`, `Linear`, or `Smooth`), the path its stops interpolate along
/// and thus whether the ramp jumps, turns corners, or flows smoothly through them.
pub const ATTR_GRADIENT_INTERPOLATION: &str = "gradient_interpolation";
/// Gradient's `bool` (implicit default `false`) for treating the stop list as a cycle, where a wrapped interval
/// interpolates from the last stop through the 1|0 boundary back to the first.
pub const ATTR_GRADIENT_CYCLIC: &str = "gradient_cyclic";
/// Gradient stop's `f64` position from 0 to 1 along the gradient, on the `List<Color>` inside a `Gradient`.
/// When the attribute is absent, stops distribute evenly across the 0 to 1 range.
pub const ATTR_POSITION: &str = "position";
/// Gradient stop's `f64` midpoint (implicit default `0.5`, linear), a factor from 0 to 1 across the distance to the next
/// stop, on the `List<Color>` inside a `Gradient`. The final stop's midpoint is ignored if "gradient_cyclic" is false.
pub const ATTR_MIDPOINT: &str = "midpoint";
/// Item's ordered list of paint passes, of type `Appearance`. Earlier coverages paint first, compositing below later ones.
pub const ATTR_APPEARANCE: &str = "appearance";
// TODO: Add a "fill_rule" attribute as a sibling of "paint" on the coverage list (uniform across covers) once a FillRule type ships
/// Coverage's `Graphic` paint (implicit default `Graphic::None`, painting nothing), on the
/// `List<Coverage>` inside an `Appearance`.
pub const ATTR_PAINT: &str = "paint";
/// Stroke coverage's line thickness (`f64`, implicit default `0.`), on the `Item<Cover>` inside a `Coverage`.
pub const ATTR_WEIGHT: &str = "weight";
/// Stroke coverage's `DashPattern` (implicit default empty, a solid line), on the `Item<Cover>` inside a `Coverage`.
pub const ATTR_DASH_PATTERN: &str = "dash_pattern";
/// Stroke coverage's dash phase offset distance (`f64`, implicit default `0.`), on the `Item<Cover>` inside a `Coverage`.
pub const ATTR_DASH_OFFSET: &str = "dash_offset";
/// Stroke coverage's `StrokeCap` (implicit default `Butt`), on the `Item<Cover>` inside a `Coverage`.
pub const ATTR_CAP: &str = "cap";
/// Stroke coverage's `StrokeJoin` (implicit default `Miter`), on the `Item<Cover>` inside a `Coverage`.
pub const ATTR_JOIN: &str = "join";
/// Stroke coverage's miter limit threshold (`f64`, implicit default `4.`), on the `Item<Cover>` inside a `Coverage`.
pub const ATTR_JOIN_MITER_LIMIT: &str = "join_miter_limit";
/// Stroke coverage's `StrokeAlign` (implicit default `Center`), on the `Item<Cover>` inside a `Coverage`.
pub const ATTR_ALIGN: &str = "align";
/// Brush stroke item's `Color` its strokes are painted with.
pub const ATTR_COLOR: &str = "color";
/// Brush stroke item's tip diameter in document-space units (`f64`).
pub const ATTR_DIAMETER: &str = "diameter";
/// Brush stroke item's edge hardness from `0.` (softest) to `1.` (hardest) (`f64`).
pub const ATTR_HARDNESS: &str = "hardness";
/// Brush stroke item's per-pass paint coverage from `0.` to `1.` (`f64`).
pub const ATTR_FLOW: &str = "flow";
/// `List<Path>` of connected strokes woven through the point pool, on the item holding a `Vector`.
/// Absent means the pool is a point cloud.
pub const ATTR_PATHS: &str = "paths";
/// Path's `CurveType` naming the stride its point references weave in, on the `List<Path>` item
/// holding the path (implicit default `Linear`).
pub const ATTR_CURVE_TYPE: &str = "curve_type";
/// Prefix of the attributes the engine keeps for itself, which are hidden from users.
pub const INTERNAL_ATTRIBUTE_PREFIX: &str = "internal:";
/// A vector item's cache of geometry derived from its point pool and paths, shared by the item's clones.
pub const ATTR_GEOMETRY_CACHE: &str = "internal:geometry_cache";
/// Point reference's `bool` tool state marking the tangent pair at this anchor occurrence as smooth,
/// on the `List<i64>` inside a `Path` (implicit default `false`). Editing tools keep a smooth pair
/// colinear. Rendering derives continuity from actual tangent geometry and does not read this.
pub const ATTR_SMOOTH: &str = "smooth";
/// Item's stable ID for external addressing (modifications, tools, animation), opt-in because indices are
/// the internal addressing: a `PointId` on a `Vector`'s point pool, or a `SegmentId` on the `List<i64>`
/// inside a `Path` at the anchor reference the segment departs from.
pub const ATTR_ID: &str = "id";
/// Text item's font size in document-space units (`f64`, implicit default `24.`).
pub const ATTR_FONT_SIZE: &str = "font_size";
/// Text item's font, as a `Resource` of the loaded font file.
pub const ATTR_FONT: &str = "font";
/// Text item's line height as a ratio of the font size (`f64`, implicit default `1.2`).
pub const ATTR_LINE_HEIGHT: &str = "line_height";
/// Text item's extra spacing between letters in document-space units (`f64`, implicit default `0.`).
pub const ATTR_LETTER_SPACING: &str = "letter_spacing";
/// Text item's maximum line-wrap width in document-space units (`f64`, implicit default `0.`, which leaves the width unbounded).
pub const ATTR_MAX_WIDTH: &str = "max_width";
/// Text item's maximum block height in document-space units, past which lines are not drawn
/// (`f64`, implicit default `0.`, which leaves the height unbounded).
pub const ATTR_MAX_HEIGHT: &str = "max_height";
/// Text item's faux-italic letter tilt angle in degrees (`f64`, implicit default `0.`).
pub const ATTR_LETTER_TILT: &str = "letter_tilt";
/// Text item's `TextAlign` horizontal alignment of lines within the block.
pub const ATTR_TEXT_ALIGN: &str = "text_align";

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

/// Overrides the type's default value for certain attributes.
fn implicit_default_value(key: &str) -> Option<Box<dyn AnyAttributeValue>> {
	match key {
		ATTR_OPACITY | ATTR_OPACITY_FILL => Some(Box::new(1_f64)),
		ATTR_MIDPOINT => Some(Box::new(0.5_f64)),
		_ => None,
	}
}

/// Appends `count` copies of `key`'s implicit default to `attribute` (see [`implicit_default_value`]).
fn pad_with_implicit_default(key: &str, attribute: &mut dyn AnyAttribute, count: usize) {
	match implicit_default_value(key) {
		Some(default) => attribute.push_repeated(&*default, count),
		None => {
			for _ in 0..count {
				attribute.push_default();
			}
		}
	}
}

// ============
// AttributeKey
// ============

/// An attribute's name. Short names are stored inline and longer ones shared, so copying an item's or a list's attributes allocates nothing for their names.
#[derive(Clone)]
enum AttributeKey {
	Inline { length: u8, bytes: [u8; AttributeKey::INLINE_CAPACITY] },
	Allocated(Arc<str>),
}

impl AttributeKey {
	/// The longest name stored inline, which keeps a key the size of a `String`.
	const INLINE_CAPACITY: usize = 22;

	fn new(key: &str) -> Self {
		let mut bytes = [0; Self::INLINE_CAPACITY];

		match bytes.get_mut(..key.len()) {
			Some(inline) => {
				inline.copy_from_slice(key.as_bytes());
				Self::Inline { length: key.len() as u8, bytes }
			}
			None => Self::Allocated(key.into()),
		}
	}

	fn as_bytes(&self) -> &[u8] {
		match self {
			Self::Inline { length, bytes } => bytes.get(..*length as usize).unwrap_or_default(),
			Self::Allocated(key) => key.as_bytes(),
		}
	}

	fn as_str(&self) -> &str {
		match self {
			// The inline bytes are a whole `&str` copied in, so they always read back as one
			Self::Inline { .. } => std::str::from_utf8(self.as_bytes()).unwrap_or_default(),
			Self::Allocated(key) => key,
		}
	}

	/// Whether this is the attribute named `key`.
	fn is(&self, key: &str) -> bool {
		self.as_bytes() == key.as_bytes()
	}
}

impl PartialEq for AttributeKey {
	fn eq(&self, other: &Self) -> bool {
		self.as_bytes() == other.as_bytes()
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

	/// Clones this value into a new shared trait object.
	fn clone_arc(&self) -> Arc<dyn AnyAttributeValue>;

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any;

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

	/// Consumes the box and returns the underlying concrete type for downcasting.
	fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;

	/// Converts the shared value into one that can be downcast to its concrete type.
	fn into_any_arc(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync>;

	/// Returns a debug-formatted string representation of this value.
	fn display_string(&self) -> String;

	/// Hashes this value into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher);

	/// Compares this value to another for value-by-value equality (object-safe wrapper around `PartialEq`).
	/// Returns `false` if the underlying types differ.
	fn eq_dyn(&self, other: &dyn AnyAttributeValue) -> bool;

	/// Wraps this scalar value into a new attribute, preceded by `preceding_defaults` implicit defaults for `key`.
	fn into_attribute(self: Arc<Self>, key: &str, preceding_defaults: usize) -> Arc<dyn AnyAttribute>;
}

impl<T: Clone + Send + Sync + Default + Sized + Debug + PartialEq + CacheHash + 'static> AnyAttributeValue for T {
	/// Clones this value into a new boxed trait object.
	fn clone_box(&self) -> Box<dyn AnyAttributeValue> {
		Box::new(self.clone())
	}

	/// Clones this value into a new shared trait object.
	fn clone_arc(&self) -> Arc<dyn AnyAttributeValue> {
		Arc::new(self.clone())
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

	/// Converts the shared value into one that can be downcast to its concrete type.
	fn into_any_arc(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
		self
	}

	/// Returns a debug-formatted string representation of this value.
	fn display_string(&self) -> String {
		format!("{:?}", self)
	}

	/// Hashes this value into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher) {
		self.cache_hash(&mut DynHasher::new(state));
	}

	/// Compares this value to another for value-by-value equality (object-safe wrapper around `PartialEq`).
	/// Returns `false` if the underlying types differ.
	fn eq_dyn(&self, other: &dyn AnyAttributeValue) -> bool {
		other.as_any().downcast_ref::<Self>().is_some_and(|other| self == other)
	}

	/// Wraps this scalar value into a new attribute, preceded by `preceding_defaults` implicit defaults for `key`.
	fn into_attribute(self: Arc<Self>, key: &str, preceding_defaults: usize) -> Arc<dyn AnyAttribute> {
		let mut attribute = Attribute::<T>(Vec::with_capacity(preceding_defaults + 1));
		pad_with_implicit_default(key, &mut attribute, preceding_defaults);
		attribute.0.push(Arc::unwrap_or_clone(self));

		Arc::new(attribute)
	}
}

impl Clone for Box<dyn AnyAttributeValue> {
	fn clone(&self) -> Self {
		(**self).clone_box()
	}
}

/// Gives write access to an attribute value, first copying it if clones of its item still share it.
fn unshared_value(value: &mut Arc<dyn AnyAttributeValue>) -> &mut dyn AnyAttributeValue {
	if Arc::get_mut(value).is_none() {
		*value = (**value).clone_arc();
	}

	Arc::get_mut(value).expect("A value that was just copied has no other owner")
}

// ===================
// TRAIT: AnyAttribute
// ===================

/// Enables type-erased storage for parallel attribute lists in a [`List`].
pub trait AnyAttribute: std::any::Any + Send + Sync {
	/// Clones this attribute into a new shared trait object.
	fn clone_arc(&self) -> Arc<dyn AnyAttribute>;

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any;

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

	/// Converts the shared attribute into one that can be downcast to its concrete type.
	fn into_any_arc(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync>;

	/// Pushes a scalar attribute value onto the end of this attribute.
	fn push(&mut self, value: Arc<dyn AnyAttributeValue>);

	/// Pushes a default value onto the end of this attribute.
	fn push_default(&mut self);

	/// Appends `count` copies of `value` (downcast to this attribute's type, or the type default if it
	/// doesn't match), filling in bulk to avoid per-element boxing and dispatch.
	fn push_repeated(&mut self, value: &dyn AnyAttributeValue, count: usize);

	/// Sets the value at the given index, padding with defaults if the attribute is shorter than `index`.
	/// Falls back to a default if the value's type doesn't match.
	fn set_at(&mut self, index: usize, value: Box<dyn AnyAttributeValue>);

	/// Creates a new attribute of the same type filled with `count` number of default values.
	fn new_with_defaults(&self, count: usize) -> Arc<dyn AnyAttribute>;

	/// Returns the number of elements in this attribute.
	fn len(&self) -> usize;

	/// Returns whether this attribute has any elements.
	fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Appends all values from another attribute of the same type.
	fn extend(&mut self, other: Arc<dyn AnyAttribute>);

	/// Returns a shared reference to the value at the requested index.
	fn get_any(&self, index: usize) -> Option<&dyn std::any::Any>;

	/// Returns a debug-formatted display string for the value at the requested index.
	fn display_at(&self, index: usize) -> Option<String>;

	/// Clones a single value from this attribute into a shared scalar attribute value.
	fn clone_value(&self, index: usize) -> Option<Arc<dyn AnyAttributeValue>>;

	/// Turns this attribute into a Vec of scalar attribute values, moving them out unless clones of its list still share it.
	fn drain(self: Arc<Self>) -> Vec<Arc<dyn AnyAttributeValue>>;

	/// Hashes every value in this attribute into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher);

	/// Compares this attribute to another for value-by-value equality (object-safe wrapper around `PartialEq`).
	/// Returns `false` if the underlying types differ.
	fn eq_dyn(&self, other: &dyn AnyAttribute) -> bool;
}

/// Adapts a `&mut dyn Hasher` so generic `CacheHash::cache_hash<H>` calls (which require `H: Sized + Hasher`) can
/// drive a trait-object hasher. Writes gather in a buffer that reaches the inner hasher whole, when the next write
/// would not fit and when this is dropped, which spares a virtual call for every value hashed.
struct DynHasher<'a> {
	inner: &'a mut dyn core::hash::Hasher,
	buffer: [u8; DynHasher::CAPACITY],
	used: usize,
}

impl<'a> DynHasher<'a> {
	const CAPACITY: usize = 128;

	fn new(inner: &'a mut dyn core::hash::Hasher) -> Self {
		Self {
			inner,
			buffer: [0; Self::CAPACITY],
			used: 0,
		}
	}

	fn flush(&mut self) {
		if let Some(gathered) = self.buffer.get(..self.used).filter(|gathered| !gathered.is_empty()) {
			self.inner.write(gathered);
		}
		self.used = 0;
	}
}

impl core::hash::Hasher for DynHasher<'_> {
	/// Reads the inner hasher's state without the bytes still buffered, which only reach it once this hasher is dropped.
	fn finish(&self) -> u64 {
		debug_assert_eq!(self.used, 0, "The buffered bytes reach the inner hasher only once this hasher is dropped");
		self.inner.finish()
	}

	fn write(&mut self, bytes: &[u8]) {
		if bytes.len() > Self::CAPACITY - self.used {
			self.flush();
		}

		// Anything too long for the buffer passes straight through, in order since the buffer was just emptied
		match self.buffer.get_mut(self.used..self.used + bytes.len()) {
			Some(space) => {
				space.copy_from_slice(bytes);
				self.used += bytes.len();
			}
			None => self.inner.write(bytes),
		}
	}
}

impl Drop for DynHasher<'_> {
	fn drop(&mut self) {
		self.flush();
	}
}

/// Gives write access to an attribute, first copying it if clones of its list still share it.
fn unshared(attribute: &mut Arc<dyn AnyAttribute>) -> &mut dyn AnyAttribute {
	if Arc::get_mut(attribute).is_none() {
		*attribute = (**attribute).clone_arc();
	}

	Arc::get_mut(attribute).expect("An attribute that was just copied has no other owner")
}

// ============
// Attribute<T>
// ============

/// Wraps a Vec<T> for attribute storage in a [`List`].
pub struct Attribute<T>(pub Vec<T>);

impl<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static> AnyAttribute for Attribute<T> {
	/// Clones this attribute into a new shared trait object.
	fn clone_arc(&self) -> Arc<dyn AnyAttribute> {
		Arc::new(Attribute(self.0.clone()))
	}

	/// Returns a shared reference to the underlying concrete type for downcasting.
	fn as_any(&self) -> &dyn std::any::Any {
		self
	}

	/// Returns a mutable reference to the underlying concrete type for downcasting.
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
		self
	}

	/// Converts the shared attribute into one that can be downcast to its concrete type.
	fn into_any_arc(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
		self
	}

	/// Pushes an attribute value onto the end of this attribute's list, downcasting it to `T`.
	/// Falls back to a default value if the type doesn't match, to maintain the attribute-length invariant.
	fn push(&mut self, value: Arc<dyn AnyAttributeValue>) {
		let value = value.into_any_arc().downcast::<T>().map(Arc::unwrap_or_clone).unwrap_or_default();
		self.0.push(value);
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
	fn new_with_defaults(&self, count: usize) -> Arc<dyn AnyAttribute> {
		// A default per item, since clones of one would share state such as a cache between items
		Arc::new(Attribute(std::iter::repeat_with(T::default).take(count).collect()))
	}

	/// Returns the number of elements in this attribute.
	fn len(&self) -> usize {
		self.0.len()
	}

	/// Appends all values from another attribute, downcasting it to the same `Attribute<T>` type.
	/// Falls back to padding with defaults if the type doesn't match, to maintain the attribute-length invariant.
	fn extend(&mut self, other: Arc<dyn AnyAttribute>) {
		let other_len = other.len();

		match other.into_any_arc().downcast::<Self>().map(Arc::try_unwrap) {
			Ok(Ok(other)) => self.0.extend(other.0),
			Ok(Err(shared)) => self.0.extend_from_slice(&shared.0),
			Err(_) => self.0.extend(std::iter::repeat_with(T::default).take(other_len)),
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

	/// Clones the value at the given index into a shared scalar attribute value.
	fn clone_value(&self, index: usize) -> Option<Arc<dyn AnyAttributeValue>> {
		self.0.get(index).map(|v| Arc::new(v.clone()) as Arc<dyn AnyAttributeValue>)
	}

	/// Returns all values as a Vec of shared scalar attribute values, moved out unless clones of the list still share them.
	fn drain(self: Arc<Self>) -> Vec<Arc<dyn AnyAttributeValue>> {
		let values = match Arc::try_unwrap(self) {
			Ok(attribute) => attribute.0,
			Err(shared) => shared.0.clone(),
		};

		values.into_iter().map(|v| Arc::new(v) as Arc<dyn AnyAttributeValue>).collect()
	}

	/// Hashes every value in this attribute into the given hasher (object-safe wrapper around `CacheHash`).
	fn cache_hash_dyn(&self, state: &mut dyn core::hash::Hasher) {
		self.0.cache_hash(&mut DynHasher::new(state));
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
#[derive(Clone, Default)]
pub struct ListDyn {
	attributes: Attributes,
}

impl ListDyn {
	/// Number of items in the underlying `List`.
	pub fn len(&self) -> usize {
		self.attributes.len
	}

	/// Whether the underlying `List` has zero items.
	pub fn is_empty(&self) -> bool {
		self.attributes.len == 0
	}

	/// Returns a reference to the attribute value at the given key and item index, downcast to `U`, if present and matching.
	pub fn attribute<U: 'static>(&self, key: &str, index: usize) -> Option<&U> {
		self.attributes.get_value(key, index)
	}
}

impl<T> From<List<T>> for ListDyn {
	fn from(list: List<T>) -> Self {
		Self { attributes: list.attributes }
	}
}

impl Debug for ListDyn {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.attributes.keys().collect();
		f.debug_struct("ListDyn").field("keys", &keys).field("len", &self.attributes.len).finish()
	}
}

impl PartialEq for ListDyn {
	fn eq(&self, other: &Self) -> bool {
		self.attributes.len == other.attributes.len && self.attributes.equals(&other.attributes)
	}
}

impl CacheHash for ListDyn {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.attributes.len.cache_hash(state);
		self.attributes.cache_hash_entries(state);
	}
}

unsafe impl StaticType for ListDyn {
	type Static = Self;
}

// ======
// Shared
// ======

/// A handle on an attribute value that an item also holds, for reading it without copying it. It is not an
/// attribute value itself, so it cannot be stored back onto an item in place of the value it reads.
pub struct Shared<T>(Arc<T>);

impl<T> std::ops::Deref for Shared<T> {
	type Target = T;

	fn deref(&self) -> &T {
		&self.0
	}
}

impl<T> From<Arc<T>> for Shared<T> {
	fn from(value: Arc<T>) -> Self {
		Self(value)
	}
}

// ===================
// ItemAttributeValues
// ===================

/// An item's attribute values with their keys, in insertion order.
type ValueEntries = Vec<(AttributeKey, Arc<dyn AnyAttributeValue>)>;

/// Scalar attribute storage for a single item.
///
/// A small ordered map of type-erased scalar attribute values, keyed by string name.
/// Used for individual attribute values in an [`Item`].
/// Linear search preserves insertion order and is likely faster than a HashMap for small attribute counts.
/// Clones share the map and every value in it until one of them is written to.
#[derive(Clone, Default)]
pub struct ItemAttributeValues(Option<Arc<ValueEntries>>);

impl Debug for ItemAttributeValues {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.keys().collect();
		f.debug_struct("Attributes").field("keys", &keys).finish()
	}
}

impl PartialEq for ItemAttributeValues {
	fn eq(&self, other: &Self) -> bool {
		let (entries, other_entries) = (self.entries(), other.entries());

		entries.len() == other_entries.len()
			&& entries
				.iter()
				.zip(other_entries)
				.all(|((self_key, self_value), (other_key, other_value))| self_key == other_key && self_value.eq_dyn(other_value.as_ref()))
	}
}

impl ItemAttributeValues {
	/// Creates an empty set of attributes.
	pub fn new() -> Self {
		Self::default()
	}

	fn from_entries(entries: ValueEntries) -> Self {
		Self((!entries.is_empty()).then(|| Arc::new(entries)))
	}

	fn into_entries(self) -> ValueEntries {
		self.0.map(Arc::unwrap_or_clone).unwrap_or_default()
	}

	fn entries(&self) -> &[(AttributeKey, Arc<dyn AnyAttributeValue>)] {
		self.0.as_deref().map_or(&[], Vec::as_slice)
	}

	/// The entries for writing, which copies the map first if clones of the item still share it.
	fn entries_mut(&mut self) -> &mut ValueEntries {
		Arc::make_mut(self.0.get_or_insert_default())
	}

	fn position(&self, key: &str) -> Option<usize> {
		self.entries().iter().position(|(existing_key, _)| existing_key.is(key))
	}

	/// Inserts an attribute with the given key and value, replacing any existing entry with the same key.
	pub fn insert<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl AsRef<str>, value: T) {
		let key = key.as_ref();
		let value: Arc<dyn AnyAttributeValue> = Arc::new(value);

		let position = self.position(key);
		let entries = self.entries_mut();
		match position.and_then(|position| entries.get_mut(position)) {
			Some((_, existing_value)) => *existing_value = value,
			None => entries.push((AttributeKey::new(key), value)),
		}
	}

	/// Gets a reference to the value of the attribute with the given key, if it exists and can be downcast to the requested type.
	pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
		self.entries()
			.iter()
			.find_map(|(existing_key, value)| if existing_key.is(key) { (**value).as_any().downcast_ref::<T>() } else { None })
	}

	/// Gets a shared handle to the value of the attribute with the given key, if it exists and can be downcast to the requested type.
	pub fn get_shared<T: Send + Sync + 'static>(&self, key: &str) -> Option<Shared<T>> {
		let (_, value) = self.entries().iter().find(|(existing_key, _)| existing_key.is(key))?;

		value.clone().into_any_arc().downcast::<T>().ok().map(Shared)
	}

	/// Gets a mutable reference to the value of the attribute with the given key, if it exists and can be downcast to the requested type.
	pub fn get_mut<T: 'static>(&mut self, key: &str) -> Option<&mut T> {
		// Checked before anything is unshared, so a miss leaves the values shared
		let position = self.entries().iter().position(|(existing_key, value)| existing_key.is(key) && (**value).as_any().is::<T>())?;
		let (_, value) = self.entries_mut().get_mut(position)?;

		unshared_value(value).as_any_mut().downcast_mut::<T>()
	}

	/// Gets a mutable reference to the value, inserting a default if it doesn't exist or has the wrong type.
	pub fn get_or_insert_default_mut<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> &mut T {
		let position = self.position(key);
		let has_type = position.and_then(|position| self.entries().get(position)).is_some_and(|(_, value)| (**value).as_any().is::<T>());

		if !has_type {
			let entries = self.entries_mut();
			if let Some(position) = position {
				entries.remove(position);
			}
			entries.push((AttributeKey::new(key), Arc::new(T::default())));
		}

		self.get_mut::<T>(key).expect("Attribute was just ensured to exist with correct type")
	}

	/// Removes and returns the value for the given key, if it exists and can be downcast to the requested type.
	pub fn remove<T: Send + Sync + 'static>(&mut self, key: &str) -> Option<T> {
		let position = self.position(key)?;
		let (_, mut value) = self.entries_mut().remove(position);
		if !(*value).as_any().is::<T>() {
			return None;
		}

		// A value that clones of the item still share is copied, since it can only be moved out of its last owner
		if Arc::get_mut(&mut value).is_none() {
			value = (*value).clone_arc();
		}
		Arc::try_unwrap(value.into_any_arc().downcast::<T>().ok()?).ok()
	}

	/// Returns an iterator over the keys of all stored attributes, in insertion order.
	pub fn keys(&self) -> impl Iterator<Item = &str> {
		self.entries().iter().map(|(key, _)| key.as_str())
	}

	/// Returns a type-erased reference to the value of the attribute with the given key, if it exists.
	pub fn get_any(&self, key: &str) -> Option<&dyn std::any::Any> {
		self.entries()
			.iter()
			.find_map(|(existing_key, value)| if existing_key.is(key) { Some((**value).as_any()) } else { None })
	}

	/// Returns an iterator over key and type-erased value pairs of all stored attributes, in insertion order.
	pub fn iter_any(&self) -> impl Iterator<Item = (&str, &dyn std::any::Any)> {
		self.entries().iter().map(|(key, value)| (key.as_str(), (**value).as_any()))
	}

	/// Returns a debug-formatted string representation of the attribute value for the given key, if it exists.
	/// The `overrides` function can provide custom formatting for specific type.
	pub fn display_value(&self, key: &str, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.entries().iter().find_map(|(k, value)| {
			if k.is(key) {
				if let Some(text) = overrides((**value).as_any()) {
					Some(text)
				} else {
					Some(value.display_string())
				}
			} else {
				None
			}
		})
	}

	/// Moves the attribute at `from_key` to `to_key`.
	/// Does nothing if `from_key` is absent, overwrites any existing `to_key`.
	pub fn rename(&mut self, from_key: &str, to_key: impl AsRef<str>) {
		let Some(position) = self.position(from_key) else { return };
		let entries = self.entries_mut();
		let (_, value) = entries.remove(position);

		let to_key = to_key.as_ref();
		match entries.iter_mut().find(|(existing_key, _)| existing_key.is(to_key)) {
			Some((_, existing_value)) => *existing_value = value,
			None => entries.push((AttributeKey::new(to_key), value)),
		}
	}

	/// Clones the attribute with `key` from `source`, replacing any existing attribute with the same key.
	pub fn insert_cloned_from(&mut self, source: &Self, key: &str) {
		let Some((_, value)) = source.entries().iter().find(|(existing_key, _)| existing_key.is(key)) else {
			return;
		};

		let value = value.clone();

		let entries = self.entries_mut();
		match entries.iter_mut().find(|(existing_key, _)| existing_key.is(key)) {
			Some((_, existing_value)) => *existing_value = value,
			None => entries.push((AttributeKey::new(key), value)),
		}
	}
}

// ==========
// Attributes
// ==========

/// One of a list's attributes with its key.
type AttributeEntry = (AttributeKey, Arc<dyn AnyAttribute>);

/// A list's attributes with their keys, in insertion order. A lone attribute is held inline, so the many small lists
/// carrying just one need no collection of their own.
#[derive(Clone, Default)]
enum AttributeEntries {
	#[default]
	None,
	One(AttributeEntry),
	Many(Arc<Vec<AttributeEntry>>),
}

impl AttributeEntries {
	fn as_slice(&self) -> &[AttributeEntry] {
		match self {
			Self::None => &[],
			Self::One(entry) => std::slice::from_ref(entry),
			Self::Many(entries) => entries,
		}
	}

	/// The entries for writing in place, which copies the collection first if clones of the list still share it.
	fn as_mut_slice(&mut self) -> &mut [AttributeEntry] {
		match self {
			Self::None => &mut [],
			Self::One(entry) => std::slice::from_mut(entry),
			Self::Many(entries) => Arc::make_mut(entries).as_mut_slice(),
		}
	}

	fn is_empty(&self) -> bool {
		self.as_slice().is_empty()
	}

	fn push(&mut self, entry: AttributeEntry) {
		*self = match std::mem::take(self) {
			Self::None => Self::One(entry),
			Self::One(first) => {
				let mut entries = Vec::with_capacity(4);
				entries.extend([first, entry]);
				Self::Many(Arc::new(entries))
			}
			Self::Many(mut entries) => {
				Arc::make_mut(&mut entries).push(entry);
				Self::Many(entries)
			}
		};
	}

	/// Takes out the entry at `position`, doing nothing when there is none there.
	fn remove(&mut self, position: usize) {
		*self = match std::mem::take(self) {
			// What is left of one or two entries is nothing or a lone entry again, neither of which needs the collection
			Self::Many(entries) if position < entries.len() && entries.len() <= 2 => entries.get(1 - position).cloned().map_or(Self::None, Self::One),
			Self::Many(mut entries) if position < entries.len() => {
				Arc::make_mut(&mut entries).remove(position);
				Self::Many(entries)
			}
			Self::One(_) if position == 0 => Self::None,
			unchanged => unchanged,
		};
	}

	fn into_vec(self) -> Vec<AttributeEntry> {
		match self {
			Self::None => Vec::new(),
			Self::One(entry) => vec![entry],
			Self::Many(entries) => Arc::unwrap_or_clone(entries),
		}
	}
}

/// The storage data structure for attributes.
///
/// A collection of type-erased parallel attributes, keyed by string name.
/// All access goes through [`List`] and [`Item`] since internals are private.
/// Clones share every attribute until one of them is written to.
/// Invariant: every attribute in `attributes` has exactly `len` elements.
#[derive(Clone, Default)]
struct Attributes {
	attributes: AttributeEntries,
	len: usize,
}

impl Debug for Attributes {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let keys: Vec<&str> = self.keys().collect();
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
		Self {
			attributes: AttributeEntries::None,
			len,
		}
	}

	fn entries(&self) -> &[AttributeEntry] {
		self.attributes.as_slice()
	}

	fn position(&self, key: &str) -> Option<usize> {
		self.entries().iter().position(|(existing_key, _)| existing_key.is(key))
	}

	/// The position of the attribute for the given key, if it exists and holds values of type `T`.
	fn typed_position<T: 'static>(&self, key: &str) -> Option<usize> {
		self.entries()
			.iter()
			.position(|(existing_key, attribute)| existing_key.is(key) && (**attribute).as_any().is::<Attribute<T>>())
	}

	/// The values of the attribute at `position` for writing, if it holds values of type `T`.
	fn values_mut<T: 'static>(&mut self, position: usize) -> Option<&mut Vec<T>> {
		let (_, attribute) = self.attributes.as_mut_slice().get_mut(position)?;

		unshared(attribute).as_any_mut().downcast_mut::<Attribute<T>>().map(|attribute| &mut attribute.0)
	}

	/// Pushes an item's scalar attributes into this attribute store.
	/// Existing attributes that the item lacks receive a default value.
	/// New attribute keys create a new attribute padded with defaults for all prior items.
	fn push_item(&mut self, item: ItemAttributeValues) {
		let len = self.len;
		self.len += 1;

		if self.attributes.is_empty() && item.0.is_none() {
			return;
		}

		let mut item_entries = item.into_entries();

		// Push values into existing attributes, or the implicit default if the item lacks that attribute
		for (attribute_key, attribute) in self.attributes.as_mut_slice() {
			let attribute = unshared(attribute);

			match item_entries.iter().position(|(key, _)| key == attribute_key) {
				Some(position) => attribute.push(item_entries.swap_remove(position).1),
				None => pad_with_implicit_default(attribute_key.as_str(), attribute, 1),
			}
		}

		// Create new attributes for any remaining item values, padded with implicit defaults for prior items
		for (key, value) in item_entries {
			let attribute = value.into_attribute(key.as_str(), len);
			self.attributes.push((key, attribute));
		}
	}

	/// Appends all attribute data from another attribute store into this one.
	/// Attributes present in only one side are padded with defaults for the other side's items.
	fn extend(&mut self, other: Attributes) {
		let (len, other_len) = (self.len, other.len);
		self.len += other_len;

		// An empty store takes on the other's attributes whole, sharing them
		if len == 0 && self.entries().is_empty() {
			self.attributes = other.attributes;
			return;
		}
		// Nothing to add or pad, so self's attributes can stay shared
		if other.attributes.is_empty() && (other_len == 0 || self.attributes.is_empty()) {
			return;
		}

		let mut other_entries = other.attributes.into_vec();

		// Extend matching attributes, or pad self's attributes with implicit defaults for the other's item count
		for (key, self_attribute) in self.attributes.as_mut_slice() {
			let self_attribute = unshared(self_attribute);

			match other_entries.iter().position(|(other_key, _)| other_key == key) {
				Some(position) => self_attribute.extend(other_entries.swap_remove(position).1),
				None => pad_with_implicit_default(key.as_str(), self_attribute, other_len),
			}
		}

		// Remaining other attributes are new, so we pad with implicit defaults for self's existing items
		for (key, other_attribute) in other_entries {
			let mut combined = other_attribute.new_with_defaults(0);
			let writable = unshared(&mut combined);
			pad_with_implicit_default(key.as_str(), writable, len);
			writable.extend(other_attribute);

			self.attributes.push((key, combined));
		}
	}

	/// Gets a reference to the value at the given index from the attribute for the given key.
	fn get_value<T: 'static>(&self, key: &str, index: usize) -> Option<&T> {
		self.entries()
			.iter()
			.find_map(|(k, attribute)| if k.is(key) { attribute.get_any(index)?.downcast_ref::<T>() } else { None })
	}

	/// Removes the entire attribute for the given key, if present.
	fn remove_attribute(&mut self, key: &str) {
		if let Some(position) = self.position(key) {
			self.attributes.remove(position);
		}
	}

	/// Creates a new attribute of type `T` filled with `key`'s implicit default for all existing items.
	fn new_attribute_padded<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&self, key: &str) -> Arc<dyn AnyAttribute> {
		let mut attribute = Attribute::<T>(Vec::with_capacity(self.len));
		pad_with_implicit_default(key, &mut attribute, self.len);

		Arc::new(attribute)
	}

	/// Finds or creates an attribute for the given key and type, returning its position.
	/// If an attribute with the key exists but has the wrong type, it is removed and replaced with a new attribute of the correct type, padded with implicit defaults.
	/// A newly created attribute is filled with `key`'s implicit default for all existing items.
	fn find_or_create_attribute<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> usize {
		if let Some(position) = self.typed_position::<T>(key) {
			return position;
		}

		let mistyped = self.position(key);
		let attribute = self.new_attribute_padded::<T>(key);

		if let Some(position) = mistyped {
			self.attributes.remove(position);
		}
		self.attributes.push((AttributeKey::new(key), attribute));

		self.entries().len() - 1
	}

	/// Gets a mutable reference to the value at the given index, creating the attribute if it doesn't exist or has the wrong type.
	fn get_or_insert_default_value<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str, index: usize) -> &mut T {
		let attribute_position = self.find_or_create_attribute::<T>(key);
		let values = self.values_mut::<T>(attribute_position).unwrap();
		&mut values[index]
	}

	/// Sets the value at the given index in the attribute for the given key.
	/// Creates the attribute with defaults if it doesn't exist.
	fn set_value<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str, index: usize, value: T) {
		let attribute_position = self.find_or_create_attribute::<T>(key);
		let values = self.values_mut::<T>(attribute_position).unwrap();
		values[index] = value;
	}

	/// Replaces the attribute for the given key with one value per item, keeping its place unless its type changes.
	fn set_attribute_values<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str, values: Vec<T>) {
		// An attribute changing type moves to the end, as it does when set one value at a time
		let (existing, same_type) = (self.position(key), self.typed_position::<T>(key).is_some());

		let attribute: Arc<dyn AnyAttribute> = Arc::new(Attribute(values));
		if same_type && let Some((_, existing_attribute)) = existing.and_then(|position| self.attributes.as_mut_slice().get_mut(position)) {
			*existing_attribute = attribute;
			return;
		}

		if let Some(position) = existing {
			self.attributes.remove(position);
		}
		self.attributes.push((AttributeKey::new(key), attribute));
	}

	/// Sets a single type-erased value at the given index of a list of `item_count` items, creating the attribute from the
	/// value's type if the key holds none.
	fn set_value_dyn(&mut self, key: &str, index: usize, value: Box<dyn AnyAttributeValue>, item_count: usize) {
		let position = self.position(key);

		if let Some((_, attribute)) = position.and_then(|position| self.attributes.as_mut_slice().get_mut(position)) {
			unshared(attribute).set_at(index, value);
		} else {
			let mut new_attribute = Arc::<dyn AnyAttributeValue>::from(value).into_attribute(key, index);
			let trailing_defaults = item_count.saturating_sub(new_attribute.len());
			pad_with_implicit_default(key, unshared(&mut new_attribute), trailing_defaults);
			self.attributes.push((AttributeKey::new(key), new_attribute));
		}
	}

	/// Returns a debug-formatted string for the value at the given index in the attribute for the given key.
	fn display_value(&self, key: &str, index: usize, overrides: fn(&dyn std::any::Any) -> Option<String>) -> Option<String> {
		self.entries().iter().find_map(|(k, attribute)| {
			if k.is(key) {
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
		self.entries().iter().find_map(|(k, attribute)| if k.is(key) { attribute.get_any(index) } else { None })
	}

	/// Returns an iterator over the keys of all stored attributes (in insertion order).
	fn keys(&self) -> impl Iterator<Item = &str> {
		self.entries().iter().map(|(key, _)| key.as_str())
	}

	/// Returns a typed slice of the attribute for the given key, if it exists and can be downcast to `Attribute<T>`.
	fn get_attribute_slice<T: 'static>(&self, key: &str) -> Option<&[T]> {
		self.entries().iter().find_map(|(k, attribute)| {
			if k.is(key) {
				(**attribute).as_any().downcast_ref::<Attribute<T>>().map(|c| c.0.as_slice())
			} else {
				None
			}
		})
	}

	/// Returns a mutable typed slice of the attribute for the given key, if it exists and can be downcast to `Attribute<T>`.
	fn get_attribute_slice_mut<T: 'static>(&mut self, key: &str) -> Option<&mut [T]> {
		// Checked before anything is unshared, so a miss leaves the attributes shared
		let position = self.typed_position::<T>(key)?;

		self.values_mut::<T>(position).map(Vec::as_mut_slice)
	}

	/// Returns a mutable typed slice of the attribute for the given key, creating a new attribute filled with defaults if it doesn't exist.
	fn get_or_create_attribute_slice_mut<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> &mut [T] {
		let position = self.find_or_create_attribute::<T>(key);
		self.values_mut::<T>(position).unwrap()
	}

	/// Clones all attribute values at the given item index into a new scalar Attributes.
	fn clone_item(&self, index: usize) -> ItemAttributeValues {
		let entries = self.entries().iter().filter_map(|(key, attribute)| Some((key.clone(), attribute.clone_value(index)?)));

		ItemAttributeValues::from_entries(entries.collect())
	}

	/// Drains all attribute data into a Vec of per-item scalar Attributes.
	fn into_item_vec(self) -> Vec<ItemAttributeValues> {
		let attributes = self.attributes.into_vec();
		let mut items: Vec<Vec<_>> = (0..self.len).map(|_| Vec::with_capacity(attributes.len())).collect();

		for (key, attribute) in attributes {
			for (item, value) in items.iter_mut().zip(attribute.drain()) {
				item.push((key.clone(), value));
			}
		}

		items.into_iter().map(ItemAttributeValues::from_entries).collect()
	}

	/// Whether both stores hold the same attributes in the same order with equal values.
	fn equals(&self, other: &Self) -> bool {
		let (entries, other_entries) = (self.entries(), other.entries());

		entries.len() == other_entries.len()
			&& entries
				.iter()
				.zip(other_entries)
				.all(|((self_key, self_attribute), (other_key, other_attribute))| self_key == other_key && self_attribute.eq_dyn(other_attribute.as_ref()))
	}

	/// Hashes every attribute's key and values.
	fn cache_hash_entries<H: core::hash::Hasher>(&self, state: &mut H) {
		for (key, attribute) in self.entries() {
			std::hash::Hash::hash(key.as_str(), state);
			attribute.cache_hash_dyn(state);
		}
	}
}

// =======
// List<T>
// =======

/// A struct-of-arrays collection where each item holds an element of type `T` alongside
/// a set of type-erased, dynamically-typed attributes stored in parallel attributes.
///
/// Elements are stored contiguously in a shared `Vec<T>`, while attributes live in an internal
/// [`Attributes`] store that keeps one attribute per attribute key. Items are accessed by
/// index through element/attribute accessor methods, or consumed as owned [`Item`]s via iteration.
/// Clones share the elements and every attribute until one of them is written to.
#[derive(Clone, Debug)]
pub struct List<T> {
	element: Arc<Vec<T>>,
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
			element: Arc::new(Vec::with_capacity(capacity)),
			attributes: Attributes::new(),
		}
	}

	/// Creates a list containing a single item with the given element and no attributes.
	pub fn new_from_element(element: T) -> Self {
		Self {
			element: Arc::new(vec![element]),
			attributes: Attributes::with_len(1),
		}
	}

	/// Creates a list holding the given elements in order, with no attributes.
	pub fn new_from_elements(elements: Vec<T>) -> Self {
		let len = elements.len();
		Self {
			element: Arc::new(elements),
			attributes: Attributes::with_len(len),
		}
	}

	/// Creates a list containing a single item from the given [`Item`], preserving its attributes.
	pub fn new_from_item(item: Item<T>) -> Self {
		let mut attributes = Attributes::new();
		attributes.push_item(item.attributes);
		Self {
			element: Arc::new(vec![item.element]),
			attributes,
		}
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

	/// A witness to this list's element storage, for telling later whether a list still holds these elements.
	pub fn elements_witness(&self) -> ElementsWitness<T> {
		ElementsWitness(Arc::downgrade(&self.element))
	}

	/// A witness to the storage of the attribute for `key`, or to its absence.
	pub fn attribute_witness(&self, key: &str) -> AttributeWitness {
		let attribute = self.attributes.position(key).map(|position| Arc::downgrade(&self.attributes.entries()[position].1));
		AttributeWitness(attribute)
	}

	// ============================
	// Attribute-oriented iteration
	// ============================

	/// Returns an iterator over shared references to all element values.
	pub fn iter_element_values(&self) -> std::slice::Iter<'_, T> {
		self.element.iter()
	}

	/// Returns an iterator over shared references to the values of a typed attribute, or `None` if the attribute doesn't exist or has the wrong type.
	pub fn iter_attribute_values<U: 'static>(&self, key: &str) -> Option<std::slice::Iter<'_, U>> {
		self.attributes.get_attribute_slice::<U>(key).map(|s| s.iter())
	}

	/// Returns an iterator over mutable references to the values of a typed attribute, or `None` if the attribute doesn't exist or has the wrong type.
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
	pub fn set_attribute<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl AsRef<str>, index: usize, value: U) {
		self.attributes.set_value(key.as_ref(), index, value);
	}

	/// Sets the attribute's value for every item at once from one value per item, replacing any values the key held (an empty list still gains the key).
	/// Logs an error and leaves the list unchanged if the number of values differs from the number of items.
	pub fn set_attribute_values<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl AsRef<str>, values: Vec<U>) {
		let key = key.as_ref();
		if values.len() != self.element.len() {
			log::error!("Attribute '{key}' was given {} values for a list of {} items", values.len(), self.element.len());
			return;
		}

		self.attributes.set_attribute_values(key, values);
	}

	/// Sets a single type-erased attribute value at the given index, creating the attribute from the value's underlying type if it doesn't exist (padded with defaults to match the list's length).
	/// Falls back to default if the value's type doesn't match an existing attribute.
	pub fn set_attribute_value_dyn(&mut self, key: impl AsRef<str>, index: usize, value: AttributeValueDyn) {
		self.attributes.set_value_dyn(key.as_ref(), index, value.0, self.element.len());
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

// ==============
// Element writes
// ==============

/// Writing to the elements copies them first if clones of the list still share them.
impl<T: Clone> List<T> {
	fn elements_mut(&mut self) -> &mut Vec<T> {
		Arc::make_mut(&mut self.element)
	}

	/// Appends an item to the end of this list.
	pub fn push(&mut self, item: Item<T>) {
		self.elements_mut().push(item.element);
		self.attributes.push_item(item.attributes);
	}

	/// Appends all items from another list into this one.
	pub fn extend(&mut self, list: List<T>) {
		// An empty list adopts the other's elements without copying them
		if self.element.is_empty() {
			self.element = list.element;
		} else {
			let elements = self.elements_mut();
			match Arc::try_unwrap(list.element) {
				Ok(other_elements) => elements.extend(other_elements),
				Err(shared) => elements.extend_from_slice(&shared),
			}
		}
		self.attributes.extend(list.attributes);
	}

	/// Returns an iterator over mutable references to all element values.
	pub fn iter_element_values_mut(&mut self) -> std::slice::IterMut<'_, T> {
		self.elements_mut().iter_mut()
	}

	/// Returns a mutable reference to the element at the given index, or `None` if out of bounds.
	pub fn element_mut(&mut self, index: usize) -> Option<&mut T> {
		if index >= self.element.len() {
			return None;
		}
		self.elements_mut().get_mut(index)
	}

	/// Returns disjoint mutable references to the element slice and a typed attribute slice, creating the attribute with defaults if it doesn't exist.
	/// This enables simultaneous mutable access to elements and a single attribute without borrowing conflicts.
	pub fn element_and_attribute_slices_mut<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: &str) -> (&mut [T], &mut [U]) {
		let Self { element, attributes } = self;
		(Arc::make_mut(element).as_mut_slice(), attributes.get_or_create_attribute_slice_mut::<U>(key))
	}
}

/// Identifies a list's element storage without keeping it alive.
/// A write moves observed elements to new storage, so a list that matches still holds the elements the witness saw.
pub struct ElementsWitness<T>(std::sync::Weak<Vec<T>>);

impl<T> Clone for ElementsWitness<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<T> ElementsWitness<T> {
	/// Whether `list` holds the very elements this witness saw, unchanged.
	pub fn matches(&self, list: &List<T>) -> bool {
		std::ptr::eq(self.0.as_ptr(), Arc::as_ptr(&list.element))
	}
}

/// Identifies the storage of one of a list's attributes, or its absence, without keeping it alive.
/// A write copies an observed attribute to new storage, so a list that matches still holds the values the witness saw.
#[derive(Clone)]
pub struct AttributeWitness(Option<std::sync::Weak<dyn AnyAttribute>>);

impl AttributeWitness {
	/// Whether `list` holds the very attribute for `key` this witness saw, unchanged (or still lacks it).
	pub fn matches<T>(&self, list: &List<T>, key: &str) -> bool {
		let current = list.attributes.position(key).map(|position| &list.attributes.entries()[position].1);
		match (&self.0, current) {
			(Some(witnessed), Some(current)) => std::ptr::addr_eq(witnessed.as_ptr(), Arc::as_ptr(current)),
			(None, None) => true,
			_ => false,
		}
	}
}

impl<T: BoundingBox> BoundingBox for List<T> {
	/// Computes the combined bounding box of all items, composing each item's transform attribute with the given transform.
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let mut combined_bounds = None;

		for (index, item_transform) in self.iter_attribute_values_or_default::<DAffine2>(ATTR_TRANSFORM).enumerate() {
			match T::bounding_box_in_list(self, index, transform * item_transform, include_stroke, false) {
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

		for (index, item_transform) in self.iter_attribute_values_or_default::<DAffine2>(ATTR_TRANSFORM).enumerate() {
			match T::bounding_box_in_list(self, index, transform * item_transform, include_stroke, true) {
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

impl<T: Clone> IntoIterator for List<T> {
	type Item = Item<T>;
	type IntoIter = ItemIter<T>;

	/// Consumes a [`List`] and returns an iterator of [`Item`]s, each containing the owned data of the respective item from the original list.
	/// The elements move out unless clones of the list still share them, in which case they are copied.
	fn into_iter(self) -> Self::IntoIter {
		let attributes = self.attributes.into_item_vec();
		ItemIter {
			element: Arc::unwrap_or_clone(self.element).into_iter(),
			attributes: attributes.into_iter(),
		}
	}
}

impl<T> Default for List<T> {
	fn default() -> Self {
		Self {
			element: Arc::new(Vec::new()),
			attributes: Attributes::new(),
		}
	}
}

impl<T: CacheHash> CacheHash for List<T> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.element.as_slice().cache_hash(state);

		// Hash every attribute (key + values) rather than just the well-known ones, so changes to user-defined keys
		// (e.g., gradient_form, gradient_spread) invalidate downstream graph caches as expected
		self.attributes.cache_hash_entries(state);
	}
}

impl<T: PartialEq> PartialEq for List<T> {
	fn eq(&self, other: &Self) -> bool {
		// Attributes participate in equality so the `a == b` ⇒ `hash(a) == hash(b)` contract holds with `cache_hash`
		(Arc::ptr_eq(&self.element, &other.element) || self.element == other.element) && self.attributes.equals(&other.attributes)
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
		let mut elements = Vec::with_capacity(lower_bound);
		let mut attributes = Attributes::new();

		for item in iter {
			elements.push(item.element);
			attributes.push_item(item.attributes);
		}

		Self {
			element: Arc::new(elements),
			attributes,
		}
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

impl<T: BoundingBox> BoundingBox for Item<T> {
	/// Computes the element's bounding box, composing the item's transform attribute with the given transform.
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM);
		T::bounding_box_in_item(self, transform * item_transform, include_stroke, false)
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM);
		T::bounding_box_in_item(self, transform * item_transform, include_stroke, true)
	}
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
		for (key, attribute) in self.attributes.entries() {
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

	/// Returns a reference to the attribute value for the given key, if it exists and is of the requested type.
	pub fn attribute<U: 'static>(&self, key: &str) -> Option<&U> {
		self.attributes.get(key)
	}

	/// Returns a shared handle to the attribute value for the given key, if it exists and is of the requested type.
	/// The handle goes on reading the value without copying it, whatever then happens to the item. Writing or removing
	/// the item's value while a handle is held copies the value first, so a handle is best dropped before then.
	pub fn attribute_shared<U: Send + Sync + 'static>(&self, key: &str) -> Option<Shared<U>> {
		self.attributes.get_shared(key)
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
	pub fn set_attribute<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(&mut self, key: impl AsRef<str>, value: U) {
		self.attributes.insert(key, value);
	}

	/// Sets the attribute value for the given key and returns the item, enabling builder-style chaining.
	pub fn with_attribute<U: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static>(mut self, key: impl AsRef<str>, value: U) -> Self {
		self.set_attribute(key, value);
		self
	}

	/// Removes and returns the attribute value for the given key, if it exists and is of the requested type.
	pub fn remove_attribute<U: Send + Sync + 'static>(&mut self, key: &str) -> Option<U> {
		self.attributes.remove(key)
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
		let transform = self.attribute_mut_or_insert_default::<DAffine2>(ATTR_TRANSFORM);
		*transform *= *modification;
	}

	/// Left-multiplies the modification into the item's transform attribute.
	fn left_apply_transform(&mut self, modification: &DAffine2) {
		let transform = self.attribute_mut_or_insert_default::<DAffine2>(ATTR_TRANSFORM);
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

	// An item that doesn't set opacity must read as fully opaque even once a sibling introduces the
	// opacity attribute, otherwise the dense store pads it with f64's `0.` default and it vanishes.
	#[test]
	fn implicit_opacity_default_is_opaque() {
		// Collecting items (the path Boolean Operation takes when merging operands)
		let mut collected = List::<()>::new();
		collected.push(Item::new_from_element(()));
		collected.push(Item::new_from_element(()).with_attribute(ATTR_OPACITY, 1_f64));
		assert_eq!(collected.attribute_cloned_or_default::<f64>(ATTR_OPACITY, 0), 1.);

		// Extending one list with another
		let mut base = List::<()>::new();
		base.push(Item::new_from_element(()));
		let mut tail = List::<()>::new();
		tail.push(Item::new_from_element(()).with_attribute(ATTR_OPACITY_FILL, 1_f64));
		base.extend(tail);
		assert_eq!(base.attribute_cloned_or_default::<f64>(ATTR_OPACITY_FILL, 0), 1.);

		// Setting one item's opacity leaves the others opaque, not transparent
		let mut indexed = List::<()>::new();
		indexed.push(Item::new_from_element(()));
		indexed.push(Item::new_from_element(()));
		indexed.set_attribute(ATTR_OPACITY, 1, 0.5_f64);
		assert_eq!(indexed.attribute_cloned_or_default::<f64>(ATTR_OPACITY, 0), 1.);
		assert_eq!(indexed.attribute_cloned_or_default::<f64>(ATTR_OPACITY, 1), 0.5);

		// A non-opacity numeric attribute still falls back to its type default
		let mut other = List::<()>::new();
		other.push(Item::new_from_element(()));
		other.push(Item::new_from_element(()).with_attribute(ATTR_START, 5_u64));
		assert_eq!(other.attribute_cloned_or_default::<u64>(ATTR_START, 0), 0);
	}

	// Whole-attribute writes replace an existing attribute in place and refuse a value count that doesn't match
	#[test]
	fn set_attribute_values_writes_every_item_at_once() {
		let mut list = List::new_from_elements(vec![10_u32, 20, 30]);
		list.set_attribute(ATTR_START, 0, 7_u64);
		list.set_attribute_values(ATTR_END, vec![1_u64, 2, 3]);
		list.set_attribute_values(ATTR_START, vec![4_u64, 5, 6]);

		assert_eq!(list.len(), 3);
		assert_eq!(list.attribute_keys().collect::<Vec<_>>(), [ATTR_START, ATTR_END]);
		assert_eq!(list.iter_attribute_values::<u64>(ATTR_START).unwrap().copied().collect::<Vec<_>>(), [4, 5, 6]);

		list.set_attribute_values(ATTR_END, vec![9_u64]);
		assert_eq!(list.iter_attribute_values::<u64>(ATTR_END).unwrap().copied().collect::<Vec<_>>(), [1, 2, 3]);

		// Changing an attribute's type moves it last, whichever way it is written
		let mut one_at_a_time = list.clone();
		for index in 0..3 {
			one_at_a_time.set_attribute(ATTR_START, index, index as f64);
		}
		list.set_attribute_values(ATTR_START, vec![0., 1., 2.]);
		assert_eq!(list.attribute_keys().collect::<Vec<_>>(), [ATTR_END, ATTR_START]);
		assert_eq!(list, one_at_a_time);
	}

	fn hash_of<T: CacheHash>(value: &T) -> u64 {
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		value.cache_hash(&mut hasher);
		core::hash::Hasher::finish(&hasher)
	}

	// Attribute values are hashed through a buffer, which must not let a change slip by wherever it falls
	#[test]
	fn attribute_values_all_reach_the_hash() {
		let build = |changed: Option<usize>| {
			let mut list = List::new_from_elements(vec![0_u8; 300]);
			let mut values: Vec<u64> = (0..300).collect();
			if let Some(index) = changed {
				values[index] += 1;
			}
			list.set_attribute_values(ATTR_START, values);
			list.set_attribute_values(ATTR_NAME, vec![String::from("a string long enough to pass straight through the buffer when repeated").repeat(3); 300]);
			list
		};

		let hash = hash_of(&build(None));
		assert_eq!(hash, hash_of(&build(None)));
		for index in [0, 15, 16, 17, 127, 128, 299] {
			assert_ne!(hash, hash_of(&build(Some(index))), "a change at {index} went unnoticed");
		}
	}

	/// Keeps every byte it is given, in order, so a buffered stream can be checked against what went into it.
	#[derive(Default)]
	struct RecordingHasher(Vec<u8>);

	impl core::hash::Hasher for RecordingHasher {
		fn finish(&self) -> u64 {
			0
		}

		fn write(&mut self, bytes: &[u8]) {
			self.0.extend_from_slice(bytes);
		}
	}

	// Writes of every length pass through the buffer whole and in order, also from a value hashing through a buffer of its own
	#[test]
	fn buffered_writes_reach_the_inner_hasher_in_order() {
		use core::hash::Hasher;

		let lengths = [3, 125, 1, 200, 0, 128, 64, 65, 300, 127, 2];
		let pieces: Vec<Vec<u8>> = lengths.iter().enumerate().map(|(index, &length)| (0..length).map(|byte| (byte + index * 31) as u8).collect()).collect();

		let mut recorder = RecordingHasher::default();
		{
			let mut outer = DynHasher::new(&mut recorder);
			for (index, piece) in pieces.iter().enumerate() {
				// Every third piece comes from a nested value, the way a list stored as an attribute value is hashed
				if index % 3 == 2 {
					DynHasher::new(&mut outer).write(piece);
				} else {
					outer.write(piece);
				}
			}
		}

		assert_eq!(recorder.0, pieces.concat());
	}

	// A cloned list shares each attribute with its source until one of the two writes to it
	#[test]
	fn list_clones_share_attributes_until_written() {
		let mut original = List::new_from_elements(vec![1_u32, 2, 3]);
		original.set_attribute_values(ATTR_START, vec![10_u64, 20, 30]);
		original.set_attribute_values(ATTR_END, vec![1_u64, 2, 3]);
		let values = |list: &List<u32>, key: &str| list.iter_attribute_values::<u64>(key).map(|values| values.as_slice().as_ptr());

		let mut copy = original.clone();
		assert_eq!(values(&original, ATTR_START), values(&copy, ATTR_START));
		assert_eq!((&original, hash_of(&original)), (&copy, hash_of(&copy)));

		copy.set_attribute(ATTR_START, 1, 99_u64);
		assert_eq!(original.attribute::<u64>(ATTR_START, 1), Some(&20));
		assert_eq!(copy.attribute::<u64>(ATTR_START, 1), Some(&99));
		assert_ne!(original, copy);

		// Only the attribute written to stops being shared
		assert_ne!(values(&original, ATTR_START), values(&copy, ATTR_START));
		assert_eq!(values(&original, ATTR_END), values(&copy, ATTR_END));
	}

	// A list reads the same however many attributes it has held, and a lone attribute is shared with clones like any other
	#[test]
	fn a_lone_attribute_reads_like_one_among_several() {
		let mut lone = List::new_from_elements(vec![1_u32, 2, 3]);
		lone.set_attribute_values(ATTR_START, vec![10_u64, 20, 30]);
		let values = |list: &List<u32>, key: &str| list.iter_attribute_values::<u64>(key).map(|values| values.as_slice().as_ptr());

		let mut copy = lone.clone();
		assert_eq!(values(&lone, ATTR_START), values(&copy, ATTR_START));
		copy.set_attribute(ATTR_START, 1, 99_u64);
		assert_eq!((lone.attribute::<u64>(ATTR_START, 1), copy.attribute::<u64>(ATTR_START, 1)), (Some(&20), Some(&99)));
		assert_ne!(values(&lone, ATTR_START), values(&copy, ATTR_START));

		// A clone giving the lone attribute up or another type leaves the source as it was
		let mut emptied = lone.clone();
		emptied.remove_attribute(ATTR_START);
		assert_eq!(emptied, List::new_from_elements(vec![1_u32, 2, 3]));
		let mut retyped = lone.clone();
		retyped.set_attribute_values(ATTR_START, vec![0.5_f64, 1.5, 2.5]);
		assert_eq!(retyped.attribute::<f64>(ATTR_START, 2), Some(&2.5));
		assert_eq!(lone.attribute::<u64>(ATTR_START, 2), Some(&30));

		// Gaining a second attribute and losing it again leaves a list equal to one that never had it
		let mut regrown = lone.clone();
		regrown.set_attribute_values(ATTR_END, vec![1_u64, 2, 3]);
		assert_eq!(regrown.attribute_keys().collect::<Vec<_>>(), [ATTR_START, ATTR_END]);
		assert_eq!(values(&lone, ATTR_START), values(&regrown, ATTR_START));
		regrown.remove_attribute(ATTR_END);
		assert_eq!((&regrown, hash_of(&regrown)), (&lone, hash_of(&lone)));

		regrown.remove_attribute(ATTR_START);
		assert_eq!(regrown, List::new_from_elements(vec![1_u32, 2, 3]));
		assert_eq!(lone.attribute::<u64>(ATTR_START, 2), Some(&30));
	}

	// A cloned item shares each attribute value with its source until one of the two writes to it or takes it out
	#[test]
	fn item_clones_share_values_until_written() {
		let original = Item::new_from_element(()).with_attribute(ATTR_NAME, vec![1_u32, 2, 3]).with_attribute(ATTR_OPACITY, 0.5_f64);
		let name = |item: &Item<()>| item.attribute::<Vec<u32>>(ATTR_NAME).map(|name| name.as_ptr());

		let mut copy = original.clone();
		assert_eq!(name(&original), name(&copy));
		assert_eq!((&original, hash_of(&original)), (&copy, hash_of(&copy)));

		copy.attribute_mut::<Vec<u32>>(ATTR_NAME).unwrap().push(4);
		assert_eq!(original.attribute::<Vec<u32>>(ATTR_NAME), Some(&vec![1, 2, 3]));
		assert_eq!(copy.attribute::<Vec<u32>>(ATTR_NAME), Some(&vec![1, 2, 3, 4]));

		let mut emptied = original.clone();
		assert_eq!(emptied.remove_attribute::<Vec<u32>>(ATTR_NAME), Some(vec![1, 2, 3]));
		assert_eq!(emptied.attribute::<Vec<u32>>(ATTR_NAME), None);
		assert_eq!(original.attribute::<Vec<u32>>(ATTR_NAME), Some(&vec![1, 2, 3]));
	}

	// Taking a shared list apart into items and building on them leaves the list it was cloned from as it was
	#[test]
	fn items_from_a_shared_list_leave_its_source_intact() {
		let mut original = List::new_from_elements(vec![1_u32, 2]);
		original.set_attribute_values(ATTR_START, vec![10_u64, 20]);
		let starts = |list: &List<u32>| list.iter_attribute_values::<u64>(ATTR_START).unwrap().copied().collect::<Vec<_>>();

		let raise = |mut item: Item<u32>| {
			*item.attribute_mut_or_insert_default::<u64>(ATTR_START) += 1;
			item
		};
		let mut rebuilt: List<u32> = original.clone().into_iter().map(raise).collect();
		rebuilt.extend(original.clone());
		rebuilt.push(Item::new_from_element(3));

		assert_eq!(starts(&original), [10, 20]);
		assert_eq!(starts(&rebuilt), [11, 21, 10, 20, 0]);
	}

	// Keys are stored inline up to a length limit and shared beyond it, which reads the same either way
	#[test]
	fn attribute_keys_of_any_length_read_back() {
		let long_key = "a_user_defined_attribute_name_longer_than_the_inline_limit";
		let keys = [long_key, ATTR_GRADIENT_HUE_DIRECTION, "", "name with spaces"];

		let mut list = List::new_from_elements(vec![0_u8]);
		for (value, key) in keys.into_iter().enumerate() {
			list.set_attribute(key, 0, value as u64);
		}

		assert_eq!(list.attribute_keys().collect::<Vec<_>>(), keys);
		for (value, key) in keys.into_iter().enumerate() {
			assert_eq!(list.attribute::<u64>(key, 0), Some(&(value as u64)));
		}
		assert_eq!(list.attribute::<u64>("gradient_hue_directio", 0), None);
	}
}
