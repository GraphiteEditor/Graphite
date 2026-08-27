use crate::markers::{ATTR_FILL, ATTR_STROKE, Fill, Stroke};
use core_types::attribute::{Attribute, EditorLayerPath, Opacity, OpacityFill, Transform};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::lane::{LaneColumn, LaneSource};
use core_types::list::{AttributeValueDyn, Item, ItemAttributeValues, List};
use core_types::ops::{FromAnchorPosition, ListConvert};
use core_types::render_complexity::RenderComplexity;
use core_types::uuid::NodeId;
use core_types::{ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;
pub use vector_types::Vector;

/// The possible forms of graphical content that can be rendered by the Render node into either an image or SVG syntax.
/// A leaf holds its element directly; its attributes ride the containing
/// lane. Multi-element content is a [`core_types::record::Group`] run, or
/// transitionally the legacy `Graphic` list.
#[derive(Clone, Debug, CacheHash, PartialEq, DynAny)]
pub enum Graphic {
	Graphic(List<Graphic>),
	Vector(Vector),
	RasterCPU(Raster<CPU>),
	RasterGPU(Raster<GPU>),
	Color(Color),
	Gradient(GradientStops),
	Text(String),
	Group(core_types::record::Group),
}

impl Default for Graphic {
	fn default() -> Self {
		Self::Graphic(List::new())
	}
}

// Graphic
impl From<List<Graphic>> for Graphic {
	fn from(graphic: List<Graphic>) -> Self {
		Graphic::Graphic(graphic)
	}
}

/// A typed legacy list as a legacy graphic list: each item de-tables to a
/// leaf element, keeping its attributes on the containing lane.
fn detable_items<T: Clone + Send + Sync + 'static>(list: List<T>, leaf: fn(T) -> Graphic) -> List<Graphic> {
	let mut out = List::new();
	for item in list.into_iter() {
		let (element, attributes) = item.into_parts();
		out.push(Item::from_parts(leaf(element), attributes));
	}
	out
}

// Vector
impl From<Vector> for Graphic {
	fn from(vector: Vector) -> Self {
		Graphic::Vector(vector)
	}
}
impl From<List<Vector>> for Graphic {
	fn from(vector: List<Vector>) -> Self {
		Graphic::Graphic(detable_items(vector, Graphic::Vector))
	}
}

// Note: List<Vector> -> List<Graphic> conversion handled by blanket impl in gcore

// Raster<CPU>
impl From<Raster<CPU>> for Graphic {
	fn from(raster: Raster<CPU>) -> Self {
		Graphic::RasterCPU(raster)
	}
}
impl From<List<Raster<CPU>>> for Graphic {
	fn from(raster: List<Raster<CPU>>) -> Self {
		Graphic::Graphic(detable_items(raster, Graphic::RasterCPU))
	}
}
// Note: List conversions handled by blanket impl in gcore

// Raster<GPU>
impl From<Raster<GPU>> for Graphic {
	fn from(raster: Raster<GPU>) -> Self {
		Graphic::RasterGPU(raster)
	}
}
impl From<List<Raster<GPU>>> for Graphic {
	fn from(raster: List<Raster<GPU>>) -> Self {
		Graphic::Graphic(detable_items(raster, Graphic::RasterGPU))
	}
}
// Note: List conversions handled by blanket impl in gcore

// Color
impl From<Color> for Graphic {
	fn from(color: Color) -> Self {
		Graphic::Color(color)
	}
}
impl From<List<Color>> for Graphic {
	fn from(color: List<Color>) -> Self {
		Graphic::Graphic(detable_items(color, Graphic::Color))
	}
}
// Note: List conversions handled by blanket impl in gcore
// Note: List<Color> -> Option<Color> is in gcore (Color is defined there)

// GradientStops
impl From<GradientStops> for Graphic {
	fn from(gradient: GradientStops) -> Self {
		Graphic::Gradient(gradient)
	}
}
impl From<List<GradientStops>> for Graphic {
	fn from(gradient: List<GradientStops>) -> Self {
		Graphic::Graphic(detable_items(gradient, Graphic::Gradient))
	}
}

// String
impl From<String> for Graphic {
	fn from(text: String) -> Self {
		Graphic::Text(text)
	}
}
impl From<List<String>> for Graphic {
	fn from(text: List<String>) -> Self {
		Graphic::Graphic(detable_items(text, Graphic::Text))
	}
}

/// Deeply flattens a `List<Graphic>`, collecting only elements matching a specific variant (extracted by `extract_variant`)
/// and discarding all other non-matching content. Recursion through `Graphic::Graphic` sub-`List`s composes transforms and opacity.
fn flatten_graphic_list<T>(content: List<Graphic>, extract_variant: fn(Graphic) -> Option<List<T>>) -> List<T> {
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

			let (element, attributes) = current_graphic_item.into_parts();
			match element {
				// Compose the parent's transform/opacity/fill onto each child, but only for attributes the parent carries.
				// A child lacking one is padded with the composition identity (`1.` for opacity/fill, identity for transform), so composing through it is a no-op.
				Graphic::Graphic(mut sub_list) => {
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

					flatten_recursive(output, sub_list, extract_variant, lane_layer_path.as_deref());
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

/// Whether a normalized paint graphic list actually carries renderable paint.
/// A 0-item list, or a list whose first graphic is empty, is treated as no paint.
pub fn is_paint_present(graphic_list: &List<Graphic>) -> bool {
	graphic_list.element(0).is_some_and(|graphic| !graphic.is_empty())
}

/// Look up the paint graphics stored under the marker `A`, in the canonical `List<Graphic>` form.
pub fn paint_graphics<'a, A, S>(source: &'a S, index: usize) -> Option<&'a List<Graphic>>
where
	S: LaneSource,
	A: Attribute<Value<'a> = Option<&'a List<Graphic>>>,
{
	source
		.attr::<A>(index)
		// Treat a blank paint attribute as absent so an empty attribute doesn't count as painted
		.filter(|graphic_list| is_paint_present(graphic_list))
}

/// Whether the item carries a non-blank canonical `List<Graphic>` paint under the marker `A`,
/// checked by borrowing without cloning the renderable list.
pub fn has_paint<'a, A, S>(source: &'a S, index: usize) -> bool
where
	S: LaneSource,
	A: Attribute<Value<'a> = Option<&'a List<Graphic>>>,
{
	paint_graphics::<A, S>(source, index).is_some()
}

/// Whether every lane of a vector source draws as a plain clip path: fully
/// opaque, fill absent or opaque, stroke invisible or fully transparent.
pub fn vector_can_reduce_to_clip_path<S: LaneSource<Element = Vector>>(source: &S) -> bool {
	(0..source.lane_count()).all(|index| {
		let Some(element) = source.element(index) else { return false };
		let opacity: f64 = source.attr::<Opacity>(index);

		let fill_opaque_or_absent = paint_graphics::<Fill, _>(source, index).is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_opaque()));

		let stroke_invisible_or_transparent = element.stroke.as_ref().is_none_or(|stroke| !stroke.has_renderable_stroke())
			|| paint_graphics::<Stroke, _>(source, index).is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_fully_transparent()));

		opacity > 1. - f64::EPSILON && fill_opaque_or_absent && stroke_invisible_or_transparent
	})
}

/// The paint a lane carries for its interiors, in the reference form
/// [`PaintOverlay`] threads down.
#[derive(Clone, Copy, Default)]
pub struct LanePaint<'a> {
	pub fill: Option<&'a List<Graphic>>,
	pub stroke: Option<&'a List<Graphic>>,
}

impl<'a> LanePaint<'a> {
	pub const NONE: Self = Self { fill: None, stroke: None };

	pub fn is_present(&self) -> bool {
		self.fill.is_some() || self.stroke.is_some()
	}
}

/// A source's fill and stroke columns, resolved once for per-lane reads.
pub struct PaintColumns<'a, S: LaneSource + 'a> {
	fill: S::Column<'a, Fill>,
	stroke: S::Column<'a, Stroke>,
}

impl<'a, S: LaneSource> PaintColumns<'a, S> {
	pub fn new(source: &'a S) -> Self {
		Self {
			fill: source.column::<Fill>(),
			stroke: source.column::<Stroke>(),
		}
	}

	/// The lane's present, non-blank paint.
	pub fn read(&self, lane: usize) -> LanePaint<'a> {
		let present = |value: Option<Option<&'a List<Graphic>>>| value.flatten().filter(|list| is_paint_present(list));
		LanePaint {
			fill: present(self.fill.try_get(lane)),
			stroke: present(self.stroke.try_get(lane)),
		}
	}
}

/// How far a lane's paint reaches into the element beneath it, mirroring the
/// legacy conversion's paint push: vector interiors directly and vector
/// children of a nested graphic list, one level deep.
#[derive(Clone, Copy)]
pub struct PaintReach<'a> {
	pub paint: LanePaint<'a>,
	hops: u8,
}

impl<'a> PaintReach<'a> {
	pub const NONE: Self = Self { paint: LanePaint::NONE, hops: 0 };

	/// The lane's effective reach: an inherited paint stays authoritative
	/// (lane paint below a push's origin is inert in the legacy model), an
	/// absent one reads the lane's own paint.
	pub fn for_lane<S: LaneSource>(self, columns: &PaintColumns<'a, S>, index: usize) -> Self {
		match self.paint.is_present() {
			true => self,
			false => Self { paint: columns.read(index), hops: 2 },
		}
	}

	pub fn applies(&self) -> bool {
		self.hops > 0 && self.paint.is_present()
	}

	/// The reach one graphic nesting level further down.
	pub fn nested(self) -> Self {
		Self {
			paint: self.paint,
			hops: self.hops.saturating_sub(1),
		}
	}

	/// The reach entering a group's own graphic run: a spent or absent reach
	/// resets so the group's own lane paint applies at its own boundary.
	pub fn into_group_graphics(self) -> Self {
		match self.applies() {
			true => self.nested(),
			false => Self::NONE,
		}
	}
}

/// A source with a lane's paint forced over its fill and stroke columns,
/// reaching the interiors the legacy conversion's paint push reached.
pub struct PaintOverlay<'a, S> {
	inner: &'a S,
	paint: LanePaint<'a>,
}

impl<'a, S> PaintOverlay<'a, S> {
	pub fn new(inner: &'a S, paint: LanePaint<'a>) -> Self {
		Self { inner, paint }
	}
}

pub struct PaintOverlayColumn<'a, S: LaneSource + 'a, A: Attribute> {
	inner: S::Column<'a, A>,
	forced: Option<A::Value<'a>>,
}

impl<'a, S: LaneSource, A: Attribute> LaneColumn<'a, A> for PaintOverlayColumn<'a, S, A> {
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>> {
		match self.forced {
			Some(forced) => Some(forced),
			None => self.inner.try_get(lane),
		}
	}
}

/// The forced value for the marker `A`: the lane paint where `A` is this
/// crate's fill or stroke marker, absent otherwise.
fn forced_paint<'a, A: Attribute>(paint: LanePaint<'a>) -> Option<A::Value<'a>> {
	let slot = match A::NAME {
		name if name == Fill::NAME => paint.fill,
		name if name == Stroke::NAME => paint.stroke,
		_ => None,
	}?;
	// SAFETY: the census admits one value type per attribute name and panics on
	// a conflict at registration, so a marker named `fill` or `stroke` carries
	// this crate's `Option<&List<Graphic>>` value form.
	Some(unsafe { std::mem::transmute_copy::<Option<&'a List<Graphic>>, A::Value<'a>>(&Some(slot)) })
}

impl<'a, S: LaneSource> LaneSource for PaintOverlay<'a, S> {
	type Element = S::Element;
	type Column<'b, A: Attribute>
		= PaintOverlayColumn<'b, S, A>
	where
		Self: 'b;

	fn lane_count(&self) -> usize {
		self.inner.lane_count()
	}

	fn element(&self, lane: usize) -> Option<&S::Element> {
		self.inner.element(lane)
	}

	fn column<A: Attribute>(&self) -> PaintOverlayColumn<'_, S, A> {
		PaintOverlayColumn {
			inner: self.inner.column::<A>(),
			forced: forced_paint::<A>(self.paint),
		}
	}
}

/// Stores a paint attribute in the paint marker's owned form, the only representation paint readers accept.
pub fn set_paint_attribute(attributes: &mut ItemAttributeValues, key: &str, paint: impl IntoGraphicList) {
	attributes.insert(key, Some(paint.into_graphic_list()));
}

/// Stores a paint attribute at a list index in the paint marker's owned form, the only representation paint readers accept.
pub fn set_paint_attribute_at<T>(list: &mut List<T>, index: usize, key: &str, paint: impl IntoGraphicList) {
	list.set_attribute(key, index, Some(paint.into_graphic_list()));
}

/// Bake the provided transform into the per-item transforms of the paint graphics stored under the
/// canonical `List<Graphic>` fill and stroke attributes.
pub fn bake_paint_transforms(attributes: &mut ItemAttributeValues, transform: DAffine2) {
	fn bake_graphic_paint_transform(graphics: &mut List<Graphic>, transform: DAffine2) {
		for item_transform in graphics.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*item_transform = transform * *item_transform;
		}
		for graphic in graphics.iter_element_values_mut() {
			if let Graphic::Graphic(list) = graphic {
				bake_graphic_paint_transform(list, transform);
			}
		}
	}

	for paint_key in [ATTR_FILL, ATTR_STROKE] {
		if let Some(Some(graphics)) = attributes.get_mut::<Option<List<Graphic>>>(paint_key) {
			bake_graphic_paint_transform(graphics, transform);
		}
	}
}

/// Maps from a concrete element type to its corresponding `Graphic` enum variant,
/// enabling type-directed casting of typed `List`s from a `Graphic` value.
pub trait TryFromGraphic: Clone + Sized {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>>;
}

impl TryFromGraphic for Vector {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Vector(t) = graphic { Some(List::new_from_element(t)) } else { None }
	}
}

impl TryFromGraphic for Raster<CPU> {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::RasterCPU(t) = graphic { Some(List::new_from_element(t)) } else { None }
	}
}

impl TryFromGraphic for Color {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Color(t) = graphic { Some(List::new_from_element(t)) } else { None }
	}
}

impl TryFromGraphic for GradientStops {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Gradient(t) = graphic { Some(List::new_from_element(t)) } else { None }
	}
}

impl TryFromGraphic for String {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Text(t) = graphic { Some(List::new_from_element(t)) } else { None }
	}
}

// Local trait to convert types to List<Graphic> (avoids orphan rule issues)
pub trait IntoGraphicList: Clone + Send + Sync + Default + std::fmt::Debug + PartialEq + CacheHash + 'static {
	fn into_graphic_list(self) -> List<Graphic>;

	/// Deeply flattens any content of type `T` within a `List<Graphic>`, discarding all other content, and returning a flat `List<T>`.
	fn into_flattened_list<T: TryFromGraphic>(self) -> List<T>
	where
		Self: std::marker::Sized,
	{
		flatten_graphic_list(self.into_graphic_list(), T::try_from_graphic)
	}
}

impl IntoGraphicList for List<Graphic> {
	fn into_graphic_list(self) -> List<Graphic> {
		self
	}
}

impl IntoGraphicList for List<Vector> {
	fn into_graphic_list(self) -> List<Graphic> {
		detable_items(self, Graphic::Vector)
	}
}

impl IntoGraphicList for List<Raster<CPU>> {
	fn into_graphic_list(self) -> List<Graphic> {
		detable_items(self, Graphic::RasterCPU)
	}
}

impl IntoGraphicList for List<Raster<GPU>> {
	fn into_graphic_list(self) -> List<Graphic> {
		detable_items(self, Graphic::RasterGPU)
	}
}

impl IntoGraphicList for List<Color> {
	fn into_graphic_list(self) -> List<Graphic> {
		detable_items(self, Graphic::Color)
	}
}

impl IntoGraphicList for List<GradientStops> {
	fn into_graphic_list(self) -> List<Graphic> {
		detable_items(self, Graphic::Gradient)
	}
}

impl IntoGraphicList for List<String> {
	fn into_graphic_list(self) -> List<Graphic> {
		detable_items(self, Graphic::Text)
	}
}

impl IntoGraphicList for DAffine2 {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::default())
	}
}

// DAffine2
impl From<DAffine2> for Graphic {
	fn from(_: DAffine2) -> Self {
		Graphic::default()
	}
}

// DVec2
impl From<DVec2> for Graphic {
	fn from(position: DVec2) -> Self {
		Graphic::Vector(Vector::from_anchor_position(position))
	}
}
// Note: List conversions handled by blanket impl in gcore

impl Graphic {
	pub fn as_graphic(&self) -> Option<&List<Graphic>> {
		match self {
			Graphic::Graphic(graphic) => Some(graphic),
			_ => None,
		}
	}

	pub fn as_graphic_mut(&mut self) -> Option<&mut List<Graphic>> {
		match self {
			Graphic::Graphic(graphic) => Some(graphic),
			_ => None,
		}
	}

	pub fn as_vector(&self) -> Option<&Vector> {
		match self {
			Graphic::Vector(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_vector_mut(&mut self) -> Option<&mut Vector> {
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
			Graphic::Graphic(list) => all_clipped(list),
			Graphic::Group(group) => group_all_clipped(group),
			_ => false,
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			Graphic::Vector(vector) => vector_can_reduce_to_clip_path(&core_types::lane::Single(vector)),
			_ => false,
		}
	}

	pub fn is_opaque(&self) -> bool {
		match self {
			Graphic::Graphic(list) => !list.is_empty() && list.iter_element_values().all(Graphic::is_opaque),
			// A bare leaf carries no paint attribute, which rides its lane, so
			// nothing here claims opacity.
			Graphic::Vector(_) => false,
			Graphic::Color(color) => color.is_opaque(),
			Graphic::Gradient(stops) => stops.iter().all(|stop| stop.color.is_opaque()),
			Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Text(_) => false,
			Graphic::Group(group) => group_is_opaque(group),
		}
	}

	pub fn is_fully_transparent(&self) -> bool {
		match self {
			Graphic::Graphic(list) => list.iter_element_values().all(Graphic::is_fully_transparent),
			// A bare leaf carries no paint attribute, so only an unstroked
			// vector is invisible on its own.
			Graphic::Vector(vector) => vector.stroke.as_ref().is_none_or(|stroke| !stroke.has_renderable_stroke()),
			Graphic::Color(color) => color.a() == 0.,
			Graphic::Gradient(stops) => stops.iter().all(|stop| stop.color.a() == 0.),
			Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Text(_) => false,
			Graphic::Group(group) => group_is_fully_transparent(group),
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
			Graphic::Graphic(list) => list.is_empty(),
			Graphic::Group(group) => group_is_empty(group),
			_ => false,
		}
	}
}

/// One run's attribute offsets, resolved once so the lane loops read raw.
struct RunAttrs {
	transform: Option<usize>,
	opacity: Option<usize>,
	opacity_fill: Option<usize>,
	clipping_mask: Option<usize>,
}

impl RunAttrs {
	fn of(item: &core_types::record::GroupItem) -> Self {
		let layout = item.layout();
		Self {
			transform: layout.offset_of(ATTR_TRANSFORM, 0),
			opacity: layout.offset_of(ATTR_OPACITY, 0),
			opacity_fill: layout.offset_of(ATTR_OPACITY_FILL, 0),
			clipping_mask: layout.offset_of(ATTR_CLIPPING_MASK, 0),
		}
	}

	fn read_or<T: Copy>(item: &core_types::record::GroupItem, offset: Option<usize>, lane: usize, default: T) -> T {
		match offset {
			// SAFETY: the offset comes from the item's own layout.
			Some(offset) => unsafe { item.lanes().get(lane).rec().read(offset) },
			None => default,
		}
	}
}

pub fn group_is_empty(group: &core_types::record::Group) -> bool {
	group.content.is_empty()
}

fn group_all_clipped(group: &core_types::record::Group) -> bool {
	let item = &group.content;
	let attrs = RunAttrs::of(item);
	(0..item.len()).all(|lane| RunAttrs::read_or(item, attrs.clipping_mask, lane, false))
}

fn group_is_opaque(group: &core_types::record::Group) -> bool {
	let item = &group.content;
	let attrs = RunAttrs::of(item);
	let lanes = item.typed_lanes::<Graphic>();
	!item.is_empty()
		&& (0..item.len()).all(|lane| {
			RunAttrs::read_or(item, attrs.opacity, lane, 1.) >= 1.
				&& RunAttrs::read_or(item, attrs.opacity_fill, lane, 1.) >= 1.
				&& lanes.as_ref().is_some_and(|lanes| lanes.element_ref(lane).is_opaque())
		})
}

fn group_is_fully_transparent(group: &core_types::record::Group) -> bool {
	let item = &group.content;
	let attrs = RunAttrs::of(item);
	let lanes = item.typed_lanes::<Graphic>();
	(0..item.len()).all(|lane| RunAttrs::read_or(item, attrs.opacity, lane, 1.) <= 0. || lanes.as_ref().is_some_and(|lanes| lanes.element_ref(lane).is_fully_transparent()))
}

fn group_bounding_box(group: &core_types::record::Group, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> RenderBoundingBox {
	fn combine(combined: &mut Option<[DVec2; 2]>, any_infinite: &mut bool, bounds: RenderBoundingBox, thumbnail: bool) -> Option<RenderBoundingBox> {
		match bounds {
			RenderBoundingBox::None => None,
			RenderBoundingBox::Infinite if thumbnail => {
				*any_infinite = true;
				None
			}
			RenderBoundingBox::Infinite => Some(RenderBoundingBox::Infinite),
			RenderBoundingBox::Rectangle(bounds) => {
				*combined = Some(match *combined {
					Some(existing) => core_types::math::quad::Quad::combine_bounds(existing, bounds),
					None => bounds,
				});
				None
			}
		}
	}
	fn typed_run<T: 'static + BoundingBox>(item: &core_types::record::GroupItem, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> Option<RenderBoundingBox> {
		let lanes = item.typed_lanes::<T>()?;
		let transform_offset = RunAttrs::of(item).transform;
		let mut combined = None;
		let mut any_infinite = false;
		for lane in 0..lanes.len() {
			let lane_transform = transform * RunAttrs::read_or(item, transform_offset, lane, DAffine2::IDENTITY);
			let element = lanes.element_ref(lane);
			let bounds = match thumbnail {
				true => element.thumbnail_bounding_box(lane_transform, include_stroke),
				false => element.bounding_box(lane_transform, include_stroke),
			};
			if let Some(short_circuit) = combine(&mut combined, &mut any_infinite, bounds, thumbnail) {
				return Some(short_circuit);
			}
		}
		Some(match (combined, any_infinite) {
			(Some(bounds), _) => RenderBoundingBox::Rectangle(bounds),
			(None, true) => RenderBoundingBox::Infinite,
			(None, false) => RenderBoundingBox::None,
		})
	}
	fn run_bounding_box(item: &core_types::record::GroupItem, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> RenderBoundingBox {
		None.or_else(|| typed_run::<Graphic>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Vector>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Raster<CPU>>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Raster<GPU>>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<Color>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<GradientStops>(item, transform, include_stroke, thumbnail))
			.or_else(|| typed_run::<String>(item, transform, include_stroke, thumbnail))
			.unwrap_or(RenderBoundingBox::Infinite)
	}
	run_bounding_box(&group.content, transform, include_stroke, thumbnail)
}

/// One typed run as an owned list, elements cloned and every attribute copied
/// through its erased read. Content keeps its native form; the legacy
/// conversions layer their mapping on top.
pub fn run_to_list<T: Clone + Send + Sync + 'static>(item: &core_types::record::GroupItem) -> Option<List<T>> {
	let lanes = item.typed_lanes::<T>()?;
	let mut list = List::new();
	for lane in 0..lanes.len() {
		list.push(Item::new_from_element(lanes.element_ref(lane).clone()));
	}
	for field in &item.layout().fields {
		for lane in 0..lanes.len() {
			// SAFETY: the offset comes from the item's own layout.
			let value = unsafe { (field.read_erased)(item.lanes().get(lane).rec().ptr().add(field.offset)) };
			list.set_attribute_value_dyn(field.name, lane, AttributeValueDyn(value));
		}
	}
	Some(list)
}

/// Converts the group content of the list's paint attribute values to legacy
/// form, so a legacy product owns everything its attributes reach.
pub fn map_paint_attrs_to_legacy<T: 'static>(list: &mut List<T>) {
	for key in [ATTR_FILL, ATTR_STROKE, crate::markers::ATTR_EDITOR_MERGED_LAYERS] {
		let Some(values) = list.iter_attribute_values_mut::<Option<List<Graphic>>>(key) else { continue };
		for value in values.flatten() {
			for element in value.iter_element_values_mut() {
				*element = map_groups_to_legacy(element);
			}
		}
	}
}

/// One typed run as a legacy list: [`run_to_list`] with the paint attribute
/// contents converted to their legacy form.
pub(crate) fn run_to_legacy_list<T: Clone + Send + Sync + 'static>(item: &core_types::record::GroupItem) -> Option<List<T>> {
	let mut list = run_to_list::<T>(item)?;
	map_paint_attrs_to_legacy(&mut list);
	Some(list)
}

/// One step of the vector-row walk: continue to the next row or stop early.
pub enum RowStep {
	Continue,
	Stop,
}

/// The ancestor composition a flattened row inherits: transform, opacity and
/// fill opacity multiply down, each composing only where some ancestor
/// carries the attribute, matching the legacy flatten.
#[derive(Clone, Copy)]
struct FlattenScale {
	has_transform: bool,
	transform: DAffine2,
	has_opacity: bool,
	opacity: f64,
	has_fill_opacity: bool,
	fill_opacity: f64,
}

impl FlattenScale {
	const ROOT: Self = Self {
		has_transform: false,
		transform: DAffine2::IDENTITY,
		has_opacity: false,
		opacity: 1.,
		has_fill_opacity: false,
		fill_opacity: 1.,
	};

	fn composed<S: LaneSource>(self, source: &S, lane: usize) -> Self {
		let transform = source.try_attr::<Transform>(lane);
		let opacity = source.try_attr::<Opacity>(lane);
		let fill_opacity = source.try_attr::<OpacityFill>(lane);
		Self {
			has_transform: self.has_transform || transform.is_some(),
			transform: self.transform * transform.unwrap_or(DAffine2::IDENTITY),
			has_opacity: self.has_opacity || opacity.is_some(),
			opacity: self.opacity * opacity.unwrap_or(1.),
			has_fill_opacity: self.has_fill_opacity || fill_opacity.is_some(),
			fill_opacity: self.fill_opacity * fill_opacity.unwrap_or(1.),
		}
	}
}

/// A graphic level in either of its two storages, as one lane source.
#[derive(Clone, Copy)]
pub enum GraphicLevel<'a> {
	Legacy(&'a List<Graphic>),
	Run(&'a core_types::record::GroupItem),
}

pub enum GraphicLevelColumn<'a, A: Attribute> {
	Legacy(core_types::list::ListColumn<'a, A>),
	Run(core_types::record::RunColumn<'a, A>),
}

impl<'a, A: Attribute> core_types::lane::LaneColumn<'a, A> for GraphicLevelColumn<'a, A> {
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>> {
		match self {
			GraphicLevelColumn::Legacy(column) => column.try_get(lane),
			GraphicLevelColumn::Run(column) => column.try_get(lane),
		}
	}
}

impl<'a> LaneSource for GraphicLevel<'a> {
	type Element = Graphic;
	type Column<'b, A: Attribute>
		= GraphicLevelColumn<'b, A>
	where
		Self: 'b;

	fn lane_count(&self) -> usize {
		match self {
			GraphicLevel::Legacy(list) => list.len(),
			GraphicLevel::Run(item) => item.len(),
		}
	}

	fn element(&self, lane: usize) -> Option<&Graphic> {
		match self {
			GraphicLevel::Legacy(list) => list.element(lane),
			GraphicLevel::Run(item) => {
				let lanes = item.typed_lanes::<Graphic>()?;
				if lane >= lanes.len() {
					return None;
				}
				// SAFETY: the layout records the element type, and a parked
				// element stores its reference at offset 0.
				Some(unsafe { core_types::record::borrow_element::<Graphic>(item.lanes().get(lane).rec()) })
			}
		}
	}

	fn column<A: Attribute>(&self) -> GraphicLevelColumn<'_, A> {
		match self {
			GraphicLevel::Legacy(list) => GraphicLevelColumn::Legacy(list.column::<A>()),
			GraphicLevel::Run(item) => GraphicLevelColumn::Run(core_types::record::RunColumn::of(item)),
		}
	}
}

/// The lane's attributes as an owned set, read through the erased glue.
pub fn run_lane_attributes(item: &core_types::record::GroupItem, lane: usize) -> ItemAttributeValues {
	let mut scratch: List<Vector> = List::new_from_element(Vector::default());
	for field in &item.layout().fields {
		// SAFETY: the offset comes from the item's own layout.
		let value = unsafe { (field.read_erased)(item.lanes().get(lane).rec().ptr().add(field.offset)) };
		scratch.set_attribute_value_dyn(field.name, 0, AttributeValueDyn(value));
	}
	scratch.clone_item_attributes(0)
}

/// The lane's attributes as an owned set, from either level storage.
pub fn lane_attributes(level: GraphicLevel<'_>, lane: usize) -> ItemAttributeValues {
	match level {
		GraphicLevel::Legacy(list) => list.clone_item_attributes(lane),
		GraphicLevel::Run(item) => run_lane_attributes(item, lane),
	}
}

/// One flattened vector row served by [`walk_vector_rows`]: cheap probes
/// first, the full row built on demand.
pub struct VectorRow<'w> {
	source: RowSourceRef<'w>,
	scale: FlattenScale,
	layer_path: Option<&'w [NodeId]>,
	paint: LanePaint<'w>,
}

enum RowSourceRef<'w> {
	/// A de-tabled vector leaf on a graphic lane: the lane is the row.
	Lane(GraphicLevel<'w>, usize),
	/// A lane of a vector run.
	Run(&'w core_types::record::RunView<'w, Vector>, &'w core_types::record::GroupItem, usize),
}

impl VectorRow<'_> {
	/// The row's vector, borrowed.
	pub fn element(&self) -> &Vector {
		match &self.source {
			RowSourceRef::Lane(level, index) => level.element(*index).and_then(Graphic::as_vector).expect("the walk visits vector lanes"),
			RowSourceRef::Run(run, _, index) => LaneSource::element(*run, *index).expect("the walk visits held lanes"),
		}
	}

	/// Whether the built row will carry fill paint: the reaching lane paint,
	/// else the row's own.
	pub fn has_fill(&self) -> bool {
		if self.paint.fill.is_some() {
			return true;
		}
		match &self.source {
			RowSourceRef::Lane(level, index) => paint_graphics::<Fill, _>(level, *index).is_some(),
			RowSourceRef::Run(run, _, index) => paint_graphics::<Fill, _>(*run, *index).is_some(),
		}
	}

	/// Builds the row at the end of `out`, applying the reach paint and the
	/// inherited composition.
	pub fn build_into(&self, out: &mut List<Vector>) {
		let index = out.len();
		match &self.source {
			RowSourceRef::Lane(level, lane) => {
				let vector = self.element().clone();
				out.push(Item::from_parts(vector, lane_attributes(*level, *lane)));
			}
			RowSourceRef::Run(run, item, lane) => {
				let vector = LaneSource::element(*run, *lane).expect("the walk visits held lanes").clone();
				out.push(Item::from_parts(vector, run_lane_attributes(item, *lane)));
			}
		}
		for (key, slot) in [(ATTR_FILL, self.paint.fill), (ATTR_STROKE, self.paint.stroke)] {
			if let Some(paint) = slot {
				set_paint_attribute_at(out, index, key, paint.clone());
			}
		}
		if self.scale.has_transform || out.attribute::<DAffine2>(ATTR_TRANSFORM, index).is_some() {
			let row_transform: DAffine2 = out.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			out.set_attribute(ATTR_TRANSFORM, index, self.scale.transform * row_transform);
		}
		if self.scale.has_opacity || out.attribute::<f64>(ATTR_OPACITY, index).is_some() {
			let row_opacity: f64 = out.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			out.set_attribute(ATTR_OPACITY, index, self.scale.opacity * row_opacity);
		}
		if self.scale.has_fill_opacity || out.attribute::<f64>(ATTR_OPACITY_FILL, index).is_some() {
			let row_fill: f64 = out.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			out.set_attribute(ATTR_OPACITY_FILL, index, self.scale.fill_opacity * row_fill);
		}
		if let Some(layer_path) = self.layer_path {
			out.set_attribute(ATTR_EDITOR_LAYER_PATH, index, layer_path.to_vec());
		}
	}
}

fn walk_rows_of_run(
	item: &core_types::record::GroupItem,
	scale: FlattenScale,
	layer_path: Option<&[NodeId]>,
	paint: LanePaint<'_>,
	visit: &mut dyn FnMut(VectorRow<'_>) -> RowStep,
) -> RowStep {
	let Some(run) = core_types::record::RunView::<Vector>::new(item) else {
		return RowStep::Continue;
	};
	for lane in 0..item.len() {
		if let RowStep::Stop = visit(VectorRow {
			source: RowSourceRef::Run(&run, item, lane),
			scale,
			layer_path,
			paint,
		}) {
			return RowStep::Stop;
		}
	}
	RowStep::Continue
}

/// Walks a graphic level into its flattened vector rows, matching the legacy
/// push-then-flatten lowering: lane paint threads with [`PaintReach`],
/// ancestor transform, opacity and fill opacity compose down, the containing
/// level's parent layer path overwrites its rows, and non-vector content is
/// discarded. A de-tabled leaf's row is its lane, attributes included.
pub fn walk_vector_rows(level: GraphicLevel<'_>, visit: &mut dyn FnMut(VectorRow<'_>) -> RowStep) {
	walk_vector_rows_impl(level, FlattenScale::ROOT, None, PaintReach::NONE, visit);
}

fn walk_vector_rows_impl<'a>(
	level: GraphicLevel<'a>,
	scale: FlattenScale,
	parent_layer_path: Option<&'a [NodeId]>,
	inherited: PaintReach<'a>,
	visit: &mut dyn FnMut(VectorRow<'_>) -> RowStep,
) -> RowStep {
	if let GraphicLevel::Run(item) = level {
		// A vector-typed run is already its rows.
		if item.typed_lanes::<Vector>().is_some() {
			let paint = match inherited.applies() {
				true => inherited.paint,
				false => LanePaint::NONE,
			};
			return walk_rows_of_run(item, scale, parent_layer_path, paint, visit);
		}
	}
	let columns = PaintColumns::new(&level);
	for index in 0..level.lane_count() {
		let Some(element) = level.element(index) else { continue };
		let reach = inherited.for_lane(&columns, index);
		let row_paint = match reach.applies() {
			true => reach.paint,
			false => LanePaint::NONE,
		};
		let step = match element {
			Graphic::Vector(_) => visit(VectorRow {
				source: RowSourceRef::Lane(level, index),
				scale,
				layer_path: parent_layer_path,
				paint: row_paint,
			}),
			Graphic::Graphic(children) => walk_vector_rows_impl(
				GraphicLevel::Legacy(children),
				scale.composed(&level, index),
				level.try_attr::<EditorLayerPath>(index),
				reach.nested(),
				visit,
			),
			Graphic::Group(group) => {
				let item = &group.content;
				if item.typed_lanes::<Vector>().is_some() {
					walk_rows_of_run(item, scale.composed(&level, index), level.try_attr::<EditorLayerPath>(index), row_paint, visit)
				} else if item.typed_lanes::<Graphic>().is_some() {
					walk_vector_rows_impl(
						GraphicLevel::Run(item),
						scale.composed(&level, index),
						level.try_attr::<EditorLayerPath>(index),
						reach.into_group_graphics(),
						visit,
					)
				} else {
					RowStep::Continue
				}
			}
			_ => RowStep::Continue,
		};
		if let RowStep::Stop = step {
			return RowStep::Stop;
		}
	}
	RowStep::Continue
}

/// The level's flattened vector rows as one owned list, the walk's collect
/// form.
pub fn flatten_vector_rows(level: GraphicLevel<'_>) -> List<Vector> {
	let mut out = List::new();
	walk_vector_rows(level, &mut |row| {
		row.build_into(&mut out);
		RowStep::Continue
	});
	out
}

/// One typed run as the legacy list its `Render` impl consumes, nested
/// groups converted to their legacy form.
pub fn run_to_render_list<T: Clone + Send + Sync + 'static>(item: &core_types::record::GroupItem) -> Option<List<T>> {
	let mut list = run_to_legacy_list::<T>(item)?;
	if let Some(graphics) = (&mut list as &mut dyn std::any::Any).downcast_mut::<List<Graphic>>() {
		for element in graphics.iter_element_values_mut() {
			*element = map_groups_to_legacy(element);
		}
		push_lane_paint_into_interiors(graphics);
	}
	Some(list)
}

/// The transitional paint placement: a lane-level fill or stroke paint
/// attribute moves onto the vector interiors the legacy paint readers
/// inspect, reaching as far as the pre-flip broadcast did.
fn push_lane_paint_into_interiors(list: &mut List<Graphic>) {
	for index in 0..list.len() {
		for key in [ATTR_FILL, ATTR_STROKE] {
			let stored = list.attribute::<Option<List<Graphic>>>(key, index).and_then(|optional| optional.as_ref());
			let Some(paint) = stored.filter(|paint| is_paint_present(paint)).cloned() else {
				continue;
			};
			let Some(Graphic::Graphic(children)) = list.element_mut(index) else { continue };
			for child in 0..children.len() {
				if matches!(children.element(child), Some(Graphic::Vector(_))) {
					set_paint_attribute_at(children, child, key, paint.clone());
				}
			}
		}
	}
}

/// The graphic with every `Group` deep-copied to its owned form, which
/// survives the arena generation but cannot be read until
/// [`map_groups_to_resident`] re-parks it into a serving arena.
pub fn map_groups_to_owned(graphic: &Graphic) -> Graphic {
	match graphic {
		Graphic::Group(group) => Graphic::Group(group.copy_out()),
		Graphic::Graphic(children) => {
			let mut children = children.clone();
			for child in children.iter_element_values_mut() {
				*child = map_groups_to_owned(child);
			}
			Graphic::Graphic(children)
		}
		other => other.clone(),
	}
}

/// The graphic with every owned `Group` re-parked into `arena`; `None`
/// reports arena exhaustion.
pub fn map_groups_to_resident(graphic: &Graphic, arena: &core_types::arena::Arena) -> Option<Graphic> {
	match graphic {
		Graphic::Group(group) => group.replay(arena).map(Graphic::Group),
		Graphic::Graphic(children) => {
			let mut children = children.clone();
			for child in children.iter_element_values_mut() {
				*child = map_groups_to_resident(child, arena)?;
			}
			Some(Graphic::Graphic(children))
		}
		other => Some(other.clone()),
	}
}

/// The deep copy-out for `Graphic` elements: a plain clone of a group
/// interior would carry frame pointers into the evaluation's arena, so memo
/// and capture seams copy out the owned-group form.
///
/// # Safety
/// `ptr` must point at a live parked `Graphic` element field.
unsafe fn deep_clone_graphic(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
	let graphic = unsafe { core_types::record::borrow_element::<Graphic>(core_types::record::Rec::new(ptr)) };
	Box::new(map_groups_to_owned(graphic))
}

/// The deep re-park for `Graphic` elements: owned groups replay into the
/// serving arena before the graphic parks.
///
/// # Safety
/// `value` must hold a `Graphic` and `dst` must be a live `Graphic` element
/// field.
unsafe fn deep_repark_graphic(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &core_types::arena::Arena) -> Option<()> {
	let graphic = value.downcast_ref::<Graphic>().expect("an element replays at its own type");
	let resident = map_groups_to_resident(graphic, arena)?;
	unsafe { core_types::record::write_element(dst, resident, arena) }
}

fn graphic_contains_groups(graphic: &Graphic) -> bool {
	match graphic {
		Graphic::Group(_) => true,
		Graphic::Graphic(children) => list_contains_groups(children),
		_ => false,
	}
}

fn list_contains_groups(list: &List<Graphic>) -> bool {
	(0..list.len()).any(|index| list.element(index).is_some_and(graphic_contains_groups))
}

/// The deep copy-out for graphic-list field values (the paint markers' owned
/// form): content groups leave in their owned form. Declines (`None`) for
/// group-free content, which already owns everything.
fn deep_clone_graphic_list(value: &dyn core_types::list::AnyAttributeValue) -> Option<Box<dyn core_types::list::AnyAttributeValue>> {
	let list = value.as_any().downcast_ref::<Option<List<Graphic>>>().expect("a graphic list field deep-copies at its own type");
	let list = list.as_ref().filter(|list| list_contains_groups(list))?;
	let mut list = list.clone();
	for element in list.iter_element_values_mut() {
		*element = map_groups_to_owned(element);
	}
	Some(Box::new(Some(list)))
}

/// The deep replay for graphic-list field values: owned content groups replay
/// into the serving arena before the field re-parks. `Some(None)` declines
/// for group-free content; `None` reports arena exhaustion.
fn deep_repark_graphic_list(value: &dyn core_types::list::AnyAttributeValue, arena: &core_types::arena::Arena) -> Option<Option<Box<dyn core_types::list::AnyAttributeValue>>> {
	let list = value.as_any().downcast_ref::<Option<List<Graphic>>>().expect("a graphic list field replays at its own type");
	let Some(list) = list.as_ref().filter(|list| list_contains_groups(list)) else {
		return Some(None);
	};
	let mut list = list.clone();
	for element in list.iter_element_values_mut() {
		*element = map_groups_to_resident(element, arena)?;
	}
	Some(Some(Box::new(Some(list))))
}

const _: () = {
	fn register_all() {
		core_types::record::register_deep_element_clone::<Graphic>(deep_clone_graphic, deep_repark_graphic);
		core_types::record::register_deep_field_value::<Option<List<Graphic>>>(deep_clone_graphic_list, deep_repark_graphic_list);
	}

	#[cfg(not(target_family = "wasm"))]
	#[core_types::ctor::ctor]
	fn register() {
		register_all();
	}

	#[cfg(target_family = "wasm")]
	#[unsafe(export_name = "__node_registry_deep_element_graphic")]
	extern "C" fn register() {
		register_all();
	}
};

/// The graphic with every `Group` converted to its legacy form.
/// The count [`map_groups_to_legacy`] would expose through [`Graphic::as_vector`],
/// read from the run's lanes instead of materializing the legacy list. Mirrors
/// [`group_to_legacy_graphic`]'s typed-run path, where `Vector` is tried first.
pub fn direct_vector_len(graphic: &Graphic) -> usize {
	match graphic {
		Graphic::Vector(_) => 1,
		Graphic::Group(group) => match &group.row {
			None => group.content.typed_lanes::<Vector>().map_or(0, |lanes| lanes.len()),
			_ => 0,
		},
		_ => 0,
	}
}

pub fn map_groups_to_legacy(graphic: &Graphic) -> Graphic {
	match graphic {
		Graphic::Group(group) => group_to_legacy_graphic(group),
		Graphic::Graphic(children) => {
			let mut children = children.clone();
			for child in children.iter_element_values_mut() {
				*child = map_groups_to_legacy(child);
			}
			map_paint_attrs_to_legacy(&mut children);
			Graphic::Graphic(children)
		}
		other => other.clone(),
	}
}

/// The group as one legacy graphic. A bare (row-less) wrap of a single typed
/// run keeps the run's typed variant, matching the `Into<Graphic>` the
/// pre-flip wrap applied; everything else becomes the legacy group list.
pub fn group_to_legacy_graphic(group: &core_types::record::Group) -> Graphic {
	if group.row.is_none() {
		let item = &group.content;
		let typed = None
			.or_else(|| run_to_legacy_list::<Vector>(item).map(|list| detable_items(list, Graphic::Vector)))
			.or_else(|| run_to_legacy_list::<Raster<CPU>>(item).map(|list| detable_items(list, Graphic::RasterCPU)))
			.or_else(|| run_to_legacy_list::<Raster<GPU>>(item).map(|list| detable_items(list, Graphic::RasterGPU)))
			.or_else(|| run_to_legacy_list::<Color>(item).map(|list| detable_items(list, Graphic::Color)))
			.or_else(|| run_to_legacy_list::<GradientStops>(item).map(|list| detable_items(list, Graphic::Gradient)))
			.or_else(|| run_to_legacy_list::<String>(item).map(|list| detable_items(list, Graphic::Text)));
		if let Some(typed) = typed {
			return Graphic::Graphic(typed);
		}
	}
	Graphic::Graphic(group_to_legacy_list(group))
}

/// The group as a legacy `List<Graphic>`: a `Graphic` run becomes the items,
/// another typed run becomes one item holding its typed list.
pub fn group_to_legacy_list(group: &core_types::record::Group) -> List<Graphic> {
	let item = &group.content;
	if let Some(mut list) = run_to_legacy_list::<Graphic>(item) {
		for element in list.iter_element_values_mut() {
			*element = map_groups_to_legacy(element);
		}
		push_lane_paint_into_interiors(&mut list);
		return list;
	}
	None.or_else(|| run_to_legacy_list::<Vector>(item).map(|list| detable_items(list, Graphic::Vector)))
		.or_else(|| run_to_legacy_list::<Raster<CPU>>(item).map(|list| detable_items(list, Graphic::RasterCPU)))
		.or_else(|| run_to_legacy_list::<Raster<GPU>>(item).map(|list| detable_items(list, Graphic::RasterGPU)))
		.or_else(|| run_to_legacy_list::<Color>(item).map(|list| detable_items(list, Graphic::Color)))
		.or_else(|| run_to_legacy_list::<GradientStops>(item).map(|list| detable_items(list, Graphic::Gradient)))
		.or_else(|| run_to_legacy_list::<String>(item).map(|list| detable_items(list, Graphic::Text)))
		.unwrap_or_default()
}

fn group_render_complexity(group: &core_types::record::Group) -> usize {
	fn typed_run<T: 'static + RenderComplexity>(item: &core_types::record::GroupItem) -> Option<usize> {
		let lanes = item.typed_lanes::<T>()?;
		Some((0..lanes.len()).map(|lane| lanes.element_ref(lane).render_complexity()).sum())
	}
	let item = &group.content;
	None.or_else(|| typed_run::<Graphic>(item))
		.or_else(|| typed_run::<Vector>(item))
		.or_else(|| typed_run::<Raster<CPU>>(item))
		.or_else(|| typed_run::<Raster<GPU>>(item))
		.or_else(|| typed_run::<Color>(item))
		.or_else(|| typed_run::<GradientStops>(item))
		.or_else(|| typed_run::<String>(item))
		.unwrap_or(item.len())
}

impl BoundingBox for Graphic {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::Vector(vector) => BoundingBox::bounding_box(vector, transform, include_stroke),
			Graphic::RasterCPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::RasterGPU(raster) => raster.bounding_box(transform, include_stroke),
			Graphic::Graphic(list) => list.bounding_box(transform, include_stroke),
			Graphic::Color(color) => color.bounding_box(transform, include_stroke),
			Graphic::Gradient(gradient) => gradient.bounding_box(transform, include_stroke),
			Graphic::Text(text) => text.bounding_box(transform, include_stroke),
			Graphic::Group(group) => group_bounding_box(group, transform, include_stroke, false),
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::Vector(vector) => vector.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterCPU(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterGPU(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Graphic(graphic) => graphic.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Color(color) => color.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Gradient(gradient) => gradient.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Text(list) => list.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Group(group) => group_bounding_box(group, transform, include_stroke, true),
		}
	}
}

impl ListConvert<Graphic> for Vector {
	fn convert_item(self) -> Graphic {
		Graphic::Vector(self)
	}
}
impl ListConvert<Graphic> for Raster<CPU> {
	fn convert_item(self) -> Graphic {
		Graphic::RasterCPU(self)
	}
}
impl ListConvert<Graphic> for Raster<GPU> {
	fn convert_item(self) -> Graphic {
		Graphic::RasterGPU(self)
	}
}

impl RenderComplexity for Graphic {
	fn render_complexity(&self) -> usize {
		match self {
			Self::Graphic(list) => list.render_complexity(),
			Self::Vector(list) => list.render_complexity(),
			Self::RasterCPU(list) => list.render_complexity(),
			Self::RasterGPU(list) => list.render_complexity(),
			Self::Color(list) => list.render_complexity(),
			Self::Gradient(list) => list.render_complexity(),
			Self::Text(list) => list.render_complexity(),
			Self::Group(group) => group_render_complexity(group),
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

	fn vector_graphic() -> Graphic {
		Graphic::Vector(Vector::default())
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

		let mut group = List::new_from_element(Graphic::Graphic(List::new_from_element(vector_graphic())));
		group.set_attribute(ATTR_OPACITY, 0, 0.5_f64);
		let flattened: List<Vector> = group.into_flattened_list();
		assert_eq!(flattened.attribute_cloned_or_default::<f64>(ATTR_OPACITY, 0), 0.5);
	}
}

#[cfg(test)]
mod run_tests {
	use super::*;
	use core_types::attribute::Attribute;
	use core_types::bounds::BoundingBox;
	use core_types::lane::LaneSource;
	use core_types::node::RecordBatch;
	use core_types::record::{FieldWrite, GroupItem, Layout, RunView, element_write_hashed};
	use glam::{DAffine2, DVec2};
	use vector_types::subpath::Subpath;
	use vector_types::vector::PointId;

	fn unit_square_at(corner: DVec2) -> Vector {
		Vector::from_subpath(Subpath::<PointId>::new_rectangle(corner, corner + DVec2::ONE))
	}

	#[test]
	fn a_run_serves_the_parked_paint_reference() {
		let paint = List::new_from_element(Graphic::Color(Color::BLACK));
		let vector = unit_square_at(DVec2::ZERO);

		let layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)]);
		let mut bytes = vec![0u8; layout.lane_stride()];
		// SAFETY: `bytes` is one lane of `layout`; a parked element stores its
		// reference, and the fill field stores the marker's value form.
		unsafe {
			let base = bytes.as_mut_ptr();
			base.cast::<&Vector>().write(&vector);
			base.add(layout.offset_of(Fill::NAME, 0).unwrap()).cast::<Option<&List<Graphic>>>().write(Some(&paint));
		}
		// SAFETY: `bytes` holds one lane of `layout` at its stride.
		let item = unsafe { GroupItem::from_resident(RecordBatch::new(bytes.as_ptr(), 1, &layout)) };
		let run = RunView::<Vector>::new(&item).expect("the run holds vector elements");

		assert_eq!(run.attr::<Fill>(0), Some(&paint));
		assert_eq!(paint_graphics::<Fill, _>(&run, 0), Some(&paint));
		assert_eq!(paint_graphics::<Stroke, _>(&run, 0), None);

		let legacy = run_to_legacy_list::<Vector>(&item).expect("the run lowers to a legacy vector list");
		assert_eq!(paint_graphics::<Fill, _>(&legacy, 0), paint_graphics::<Fill, _>(&run, 0));
	}

	#[test]
	fn an_owned_group_replays_content_equal_after_the_source_dies() {
		let paint = List::new_from_element(Graphic::Color(Color::BLACK));
		let vector = unit_square_at(DVec2::ZERO);

		let layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)]);
		let mut bytes = vec![0u8; layout.lane_stride()];
		// SAFETY: `bytes` is one lane of `layout`; a parked element stores its
		// reference, and the fill field stores the marker's value form.
		unsafe {
			let base = bytes.as_mut_ptr();
			base.cast::<&Vector>().write(&vector);
			base.add(layout.offset_of(Fill::NAME, 0).unwrap()).cast::<Option<&List<Graphic>>>().write(Some(&paint));
		}
		let (owned, expected) = {
			// SAFETY: `bytes` holds one lane of `layout` at its stride.
			let item = unsafe { GroupItem::from_resident(RecordBatch::new(bytes.as_ptr(), 1, &layout)) };
			let group = core_types::record::Group {
				row: None,
				content: item,
			};
			let expected = group_to_legacy_list(&group);
			(map_groups_to_owned(&Graphic::Group(group)), expected)
		};
		bytes.fill(u8::MAX);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let resident = map_groups_to_resident(&owned, &arena).expect("the arena holds the replay");
		let Graphic::Group(group) = &resident else { panic!("the replay keeps the group form") };
		assert_eq!(group_to_legacy_list(group), expected);
	}

	#[test]
	fn an_owned_group_replays_nested_groups_through_the_element_glue() {
		let vector = unit_square_at(DVec2::ZERO);
		let inner_layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[]);
		let mut inner_bytes = vec![0u8; inner_layout.lane_stride()];
		// SAFETY: `inner_bytes` is one lane of `inner_layout`; a parked element
		// stores its reference.
		unsafe { inner_bytes.as_mut_ptr().cast::<&Vector>().write(&vector) };
		// SAFETY: `inner_bytes` holds one lane of `inner_layout` at its stride.
		let inner_item = unsafe { GroupItem::from_resident(RecordBatch::new(inner_bytes.as_ptr(), 1, &inner_layout)) };
		let nested = Graphic::Group(core_types::record::Group {
			row: None,
			content: inner_item,
		});

		let outer_layout = Layout::default().with_writes(0, element_write_hashed::<Graphic>(), &[]);
		let mut outer_bytes = vec![0u8; outer_layout.lane_stride()];
		// SAFETY: `outer_bytes` is one lane of `outer_layout`; a parked element
		// stores its reference.
		unsafe { outer_bytes.as_mut_ptr().cast::<&Graphic>().write(&nested) };
		let (owned, expected) = {
			// SAFETY: `outer_bytes` holds one lane of `outer_layout` at its stride.
			let outer_item = unsafe { GroupItem::from_resident(RecordBatch::new(outer_bytes.as_ptr(), 1, &outer_layout)) };
			let group = core_types::record::Group {
				row: None,
				content: outer_item,
			};
			let expected = group_to_legacy_list(&group);
			(map_groups_to_owned(&Graphic::Group(group)), expected)
		};
		outer_bytes.fill(u8::MAX);
		inner_bytes.fill(u8::MAX);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let resident = map_groups_to_resident(&owned, &arena).expect("the arena holds the replay");
		let Graphic::Group(group) = &resident else { panic!("the replay keeps the group form") };
		assert_eq!(group_to_legacy_list(group), expected);
	}

	fn native_group_paint(vector: &Vector, inner_layout: &Layout, inner_bytes: &mut Vec<u8>) -> List<Graphic> {
		// SAFETY: `inner_bytes` is one lane of `inner_layout`; a parked element
		// stores its reference.
		unsafe { inner_bytes.as_mut_ptr().cast::<&Vector>().write(vector) };
		// SAFETY: `inner_bytes` holds one lane of `inner_layout` at its stride.
		let inner_item = unsafe { GroupItem::from_resident(RecordBatch::new(inner_bytes.as_ptr(), 1, inner_layout)) };
		List::new_from_element(Graphic::Group(core_types::record::Group {
			row: None,
			content: inner_item,
		}))
	}

	#[test]
	fn an_owned_run_deep_copies_graphic_list_fields() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let inner_layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[]);
		let mut inner_bytes = vec![0u8; inner_layout.lane_stride()];
		let paint = native_group_paint(&inner_vector, &inner_layout, &mut inner_bytes);

		let vector = unit_square_at(DVec2::new(4., 4.));
		let layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)]);
		let mut bytes = vec![0u8; layout.lane_stride()];
		// SAFETY: `bytes` is one lane of `layout`; a parked element stores its
		// reference, and the fill field stores the marker's value form.
		unsafe {
			bytes.as_mut_ptr().cast::<&Vector>().write(&vector);
			bytes.as_mut_ptr().add(layout.offset_of(Fill::NAME, 0).unwrap()).cast::<Option<&List<Graphic>>>().write(Some(&paint));
		}
		let (owned, expected) = {
			// SAFETY: `bytes` holds one lane of `layout` at its stride.
			let item = unsafe { GroupItem::from_resident(RecordBatch::new(bytes.as_ptr(), 1, &layout)) };
			(item.copy_out(), map_groups_to_legacy(paint.element(0).unwrap()))
		};
		bytes.fill(u8::MAX);
		inner_bytes.fill(u8::MAX);
		drop(paint);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let replayed = owned.replay(&arena).expect("the arena holds the replay");
		let run = RunView::<Vector>::new(&replayed).expect("the run holds vector elements");
		let served = run.attr::<Fill>(0).expect("the fill replays present");
		assert_eq!(map_groups_to_legacy(served.element(0).unwrap()), expected);
	}

	#[test]
	fn an_owned_record_deep_copies_graphic_list_fields() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let inner_layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[]);
		let mut inner_bytes = vec![0u8; inner_layout.lane_stride()];
		let paint = native_group_paint(&inner_vector, &inner_layout, &mut inner_bytes);

		let vector = unit_square_at(DVec2::new(4., 4.));
		let layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)]);
		let offset = layout.offset_of(Fill::NAME, 0).unwrap();
		let mut bytes = vec![0u8; layout.lane_stride()];
		// SAFETY: as in the run test above.
		unsafe {
			bytes.as_mut_ptr().cast::<&Vector>().write(&vector);
			bytes.as_mut_ptr().add(offset).cast::<Option<&List<Graphic>>>().write(Some(&paint));
		}
		// SAFETY: `bytes` is a live record of `layout`.
		let owned = unsafe { core_types::record::OwnedRecord::copy_out(&layout, core_types::record::Rec::new(bytes.as_ptr())) };
		let expected = map_groups_to_legacy(paint.element(0).unwrap());
		bytes.fill(u8::MAX);
		inner_bytes.fill(u8::MAX);
		drop(paint);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		core_types::record::stack::reserve(layout.frame_bytes());
		let value = owned.replay(&layout, &arena).expect("the arena holds the replay");
		// SAFETY: the replay wrote a record of `layout`.
		let served = unsafe { layout.rec(&value).read::<Option<&List<Graphic>>>(offset) }.expect("the fill replays present");
		assert_eq!(map_groups_to_legacy(served.element(0).unwrap()), expected);
	}

	#[test]
	fn a_legacy_list_owns_its_paint_attr_content() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let inner_layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[]);
		let mut inner_bytes = vec![0u8; inner_layout.lane_stride()];
		let paint = native_group_paint(&inner_vector, &inner_layout, &mut inner_bytes);

		let vector = unit_square_at(DVec2::new(4., 4.));
		let layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)]);
		let mut bytes = vec![0u8; layout.lane_stride()];
		// SAFETY: `bytes` is one lane of `layout`; a parked element stores its
		// reference, and the fill field stores the marker's value form.
		unsafe {
			bytes.as_mut_ptr().cast::<&Vector>().write(&vector);
			bytes.as_mut_ptr().add(layout.offset_of(Fill::NAME, 0).unwrap()).cast::<Option<&List<Graphic>>>().write(Some(&paint));
		}
		let (legacy, expected) = {
			// SAFETY: `bytes` holds one lane of `layout` at its stride.
			let item = unsafe { GroupItem::from_resident(RecordBatch::new(bytes.as_ptr(), 1, &layout)) };
			let legacy = run_to_legacy_list::<Vector>(&item).expect("the run lowers to a legacy vector list");
			(legacy, map_groups_to_legacy(paint.element(0).unwrap()))
		};
		bytes.fill(u8::MAX);
		inner_bytes.fill(u8::MAX);
		drop(paint);

		let served = legacy.attribute::<Option<List<Graphic>>>(Fill::NAME, 0).expect("the fill attribute rides the list");
		let served = served.as_ref().expect("the fill is present");
		assert_eq!(served.element(0).unwrap(), &expected);
	}

	#[test]
	fn a_run_list_keeps_native_group_elements() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let inner_layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[]);
		let mut inner_bytes = vec![0u8; inner_layout.lane_stride()];
		let content = native_group_paint(&inner_vector, &inner_layout, &mut inner_bytes);
		let element = content.element(0).unwrap();

		let layout = Layout::default().with_writes(0, element_write_hashed::<Graphic>(), &[]);
		let mut bytes = vec![0u8; layout.lane_stride()];
		// SAFETY: `bytes` is one lane of `layout`; a parked element stores its
		// reference.
		unsafe { bytes.as_mut_ptr().cast::<&Graphic>().write(element) };
		// SAFETY: `bytes` holds one lane of `layout` at its stride.
		let item = unsafe { GroupItem::from_resident(RecordBatch::new(bytes.as_ptr(), 1, &layout)) };
		let list = run_to_list::<Graphic>(&item).expect("the run holds graphic lanes");
		assert!(matches!(list.element(0), Some(Graphic::Group(_))), "the list keeps the native group form");
	}

	#[test]
	fn the_vector_row_walk_matches_the_legacy_flatten() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let inner_layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[]);
		let mut inner_bytes = vec![0u8; inner_layout.lane_stride()];
		// SAFETY: `inner_bytes` is one lane of `inner_layout`; a parked element
		// stores its reference.
		unsafe { inner_bytes.as_mut_ptr().cast::<&Vector>().write(&inner_vector) };
		// SAFETY: `inner_bytes` holds one lane of `inner_layout` at its stride.
		let inner_item = unsafe { GroupItem::from_resident(RecordBatch::new(inner_bytes.as_ptr(), 1, &inner_layout)) };

		let mut painted = List::new();
		painted.push(Item::new_from_element(Graphic::Vector(unit_square_at(DVec2::ZERO))));
		painted.push(Item::new_from_element(Graphic::Vector(unit_square_at(DVec2::ONE))));
		painted.set_attribute(core_types::ATTR_TRANSFORM, 0, DAffine2::from_translation(DVec2::new(1., 0.)));
		painted.set_attribute(core_types::ATTR_TRANSFORM, 1, DAffine2::from_translation(DVec2::new(0., 1.)));
		set_paint_attribute_at(&mut painted, 1, ATTR_FILL, List::new_from_element(Graphic::Color(Color::WHITE)));

		let mut nested = List::new_from_element(Graphic::Vector(unit_square_at(DVec2::new(2., 2.))));
		nested.set_attribute(core_types::ATTR_TRANSFORM, 0, DAffine2::from_scale(DVec2::splat(2.)));

		let mut top = List::new();
		top.push(Item::new_from_element(Graphic::Graphic(painted)));
		top.push(Item::new_from_element(Graphic::Graphic(nested)));
		top.push(Item::new_from_element(Graphic::Group(core_types::record::Group {
			row: None,
			content: inner_item,
		})));
		top.push(Item::new_from_element(Graphic::Color(Color::BLACK)));
		top.push(Item::new_from_element(Graphic::Vector(unit_square_at(DVec2::new(6., 0.)))));
		top.set_attribute(core_types::ATTR_TRANSFORM, 0, DAffine2::from_translation(DVec2::new(5., 5.)));
		top.set_attribute(core_types::ATTR_EDITOR_LAYER_PATH, 0, vec![core_types::uuid::NodeId(7)]);
		set_paint_attribute_at(&mut top, 0, ATTR_FILL, List::new_from_element(Graphic::Color(Color::BLACK)));
		top.set_attribute(core_types::ATTR_OPACITY, 1, 0.5);
		top.set_attribute(core_types::ATTR_TRANSFORM, 2, DAffine2::from_scale(DVec2::splat(3.)));
		top.set_attribute(core_types::ATTR_TRANSFORM, 4, DAffine2::from_translation(DVec2::new(0., 7.)));
		top.set_attribute(core_types::ATTR_EDITOR_LAYER_PATH, 4, vec![core_types::uuid::NodeId(9)]);
		set_paint_attribute_at(&mut top, 4, ATTR_FILL, List::new_from_element(Graphic::Color(Color::WHITE)));

		let legacy = {
			let mut list = top.clone();
			for element in list.iter_element_values_mut() {
				*element = map_groups_to_legacy(element);
			}
			push_lane_paint_into_interiors(&mut list);
			list.into_flattened_list::<Vector>()
		};
		let native = flatten_vector_rows(GraphicLevel::Legacy(&top));
		assert_eq!(native.len(), legacy.len());
		for row in 0..native.len() {
			assert_eq!(native.attribute::<DAffine2>(core_types::ATTR_TRANSFORM, row), legacy.attribute::<DAffine2>(core_types::ATTR_TRANSFORM, row), "transform, row {row}");
			assert_eq!(native.attribute::<f64>(core_types::ATTR_OPACITY, row), legacy.attribute::<f64>(core_types::ATTR_OPACITY, row), "opacity, row {row}");
			assert_eq!(
				native.attribute::<Vec<core_types::uuid::NodeId>>(core_types::ATTR_EDITOR_LAYER_PATH, row),
				legacy.attribute::<Vec<core_types::uuid::NodeId>>(core_types::ATTR_EDITOR_LAYER_PATH, row),
				"layer path, row {row}"
			);
			assert_eq!(native.attribute::<Option<List<Graphic>>>(ATTR_FILL, row), legacy.attribute::<Option<List<Graphic>>>(ATTR_FILL, row), "fill, row {row}");
		}
		assert_eq!(native, legacy);
	}

	#[test]
	fn a_run_and_its_legacy_list_agree_on_bounding_boxes() {
		let vectors = [unit_square_at(DVec2::ZERO), unit_square_at(DVec2::new(4., 4.))];
		let transforms = [DAffine2::from_translation(DVec2::new(1., 2.)), DAffine2::from_scale(DVec2::splat(3.))];

		let layout = Layout::default().with_writes(0, element_write_hashed::<Vector>(), &[FieldWrite::of::<core_types::attribute::Transform>(0)]);
		let stride = layout.lane_stride();
		let mut bytes = vec![0u8; stride * 2];
		// SAFETY: `bytes` is `stride` per lane, and the offsets come from `layout`.
		unsafe {
			for lane in 0..2 {
				let base = bytes.as_mut_ptr().add(lane * stride);
				base.cast::<&Vector>().write(&vectors[lane]);
				base.add(layout.offset_of(core_types::ATTR_TRANSFORM, 0).unwrap()).cast::<DAffine2>().write(transforms[lane]);
			}
		}
		// SAFETY: `bytes` holds two lanes of `layout` at its stride.
		let item = unsafe { GroupItem::from_resident(RecordBatch::new(bytes.as_ptr(), 2, &layout)) };
		let run = RunView::<Vector>::new(&item).expect("the run holds vector elements");
		let legacy = run_to_legacy_list::<Vector>(&item).expect("the run lowers to a legacy vector list");

		let outer = DAffine2::from_angle(0.3);
		for include_stroke in [false, true] {
			let bounds = run.bounding_box(outer, include_stroke);
			assert_eq!(bounds, legacy.bounding_box(outer, include_stroke));
			assert!(matches!(bounds, RenderBoundingBox::Rectangle(_)));
			assert_eq!(run.thumbnail_bounding_box(outer, include_stroke), legacy.thumbnail_bounding_box(outer, include_stroke));
		}
	}
}

#[cfg(test)]
mod graphic_is_opaque_tests {
	use vector_types::GradientStop;

	use super::*;

	fn color_graphic(alpha: f64) -> Graphic {
		let color = Color::from_rgbaf32(1., 0., 0., alpha as f32).unwrap();
		Graphic::Color(color)
	}

	fn gradient_graphic(gradient: GradientStops) -> Graphic {
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
		let g = Graphic::Vector(Vector::default());
		assert!(!g.is_opaque());
	}

	#[test]
	fn gradient_with_all_opaque_stops_is_opaque() {
		let color_1 = Color::from_rgbaf32(1., 0., 0., 1.).unwrap();
		let color_2 = Color::from_rgbaf32(1., 0., 0., 1.).unwrap();
		let gradient = GradientStops::new(vec![
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
		let gradient = GradientStops::new(vec![
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
