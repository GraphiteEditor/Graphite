use core_types::attribute::{Attr, BrushColor, Diameter, Flow, Hardness};
use core_types::registry::types::Percentage;
use core_types::{Color, Ctx};
use graphic_types::Graphic;

pub mod basic_brush;

pub use brush_types::*;

/// Carries the brush strokes drawn by the Brush tool as graphical content, stamping the
/// style each stroke is painted with onto its own lane.
#[node_macro::node(category("Raster: Brush"))]
fn brush_strokes<'e>(
	_: impl Ctx,
	stroke: Stroke,
	color: IList<Color>,
	#[unit(" px")]
	#[default(40.)]
	diameter: f64,
	#[default(0.)] hardness: Percentage,
	#[default(100.)] flow: Percentage,
) -> (Graphic<'e>, Attr<'e, BrushColor>, Attr<'e, Diameter>, Attr<'e, Hardness>, Attr<'e, Flow>) {
	(
		Graphic::Stroke(stroke),
		Attr(if color.is_empty() { Color::default() } else { *color.element_ref(0) }),
		Attr(diameter.max(0.)),
		Attr((hardness / 100.).clamp(0., 1.)),
		Attr((flow / 100.).clamp(0., 1.)),
	)
}
