use crate::appearance::{Appearance, Cover, Coverage};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_APPEARANCE, ATTR_PAINT, Item, ItemAttributeValues, List, NodeIdPath};
use core_types::math::quad::Quad;
use core_types::none;
use core_types::ops::FromAnchorPosition;
use core_types::render_complexity::RenderComplexity;
use core_types::transform::Transform;
use core_types::{ATTR_CLIPPING_MASK, ATTR_EDITOR_LAYER_PATH, ATTR_GRADIENT_SPREAD, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, Color};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use raster_types::{CPU, GPU, Raster};
pub use vector_types::Vector;
use vector_types::{Gradient, GradientSpread};

/// The possible forms of graphical content that can be rendered by the Render node (to targets like SVG and raster) or another render boundary node.
#[derive(Clone, Debug, CacheHash, PartialEq, DynAny)]
pub enum Graphic {
	/// No content, akin to CSS `none`, represented visually by a red slash.
	None(Item<none::None>),
	Graphic(Box<Item<Graphic>>),
	Vector(Box<Item<Vector>>),
	RasterCPU(Box<Item<Raster<CPU>>>),
	RasterGPU(Item<Raster<GPU>>),
	Color(Item<Color>),
	Gradient(Item<Gradient>),
	Text(Item<String>),
	NoneList(List<none::None>),
	GraphicList(List<Graphic>),
	VectorList(List<Vector>),
	RasterCPUList(List<Raster<CPU>>),
	RasterGPUList(List<Raster<GPU>>),
	ColorList(List<Color>),
	GradientList(List<Gradient>),
	TextList(List<String>),
}

impl Default for Graphic {
	fn default() -> Self {
		Graphic::None(Item::default())
	}
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
		Graphic::Vector(Box::new(Item::new_from_element(vector)))
	}
}
impl From<Item<Vector>> for Graphic {
	fn from(vector: Item<Vector>) -> Self {
		Graphic::Vector(Box::new(vector))
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
		Graphic::RasterCPU(Box::new(Item::new_from_element(raster)))
	}
}
impl From<Item<Raster<CPU>>> for Graphic {
	fn from(raster: Item<Raster<CPU>>) -> Self {
		Graphic::RasterCPU(Box::new(raster))
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
		Graphic::RasterGPU(Item::new_from_element(raster))
	}
}
impl From<Item<Raster<GPU>>> for Graphic {
	fn from(raster: Item<Raster<GPU>>) -> Self {
		Graphic::RasterGPU(raster)
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
		Graphic::Color(Item::new_from_element(color))
	}
}
impl From<Item<Color>> for Graphic {
	fn from(color: Item<Color>) -> Self {
		Graphic::Color(color)
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
		Graphic::Gradient(Item::new_from_element(gradient))
	}
}
impl From<Item<Gradient>> for Graphic {
	fn from(gradient: Item<Gradient>) -> Self {
		Graphic::Gradient(gradient)
	}
}
impl From<List<Gradient>> for Graphic {
	fn from(gradient: List<Gradient>) -> Self {
		Graphic::GradientList(gradient)
	}
}

// String
impl From<String> for Graphic {
	fn from(text: String) -> Self {
		Graphic::Text(Item::new_from_element(text))
	}
}
impl From<Item<String>> for Graphic {
	fn from(text: Item<String>) -> Self {
		Graphic::Text(text)
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
		&& !matches!(content.element(0), Some(Graphic::Graphic(_)) | Some(Graphic::GraphicList(_)))
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
			// Whether the parent carries each composed attribute: a structural fact (attribute presence), never a value comparison.
			// Flattening composes a parent attribute onto its children only when the parent has it,
			// so an absent parent attribute never invents an attribute the children didn't already have.
			let parent_has_transform = current_graphic_item.attribute::<DAffine2>(ATTR_TRANSFORM).is_some();
			let parent_has_opacity = current_graphic_item.attribute::<f64>(ATTR_OPACITY).is_some();
			let parent_has_fill = current_graphic_item.attribute::<f64>(ATTR_OPACITY_FILL).is_some();
			let parent_has_layer_path = current_graphic_item.attribute::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH).is_some();
			let parent_appearance = current_graphic_item.attribute::<Appearance>(ATTR_APPEARANCE).and_then(Appearance::declared).cloned();

			let layer_path: NodeIdPath = current_graphic_item.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH);
			let current_transform: DAffine2 = current_graphic_item.attribute_cloned_or_default(ATTR_TRANSFORM);
			let current_opacity: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY, 1.);
			let current_fill: f64 = current_graphic_item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.);

			// A boxed single graphic is the rank-0 spelling of the same nesting, so it flattens through the list path
			let current_element = match current_graphic_item.into_element() {
				Graphic::Graphic(item) => Graphic::GraphicList(List::new_from_item(*item)),
				element => element,
			};

			match current_element {
				// Compose the parent's transform/opacity/fill onto each child, but only for attributes the parent carries.
				// A child lacking one is padded with the composition identity (`1.` for opacity/fill, identity for transform), so composing through it is a no-op.
				Graphic::GraphicList(mut sub_list) => {
					// A group's first child has no preceding sibling, so its clipping flag is inert until splicing
					// hands it the group's own predecessor. Clear it (keeping the attribute) to stay clip-neutral.
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
	fn bake_item_transform<T>(item: &mut Item<T>, transform: DAffine2) {
		let baked = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);
		item.set_attribute(ATTR_TRANSFORM, baked);
	}

	fn bake_list_transform<T>(list: &mut List<T>, transform: DAffine2) {
		for item_transform in list.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*item_transform = transform * *item_transform;
		}
	}

	fn bake_graphic_transform(graphic: &mut Graphic, transform: DAffine2) {
		match graphic {
			Graphic::Graphic(item) => bake_item_transform(item, transform),
			Graphic::Vector(item) => bake_item_transform(item, transform),
			Graphic::RasterCPU(item) => bake_item_transform(item, transform),
			Graphic::RasterGPU(item) => bake_item_transform(item, transform),
			Graphic::Gradient(item) => bake_item_transform(item, transform),
			Graphic::Text(item) => bake_item_transform(item, transform),
			Graphic::GraphicList(list) => bake_list_transform(list, transform),
			Graphic::VectorList(list) => bake_list_transform(list, transform),
			Graphic::RasterCPUList(list) => bake_list_transform(list, transform),
			Graphic::RasterGPUList(list) => bake_list_transform(list, transform),
			Graphic::GradientList(list) => bake_list_transform(list, transform),
			Graphic::TextList(list) => bake_list_transform(list, transform),
			// A color has no spatial extent, so there is no placement for a transform to move
			Graphic::None(_) | Graphic::NoneList(_) | Graphic::Color(_) | Graphic::ColorList(_) => {}
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
		match graphic {
			Graphic::Vector(item) => Some(List::new_from_item(*item)),
			Graphic::VectorList(list) => Some(list),
			_ => None,
		}
	}
}

impl TryFromGraphic for Raster<CPU> {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		match graphic {
			Graphic::RasterCPU(item) => Some(List::new_from_item(*item)),
			Graphic::RasterCPUList(list) => Some(list),
			_ => None,
		}
	}
}

impl TryFromGraphic for Color {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		match graphic {
			Graphic::Color(item) => Some(List::new_from_item(item)),
			Graphic::ColorList(list) => Some(list),
			_ => None,
		}
	}
}

impl TryFromGraphic for Gradient {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		match graphic {
			Graphic::Gradient(item) => Some(List::new_from_item(item)),
			Graphic::GradientList(list) => Some(list),
			_ => None,
		}
	}
}

impl TryFromGraphic for String {
	fn try_from_graphic(graphic: Graphic) -> Option<List<Self>> {
		match graphic {
			Graphic::Text(item) => Some(List::new_from_item(item)),
			Graphic::TextList(list) => Some(list),
			_ => None,
		}
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

impl IntoGraphicList for List<String> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::TextList(self))
	}
}

// TODO: Remove this
impl IntoGraphicList for Item<DAffine2> {
	fn into_graphic_list(self) -> List<Graphic> {
		List::new_from_element(Graphic::default())
	}
}

// DAffine2
// TODO: Remove this
impl From<Item<DAffine2>> for Graphic {
	fn from(_: Item<DAffine2>) -> Self {
		Graphic::default()
	}
}

// DVec2
impl From<Item<DVec2>> for Graphic {
	fn from(position: Item<DVec2>) -> Self {
		let (position, attributes) = position.into_parts();

		Graphic::Vector(Box::new(Item::from_parts(Vector::from_anchor_position(position), attributes)))
	}
}
impl From<List<DVec2>> for Graphic {
	fn from(positions: List<DVec2>) -> Self {
		let anchors = positions.into_iter().map(|position| {
			let (position, attributes) = position.into_parts();

			Item::from_parts(Vector::from_anchor_position(position), attributes)
		});

		Graphic::VectorList(anchors.collect())
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
			!list.is_empty() && list.iter_attribute_values_or_default::<bool>(ATTR_CLIPPING_MASK).all(|clip| clip)
		}

		fn item_clipped<T>(item: &Item<T>) -> bool {
			item.attribute_cloned_or_default::<bool>(ATTR_CLIPPING_MASK)
		}

		match self {
			Graphic::None(item) => item_clipped(item),
			Graphic::Graphic(item) => item_clipped(item),
			Graphic::Vector(item) => item_clipped(item),
			Graphic::RasterCPU(item) => item_clipped(item),
			Graphic::RasterGPU(item) => item_clipped(item),
			Graphic::Color(item) => item_clipped(item),
			Graphic::Gradient(item) => item_clipped(item),
			Graphic::Text(item) => item_clipped(item),
			Graphic::NoneList(list) => all_clipped(list),
			Graphic::VectorList(list) => all_clipped(list),
			Graphic::GraphicList(list) => all_clipped(list),
			Graphic::RasterCPUList(list) => all_clipped(list),
			Graphic::RasterGPUList(list) => all_clipped(list),
			Graphic::ColorList(list) => all_clipped(list),
			Graphic::GradientList(list) => all_clipped(list),
			Graphic::TextList(list) => all_clipped(list),
		}
	}

	pub fn can_reduce_to_clip_path(&self) -> bool {
		match self {
			Graphic::Vector(item) => vector_can_reduce_to_clip_path(item.attribute_cloned_or(ATTR_OPACITY, 1.), item.attribute::<Appearance>(ATTR_APPEARANCE)),
			Graphic::VectorList(list) => {
				(0..list.len()).all(|index| vector_can_reduce_to_clip_path(list.attribute_cloned_or(ATTR_OPACITY, index, 1.), list.attribute::<Appearance>(ATTR_APPEARANCE, index)))
			}
			_ => false,
		}
	}

	pub fn is_guaranteed_fully_opaque(&self) -> bool {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => false,
			// The group's own opacity scales whatever it wraps, so full alpha there is a precondition
			Graphic::Graphic(item) => item_opacity_is_full(item) && item.element().is_guaranteed_fully_opaque(),
			Graphic::GraphicList(list) => !list.is_empty() && every_item_has_full_opacity(list) && list.iter_element_values().all(Graphic::is_guaranteed_fully_opaque),
			Graphic::Vector(item) => vector_is_guaranteed_fully_opaque(
				item.attribute_cloned_or(ATTR_OPACITY, 1.),
				item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.),
				item.attribute::<Appearance>(ATTR_APPEARANCE),
			),
			Graphic::VectorList(list) => {
				!list.is_empty()
					&& (0..list.len()).all(|index| {
						vector_is_guaranteed_fully_opaque(
							list.attribute_cloned_or(ATTR_OPACITY, index, 1.),
							list.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.),
							list.attribute::<Appearance>(ATTR_APPEARANCE, index),
						)
					})
			}
			Graphic::RasterCPU(_) | Graphic::RasterCPUList(_) => false,
			Graphic::RasterGPU(_) | Graphic::RasterGPUList(_) => false,
			Graphic::Color(item) => item_opacity_is_full(item) && item.element().is_opaque(),
			Graphic::ColorList(list) => !list.is_empty() && every_item_has_full_opacity(list) && list.iter_element_values().all(|color| color.is_opaque()),
			// A `Clear` spread cuts off to transparency past the ends, leaving the rest of the region unpainted
			Graphic::Gradient(item) => {
				item_opacity_is_full(item)
					&& item.attribute_cloned_or_default::<GradientSpread>(ATTR_GRADIENT_SPREAD) != GradientSpread::Clear
					&& item.element().iter().all(|stop| stop.color.is_opaque())
			}
			Graphic::GradientList(list) => {
				!list.is_empty()
					&& every_item_has_full_opacity(list)
					&& (0..list.len()).all(|index| {
						list.attribute_cloned_or_default::<GradientSpread>(ATTR_GRADIENT_SPREAD, index) != GradientSpread::Clear
							&& list.element(index).is_some_and(|stops| stops.iter().all(|stop| stop.color.is_opaque()))
					})
			}
			Graphic::Text(_) | Graphic::TextList(_) => false,
		}
	}

	pub fn is_guaranteed_fully_transparent(&self) -> bool {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => true,
			Graphic::Graphic(item) => item_opacity_is_zero(item) || item.element().is_guaranteed_fully_transparent(),
			Graphic::GraphicList(list) => every_item_has_zero_opacity(list) || list.iter_element_values().all(Graphic::is_guaranteed_fully_transparent),
			Graphic::Vector(item) => vector_is_guaranteed_fully_transparent(
				item.attribute_cloned_or(ATTR_OPACITY, 1.),
				item.attribute_cloned_or(ATTR_OPACITY_FILL, 1.),
				item.attribute::<Appearance>(ATTR_APPEARANCE),
			),
			Graphic::VectorList(list) => (0..list.len()).all(|index| {
				vector_is_guaranteed_fully_transparent(
					list.attribute_cloned_or(ATTR_OPACITY, index, 1.),
					list.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.),
					list.attribute::<Appearance>(ATTR_APPEARANCE, index),
				)
			}),
			Graphic::Color(item) => item_opacity_is_zero(item) || item.element().a() == 0.,
			Graphic::ColorList(list) => every_item_has_zero_opacity(list) || list.iter_element_values().all(|color| color.a() == 0.),
			// A stopless ramp paints as solid black, matching `Gradient::evaluate`, so it counts as transparent only once it has stops
			Graphic::Gradient(item) => item_opacity_is_zero(item) || (!item.element().is_empty() && item.element().iter().all(|stop| stop.color.a() == 0.)),
			Graphic::GradientList(list) => every_item_has_zero_opacity(list) || list.iter_element_values().all(|stops| !stops.is_empty() && stops.iter().all(|stop| stop.color.a() == 0.)),
			// Their content is never inspected, so zeroed opacity is the only invisibility these can report
			Graphic::RasterCPU(item) => item_opacity_is_zero(item),
			Graphic::RasterGPU(item) => item_opacity_is_zero(item),
			Graphic::Text(item) => item_opacity_is_zero(item),
			Graphic::RasterCPUList(list) => every_item_has_zero_opacity(list),
			Graphic::RasterGPUList(list) => every_item_has_zero_opacity(list),
			Graphic::TextList(list) => every_item_has_zero_opacity(list),
		}
	}

	/// True if this paint fully, opaquely covers the entire fill region.
	pub fn is_guaranteed_to_cover_opaquely(&self) -> bool {
		matches!(self, Graphic::Color(_) | Graphic::Gradient(_) | Graphic::ColorList(_) | Graphic::GradientList(_)) && self.is_guaranteed_fully_opaque()
	}

	/// Returns true if this graphic contains no content.
	pub fn is_empty(&self) -> bool {
		match self {
			// A leaf always holds exactly one element, so only the none-typed content is truly empty
			Graphic::None(_) | Graphic::NoneList(_) => true,
			Graphic::Graphic(_) | Graphic::Vector(_) | Graphic::RasterCPU(_) | Graphic::RasterGPU(_) | Graphic::Color(_) | Graphic::Gradient(_) | Graphic::Text(_) => false,
			Graphic::GraphicList(list) => list.is_empty(),
			Graphic::VectorList(list) => list.is_empty(),
			Graphic::ColorList(list) => list.is_empty(),
			Graphic::GradientList(list) => list.is_empty(),
			Graphic::RasterCPUList(list) => list.is_empty(),
			Graphic::RasterGPUList(list) => list.is_empty(),
			Graphic::TextList(list) => list.is_empty(),
		}
	}
}

/// Whether a vector object's own opacity and paint let a clipper reduce to an SVG `<clipPath>` instead of a `<mask>`.
fn vector_can_reduce_to_clip_path(opacity: f64, appearance: Option<&Appearance>) -> bool {
	let fills_opaque_or_absent = appearance.is_none_or(|appearance| {
		appearance
			.covers_with_paints()
			.filter(|(coverage, _)| coverage.cover() == Cover::Fill)
			.all(|(_, paint)| paint.is_none_or(Graphic::is_guaranteed_fully_opaque))
	});

	let strokes_invisible_or_transparent = appearance.is_none_or(|appearance| {
		appearance
			.covers_with_paints()
			.filter(|(coverage, _)| coverage.cover() == Cover::Stroke)
			.all(|(coverage, paint)| !coverage.stroke_params().has_renderable_stroke() || paint.is_none_or(Graphic::is_guaranteed_fully_transparent))
	});

	opacity > 1. - f64::EPSILON && fills_opaque_or_absent && strokes_invisible_or_transparent
}

/// Whether a vector object paints its whole interior at full alpha, so nothing behind it can show through.
fn vector_is_guaranteed_fully_opaque(opacity: f64, opacity_fill: f64, appearance: Option<&Appearance>) -> bool {
	let fill_opaque = opacity_fill >= 1. - f64::EPSILON
		&& appearance.is_some_and(|appearance| {
			appearance
				.covers_with_paints()
				.any(|(coverage, paint)| coverage.cover() == Cover::Fill && paint.is_some_and(Graphic::is_guaranteed_fully_opaque))
		});

	let strokes_opaque_or_invisible = appearance.is_none_or(|appearance| {
		appearance
			.covers_with_paints()
			.filter(|(coverage, _)| coverage.cover() == Cover::Stroke)
			.all(|(coverage, paint)| !coverage.stroke_params().has_renderable_stroke() || paint.is_some_and(Graphic::is_guaranteed_fully_opaque))
	});

	opacity >= 1. - f64::EPSILON && fill_opaque && strokes_opaque_or_invisible
}

/// Whether a vector object draws nothing visible, either through its opacity or through its paint.
fn vector_is_guaranteed_fully_transparent(opacity: f64, opacity_fill: f64, appearance: Option<&Appearance>) -> bool {
	if opacity <= f64::EPSILON {
		return true;
	}

	let fills_invisible = opacity_fill <= f64::EPSILON
		|| appearance.is_none_or(|appearance| {
			appearance
				.covers_with_paints()
				.filter(|(coverage, _)| coverage.cover() == Cover::Fill)
				.all(|(_, paint)| paint.is_none_or(Graphic::is_guaranteed_fully_transparent))
		});

	let strokes_invisible = appearance.is_none_or(|appearance| {
		appearance
			.covers_with_paints()
			.filter(|(coverage, _)| coverage.cover() == Cover::Stroke)
			.all(|(coverage, paint)| !coverage.stroke_params().has_renderable_stroke() || paint.is_none_or(Graphic::is_guaranteed_fully_transparent))
	});

	fills_invisible && strokes_invisible
}

/// Whether a lone item's opacity zeroes it out, independent of what its element holds.
fn item_opacity_is_zero<T>(item: &Item<T>) -> bool {
	item.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) <= f64::EPSILON
}

/// Whether every item of a list is zeroed out by its opacity, which an empty list satisfies with nothing to draw.
fn every_item_has_zero_opacity<T>(list: &List<T>) -> bool {
	(0..list.len()).all(|index| list.attribute_cloned_or::<f64>(ATTR_OPACITY, index, 1.) <= f64::EPSILON)
}

/// Whether a lone item passes its content through at full opacity, covering both factors the renderer multiplies together.
fn item_opacity_is_full<T>(item: &Item<T>) -> bool {
	item.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) >= 1. - f64::EPSILON && item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.) >= 1. - f64::EPSILON
}

/// Whether every item of a list passes its content through with full opacity.
fn every_item_has_full_opacity<T>(list: &List<T>) -> bool {
	(0..list.len()).all(|index| list.attribute_cloned_or::<f64>(ATTR_OPACITY, index, 1.) >= 1. - f64::EPSILON && list.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, index, 1.) >= 1. - f64::EPSILON)
}

/// Bounding box of one vector, inflated by its appearance's stroke when `include_stroke` is true.
/// Stroke parameters live on the item attribute, out of reach of the element-level impl.
fn vector_bounding_box(vector: &Vector, composed_transform: DAffine2, appearance: Option<&Appearance>, include_stroke: bool) -> Option<[DVec2; 2]> {
	let mut bounds = vector.bounding_box_with_transform(composed_transform)?;

	// The full line width (not half) accounts for different styles of stroke caps
	if include_stroke && let Some(stroke) = appearance.and_then(|appearance| appearance.first_coverage_of(Cover::Stroke)).map(Coverage::stroke_params) {
		let scale = composed_transform.scale_magnitudes();
		let offset = DVec2::splat(stroke.weight() * scale.x.max(scale.y) * stroke.join_miter_limit);
		bounds = [bounds[0] - offset, bounds[1] + offset];
	}

	Some(bounds)
}

/// Bounding box of a lone vector, inflating it by its appearance's stroke when `include_stroke`.
pub fn vector_item_bounding_box(item: &Item<Vector>, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
	let composed_transform = transform * item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM);

	match vector_bounding_box(item.element(), composed_transform, item.attribute::<Appearance>(ATTR_APPEARANCE), include_stroke) {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

/// Combined bounding box of a vector list's items, inflating each item by its appearance's stroke when `include_stroke`.
pub fn vector_list_bounding_box(list: &List<Vector>, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
	let mut combined_bounds: Option<[DVec2; 2]> = None;

	for index in 0..list.len() {
		let Some(element) = list.element(index) else { continue };
		let item_transform: DAffine2 = list.attribute_cloned_or_default(ATTR_TRANSFORM, index);
		let appearance = list.attribute::<Appearance>(ATTR_APPEARANCE, index);

		let Some(bounds) = vector_bounding_box(element, transform * item_transform, appearance, include_stroke) else {
			continue;
		};

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
			Graphic::None(_) | Graphic::NoneList(_) => RenderBoundingBox::None,
			Graphic::Graphic(item) => item.bounding_box(transform, include_stroke),
			Graphic::Vector(item) => vector_item_bounding_box(item, transform, include_stroke),
			Graphic::RasterCPU(item) => item.bounding_box(transform, include_stroke),
			Graphic::RasterGPU(item) => item.bounding_box(transform, include_stroke),
			Graphic::Color(item) => item.bounding_box(transform, include_stroke),
			Graphic::Gradient(item) => item.bounding_box(transform, include_stroke),
			Graphic::Text(item) => item.bounding_box(transform, include_stroke),
			Graphic::VectorList(list) => vector_list_bounding_box(list, transform, include_stroke),
			Graphic::RasterCPUList(list) => list.bounding_box(transform, include_stroke),
			Graphic::RasterGPUList(list) => list.bounding_box(transform, include_stroke),
			Graphic::GraphicList(list) => list.bounding_box(transform, include_stroke),
			Graphic::ColorList(list) => list.bounding_box(transform, include_stroke),
			Graphic::GradientList(list) => list.bounding_box(transform, include_stroke),
			Graphic::TextList(list) => list.bounding_box(transform, include_stroke),
		}
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		match self {
			Graphic::None(_) | Graphic::NoneList(_) => RenderBoundingBox::None,
			Graphic::Graphic(item) => item.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Vector(item) => vector_item_bounding_box(item, transform, include_stroke),
			Graphic::RasterCPU(item) => item.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterGPU(item) => item.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Color(item) => item.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Gradient(item) => item.thumbnail_bounding_box(transform, include_stroke),
			Graphic::Text(item) => item.thumbnail_bounding_box(transform, include_stroke),
			Graphic::VectorList(vector) => vector_list_bounding_box(vector, transform, include_stroke),
			Graphic::RasterCPUList(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::RasterGPUList(raster) => raster.thumbnail_bounding_box(transform, include_stroke),
			Graphic::GraphicList(list) => list.thumbnail_bounding_box(transform, include_stroke),
			Graphic::ColorList(color) => color.thumbnail_bounding_box(transform, include_stroke),
			Graphic::GradientList(gradient) => gradient.thumbnail_bounding_box(transform, include_stroke),
			Graphic::TextList(list) => list.thumbnail_bounding_box(transform, include_stroke),
		}
	}
}

impl RenderComplexity for Graphic {
	fn render_complexity(&self) -> usize {
		match self {
			Self::None(_) | Self::NoneList(_) => 0,
			Self::Graphic(item) => item.render_complexity(),
			Self::Vector(item) => item.render_complexity(),
			Self::RasterCPU(item) => item.render_complexity(),
			Self::RasterGPU(item) => item.render_complexity(),
			Self::Color(item) => item.render_complexity(),
			Self::Gradient(item) => item.render_complexity(),
			Self::Text(item) => item.render_complexity(),
			Self::GraphicList(list) => list.render_complexity(),
			Self::VectorList(list) => list.render_complexity(),
			Self::RasterCPUList(list) => list.render_complexity(),
			Self::RasterGPUList(list) => list.render_complexity(),
			Self::ColorList(list) => list.render_complexity(),
			Self::GradientList(list) => list.render_complexity(),
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
	use core_types::list::{ATTR_POSITION, List};
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

	fn layer_stamps(flattened: &List<Vector>) -> Vec<Option<NodeId>> {
		(0..flattened.len())
			.map(|index| {
				flattened
					.attribute_cloned_or_default::<NodeIdPath>(ATTR_EDITOR_LAYER_PATH, index)
					.0
					.iter_element_values()
					.next_back()
					.copied()
			})
			.collect()
	}

	// Round-tripping through that wrapper must not collapse the items' distinct stamps onto item 0's
	#[test]
	fn round_trip_through_the_wrapper_preserves_per_item_layer_paths() {
		let flattened: List<Vector> = vector_list_stamped_with_layers([7, 9]).into_flattened_list();

		assert_eq!(layer_stamps(&flattened), [Some(NodeId(7)), Some(NodeId(9))]);
	}

	// The embedding adapter reaches the same flattened stamps as the wrapper, each item carrying its own inside its variant
	#[test]
	fn embedding_each_item_preserves_per_item_layer_paths() {
		let embedded: List<Graphic> = vector_list_stamped_with_layers([7, 9]).into_iter().map(|item| Item::new_from_element(Graphic::from(item))).collect();

		let flattened: List<Vector> = embedded.into_flattened_list();

		assert_eq!(layer_stamps(&flattened), [Some(NodeId(7)), Some(NodeId(9))]);
	}

	// Flattening must not invent attributes that neither the parent graphic nor the child carried
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
		use core_types::Color;

		let solid = |color: Color| Graphic::ColorList(List::new_from_element(color));

		// Declaring an appearance on item 0 forces the attribute, padding item 1 with the empty appearance
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

		assert_eq!(color_of(0), Some(Color::BLACK), "a declared item should keep its own appearance");
		assert_eq!(color_of(1), Some(Color::WHITE), "a padded item should inherit the parent appearance");
	}

	#[test]
	fn embedded_item_keeps_its_attributes_inside_the_variant() {
		let color = Item::new_from_element(Color::RED).with_attribute(ATTR_POSITION, 0.25_f64);

		let Graphic::Color(inner) = Graphic::from(color) else { panic!("expected a color graphic") };
		assert_eq!(inner.element(), &Color::RED);
		assert_eq!(inner.attribute::<f64>(ATTR_POSITION), Some(&0.25));
	}

	#[test]
	fn embedded_list_becomes_one_graphic_holding_every_element() {
		let mut colors = List::new_from_element(Color::RED);
		colors.push(Item::new_from_element(Color::BLUE));

		let Graphic::ColorList(inner) = Graphic::from(colors) else {
			panic!("expected a color list graphic")
		};
		assert_eq!(inner.len(), 2, "a whole list embeds as one graphic holding all its elements");
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
		assert!(g.is_guaranteed_fully_opaque());
	}

	#[test]
	fn transparent_color_is_not_opaque() {
		let g = color_graphic(0.5);
		assert!(!g.is_guaranteed_fully_opaque());
	}

	#[test]
	fn vector_is_not_opaque() {
		let g = Graphic::VectorList(List::default());
		assert!(!g.is_guaranteed_fully_opaque());
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
		assert!(g.is_guaranteed_fully_opaque());
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
		assert!(!g.is_guaranteed_fully_opaque());
	}

	#[test]
	fn gradient_with_clear_spread_is_not_opaque() {
		let opaque = Color::from_rgbaf32(1., 0., 0., 1.).unwrap();
		let gradient = Gradient::new(vec![
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: opaque,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: opaque,
			},
		]);

		let mut gradient_list = List::new_from_element(gradient);
		gradient_list.set_attribute(ATTR_GRADIENT_SPREAD, 0, GradientSpread::Clear);

		assert!(
			!Graphic::GradientList(gradient_list).is_guaranteed_fully_opaque(),
			"a clear spread leaves the region past the ends unpainted"
		);
	}

	#[test]
	fn partial_group_opacity_is_not_opaque() {
		let mut list = List::new_from_element(color_graphic(1.));
		assert!(Graphic::GraphicList(list.clone()).is_guaranteed_fully_opaque());

		list.set_attribute(ATTR_OPACITY, 0, 0.5);
		assert!(!Graphic::GraphicList(list.clone()).is_guaranteed_fully_opaque());

		list.set_attribute(ATTR_OPACITY, 0, 0.);
		assert!(Graphic::GraphicList(list).is_guaranteed_fully_transparent());
	}

	#[test]
	fn partial_leaf_group_opacity_is_not_opaque() {
		let item = Item::new_from_element(color_graphic(1.));
		assert!(Graphic::Graphic(Box::new(item.clone())).is_guaranteed_fully_opaque());

		let reduced = item.with_attribute(ATTR_OPACITY, 0.5);
		assert!(!Graphic::Graphic(Box::new(reduced)).is_guaranteed_fully_opaque());
	}
}
