use core_types::{
	ATTR_BACKGROUND, ATTR_CLIP, ATTR_DIMENSIONS, ATTR_LOCATION, CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl,
	table::{Table, TableRow},
	transform::TransformMut,
};
use glam::{DAffine2, DVec2};
use graphic_types::{
	Vector,
	graphic::{Graphic, IntoGraphicTable},
};
use raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;

/// Constructs a new single-item `Table<Table<Graphic>>` (an artboard table) where the row's element is
/// the artboard's content and the metadata (label, location, dimensions, background, clip) is stored as
/// per-row attributes.
#[node_macro::node(category(""))]
pub async fn create_artboard<T: IntoGraphicTable + 'n>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	/// Graphics to include within the artboard.
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
		Context -> DAffine2,
	)]
	content: impl Node<Context<'static>, Output = T>,
	/// Coordinate of the top-left corner of the artboard within the document.
	location: DVec2,
	/// Width and height of the artboard within the document.
	dimensions: DVec2,
	/// Color of the artboard background.
	background: Table<Color>,
	/// Whether to cut off the contained content that extends outside the artboard, or keep it visible.
	#[default(true)]
	clip: bool,
) -> Table<Table<Graphic>> {
	let footprint = ctx.try_footprint().copied();
	let mut new_ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.translate(location);
		new_ctx = new_ctx.with_footprint(footprint);
	}
	let content = content.eval(new_ctx.into_context()).await.into_graphic_table();

	// Normalize so `location` is the top-left corner and `dimensions` are positive (allowing negative input
	// dimensions to represent dragging from the opposite corner).
	let dimensions_clamped = dimensions.max(DVec2::ONE);
	let normalized_location = location.min(location + dimensions_clamped);
	let normalized_dimensions = dimensions_clamped.abs();

	let background = background.element(0).copied().unwrap_or(Color::WHITE);

	// The artboard's user-visible name is its parent layer's display name; not stored as an attribute here so
	// it can't go stale. The data panel resolves it live from the row's `editor:layer_path` NodeId via the network
	// interface, so renaming the layer reflects everywhere on the next refresh.
	Table::new_from_row(
		TableRow::new_from_element(content)
			.with_attribute(ATTR_LOCATION, normalized_location)
			.with_attribute(ATTR_DIMENSIONS, normalized_dimensions)
			.with_attribute(ATTR_BACKGROUND, background)
			.with_attribute(ATTR_CLIP, clip),
	)
}
