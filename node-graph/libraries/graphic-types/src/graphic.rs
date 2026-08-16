use crate::appearance::{Appearance, Cover, Coverage};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_APPEARANCE, ATTR_PAINT, Item, ItemAttributeValues, List, NodeIdPath};
use core_types::math::quad::Quad;
use core_types::ops::FromAnchorPosition;
use core_types::render_complexity::RenderComplexity;
use core_types::transform::Transform;
use core_types::{ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use raster_types::{CPU, GPU, Raster};
use vector_types::Gradient;
pub use vector_types::Vector;
use vector_types::gradient::MeshGradient;

/// The possible forms of graphical content that can be rendered by the Render node into either an image or SVG syntax.
#[derive(Clone, Debug, Default, CacheHash, PartialEq, DynAny)]
pub enum Graphic {
	/// The absence of graphical content, like CSS's `none` keyword: painting it produces nothing.
	#[default]
	None,
	GraphicList(List<Graphic>),
	VectorList(List<Vector>),
	RasterCPUList(List<Raster<CPU>>),
	RasterGPUList(List<Raster<GPU>>),
	ColorList(List<Color>),
	GradientList(List<Gradient>),
	MeshGradientList(List<MeshGradient>),
	TextList(List<String>),
}

// GraphicList
impl From<List<Graphic>> for Graphic {
	fn from(graphic: List<Graphic>) -> Self {
		Graphic::GraphicList(graphic)
	}
}

// Vector
impl From<Vector> for Graphic {
	fn from(vector: Vector) -> Self {
		Graphic::VectorList(List::new_from_element(vector))
	}
}
impl From<List<Vector>> for Graphic {
	fn from(vector: List<Vector>) -> Self {
		Graphic::VectorList(vector)
	}
}

// Note: List<Vector> -> List<Graphic> conversion handled by blanket impl in gcore

// Raster<CPU>
impl From<Raster<CPU>> for Graphic {
	fn from(raster: Raster<CPU>) -> Self {
		Graphic::RasterCPUList(List::new_from_element(raster))
	}
}
impl From<List<Raster<CPU>>> for Graphic {
	fn from(raster: List<Raster<CPU>>) -> Self {
		Graphic::RasterCPUList(raster)
	}
}
// Note: List conversions handled by blanket impl in gcore

// Raster<GPU>
impl From<Raster<GPU>> for Graphic {
	fn from(raster: Raster<GPU>) -> Self {
		Graphic::RasterGPUList(List::new_from_element(raster))
	}
}
impl From<List<Raster<GPU>>> for Graphic {
	fn from(raster: List<Raster<GPU>>) -> Self {
		Graphic::RasterGPUList(raster)
	}
}
// Note: List conversions handled by blanket impl in gcore

// Color
impl From<Color> for Graphic {
	fn from(color: Color) -> Self {
		Graphic::ColorList(List::new_from_element(color))
	}
}
impl From<List<Color>> for Graphic {
	fn from(color: List<Color>) -> Self {
		Graphic::ColorList(color)
	}
}
// Note: List conversions handled by blanket impl in gcore
// Note: List<Color> -> Option<Color> is in gcore (Color is defined there)

// Gradient
impl From<Gradient> for Graphic {
	fn from(gradient: Gradient) -> Self {
		Graphic::GradientList(List::new_from_element(gradient))
	}
}
impl From<List<Gradient>> for Graphic {
	fn from(gradient: List<Gradient>) -> Self {
		Graphic::GradientList(gradient)
	}
}

// MeshGradient
impl From<MeshGradient> for Graphic {
	fn from(mesh_gradient: MeshGradient) -> Self {
		Graphic::MeshGradientList(List::new_from_element(mesh_gradient))
	}
}
impl From<List<MeshGradient>> for Graphic {
	fn from(mesh_gradient: List<MeshGradient>) -> Self {
		Graphic::MeshGradientList(mesh_gradient)
	}
}

// String
impl From<String> for Graphic {
	fn from(text: String) -> Self {
		Graphic::TextList(List::new_from_element(text))
	}
}
impl From<List<String>> for Graphic {
	fn from(text: List<String>) -> Self {
		Graphic::TextList(text)
	}
}

/// Whether the list is a single leaf item carrying nothing to compose onto its contents, so flattening it
/// collapses no structure and rebuilding or snapshotting the result would be busywork.
pub fn is_lone_anonymous_leaf(content: &List<Graphic>) -> bool {
	content.len() == 1
		&& !matches!(content.element(0), Some(Graphic::GraphicList(_)))
		&& content.attribute::<DAffine2>(ATTR_TRANSFORM, 0).is_none()
		&& content.attribute::<f64>(ATTR_OPACITY, 0).is_none()
		&& content.attribute::<f64>(ATTR_OPACITY_FILL, 0).is_none()
		&& content.attribute::<Appearance>(ATTR_APPEARANCE, 0).is_none()
		&& content.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH, 0).is_none()
}

/// Deeply flattens a `List<Graphic>`, collecting only elements matching a specific variant (extracted by `extract_variant`)
/// and discarding all other non-matching content. Recursion through `Graphic::GraphicList` sub-`List`s composes transforms and opacity.
fn flatten_graphic_list<T>(content: List<Graphic>, extract_variant: fn(Graphic) -> Option<List<T>>) -> List<T> {
	// Its list is already the flat answer, so hand it back rather than rebuilding it item by item
	if is_lone_anonymous_leaf(&content) {
		let Some(item) = content.into_iter().next() else { return List::new() };

		return extract_variant(item.into_element()).unwrap_or_default();
	}

	fn flatten_recursive<T>(output: &mut List<T>, current_graphic_list: List<Graphic>, extract_variant: fn(Graphic) -> Option<List<T>>) {
		for current_graphic_item in current_graphic_list.into_iter() {
			// Whether the parent carries each composed attribute: a structural fact (column presence), never a value comparison.
			// Flattening composes a parent attribute onto its children only when the parent has it,
			// so an absent parent attribute never invents a column the children didn't already have.
			let parent_has_transform = current_graphic_item.attribute::<DAffine2>(ATTR_TRANSFORM).is_some();
			let parent_has_opacity = current_graphic_item.attribute::<f64>(ATTR_OPACITY).is_some();
			let parent_has_fill = current_graphic_item.attribute::<f64>(ATTR_OPACITY_FILL).is_some();
			let parent_has_layer_path = current_graphic_item.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH).is_some();
			let parent_appearance = current_graphic_item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared).cloned();

			let layer_path: NodeIdPath = current_graphic_item.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH);
			let current_transform: DAffine2 = current_graphic_item.attribute_cloned_or_default(ATTR_TRANSFORM);
			let current_opacity: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY, 1.);
			let current_fill: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);

			match current_graphic_item.into_element() {
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
							if let Some(appearance) = &parent_appearance
								&& item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared).is_none()
							{
								item.set_attribute(ATTR_APPEARANCE, appearance.clone());
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

/// Bake the provided transform into the per-item transforms of the appearance's paint graphics.
pub fn bake_paint_transforms(attributes: &mut ItemAttributeValues, transform: DAffine2) {
	fn bake_list_transform<T>(list: &mut List<T>, transform: DAffine2) {
		for item_transform in list.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*item_transform = transform * *item_transform;
		}
	}

	fn bake_graphic_transform(graphic: &mut Graphic, transform: DAffine2) {
		match graphic {
			Graphic::None => {}
			Graphic::GraphicList(list) => bake_list_transform(list, transform),
			Graphic::VectorList(list) => bake_list_transform(list, transform),
			Graphic::RasterCPUList(list) => bake_list_transform(list, transform),
			Graphic::RasterGPUList(list) => bake_list_transform(list, transform),
			Graphic::GradientList(list) => bake_list_transform(list, transform),
			Graphic::MeshGradientList(list) => bake_list_transform(list, transform),
			Graphic::TextList(list) => bake_list_transform(list, transform),
			Graphic::ColorList(_) => {}
		}
	}

	if let Some(appearance) = attributes.get_mut::<Appearance>(ATTR_APPEARANCE)
		&& let Some(paints) = appearance.0.iter_attribute_values_mut::<Graphic>(ATTR_PAINT)
	{
		for paint in paints {
			bake_graphic_transform(paint, transform);
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
		if let Graphic::VectorList(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Raster<CPU> {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::RasterCPUList(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Color {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::ColorList(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for Gradient {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::GradientList(t) = graphic { Some(t) } else { None }
	}
}

impl TryFromGraphic for String {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		if let Graphic::TextList(t) = graphic { Some(t) } else { None }
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
		// A synthetic container, not a real group layer, so it carries no `editor:layer_path` that would
		// overwrite the inner items' own stamps when flattened back out
		List::new_from_element(Graphic::VectorList(self))
	}
}

impl IntoGraphicList for List<Raster<CPU>> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::RasterCPUList(self))
	}
}

impl IntoGraphicList for List<Raster<GPU>> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::RasterGPUList(self))
	}
}

impl IntoGraphicList for List<Color> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::ColorList(self))
	}
}

impl IntoGraphicList for List<Gradient> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::GradientList(self))
	}
}

impl IntoGraphicList for List<MeshGradient> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::MeshGradientList(self))
	}
}

impl IntoGraphicList for List<String> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::TextList(self))
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
		Graphic::VectorList(List::new_from_element(Vector::from_anchor_position(position.into_element())))
	}
}
// Note: List conversions handled by blanket impl in gcore

impl Graphic {
	pub fn as_graphic_list(&self) -> Option<&List<Graphic>> {
		match self {
			Graphic::GraphicList(graphic_list) => Some(graphic_list),
			_ => None,
		}
	}

	pub fn as_graphic_list_mut(&mut self) -> Option<&mut List<Graphic>> {
		match self {
			Graphic::GraphicList(graphic_list) => Some(graphic_list),
			_ => None,
		}
	}

	pub fn as_vector_list(&self) -> Option<&List<Vector>> {
		match self {
			Graphic::VectorList(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_vector_list_mut(&mut self) -> Option<&mut List<Vector>> {
		match self {
			Graphic::VectorList(vector) => Some(vector),
			_ => None,
		}
	}

	pub fn as_raster_cpu_list(&self) -> Option<&List<Raster<CPU>>> {
		match self {
			Graphic::RasterCPUList(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn as_raster_cpu_list_mut(&mut self) -> Option<&mut List<Raster<CPU>>> {
		match self {
			Graphic::RasterCPUList(raster) => Some(raster),
			_ => None,
		}
	}

	pub fn had_clip_enabled(&self) -> bool {
		fn all_clipped<T>(list: &List<T>) -> bool {
			list.iter_attribute_values_or_default::<bool>(ATTR_CLIPPING_MASK).all(|clip| clip)
		}

		match self {
			Graphic::None => true,
			Graphic::VectorList(list) => all_clipped(list),
			Graphic::GraphicList(list) => all_clipped(list),
			Graphic::RasterCPUList(list) => all_clipped(list),
			Graphic::RasterGPUList(list) => all_clipped(list),
			Graphic::ColorList(list) => all_clipped(list),
			Graphic::GradientList(list) => all_clipped(list),
			Graphic::MeshGradientList(list) => all_clipped(list),
			Graphic::TextList(list) => all_clipped(list),
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			Graphic::VectorList(vector) => (0..vector.len()).all(|index| {
				let opacity: f64 = vector.attribute_cloned_or(ATTR_OPACITY, index, 1.);
				let appearance = vector.attribute::<Appearance>(ATTR_APPEARANCE, index);

				let fills_opaque_or_absent = appearance.is_none_or(|appearance| {
					appearance
						.covers_with_paints()
						.filter(|(coverage, _)| coverage.cover() == Cover::Fill)
						.all(|(_, paint)| paint.is_none_or(Graphic::is_opaque))
				});

				let strokes_invisible_or_transparent = appearance.is_none_or(|appearance| {
					appearance
						.covers_with_paints()
						.filter(|(coverage, _)| coverage.cover() == Cover::Stroke)
						.all(|(coverage, paint)| !coverage.stroke_params().has_renderable_stroke() || paint.is_none_or(Graphic::is_fully_transparent))
				});

				opacity > 1. - f64::EPSILON && fills_opaque_or_absent && strokes_invisible_or_transparent
			}),
			_ => false,
		}
	}

	pub fn is_opaque(&self) -> bool {
		match self {
			Graphic::None => false,
			Graphic::GraphicList(list) => !list.is_empty() && list.iter_element_values().all(Graphic::is_opaque),
			Graphic::VectorList(list) => {
				!list.is_empty()
					&& (0..list.len()).all(|i| {
						let opacity: f64 = list.attribute_cloned_or(ATTR_OPACITY, i, 1.);
						let opacity_fill: f64 = list.attribute_cloned_or(ATTR_OPACITY_FILL, i, 1.);
						let appearance = list.attribute::<Appearance>(ATTR_APPEARANCE, i);

						let fill_opaque = opacity_fill >= 1. - f64::EPSILON
							&& appearance.is_some_and(|appearance| {
								appearance
									.covers_with_paints()
									.any(|(coverage, paint)| coverage.cover() == Cover::Fill && paint.is_some_and(Graphic::is_opaque))
							});

						let strokes_opaque_or_invisible = appearance.is_none_or(|appearance| {
							appearance
								.covers_with_paints()
								.filter(|(coverage, _)| coverage.cover() == Cover::Stroke)
								.all(|(coverage, paint)| !coverage.stroke_params().has_renderable_stroke() || paint.is_some_and(Graphic::is_opaque))
						});

						opacity >= 1. - f64::EPSILON && fill_opaque && strokes_opaque_or_invisible
					})
			}
			Graphic::ColorList(list) => list.element(0).is_some_and(|color| color.is_opaque()),
			Graphic::GradientList(list) => list.element(0).is_some_and(|stops| stops.iter().all(|stop| stop.color.is_opaque())),
			// TODO: Graphic::MeshGradientList should be able to have this check
			Graphic::RasterCPUList(_) | Graphic::RasterGPUList(_) | Graphic::TextList(_) | Graphic::MeshGradientList(_) => false,
		}
	}

	pub fn is_fully_transparent(&self) -> bool {
		match self {
			Graphic::None => true,
			Graphic::GraphicList(list) => list.iter_element_values().all(Graphic::is_fully_transparent),
			Graphic::VectorList(list) => (0..list.len()).all(|i| {
				let opacity: f64 = list.attribute_cloned_or(ATTR_OPACITY, i, 1.);
				if opacity <= f64::EPSILON {
					return true;
				}
				let opacity_fill: f64 = list.attribute_cloned_or(ATTR_OPACITY_FILL, i, 1.);
				let appearance = list.attribute::<Appearance>(ATTR_APPEARANCE, i);

				let fills_invisible = opacity_fill <= f64::EPSILON
					|| appearance.is_none_or(|appearance| {
						appearance
							.covers_with_paints()
							.filter(|(coverage, _)| coverage.cover() == Cover::Fill)
							.all(|(_, paint)| paint.is_none_or(Graphic::is_fully_transparent))
					});

				let strokes_invisible = appearance.is_none_or(|appearance| {
					appearance
						.covers_with_paints()
						.filter(|(coverage, _)| coverage.cover() == Cover::Stroke)
						.all(|(coverage, paint)| !coverage.stroke_params().has_renderable_stroke() || paint.is_none_or(Graphic::is_fully_transparent))
				});

				fills_invisible && strokes_invisible
			}),
			Graphic::ColorList(list) => list.iter_element_values().all(|color| color.a() == 0.),
			Graphic::GradientList(list) => list.iter_element_values().all(|stops| stops.iter().all(|stop| stop.color.a() == 0.)),
			// TODO: Graphic::MeshGradientList should be able to have this check
			Graphic::RasterCPUList(_) | Graphic::RasterGPUList(_) | Graphic::TextList(_) | Graphic::MeshGradientList(_) => false,
		}
	}

	/// True if this paint opaquely covers the entire fill region.
	/// Vector, Raster, and a nested Graphic may leave gaps, so they return false.
	pub fn covers_opaquely(&self) -> bool {
		matches!(self, Graphic::ColorList(_) | Graphic::GradientList(_)) && self.is_opaque()
	}

	/// Returns true if this graphic contains no content.
	pub fn is_empty(&self) -> bool {
		match self {
			Graphic::None => true,
			Graphic::GraphicList(list) => list.is_empty(),
			Graphic::VectorList(list) => list.is_empty(),
			Graphic::ColorList(list) => list.is_empty(),
			Graphic::GradientList(list) => list.is_empty(),
			Graphic::MeshGradientList(list) => list.is_empty(),
			Graphic::RasterCPUList(list) => list.is_empty(),
			Graphic::RasterGPUList(list) => list.is_empty(),
			Graphic::TextList(list) => list.is_empty(),
		}
	}
}

/// Combined bounding box of a vector list's rows, inflating each row by its appearance's stroke when `include_stroke`.
/// Stroke parameters live on the row attribute, out of reach of the element-level impl.
pub fn vector_list_bounding_box(list: &List<Vector>, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
	let mut combined_bounds: Option<[DVec2; 2]> = None;

	for index in 0..list.len() {
		let Some(element) = list.element(index) else { continue };
		let item_transform: DAffine2 = list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
		let row_transform = transform * item_transform;

		let Some(mut bounds) = element.bounding_box_with_transform(row_transform) else { continue };

		// The full line width (not half) accounts for different styles of stroke caps
		if include_stroke
			&& let Some(stroke) = list
				.attribute::<Appearance>(ATTR_APPEARANCE, index)
				.and_then(|appearance| appearance.first_coverage_of(Cover::Stroke))
				.map(Coverage::stroke_params)
		{
			let scale = row_transform.scale_magnitudes();
			let offset = DVec2::splat(stroke.weight() * scale.x.max(scale.y) * stroke.join_miter_limit);
			bounds = [bounds[0] - offset, bounds[1] + offset];
		}

		combined_bounds = Some(match combined_bounds {
			Some(existing) => Quad::combine_bounds(existing, bounds),
			None => bounds,
		});
	}

	match combined_bounds {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

impl BoundingBox for Graphic {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::None => RenderBoundingBox::None,
			Graphic::VectorList(list) => vector_list_bounding_box(list, transform, include_stroke),
			Graphic::RasterCPUList(list) => list.bounding_box(transform, include_stroke),
			Graphic::RasterGPUList(list) => list.bounding_box(transform, include_stroke),
			Graphic::GraphicList(list) => list.bounding_box(transform, include_stroke),
			Graphic::ColorList(list) => list.bounding_box(transform, include_stroke),
			Graphic::GradientList(list) => list.bounding_box(transform, include_stroke),
			Graphic::MeshGradientList(list) => list.bounding_box(transform, include_stroke),
			Graphic::TextList(list) => list.bounding_box(transform, include_stroke),
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::None => RenderBoundingBox::None,
			Graphic::VectorList(vector) => vector_list_bounding_box(vector, transform, include_stroke),
			Graphic::RasterCPUList(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterGPUList(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::GraphicList(list) => list.thumbnail_bounding_box(transform, include_stroke),
			Graphic::ColorList(color) => color.thumbnail_bounding_box(transform, include_stroke),
			Graphic::GradientList(gradient) => gradient.thumbnail_bounding_box(transform, include_stroke),
			Graphic::MeshGradientList(gradient) => gradient.thumbnail_bounding_box(transform, include_stroke),
			Graphic::TextList(list) => list.thumbnail_bounding_box(transform, include_stroke),
		}
	}
}

impl RenderComplexity for Graphic {
	fn render_complexity(&self) -> usize {
		match self {
			Self::None => 0,
			Self::GraphicList(list) => list.render_complexity(),
			Self::VectorList(list) => {
				let element_complexity = list.render_complexity();

				// A mesh gradient paint costs far more to render than the geometry it covers, so an element's
				// appearance counts toward its complexity — that is what keeps its thumbnail from being attempted.
				let paint_complexity = list
					.iter_attribute_values::<Appearance>(ATTR_APPEARANCE)
					.into_iter()
					.flatten()
					.filter_map(|appearance| appearance.0.iter_attribute_values::<Graphic>(ATTR_PAINT))
					.flatten()
					.map(|paint| paint.render_complexity())
					.fold(0, usize::saturating_add);

				element_complexity.saturating_add(paint_complexity)
			}
			Self::RasterCPUList(list) => list.render_complexity(),
			Self::RasterGPUList(list) => list.render_complexity(),
			Self::ColorList(list) => list.render_complexity(),
			Self::GradientList(list) => list.render_complexity(),
			Self::MeshGradientList(list) => list.render_complexity(),
			Self::TextList(list) => list.render_complexity(),
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
	use core_types::uuid::NodeId;

	fn vector_graphic() -> Graphic {
		Graphic::VectorList(List::new_from_element(Vector::default()))
	}

	fn vector_list_stamped_with_layers(layers: [u64; 2]) -> List<Vector> {
		let mut list = List::new();

		for layer in layers {
			let mut item = Item::new_from_element(Vector::default());
			item.set_attribute(ATTR_EDITOR_LAYER_PATH, NodeIdPath::from(vec![NodeId(layer)]));
			list.push(item);
		}

		list
	}

	// The wrapper minted for a typed list is a container rather than a layer, so it must never claim a layer path
	#[test]
	fn wrapping_a_typed_list_leaves_the_wrapper_anonymous() {
		let graphic_list = vector_list_stamped_with_layers([7, 9]).into_graphic_list();

		assert_eq!(graphic_list.len(), 1);
		assert!(!graphic_list.attribute_keys().any(|key| key == ATTR_EDITOR_LAYER_PATH));
	}

	// Round-tripping through that wrapper must not collapse the items' distinct stamps onto item 0's
	#[test]
	fn round_trip_through_the_wrapper_preserves_per_item_layer_paths() {
		let flattened: List<Vector> = vector_list_stamped_with_layers([7, 9]).into_flattened_list();

		let layers = (0..flattened.len())
			.map(|index| {
				flattened
					.attribute_cloned_or_default::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH, index)
					.0
					.iter_element_values()
					.next_back()
					.copied()
			})
			.collect::<Vec<_>>();

		assert_eq!(layers, [Some(NodeId(7)), Some(NodeId(9))]);
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
	fn flatten_cascades_into_padded_empty_appearance_rows() {
		use core_types::Color;

		let solid = |color: Color| Graphic::ColorList(List::new_from_element(color));

		// Declaring an appearance on row 0 forces the column, padding row 1 with the empty appearance
		let mut inner = List::new();
		inner.push(Item::new_from_element(Vector::default()));
		inner.push(Item::new_from_element(Vector::default()));
		inner.set_attribute(ATTR_APPEARANCE, 0, Appearance::new_single(Coverage::new_fill(), solid(Color::BLACK)));

		let mut outer = List::new_from_element(Graphic::VectorList(inner));
		outer.set_attribute(ATTR_APPEARANCE, 0, Appearance::new_single(Coverage::new_fill(), solid(Color::WHITE)));

		let flattened: List<Vector> = outer.into_flattened_list();
		let color_of = |index: usize| {
			let appearance = flattened.attribute::<Appearance>(ATTR_APPEARANCE, index)?;
			let Graphic::ColorList(colors) = appearance.paint_at(0)? else { return None };
			colors.element(0).copied()
		};

		assert_eq!(color_of(0), Some(Color::BLACK), "a declared row should keep its own appearance");
		assert_eq!(color_of(1), Some(Color::WHITE), "a padded row should inherit the parent appearance");
	}
}

#[cfg(test)]
mod graphic_is_opaque_tests {
	use core_types::ATTR_GRADIENT_SPREAD;
	use vector_types::{GradientSpread, GradientStop};

	use super::*;

	fn color_graphic(alpha: f64) -> Graphic {
		let color = Color::from_rgbaf32(1., 0., 0., alpha as f32).unwrap();
		Graphic::ColorList(List::new_from_element(color))
	}

	fn gradient_graphic(gradient: Gradient) -> Graphic {
		let mut gradient_list = List::new_from_element(gradient);
		gradient_list.set_attribute(ATTR_GRADIENT_SPREAD, 0, GradientSpread::Pad);
		Graphic::GradientList(gradient_list)
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
		let g = Graphic::VectorList(List::default());
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
