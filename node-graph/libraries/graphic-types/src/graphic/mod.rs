mod glue;
mod legacy;
mod paint;
mod walk;

pub(crate) use glue::list_contains_groups;
pub use glue::{map_groups_to_owned, map_groups_to_persistent, map_groups_to_resident};
pub(crate) use legacy::run_to_legacy_list;
pub use legacy::{group_to_legacy_graphic, group_to_legacy_list, map_groups_to_legacy, map_paint_attrs_to_legacy, run_to_list};
pub use paint::{
	LanePaint, PaintColumns, PaintOverlay, PaintOverlayColumn, PaintReach, bake_paint_transforms, has_paint, is_paint_present, paint_graphics, set_paint_attribute, set_paint_attribute_at,
	vector_can_reduce_to_clip_path,
};
pub use walk::{GraphicLevel, GraphicLevelColumn, RowStep, VectorRow, direct_vector_len, flatten_vector_rows, group_is_empty, lane_attributes, run_lane_attributes, walk_vector_rows};
use walk::{group_all_clipped, group_bounding_box, group_is_fully_transparent, group_is_opaque, group_render_complexity};

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
use vector_types::GradientStops;
pub use vector_types::Vector;

/// The possible forms of graphical content that can be rendered by the Render node into either an image or SVG syntax.
/// A leaf holds its element directly; its attributes ride the containing
/// lane. Multi-element content is a [`core_types::record::Group`] run, or
/// transitionally the legacy `Graphic` list.
#[derive(Clone, Debug, CacheHash, PartialEq, DynAny)]
pub enum Graphic<'e> {
	Graphic(List<Graphic<'e>>),
	Vector(Vector),
	RasterCPU(Raster<CPU>),
	RasterGPU(Raster<GPU>),
	Color(Color),
	Gradient(GradientStops),
	Text(String),
	Group(core_types::record::Group<'e>),
}

impl Default for Graphic<'_> {
	fn default() -> Self {
		Self::Graphic(List::new())
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

into_graphic_element! {
	Vector: Vector;
	RasterCPU: Raster<CPU>;
	RasterGPU: Raster<GPU>;
	Color: Color;
	Gradient: GradientStops;
	Text: String;
}

impl IntoGraphicElement for Graphic<'static> {
	fn into_graphic_element(self, _arena: &core_types::arena::Arena) -> Option<Graphic<'_>> {
		Some(self)
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
		Graphic::Vector(vector)
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

// GradientStops
impl From<GradientStops> for Graphic<'_> {
	fn from(gradient: GradientStops) -> Self {
		Graphic::Gradient(gradient)
	}
}

// String
impl From<String> for Graphic<'_> {
	fn from(text: String) -> Self {
		Graphic::Text(text)
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
				// A bridge row's native group flattens through its legacy lowering.
				Graphic::Group(group) => {
					let lowered = List::new_from_item(Item::from_parts(group_to_legacy_graphic(&group), attributes.clone()));
					flatten_recursive(output, lowered, extract_variant, parent_layer_path);
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
		detable_items(self, Graphic::Vector)
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

impl IntoGraphicList for List<GradientStops> {
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

// DVec2
impl From<DVec2> for Graphic<'_> {
	fn from(position: DVec2) -> Self {
		Graphic::Vector(Vector::from_anchor_position(position))
	}
}
// Note: List conversions handled by blanket impl in gcore

impl<'e> Graphic<'e> {
	pub fn as_graphic(&self) -> Option<&List<Graphic<'_>>> {
		match self {
			Graphic::Graphic(graphic) => Some(graphic),
			_ => None,
		}
	}

	pub fn as_graphic_mut(&mut self) -> Option<&mut List<Graphic<'e>>> {
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

impl BoundingBox for Graphic<'_> {
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

impl<'e> ListConvert<Graphic<'e>> for Vector {
	fn convert_item(self) -> Graphic<'e> {
		Graphic::Vector(self)
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

	fn vector_graphic() -> Graphic<'static> {
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
mod graphic_is_opaque_tests {
	use vector_types::GradientStop;

	use super::*;

	fn color_graphic(alpha: f64) -> Graphic<'static> {
		let color = Color::from_rgbaf32(1., 0., 0., alpha as f32).unwrap();
		Graphic::Color(color)
	}

	fn gradient_graphic(gradient: GradientStops) -> Graphic<'static> {
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

#[cfg(test)]
mod test_support {
	use super::Graphic;
	use core_types::list::List;
	use core_types::record::{RunBuilder, element_write_hashed};
	use glam::DVec2;
	use vector_types::Vector;
	use vector_types::subpath::Subpath;
	use vector_types::vector::PointId;

	pub(in crate::graphic) fn unit_square_at(corner: DVec2) -> Vector {
		Vector::from_subpath(Subpath::<PointId>::new_rectangle(corner, corner + DVec2::ONE))
	}

	pub(in crate::graphic) fn native_group_paint<'a>(vector: &Vector, arena: &'a core_types::arena::Arena) -> List<Graphic<'a>> {
		let mut builder = RunBuilder::new(arena, element_write_hashed::<Vector>(), &[], 1).unwrap();
		builder.push(vector.clone()).unwrap();
		List::new_from_element(Graphic::Group(core_types::record::Group { row: None, content: builder.finish() }))
	}
}
