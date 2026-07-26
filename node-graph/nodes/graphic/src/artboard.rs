use core_types::gpoll::Interrupt;
use core_types::list::{Item, List};
use core_types::transform::TransformMut;
use core_types::{ATTR_BACKGROUND, ATTR_CLIP, ATTR_DIMENSIONS, ATTR_LOCATION, Color, Context, Ctx, DeriveCtx, ExtractFootprint};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{Artboard, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;

/// Constructs a single-element `Artboard[]` with the given content and metadata stored as row attributes.
#[node_macro::node(category(""))]
pub fn create_artboard<T: IntoGraphicList>(
	ctx: impl Ctx + ExtractFootprint + DeriveCtx,
	/// Graphics to include within the artboard.
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<String>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
		Context -> DAffine2,
	)]
	content: impl Node<Context<'_>, Output = T>,
	/// Coordinate of the top-left corner of the artboard within the document.
	location: DVec2,
	/// Width and height of the artboard within the document.
	dimensions: DVec2,
	/// Color of the artboard background.
	background: List<Color>,
	/// Whether to cut off the contained content that extends outside the artboard, or keep it visible.
	#[default(true)]
	clip: bool,
) -> Result<List<Artboard>, Interrupt> {
	let translated = ctx.modify_footprint(|footprint| footprint.translate(location));
	let content = content.eval(&translated.ctx())?.into_graphic_list();

	// Normalize so `location` is the top-left corner and `dimensions` are positive (allowing negative input
	// dimensions to represent dragging from the opposite corner). Compute the corner using the raw signed
	// dimensions before clamping, otherwise negative inputs collapse to the original corner instead of inverting.
	let normalized_location = location.min(location + dimensions);
	let normalized_dimensions = dimensions.abs().max(DVec2::ONE);

	let background = background.element(0).copied().unwrap_or(Color::WHITE);

	// Name is not stored here, it's resolved live from the parent layer's display name
	Ok(List::new_from_item(
		Item::new_from_element(Artboard::new(content))
			.with_attribute(ATTR_LOCATION, normalized_location)
			.with_attribute(ATTR_DIMENSIONS, normalized_dimensions)
			.with_attribute(ATTR_BACKGROUND, background)
			.with_attribute(ATTR_CLIP, clip),
	))
}
