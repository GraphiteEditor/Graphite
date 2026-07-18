use core_types::attr;
use core_types::list::{Item, List};
use core_types::transform::TransformMut;
use core_types::{CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::{Graphic, IntoGraphicList};
use graphic_types::{Artboard, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::Gradient;

/// Constructs a single-element `Artboard[]` with the given content and metadata stored as row attributes.
#[node_macro::node(category(""))]
pub async fn create_artboard<T: IntoGraphicList>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	/// Graphics to include within the artboard.
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<String>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> Item<DAffine2>,
	)]
	content: impl Node<Context<'static>, Output = T>,
	/// Coordinate of the top-left corner of the artboard within the document.
	location: Item<DVec2>,
	/// Width and height of the artboard within the document.
	dimensions: Item<DVec2>,
	/// Color of the artboard background.
	background: Item<Color>,
	/// Whether to cut off the contained content that extends outside the artboard, or keep it visible.
	#[default(true)]
	clip: Item<bool>,
) -> Item<Artboard> {
	let (location, dimensions, clip) = (location.into_element(), dimensions.into_element(), clip.into_element());

	let footprint = ctx.try_footprint().copied();
	let mut new_ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.translate(location);
		new_ctx = new_ctx.with_footprint(footprint);
	}
	let content = content.eval(new_ctx.into_context()).await.into_graphic_list();

	// Normalize so `location` is the top-left corner and `dimensions` are positive (allowing negative input
	// dimensions to represent dragging from the opposite corner). Compute the corner using the raw signed
	// dimensions before clamping, otherwise negative inputs collapse to the original corner instead of inverting.
	let normalized_location = location.min(location + dimensions);
	let normalized_dimensions = dimensions.abs().max(DVec2::ONE);

	let background = background.into_element();

	// Name is not stored here, it's resolved live from the parent layer's display name
	Item::new_from_element(Artboard::new(content))
		.with_attr::<attr::Location>(normalized_location)
		.with_attr::<attr::Dimensions>(normalized_dimensions)
		.with_attr::<attr::Background>(background)
		.with_attr::<attr::Clip>(clip)
}
