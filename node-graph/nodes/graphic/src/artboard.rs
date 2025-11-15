use core_types::{CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl, table::Table, transform::TransformMut};
use graphic_types::{Artboard, Vector, graphic::{Graphic, IntoGraphicTable}};
use glam::{DAffine2, DVec2, IVec2};
use raster_types::{CPU, GPU, Raster};
use vector_types::GradientStops;

/// Constructs a new single artboard table with the chosen properties.
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
	/// Name of the artboard, shown in parts of the editor.
	label: String,
	/// Coordinate of the top-left corner of the artboard within the document.
	location: DVec2,
	/// Width and height of the artboard within the document. Only integers are valid.
	dimensions: DVec2,
	/// Color of the artboard background. Only positive integers are valid.
	background: Table<Color>,
	/// Whether to cut off the contained content that extends outside the artboard, or keep it visible.
	clip: bool,
) -> Table<Artboard> {
	let location = location.as_ivec2();

	let footprint = ctx.try_footprint().copied();
	let mut new_ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.translate(location.as_dvec2());
		new_ctx = new_ctx.with_footprint(footprint);
	}
	let content = content.eval(new_ctx.into_context()).await.into_graphic_table();

	let dimensions = dimensions.as_ivec2().max(IVec2::ONE);

	let location = location.min(location + dimensions);

	let dimensions = dimensions.abs();

	let background: Option<Color> = background.into();
	let background = background.unwrap_or(Color::WHITE);

	Table::new_from_element(Artboard {
		content,
		label,
		location,
		dimensions,
		background,
		clip,
	})
}
