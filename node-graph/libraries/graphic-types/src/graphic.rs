use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_FILL, ATTR_STROKE, Item, ItemAttributeValues, List, NodeIdPath};
use core_types::ops::FromAnchorPosition;
use core_types::render_complexity::RenderComplexity;
use core_types::{ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use raster_types::{CPU, GPU, Raster};
use std::borrow::Cow;
use vector_types::Gradient;
pub use vector_types::Vector;

/// The possible forms of graphical content that can be rendered by the Render node into either an image or SVG syntax.
#[derive(Clone, Debug, Default, CacheHash, PartialEq, DynAny)]
pub enum Graphic {
	/// The absence of graphical content, like CSS's `none` keyword: painting it produces nothing.
	#[default]
	None,
	Graphic(List<Graphic>),
	Vector(List<Vector>),
	RasterCPU(List<Raster<CPU>>),
	RasterGPU(List<Raster<GPU>>),
	Color(List<Color>),
	Gradient(List<Gradient>),
	Text(List<String>),
}

// Graphic
impl From<List<Graphic>> for Graphic {
	fn from(graphic: List<Graphic>) -> Self {
		Graphic::Graphic(graphic)
	}
}

// Vector
impl From<Vector> for Graphic {
	fn from(vector: Vector) -> Self {
		Graphic::Vector(List::new_from_element(vector))
	}
}
impl From<List<Vector>> for Graphic {
	fn from(vector: List<Vector>) -> Self {
		Graphic::Vector(vector)
	}
}

// Note: List<Vector> -> List<Graphic> conversion handled by blanket impl in gcore

// Raster<CPU>
impl From<Raster<CPU>> for Graphic {
	fn from(raster: Raster<CPU>) -> Self {
		Graphic::RasterCPU(List::new_from_element(raster))
	}
}
impl From<List<Raster<CPU>>> for Graphic {
	fn from(raster: List<Raster<CPU>>) -> Self {
		Graphic::RasterCPU(raster)
	}
}
// Note: List conversions handled by blanket impl in gcore

// Raster<GPU>
impl From<Raster<GPU>> for Graphic {
	fn from(raster: Raster<GPU>) -> Self {
		Graphic::RasterGPU(List::new_from_element(raster))
	}
}
impl From<List<Raster<GPU>>> for Graphic {
	fn from(raster: List<Raster<GPU>>) -> Self {
		Graphic::RasterGPU(raster)
	}
}
// Note: List conversions handled by blanket impl in gcore

// Color
impl From<Color> for Graphic {
	fn from(color: Color) -> Self {
		Graphic::Color(List::new_from_element(color))
	}
}
impl From<List<Color>> for Graphic {
	fn from(color: List<Color>) -> Self {
		Graphic::Color(color)
	}
}
// Note: List conversions handled by blanket impl in gcore
// Note: List<Color> -> Option<Color> is in gcore (Color is defined there)

// Gradient
impl From<Gradient> for Graphic {
	fn from(gradient: Gradient) -> Self {
		Graphic::Gradient(List::new_from_element(gradient))
	}
}
impl From<List<Gradient>> for Graphic {
	fn from(gradient: List<Gradient>) -> Self {
		Graphic::Gradient(gradient)
	}
}

// String
impl From<String> for Graphic {
	fn from(text: String) -> Self {
		Graphic::Text(List::new_from_element(text))
	}
}
impl From<List<String>> for Graphic {
	fn from(text: List<String>) -> Self {
		Graphic::Text(text)
	}
}

/// Deeply flattens a `List<Graphic>`, collecting only elements matching a specific variant (extracted by `extract_variant`)
/// and discarding all other non-matching content. Recursion through `Graphic::Graphic` sub-`List`s composes transforms and opacity.
fn flatten_graphic_list<T>(content: List<Graphic>, extract_variant: fn(Graphic) -> Option<List<T>>) -> List<T> {
	fn flatten_recursive<T>(output: &mut List<T>, current_graphic_list: List<Graphic>, extract_variant: fn(Graphic) -> Option<List<T>>) {
		for current_graphic_item in current_graphic_list.into_iter() {
			// Whether the parent carries each attribute: a structural fact (column presence), never a value comparison.
			// Flattening composes a parent attribute onto its children only when the parent has it,
			// so an absent parent attribute never invents a column the children didn't already have.
			let parent_has_transform = current_graphic_item.attribute::<DAffine2>(ATTR_TRANSFORM).is_some();
			let parent_has_opacity = current_graphic_item.attribute::<f64>(ATTR_OPACITY).is_some();
			let parent_has_fill = current_graphic_item.attribute::<f64>(ATTR_OPACITY_FILL).is_some();
			let parent_has_layer_path = current_graphic_item.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH).is_some();

			let layer_path: NodeIdPath = current_graphic_item.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH);
			let current_transform: DAffine2 = current_graphic_item.attribute_cloned_or_default(ATTR_TRANSFORM);
			let current_opacity: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY, 1.);
			let current_fill: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);

			match current_graphic_item.into_element() {
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

					flatten_recursive(output, sub_list, extract_variant);
				}
				// Extract the target variant and push its items, composing the parent's attributes onto each
				other => {
					if let Some(typed_list) = extract_variant(other) {
						for mut item in typed_list.into_iter() {
							// Each `|| item.attribute(...)` keeps an attribute the item itself carries
							// (recomposed with the parent's identity value) even when the parent lacks it
							if parent_has_transform || item.attribute::<DAffine2>(ATTR_TRANSFORM).is_some() {
								let item_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
								item.set_attribute(ATTR_TRANSFORM, current_transform * item_transform);
							}
							if parent_has_opacity || item.attribute::<f64>(ATTR_OPACITY).is_some() {
								let item_opacity: f64 = item.attribute_cloned_or(ATTR_OPACITY, 1.);
								item.set_attribute(ATTR_OPACITY, current_opacity * item_opacity);
							}
							if parent_has_fill || item.attribute::<f64>(ATTR_OPACITY_FILL).is_some() {
								let item_fill: f64 = item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);
								item.set_attribute(ATTR_OPACITY_FILL, current_fill * item_fill);
							}
							if parent_has_layer_path {
								item.set_attribute(ATTR_EDITOR_LAYER_PATH, layer_path.clone());
							}

							output.push(item);
						}
					}
				}
			}
		}
	}

	let mut output = List::new();
	flatten_recursive(&mut output, content, extract_variant);
	output
}

/// Whether a normalized paint graphic list actually carries renderable paint.
/// A 0-item list, or a list whose first graphic is empty, is treated as no paint.
pub fn is_paint_present(graphic_list: &List<Graphic>) -> bool {
	graphic_list.element(0).is_some_and(|graphic| !graphic.is_empty())
}

/// Look up the paint graphics stored under attribute for a vector item, in the canonical `List<Graphic>` form.
pub fn graphic_list_at<'a>(list: &'a List<Vector>, index: usize, attribute: &str) -> Option<Cow<'a, List<Graphic>>> {
	list.attribute::<List<Graphic>>(attribute, index)
		.map(Cow::Borrowed)
		// Treat a blank paint attribute as absent so an empty attribute doesn't count as painted
		.filter(|graphic_list| is_paint_present(graphic_list))
}

/// Whether the item carries a non-blank canonical `List<Graphic>` paint attribute,
/// checked by borrowing without cloning the renderable list.
pub fn has_paint_at(list: &List<Vector>, index: usize, attribute: &str) -> bool {
	list.attribute::<List<Graphic>>(attribute, index).is_some_and(is_paint_present)
}

/// Materializes a paint picker choice as the canonical single-graphic `List<Graphic>` paint form.
pub fn fill_choice_to_paint(choice: vector_types::vector::style::FillChoice) -> List<Graphic> {
	use vector_types::vector::style::FillChoice;

	let graphic = match choice {
		FillChoice::None => Graphic::None,
		FillChoice::Solid(color) => Graphic::Color(List::new_from_element(color)),
		FillChoice::Gradient(stops) => Graphic::Gradient(List::new_from_element(stops)),
	};
	List::new_from_element(graphic)
}

/// Stores a paint attribute in its canonical `List<Graphic>` form, the only representation paint readers accept.
pub fn set_paint_attribute(attributes: &mut ItemAttributeValues, key: &str, paint: impl IntoGraphicList) {
	attributes.insert(key, paint.into_graphic_list());
}

/// Stores a paint attribute at a list index in its canonical `List<Graphic>` form, the only representation paint readers accept.
pub fn set_paint_attribute_at<T>(list: &mut List<T>, index: usize, key: &str, paint: impl IntoGraphicList) {
	list.set_attribute(key, index, paint.into_graphic_list());
}

/// Bake the provided transform into the per-item transforms of the paint graphics stored under the
/// canonical `List<Graphic>` fill and stroke attributes.
pub fn bake_paint_transforms(attributes: &mut ItemAttributeValues, transform: DAffine2) {
	fn bake_list_transform<T>(list: &mut List<T>, transform: DAffine2) {
		for item_transform in list.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*item_transform = transform * *item_transform;
		}
	}

	fn bake_graphic_paint_transform(graphics: &mut List<Graphic>, transform: DAffine2) {
		for graphic in graphics.iter_element_values_mut() {
			match graphic {
				Graphic::None => {}
				Graphic::Graphic(list) => bake_list_transform(list, transform),
				Graphic::Vector(list) => bake_list_transform(list, transform),
				Graphic::RasterCPU(list) => bake_list_transform(list, transform),
				Graphic::RasterGPU(list) => bake_list_transform(list, transform),
				Graphic::Gradient(list) => bake_list_transform(list, transform),
				Graphic::Text(list) => bake_list_transform(list, transform),
				Graphic::Color(_) => {}
			}
		}
	}

	for paint_key in [ATTR_FILL, ATTR_STROKE] {
		if let Some(graphics) = attributes.get_mut::<List<Graphic>>(paint_key) {
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
		if let Graphic::Vector(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Raster<CPU> {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::RasterCPU(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Color {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Color(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Gradient {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Gradient(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for String {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::Text(t) = graphic { Some(t) } else { None }
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
		// Propagate the `editor:layer_path` column (if present) from item 0 onto the wrapper Graphic item so a
		// subsequent `flatten_graphic_list` doesn't drop the inner Vector's layer stamp
		let layer_path = self.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH, 0).cloned();
		let mut graphic_list = List::new_from_element(Graphic::Vector(self));
		if let Some(layer_path) = layer_path {
			graphic_list.set_attribute(ATTR_EDITOR_LAYER_PATH, 0, layer_path);
		}
		graphic_list
	}
}

impl IntoGraphicList for List<Raster<CPU>> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::RasterCPU(self))
	}
}

impl IntoGraphicList for List<Raster<GPU>> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::RasterGPU(self))
	}
}

impl IntoGraphicList for List<Color> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::Color(self))
	}
}

impl IntoGraphicList for List<Gradient> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::Gradient(self))
	}
}

impl IntoGraphicList for List<String> {
	fn into_graphic_list(self) -> List<Graphic> {
		let layer_path = self.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH, 0).cloned();
		let mut graphic_list = List::new_from_element(Graphic::Text(self));
		if let Some(layer_path) = layer_path {
			graphic_list.set_attribute(ATTR_EDITOR_LAYER_PATH, 0, layer_path);
		}
		graphic_list
	}
}

impl IntoGraphicList for Item<DAffine2> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::default())
	}
}

// DAffine2
impl From<Item<DAffine2>> for Graphic {
	fn from(_: Item<DAffine2>) -> Self {
		Graphic::default()
	}
}

// DVec2
impl From<Item<DVec2>> for Graphic {
	fn from(position: Item<DVec2>) -> Self {
		Graphic::Vector(List::new_from_element(Vector::from_anchor_position(position.into_element())))
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

	pub fn as_vector(&self) -> Option<&List<Vector>> {
		match self {
			Graphic::Vector(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_vector_mut(&mut self) -> Option<&mut List<Vector>> {
		match self {
			Graphic::Vector(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_raster(&self) -> Option<&List<Raster<CPU>>> {
		match self {
			Graphic::RasterCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_mut(&mut self) -> Option<&mut List<Raster<CPU>>> {
		match self {
			Graphic::RasterCPU(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn had_clip_enabled(&self) -> bool {
		fn all_clipped<T>(list: &List<T>) -> bool {
			list.iter_attribute_values_or_default::<bool>(ATTR_CLIPPING_MASK).all(|clip| clip)
		}

		match self {
			Graphic::None => true,
			Graphic::Vector(list) => all_clipped(list),
			Graphic::Graphic(list) => all_clipped(list),
			Graphic::RasterCPU(list) => all_clipped(list),
			Graphic::RasterGPU(list) => all_clipped(list),
			Graphic::Color(list) => all_clipped(list),
			Graphic::Gradient(list) => all_clipped(list),
			Graphic::Text(list) => all_clipped(list),
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			Graphic::Vector(vector) => (0..vector.len()).all(|index| {
				let Some(element) = vector.element(index) else { return false };
				let opacity: f64 = vector.attribute_cloned_or(ATTR_OPACITY, index, 1.);

				let fill_opaque_or_absent = graphic_list_at(vector, index, ATTR_FILL).is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_opaque()));

				let stroke_invisible_or_transparent = element.stroke.as_ref().is_none_or(|stroke| !stroke.has_renderable_stroke())
					|| graphic_list_at(vector, index, ATTR_STROKE).is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_fully_transparent()));

				opacity > 1. - f64::EPSILON && fill_opaque_or_absent && stroke_invisible_or_transparent
			}),
			_ => false,
		}
	}

	pub fn is_opaque(&self) -> bool {
		match self {
			Graphic::None => false,
			Graphic::Graphic(list) => !list.is_empty() && list.iter_element_values().all(Graphic::is_opaque),
			Graphic::Vector(list) => {
				let is_paint_opaque_at = |key: &str, index: usize| graphic_list_at(list, index, key).is_some_and(|graphic_list| graphic_list.element(0).is_some_and(|graphic| graphic.is_opaque()));

				!list.is_empty()
					&& (0..list.len()).all(|i| {
						let Some(vector) = list.element(i) else { return false };
						let opacity: f64 = list.attribute_cloned_or(ATTR_OPACITY, i, 1.);
						let opacity_fill: f64 = list.attribute_cloned_or(ATTR_OPACITY_FILL, i, 1.);
						let fill_opaque = opacity_fill >= 1. - f64::EPSILON && is_paint_opaque_at(ATTR_FILL, i);
						let stroke_opaque_or_invisible = vector.stroke.as_ref().is_none_or(|stroke| !stroke.has_renderable_stroke()) || is_paint_opaque_at(ATTR_STROKE, i);
						opacity >= 1. - f64::EPSILON && fill_opaque && stroke_opaque_or_invisible
					})
			}
			Graphic::Color(list) => list.element(0).is_some_and(|color| color.is_opaque()),
			Graphic::Gradient(list) => list.element(0).is_some_and(|stops| stops.iter().all(|stop| stop.color.is_opaque())),
			Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Text(_) => false,
		}
	}

	pub fn is_fully_transparent(&self) -> bool {
		match self {
			Graphic::None => true,
			Graphic::Graphic(list) => list.iter_element_values().all(Graphic::is_fully_transparent),
			Graphic::Vector(list) => (0..list.len()).all(|i| {
				let Some(vector) = list.element(i) else { return false };
				let is_paint_fully_transparent_at =
					|key: &str, index: usize| graphic_list_at(list, index, key).is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_fully_transparent()));

				let opacity: f64 = list.attribute_cloned_or(ATTR_OPACITY, i, 1.);
				if opacity <= f64::EPSILON {
					return true;
				}
				let opacity_fill: f64 = list.attribute_cloned_or(ATTR_OPACITY_FILL, i, 1.);
				let fill_invisible = opacity_fill <= f64::EPSILON || is_paint_fully_transparent_at(ATTR_FILL, i);
				let stroke_invisible = vector.stroke.as_ref().is_none_or(|stroke| !stroke.has_renderable_stroke()) || is_paint_fully_transparent_at(ATTR_STROKE, i);
				fill_invisible && stroke_invisible
			}),
			Graphic::Color(list) => list.iter_element_values().all(|color| color.a() == 0.),
			Graphic::Gradient(list) => list.iter_element_values().all(|stops| stops.iter().all(|stop| stop.color.a() == 0.)),
			Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Text(_) => false,
		}
	}

	/// True if this paint opaquely covers the entire fill region.
	/// Vector, Raster, and a nested Graphic may leave gaps, so they return false.
	pub fn covers_opaquely(&self) -> bool {
		matches!(self, Graphic::Color(_) | Graphic::Gradient(_)) && self.is_opaque()
	}

	/// Returns true if this graphic contains no content.
	pub fn is_empty(&self) -> bool {
		match self {
			Graphic::None => true,
			Graphic::Graphic(list) => list.is_empty(),
			Graphic::Vector(list) => list.is_empty(),
			Graphic::Color(list) => list.is_empty(),
			Graphic::Gradient(list) => list.is_empty(),
			Graphic::RasterCPU(list) => list.is_empty(),
			Graphic::RasterGPU(list) => list.is_empty(),
			Graphic::Text(list) => list.is_empty(),
		}
	}
}

impl BoundingBox for Graphic {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::None => RenderBoundingBox::None,
			Graphic::Vector(list) => list.bounding_box(transform, include_stroke),
			Graphic::RasterCPU(list) => list.bounding_box(transform, include_stroke),
			Graphic::RasterGPU(list) => list.bounding_box(transform, include_stroke),
			Graphic::Graphic(list) => list.bounding_box(transform, include_stroke),
			Graphic::Color(list) => list.bounding_box(transform, include_stroke),
			Graphic::Gradient(list) => list.bounding_box(transform, include_stroke),
			Graphic::Text(list) => list.bounding_box(transform, include_stroke),
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::None => RenderBoundingBox::None,
			Graphic::Vector(vector) => vector.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterCPU(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterGPU(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Graphic(graphic) => graphic.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Color(color) => color.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Gradient(gradient) => gradient.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Text(list) => list.thumbnail_bounding_box(transform, include_stroke),
		}
	}
}

impl RenderComplexity for Graphic {
	fn render_complexity(&self) -> usize {
		match self {
			Self::None => 0,
			Self::Graphic(list) => list.render_complexity(),
			Self::Vector(list) => list.render_complexity(),
			Self::RasterCPU(list) => list.render_complexity(),
			Self::RasterGPU(list) => list.render_complexity(),
			Self::Color(list) => list.render_complexity(),
			Self::Gradient(list) => list.render_complexity(),
			Self::Text(list) => list.render_complexity(),
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
		Graphic::Vector(List::new_from_element(Vector::default()))
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
	use core_types::ATTR_SPREAD_METHOD;
	use vector_types::{GradientSpreadMethod, GradientStop};

	use super::*;

	fn color_graphic(alpha: f64) -> Graphic {
		let color = Color::from_rgbaf32(1., 0., 0., alpha as f32).unwrap();
		Graphic::Color(List::new_from_element(color))
	}

	fn gradient_graphic(gradient: Gradient) -> Graphic {
		let mut gradient_list = List::new_from_element(gradient);
		gradient_list.set_attribute(ATTR_SPREAD_METHOD, 0, GradientSpreadMethod::Pad);
		Graphic::Gradient(gradient_list)
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
		let g = Graphic::Vector(List::default());
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
