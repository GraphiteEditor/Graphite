use core_types::attribute::{Attr, Background, Clip, Dimensions, Location};
use core_types::extent::{ExtentIn, LevelIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, Interrupt};
use core_types::transform::TransformMut;
use core_types::{Color, Ctx, DeriveCtx, ModifyFootprint};
use glam::DVec2;
use graphic_types::Artboard;
use graphic_types::graphic::Graphic;

/// Evaluates the content within a footprint translated by `offset`, so the
/// content culls and resolves against its position inside the artboard.
#[node_macro::node(category(""), extent(translate_footprint_extent))]
pub fn translate_footprint<T>(ctx: impl Ctx + DeriveCtx + ModifyFootprint, content: impl Node<Context<'_>, Output = T>, offset: DVec2) -> Result<T, Interrupt> {
	let translated = ctx.modify_footprint(|footprint| footprint.translate(offset));
	content.eval(&translated.ctx())
}

fn translate_footprint_extent(content: ExtentIn<'_>, _offset: ValueIn<'_, DVec2>, level: LevelIn) -> GPoll<Extent> {
	content.at(level)
}

/// Constructs an artboard element with the given content and metadata stored as attributes.
#[node_macro::node(category(""))]
pub fn create_artboard(
	_: impl Ctx,
	/// Graphics to include within the artboard.
	content: IList<Graphic>,
	/// Coordinate of the top-left corner of the artboard within the document.
	location: DVec2,
	/// Width and height of the artboard within the document.
	dimensions: DVec2,
	/// Color of the artboard background.
	background: IList<Color>,
	/// Whether to cut off the contained content that extends outside the artboard, or keep it visible.
	#[default(true)]
	clip: bool,
) -> (Artboard, Attr<Location>, Attr<Dimensions>, Attr<Background>, Attr<Clip>) {
	// SAFETY: a materialized input's frames are arena-resident.
	let item = unsafe { core_types::record::GroupItem::from_resident(content.batch()) };
	let content = core_types::list::List::new_from_element(Graphic::Group(core_types::record::Group {
		row: None,
		content: core_types::record::GroupContent::Run(item),
	}));

	// Normalize so `location` is the top-left corner and `dimensions` are positive (allowing negative input
	// dimensions to represent dragging from the opposite corner). Compute the corner using the raw signed
	// dimensions before clamping, otherwise negative inputs collapse to the original corner instead of inverting.
	let normalized_location = location.min(location + dimensions);
	let normalized_dimensions = dimensions.abs().max(DVec2::ONE);

	let background = match background.len() {
		0 => Color::WHITE,
		_ => background.get(0),
	};

	// Name is not stored here, it's resolved live from the parent layer's display name
	(Artboard::new(content), Attr(normalized_location), Attr(normalized_dimensions), Attr(background), Attr(clip))
}
