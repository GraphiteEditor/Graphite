mod glue;
mod legacy;
mod map;
mod paint;
mod walk;

pub use map::{MapVectorContent, MappableLeaf};

pub(crate) use glue::{list_contains_groups, map_attribute_groups_to_owned, map_attribute_groups_to_persistent, map_attribute_groups_to_resident};
pub use glue::{map_groups_to_owned, map_groups_to_persistent, map_groups_to_resident};
pub(crate) use legacy::run_to_legacy_list;
pub use legacy::{group_to_legacy_graphic, group_to_legacy_list, map_groups_to_legacy, map_paint_attrs_to_legacy, run_to_list};
pub use paint::{PaintColumns, PaintReach, bake_paint_transforms, is_paint_present, paint_cell_rows, vector_can_reduce_to_clip_path, vector_lane_can_reduce_to_clip_path};
pub use walk::{GraphicLevel, GraphicLevelColumn, RowStep, VectorRow, direct_vector_len, flatten_vector_rows, group_is_empty, lane_attributes, run_lane_attributes, segmented_groups, walk_vector_rows};
use walk::{group_all_clipped, group_bounding_box, group_is_fully_transparent, group_is_opaque, group_render_complexity};

use crate::appearance::Appearance;
use crate::markers::ATTR_APPEARANCE;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::{Item, List};
use core_types::ops::{FromAnchorPosition, ListConvert};
use core_types::render_complexity::RenderComplexity;
use core_types::uuid::NodeId;
use core_types::{ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use raster_types::{CPU, GPU, Raster};
use vector_types::Gradient;
pub use vector_types::Vector;

/// The possible forms of graphical content that can be rendered by the Render node into either an image or SVG syntax.
/// A leaf holds its element directly; its attributes ride the containing
/// lane. Multi-element content is a [`core_types::record::Group`] run, or
/// transitionally the legacy `Graphic` list.
#[derive(Clone, Debug, Default, CacheHash, PartialEq, DynAny)]
pub enum Graphic<'e> {
	/// The absence of graphical content, like CSS's `none` keyword: painting it produces nothing.
	#[default]
	None,
	GraphicList(List<Graphic<'e>>),
	Vector(VectorRef<'e>),
	RasterCPU(Raster<CPU>),
	RasterGPU(Raster<GPU>),
	Color(Color),
	Gradient(Gradient),
	Text(String),
	Stroke(brush_types::Stroke),
	Group(core_types::record::Group<'e>),
	/// A stack of levels as one lane: the sides by reference, rendered as if
	/// their lanes were inline.
	Segmented(core_types::record::Segmented<'e>),
}

/// A vector graphic's geometry by reference: resident in an arena for the
/// evaluation, or owned once copied out of it. Lanes sharing one geometry
/// share the park, and a clone is a pointer copy.
#[derive(Clone, Debug, DynAny)]
pub enum VectorRef<'e> {
	Resident(&'e Vector),
	Owned(std::sync::Arc<Vector>),
}

impl<'e> VectorRef<'e> {
	/// The geometry parked in `arena` for the arena's lifetime.
	pub fn park(vector: Vector, arena: &'e core_types::arena::Arena) -> Option<Self> {
		let retained = glue::vector_retained_heap(&vector);
		Some(VectorRef::Resident(arena.alloc_sized_keyed(vector, retained)?.0))
	}

	/// The owned form, which survives the arena generation.
	pub fn copy_out(&self) -> VectorRef<'static> {
		VectorRef::Owned(match self {
			VectorRef::Resident(vector) => std::sync::Arc::new((*vector).clone()),
			VectorRef::Owned(vector) => vector.clone(),
		})
	}

	/// The geometry by value: an owned holder gives it up, a resident one copies.
	pub fn into_owned(self) -> Vector {
		match self {
			VectorRef::Resident(vector) => vector.clone(),
			VectorRef::Owned(vector) => std::sync::Arc::try_unwrap(vector).unwrap_or_else(|shared| (*shared).clone()),
		}
	}

	/// Mutable access, taking the geometry into an owned copy first where it is
	/// shared or resident.
	pub fn make_mut(&mut self) -> &mut Vector {
		if let VectorRef::Resident(vector) = *self {
			*self = VectorRef::Owned(std::sync::Arc::new(vector.clone()));
		}
		match self {
			VectorRef::Owned(vector) => std::sync::Arc::make_mut(vector),
			VectorRef::Resident(_) => unreachable!("taken into an owned copy above"),
		}
	}

	/// The heap the geometry owns through this holder: a resident park is
	/// accounted where it was parked.
	pub fn retained_heap(&self) -> usize {
		match self {
			VectorRef::Resident(_) => 0,
			VectorRef::Owned(vector) => glue::vector_retained_heap(vector),
		}
	}
}

impl std::ops::Deref for VectorRef<'_> {
	type Target = Vector;
	fn deref(&self) -> &Vector {
		match self {
			VectorRef::Resident(vector) => vector,
			VectorRef::Owned(vector) => vector,
		}
	}
}

impl Default for VectorRef<'_> {
	fn default() -> Self {
		VectorRef::Owned(std::sync::Arc::new(Vector::default()))
	}
}

impl PartialEq for VectorRef<'_> {
	fn eq(&self, other: &Self) -> bool {
		std::ptr::eq(&**self, &**other) || **self == **other
	}
}

impl CacheHash for VectorRef<'_> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		(**self).cache_hash(state)
	}
}

impl From<Vector> for VectorRef<'_> {
	fn from(vector: Vector) -> Self {
		VectorRef::Owned(std::sync::Arc::new(vector))
	}
}

/// A typed legacy list as a legacy graphic list: each item de-tables to a
/// leaf element, keeping its attributes on the containing lane.
pub(in crate::graphic) fn detable_items<'e, T: Clone + Send + Sync + 'static>(list: List<T>, leaf: fn(T) -> Graphic<'e>) -> List<Graphic<'e>> {
	let mut out = List::new();
	for item in list.into_iter() {
		let (element, attributes) = item.into_parts();
		out.push(Item::from_parts(leaf(element), attributes));
	}
	out
}

/// The element-space coercion into `Graphic`: a leaf converts in place and a
/// legacy list becomes a native group built over the arena, so the coercion
/// never constructs a legacy interior.
pub trait IntoGraphicElement: Clone + Send + Sync + CacheHash + 'static {
	/// `None` reports arena exhaustion.
	fn into_graphic_element(self, arena: &core_types::arena::Arena) -> Option<Graphic<'_>>;

	/// The coercion from a borrowed element: a parked element hands out its
	/// park, so lanes sharing one value share the graphic's payload.
	fn graphic_ref<'e>(&'e self, arena: &'e core_types::arena::Arena) -> Option<Graphic<'e>> {
		self.clone().into_graphic_element(arena)
	}
}

fn list_group<T: Clone + Send + Sync + CacheHash + PartialEq + dyn_any::StaticTypeSized>(list: List<T>, arena: &core_types::arena::Arena) -> Option<Graphic<'_>>
where
	T::Static: Clone + Send + Sync,
{
	Some(Graphic::Group(core_types::record::Group {
		row: None,
		content: core_types::record::GroupItem::from_list(list, arena)?,
	}))
}

macro_rules! into_graphic_element {
	($($leaf:ident: $element:ty;)*) => {
		$(
			impl IntoGraphicElement for $element {
				fn into_graphic_element(self, _arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
					Some(Graphic::$leaf(self))
				}
			}

			impl IntoGraphicElement for List<$element> {
				fn into_graphic_element(self, arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
					list_group(self, arena)
				}
			}
		)*
	};
}

impl IntoGraphicElement for Vector {
	fn into_graphic_element(self, arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
		Some(Graphic::Vector(VectorRef::park(self, arena)?))
	}

	fn graphic_ref<'e>(&'e self, _arena: &'e core_types::arena::Arena) -> Option<Graphic<'e>> {
		Some(Graphic::Vector(VectorRef::Resident(self)))
	}
}

impl IntoGraphicElement for List<Vector> {
	fn into_graphic_element(self, arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
		list_group(self, arena)
	}
}

into_graphic_element! {
	RasterCPU: Raster<CPU>;
	RasterGPU: Raster<GPU>;
	Color: Color;
	Gradient: Gradient;
	Text: String;
}

impl IntoGraphicElement for Graphic<'static> {
	fn into_graphic_element(self, _arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
		Some(self)
	}

	fn graphic_ref<'e>(&'e self, _arena: &'e core_types::arena::Arena) -> Option<Graphic<'e>> {
		Some(self.clone())
	}
}

impl IntoGraphicElement for List<Graphic<'static>> {
	fn into_graphic_element(self, arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
		list_group(self, arena)
	}
}

// Vector
impl From<Vector> for Graphic<'_> {
	fn from(vector: Vector) -> Self {
		Graphic::Vector(vector.into())
	}
}

// Raster<CPU>
impl From<Raster<CPU>> for Graphic<'_> {
	fn from(raster: Raster<CPU>) -> Self {
		Graphic::RasterCPU(raster)
	}
}

// Raster<GPU>
impl From<Raster<GPU>> for Graphic<'_> {
	fn from(raster: Raster<GPU>) -> Self {
		Graphic::RasterGPU(raster)
	}
}

// Color
impl From<Color> for Graphic<'_> {
	fn from(color: Color) -> Self {
		Graphic::Color(color)
	}
}
// Note: List<Color> -> Option<Color> is in gcore (Color is defined there)

// Gradient
impl From<Gradient> for Graphic<'_> {
	fn from(gradient: Gradient) -> Self {
		Graphic::Gradient(gradient)
	}
}

// String
impl From<String> for Graphic<'_> {
	fn from(text: String) -> Self {
		Graphic::Text(text)
	}
}

/// Whether the list is a single leaf item carrying nothing to compose onto its contents, so flattening it
/// collapses no structure and rebuilding or snapshotting the result would be busywork.
/// Both group forms are excluded: a lone native `Group` still has interior structure to flatten.
pub fn is_lone_anonymous_leaf(content: &List<Graphic>) -> bool {
	content.len() == 1
		&& !matches!(content.element(0), Some(Graphic::GraphicList(_)) | Some(Graphic::Group(_)))
		&& content.attribute::<DAffine2>(ATTR_TRANSFORM, 0).is_none()
		&& content.attribute::<f64>(ATTR_OPACITY, 0).is_none()
		&& content.attribute::<f64>(ATTR_OPACITY_FILL, 0).is_none()
		&& content.attribute::<Vec<NodeId>>(ATTR_EDITOR_LAYER_PATH, 0).is_none()
}

/// Deeply flattens a `List<Graphic>`, collecting only elements matching a specific variant (extracted by `extract_variant`)
/// and discarding all other non-matching content. Recursion through `Graphic::GraphicList` sub-`List`s composes transforms and opacity.
fn flatten_graphic_list<T>(content: List<Graphic>, extract_variant: fn(Graphic) -> Option<List<T>>) -> List<T> {
	// Its list is already the flat answer, so hand it back rather than rebuilding it item by item
	if is_lone_anonymous_leaf(&content) {
		let Some(item) = content.into_iter().next() else { return List::new() };

		return extract_variant(item.into_element()).unwrap_or_default();
	}

	fn flatten_recursive<T>(output: &mut List<T>, current_graphic_list: List<Graphic>, extract_variant: fn(Graphic) -> Option<List<T>>, parent_layer_path: Option<&[NodeId]>) {
		for current_graphic_item in current_graphic_list.into_iter() {
			// Whether the parent carries each attribute: a structural fact (column presence), never a value comparison.
			// Flattening composes a parent attribute onto its children only when the parent has it,
			// so an absent parent attribute never invents a column the children didn't already have.
			let parent_has_transform = current_graphic_item.attribute::<DAffine2>(ATTR_TRANSFORM).is_some();
			let parent_has_opacity = current_graphic_item.attribute::<f64>(ATTR_OPACITY).is_some();
			let parent_has_fill = current_graphic_item.attribute::<f64>(ATTR_OPACITY_FILL).is_some();

			let current_transform: DAffine2 = current_graphic_item.attribute_cloned_or_default(ATTR_TRANSFORM);
			let current_opacity: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY, 1.);
			let current_fill: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
			let lane_layer_path: Option<Vec<NodeId>> = current_graphic_item.attribute::<Vec<NodeId>>(ATTR_EDITOR_LAYER_PATH).cloned();
			let parent_appearance = current_graphic_item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared).cloned();

			let (element, attributes) = current_graphic_item.into_parts();
			match element {
				// Compose the parent's transform/opacity/fill onto each child, but only for attributes the parent carries.
				// A child lacking one is padded with the composition identity (`1.` for opacity/fill, identity for transform), so composing through it is a no-op.
				Graphic::GraphicList(mut sub_list) => {
					// A group's first child has no preceding sibling, so its clipping flag is inert until splicing
					// hands it the group's own predecessor. Clear it (keeping the column) to stay clip-neutral.
					if sub_list.attribute::<bool>(ATTR_CLIPPING_MASK, 0).is_some() {
						sub_list.set_attribute(ATTR_CLIPPING_MASK, 0, false);
					}

					if parent_has_transform {
						for v in sub_list.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
							*v = current_transform * *v;
						}
					}
					if parent_has_opacity {
						for v in sub_list.iter_attribute_values_mut_or_default::<f64>(ATTR_OPACITY) {
							*v *= current_opacity;
						}
					}
					if parent_has_fill {
						for v in sub_list.iter_attribute_values_mut_or_default::<f64>(ATTR_OPACITY_FILL) {
							*v *= current_fill;
						}
					}
					// Appearance cascades into each child whose own is undeclared, since a declared child wins wholesale
					if let Some(appearance) = &parent_appearance {
						for v in sub_list.iter_attribute_values_mut_or_default::<Appearance>(ATTR_APPEARANCE) {
							if v.is_empty() {
								*v = appearance.clone();
							}
						}
					}

					flatten_recursive(output, sub_list, extract_variant, lane_layer_path.as_deref());
				}
				// A bridge row's native group flattens through its legacy lowering.
				Graphic::Group(group) => {
					let lowered = List::new_from_item(Item::from_parts(group_to_legacy_graphic(&group), attributes.clone()));
					flatten_recursive(output, lowered, extract_variant, parent_layer_path);
				}
				Graphic::Segmented(stack) => {
					for group in segmented_groups(&stack) {
						let lowered = List::new_from_item(Item::from_parts(group_to_legacy_graphic(&group), attributes.clone()));
						flatten_recursive(output, lowered, extract_variant, parent_layer_path);
					}
				}
				// A de-tabled leaf is one attr-less element; the extracted row rides with its containing lane's full attributes, paint included.
				// The enclosing group lane's own layer path overrides, one hop only, matching the native walk.
				other => {
					if let Some(typed_list) = extract_variant(other) {
						for item in typed_list.into_iter() {
							let mut row = Item::from_parts(item.into_element(), attributes.clone());
							if let Some(layer_path) = parent_layer_path {
								row.set_attribute(ATTR_EDITOR_LAYER_PATH, layer_path.to_vec());
							}
							output.push(row);
						}
					}
				}
			}
		}
	}

	let mut output = List::new();
	flatten_recursive(&mut output, content, extract_variant, None);
	output
}

/// Maps from a concrete element type to its corresponding `Graphic` enum variant,
/// enabling type-directed casting of typed `List`s from a `Graphic` value.
pub trait TryFromGraphic: Clone + Sized {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>>;

	/// The leaf's element, borrowed, where `graphic` is this type's variant.
	fn leaf_of<'a>(graphic: &'a Graphic<'_>) -> Option<&'a Self>;

	/// The leaf's element, mutably, where `graphic` is this type's variant.
	fn leaf_mut<'a>(graphic: &'a mut Graphic<'_>) -> Option<&'a mut Self>;
}

macro_rules! try_from_graphic {
	($($variant:ident: $element:ty;)*) => {
		$(
			impl TryFromGraphic for $element {
				fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
					if let Graphic::$variant(t) = graphic { Some(List::new_from_element(t)) } else { None }
				}

				fn leaf_of<'a>(graphic: &'a Graphic<'_>) -> Option<&'a Self> {
					if let Graphic::$variant(t) = graphic { Some(t) } else { None }
				}

				fn leaf_mut<'a>(graphic: &'a mut Graphic<'_>) -> Option<&'a mut Self> {
					if let Graphic::$variant(t) = graphic { Some(t) } else { None }
				}
			}
		)*
	};
}

impl TryFromGraphic for Vector {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Vector(vector) = graphic { Some(List::new_from_element(vector.into_owned())) } else { None }
	}

	fn leaf_of<'a>(graphic: &'a Graphic<'_>) -> Option<&'a Self> {
		if let Graphic::Vector(vector) = graphic { Some(vector) } else { None }
	}

	fn leaf_mut<'a>(graphic: &'a mut Graphic<'_>) -> Option<&'a mut Self> {
		if let Graphic::Vector(vector) = graphic { Some(vector.make_mut()) } else { None }
	}
}

try_from_graphic! {
	RasterCPU: Raster<CPU>;
	Color: Color;
	Gradient: Gradient;
	Text: String;
}

// Local trait to convert types to List<Graphic> (avoids orphan rule issues)
pub trait IntoGraphicList: Clone + Send + Sync + Default + std::fmt::Debug + PartialEq + CacheHash + 'static {
	fn into_graphic_list(self) -> List<Graphic<'static>>;

	/// Deeply flattens any content of type `T` within a `List<Graphic>`, discarding all other content, and returning a flat `List<T>`.
	fn into_flattened_list<T: TryFromGraphic>(self) -> List<T>
	where
		Self: std::marker::Sized,
	{
		flatten_graphic_list(self.into_graphic_list(), T::try_from_graphic)
	}
}

impl IntoGraphicList for List<Graphic<'static>> {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		self
	}
}

impl IntoGraphicList for List<Vector> {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		detable_items(self, |vector| Graphic::Vector(vector.into()))
	}
}

impl IntoGraphicList for List<Raster<CPU>> {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		detable_items(self, Graphic::RasterCPU)
	}
}

impl IntoGraphicList for List<Raster<GPU>> {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		detable_items(self, Graphic::RasterGPU)
	}
}

impl IntoGraphicList for List<Color> {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		detable_items(self, Graphic::Color)
	}
}

impl IntoGraphicList for List<Gradient> {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		detable_items(self, Graphic::Gradient)
	}
}

impl IntoGraphicList for List<String> {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		detable_items(self, Graphic::Text)
	}
}

impl IntoGraphicList for DAffine2 {
	fn into_graphic_list(self) -> List<Graphic<'static>> {
		List::new_from_element(Graphic::default())
	}
}

// DAffine2
impl From<DAffine2> for Graphic<'_> {
	fn from(_: DAffine2) -> Self {
		Graphic::default()
	}
}

// Stroke
impl From<brush_types::Stroke> for Graphic<'_> {
	fn from(stroke: brush_types::Stroke) -> Self {
		Graphic::Stroke(stroke)
	}
}

// DVec2
impl From<DVec2> for Graphic<'_> {
	fn from(position: DVec2) -> Self {
		Graphic::Vector(Vector::from_anchor_position(position).into())
	}
}

/// A coordinate becomes the vector holding it as a lone anchor, so a level of
/// coordinates coerces lane-for-lane into a level of single-point vectors.
impl IntoGraphicElement for DVec2 {
	fn into_graphic_element(self, _arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
		Some(Graphic::Vector(Vector::from_anchor_position(self).into()))
	}
}
// Note: List conversions handled by blanket impl in gcore

impl<'e> Graphic<'e> {
	pub fn as_graphic(&self) -> Option<&List<Graphic<'_>>> {
		match self {
			Graphic::GraphicList(graphic) => Some(graphic),
			_ => None,
		}
	}

	pub fn as_graphic_mut(&mut self) -> Option<&mut List<Graphic<'e>>> {
		match self {
			Graphic::GraphicList(graphic) => Some(graphic),
			_ => None,
		}
	}

	pub fn as_vector(&self) -> Option<&Vector> {
		match self {
			Graphic::Vector(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_raster(&self) -> Option<&Raster<CPU>> {
		match self {
			Graphic::RasterCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_mut(&mut self) -> Option<&mut Raster<CPU>> {
		match self {
			Graphic::RasterCPU(raster) => Some(raster),
			_ => None,
		}
	}

	/// A leaf carries no clipping attribute, which rides its containing lane.
	pub fn had_clip_enabled(&self) -> bool {
		fn all_clipped<T>(list: &List<T>) -> bool {
			list.iter_attribute_values_or_default::<bool>(ATTR_CLIPPING_MASK).all(|clip| clip)
		}

		match self {
			Graphic::None => true,
			Graphic::GraphicList(list) => all_clipped(list),
			Graphic::Group(group) => group_all_clipped(group),
			Graphic::Segmented(stack) => segmented_groups(stack).all(|group| group_all_clipped(&group)),
			_ => false,
		}
	}

	pub fn can_reduce_to_clip_path(&self, inherited_appearance: Option<&Appearance>) -> bool {
		match self {
			Graphic::Vector(vector) => vector_can_reduce_to_clip_path(&core_types::lane::Single(&**vector), inherited_appearance),
			_ => false,
		}
	}

	pub fn is_opaque(&self) -> bool {
		match self {
			Graphic::None => false,
			Graphic::GraphicList(list) => !list.is_empty() && list.iter_element_values().all(Graphic::is_opaque),
			// A bare leaf carries no paint attribute, which rides its lane, so
			// nothing here claims opacity.
			Graphic::Vector(_) => false,
			Graphic::Color(color) => color.is_opaque(),
			Graphic::Gradient(stops) => stops.iter().all(|stop| stop.color.is_opaque()),
			Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Text(_) | Graphic::Stroke(_) => false,
			Graphic::Group(group) => group_is_opaque(group),
			Graphic::Segmented(stack) => !stack.is_empty() && segmented_groups(stack).all(|group| group_is_opaque(&group)),
		}
	}

	pub fn is_fully_transparent(&self) -> bool {
		match self {
			Graphic::None => true,
			Graphic::GraphicList(list) => list.iter_element_values().all(Graphic::is_fully_transparent),
			// A bare vector leaf carries no paint or stroke of its own, so it is invisible on its own
			Graphic::Vector(_) => true,
			Graphic::Color(color) => color.a() == 0.,
			Graphic::Gradient(stops) => stops.iter().all(|stop| stop.color.a() == 0.),
			Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Text(_) | Graphic::Stroke(_) => false,
			Graphic::Group(group) => group_is_fully_transparent(group),
			Graphic::Segmented(stack) => segmented_groups(stack).all(|group| group_is_fully_transparent(&group)),
		}
	}

	/// True if this paint opaquely covers the entire fill region.
	/// Vector, Raster, and a nested Graphic may leave gaps, so they return false.
	pub fn covers_opaquely(&self) -> bool {
		matches!(self, Graphic::Color(_) | Graphic::Gradient(_)) && self.is_opaque()
	}

	/// Whether the graphic holds no content: a leaf always holds its element.
	pub fn is_empty(&self) -> bool {
		match self {
			Graphic::None => true,
			Graphic::GraphicList(list) => list.is_empty(),
			Graphic::Group(group) => group_is_empty(group),
			Graphic::Segmented(stack) => segmented_groups(stack).all(|group| group_is_empty(&group)),
			_ => false,
		}
	}
}

impl BoundingBox for Graphic<'_> {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::None => RenderBoundingBox::None,
			Graphic::Vector(vector) => BoundingBox::bounding_box(&**vector, transform, include_stroke),
			Graphic::RasterCPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::RasterGPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::GraphicList(list) => list.bounding_box(transform, include_stroke),
			Graphic::Color(color) => color.bounding_box(transform, include_stroke),
			Graphic::Gradient(gradient) => gradient.bounding_box(transform, include_stroke),
			Graphic::Text(text) => text.bounding_box(transform, include_stroke),
			Graphic::Stroke(stroke) => stroke.bounding_box(transform, include_stroke),
			Graphic::Group(group) => group_bounding_box(group, transform, include_stroke, false),
			Graphic::Segmented(stack) => walk::segmented_bounding_box(stack, transform, include_stroke, false),
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::None => RenderBoundingBox::None,
			Graphic::Vector(vector) => vector.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterCPU(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterGPU(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::GraphicList(graphic) => graphic.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Color(color) => color.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Gradient(gradient) => gradient.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Text(list) => list.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Stroke(stroke) => stroke.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Group(group) => group_bounding_box(group, transform, include_stroke, true),
			Graphic::Segmented(stack) => walk::segmented_bounding_box(stack, transform, include_stroke, true),
		}
	}
}

impl<'e> ListConvert<Graphic<'e>> for Vector {
	fn convert_item(self) -> Graphic<'e> {
		Graphic::Vector(self.into())
	}
}
impl<'e> ListConvert<Graphic<'e>> for Raster<CPU> {
	fn convert_item(self) -> Graphic<'e> {
		Graphic::RasterCPU(self)
	}
}
impl<'e> ListConvert<Graphic<'e>> for Raster<GPU> {
	fn convert_item(self) -> Graphic<'e> {
		Graphic::RasterGPU(self)
	}
}

impl RenderComplexity for Graphic<'_> {
	fn render_complexity(&self) -> usize {
		match self {
			Self::None => 0,
			Self::GraphicList(list) => list.render_complexity(),
			Self::Vector(list) => list.render_complexity(),
			Self::RasterCPU(list) => list.render_complexity(),
			Self::RasterGPU(list) => list.render_complexity(),
			Self::Color(list) => list.render_complexity(),
			Self::Gradient(list) => list.render_complexity(),
			Self::Text(list) => list.render_complexity(),
			Self::Stroke(stroke) => stroke.render_complexity(),
			Self::Group(group) => group_render_complexity(group),
			Self::Segmented(stack) => segmented_groups(stack).map(|group| group_render_complexity(&group)).sum(),
		}
	}
}

// Node definitions moved to graphic-nodes crate

pub trait AtIndex {
	type Output;
	fn at_index(&self, index: usize) -> Option<Self::Output>;
	fn at_index_from_end(&self, index: usize) -> Option<Self::Output>;
}
impl<T: Clone> AtIndex for Vec<T> {
	type Output = T;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		self.get(index).cloned()
	}

	fn at_index_from_end(&self, index: usize) -> Option<Self::Output> {
		if index == 0 || index > self.len() { None } else { self.get(self.len() - index).cloned() }
	}
}
impl<T: Clone> AtIndex for List<T> {
	type Output = List<T>;

	fn at_index(&self, index: usize) -> Option<Self::Output> {
		self.clone_item(index).map(|item| {
			let mut result_list = Self::default();
			result_list.push(item);
			result_list
		})
	}

	fn at_index_from_end(&self, index: usize) -> Option<Self::Output> {
		if index == 0 || index > self.len() { None } else { self.at_index(self.len() - index) }
	}
}

pub trait OmitIndex {
	fn omit_index(&self, index: usize) -> Self;
	fn omit_index_from_end(&self, index: usize) -> Self;
}
impl<T: Clone> OmitIndex for Vec<T> {
	fn omit_index(&self, index: usize) -> Self {
		self.iter().enumerate().filter(|(i, _)| *i != index).map(|(_, v)| v.clone()).collect()
	}

	fn omit_index_from_end(&self, index: usize) -> Self {
		if index == 0 || index > self.len() {
			return self.clone();
		}
		self.omit_index(self.len() - index)
	}
}
impl<T: Clone> OmitIndex for List<T> {
	fn omit_index(&self, index: usize) -> Self {
		let mut result = Self::default();
		for i in 0..self.len() {
			if i != index
				&& let Some(item) = self.clone_item(i)
			{
				result.push(item);
			}
		}
		result
	}

	fn omit_index_from_end(&self, index: usize) -> Self {
		if index == 0 || index > self.len() {
			return self.clone();
		}
		self.omit_index(self.len() - index)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::list::List;

	fn vector_list_stamped_with_layers(layers: [u64; 2]) -> List<Vector> {
		let mut list = List::new();

		for layer in layers {
			let mut item = Item::new_from_element(Vector::default());
			item.set_attribute(ATTR_EDITOR_LAYER_PATH, vec![NodeId(layer)]);
			list.push(item);
		}

		list
	}

	fn last_layer_stamps(list: &List<Vector>) -> Vec<Option<NodeId>> {
		(0..list.len())
			.map(|index| list.attribute_cloned_or_default::<Vec<NodeId>>(ATTR_EDITOR_LAYER_PATH, index).last().copied())
			.collect()
	}

	// Our coercion de-tables: each item becomes its own graphic lane keeping its own stamp, rather than master's
	// single wrapper item that had to be kept anonymous so it could not overwrite the inner items' stamps
	#[test]
	fn wrapping_a_typed_list_keeps_one_lane_per_item() {
		let graphic_list = vector_list_stamped_with_layers([7, 9]).into_graphic_list();

		assert_eq!(graphic_list.len(), 2, "the coercion must not collapse the items into one wrapper lane");
		let layers = (0..graphic_list.len())
			.map(|index| graphic_list.attribute_cloned_or_default::<Vec<NodeId>>(ATTR_EDITOR_LAYER_PATH, index).last().copied())
			.collect::<Vec<_>>();
		assert_eq!(layers, [Some(NodeId(7)), Some(NodeId(9))]);
	}

	// Round-tripping through the graphic form must not collapse the items' distinct stamps onto item 0's
	#[test]
	fn round_trip_through_the_wrapper_preserves_per_item_layer_paths() {
		let flattened: List<Vector> = vector_list_stamped_with_layers([7, 9]).into_flattened_list();

		assert_eq!(last_layer_stamps(&flattened), [Some(NodeId(7)), Some(NodeId(9))]);
	}

	fn vector_graphic() -> Graphic<'static> {
		Graphic::Vector(Vector::default().into())
	}

	// Flattening must not invent attribute columns that neither the parent graphic nor the child carried
	#[test]
	fn flatten_does_not_invent_attributes() {
		let graphics = List::new_from_element(vector_graphic());
		let flattened: List<Vector> = graphics.into_flattened_list();
		for key in [ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, ATTR_EDITOR_LAYER_PATH] {
			assert!(!flattened.attribute_keys().any(|k| k == key), "flatten invented the `{key}` attribute");
		}
	}

	// A parent attribute that is present must compose onto the flattened children
	#[test]
	fn flatten_propagates_present_attributes() {
		let mut graphics = List::new_from_element(vector_graphic());
		graphics.set_attribute(ATTR_OPACITY, 0, 0.5_f64);
		let flattened: List<Vector> = graphics.into_flattened_list();
		assert_eq!(flattened.attribute_cloned_or_default::<f64>(ATTR_OPACITY, 0), 0.5);

		let mut group = List::new_from_element(Graphic::GraphicList(List::new_from_element(vector_graphic())));
		group.set_attribute(ATTR_OPACITY, 0, 0.5_f64);
		let flattened: List<Vector> = group.into_flattened_list();
		assert_eq!(flattened.attribute_cloned_or_default::<f64>(ATTR_OPACITY, 0), 0.5);
	}

	// A padded (empty) appearance cell is undeclared, so the parent's appearance cascades into it while a declared sibling keeps its own
	#[test]
	fn flatten_cascades_into_padded_empty_appearance_items() {
		use crate::appearance::Coverage;

		let single = |color: Color| Appearance::new_single(Coverage::new_fill(), Graphic::Color(color));

		// Declaring an appearance on item 0 forces the attribute, padding item 1 with the empty appearance
		let mut inner = List::new();
		inner.push(Item::new_from_element(vector_graphic()));
		inner.push(Item::new_from_element(vector_graphic()));
		inner.set_attribute(ATTR_APPEARANCE, 0, single(Color::BLACK));

		let mut outer = List::new_from_element(Graphic::GraphicList(inner));
		outer.set_attribute(ATTR_APPEARANCE, 0, single(Color::WHITE));

		let flattened: List<Vector> = outer.into_flattened_list();
		let color_of = |index: usize| {
			let appearance = flattened.attribute::<Appearance>(ATTR_APPEARANCE, index)?;
			let Graphic::Color(color) = appearance.paint_at(0)? else { return None };
			Some(*color)
		};

		assert_eq!(color_of(0), Some(Color::BLACK), "a declared item should keep its own appearance");
		assert_eq!(color_of(1), Some(Color::WHITE), "a padded item should inherit the parent appearance");
	}
}

#[cfg(test)]
mod graphic_is_opaque_tests {
	use vector_types::GradientStop;

	use super::*;

	fn color_graphic(alpha: f64) -> Graphic<'static> {
		let color = Color::from_rgbaf32(1., 0., 0., alpha as f32).unwrap();
		Graphic::Color(color)
	}

	fn gradient_graphic(gradient: Gradient) -> Graphic<'static> {
		Graphic::Gradient(gradient)
	}

	#[test]
	fn opaque_color_is_opaque() {
		let g = color_graphic(1.);
		assert!(g.is_opaque());
	}

	#[test]
	fn transparent_color_is_not_opaque() {
		let g = color_graphic(0.5);
		assert!(!g.is_opaque());
	}

	#[test]
	fn vector_is_not_opaque() {
		let g = Graphic::Vector(Vector::default().into());
		assert!(!g.is_opaque());
	}

	#[test]
	fn gradient_with_all_opaque_stops_is_opaque() {
		let color_1 = Color::from_rgbaf32(1., 0., 0., 1.).unwrap();
		let color_2 = Color::from_rgbaf32(1., 0., 0., 1.).unwrap();
		let gradient = Gradient::new(vec![
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: color_1,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: color_2,
			},
		]);
		let g = gradient_graphic(gradient);
		assert!(g.is_opaque());
	}

	#[test]
	fn gradient_with_transparent_stop_is_not_opaque() {
		let color_1 = Color::from_rgbaf32(1., 0., 0., 0.5).unwrap();
		let color_2 = Color::from_rgbaf32(1., 0., 0., 1.).unwrap();
		let gradient = Gradient::new(vec![
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: color_1,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: color_2,
			},
		]);
		let g = gradient_graphic(gradient);
		assert!(!g.is_opaque());
	}
}

#[cfg(test)]
mod test_support {
	use super::Graphic;
	use core_types::list::List;
	use core_types::record::{RunBuilder, element_write_hashed};
	use glam::DVec2;
	use vector_types::Vector;
	use vector_types::kurbo::Shape;

	pub(in crate::graphic) fn unit_square_at(corner: DVec2) -> Vector {
		Vector::from_bezpath(vector_types::kurbo::Rect::new(corner.x, corner.y, corner.x + 1., corner.y + 1.).to_path(vector_types::kurbo::DEFAULT_ACCURACY))
	}

	pub(in crate::graphic) fn native_group_paint<'a>(vector: &Vector, arena: &'a core_types::arena::Arena) -> List<Graphic<'a>> {
		let mut builder = RunBuilder::new(arena, element_write_hashed::<Vector>(), &[], 1).unwrap();
		builder.push(vector.clone()).unwrap();
		List::new_from_element(Graphic::Group(core_types::record::Group { row: None, content: builder.finish() }))
	}
}

#[cfg(test)]
mod relift_tests {
	use super::Graphic;
	use core_types::arena::Arena;
	use core_types::record::{Group, RunBuilder, element_write_hashed};
	use dyn_any::Relift;
	use vector_types::Vector;

	/// `Relift` inverts the erasure a registry row keys on, so an element's `'static`
	/// spelling names the borrowing form that live code needs.
	#[test]
	fn a_graphic_relifts_to_the_arena_lifetime() {
		fn resident<'a>(arena: &'a Arena) -> <Graphic<'static> as Relift>::Live<'a> {
			let mut builder = RunBuilder::new(arena, element_write_hashed::<Vector>(), &[], 1).unwrap();
			builder.push(Vector::default()).unwrap();

			Graphic::Group(Group { row: None, content: builder.finish() })
		}

		let arena = Arena::new(1 << 16).unwrap();
		let live: Graphic<'_> = resident(&arena);

		assert!(matches!(live, Graphic::Group(_)), "the relifted spelling is the same type, only its borrows differ");
	}

	/// A borrow-free element relifts to itself, so code over it never spells a lifetime.
	#[test]
	fn a_borrow_free_element_relifts_to_itself() {
		fn same<'a>(vector: Vector) -> <Vector as Relift>::Live<'a> {
			vector
		}

		assert_eq!(same(Vector::default()), Vector::default());
	}
}
