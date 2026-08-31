use core_types::list::{ATTR_COLOR, ATTR_DIAMETER, ATTR_FLOW, ATTR_HARDNESS, Item, List};
use core_types::registry::types::Percentage;
use core_types::{Color, Ctx};
use graphic_types::Graphic;

pub mod basic_brush;

pub use brush_types::*;

pub(crate) const DEFAULT_DIAMETER: f64 = 40.;
pub(crate) const DEFAULT_HARDNESS: f64 = 0.;
pub(crate) const DEFAULT_FLOW: f64 = 100.;
pub(crate) const DEFAULT_COLOR: Color = Color::BLACK;

#[node_macro::node(category("Raster: Brush"))]
fn brush_strokes(
	_: impl Ctx,
	strokes: List<Stroke>,
	color: List<Color>,
	#[default(DEFAULT_DIAMETER)] diameter: Item<f64>,
	#[default(DEFAULT_HARDNESS)] hardness: Item<Percentage>,
	#[default(DEFAULT_FLOW)] flow: Item<Percentage>,
) -> List<Graphic> {
	let (diameter, hardness, flow) = (diameter.into_element(), hardness.into_element(), flow.into_element());
	List::new_from_item(
		Item::new_from_element(Graphic::from(strokes))
			.with_attribute(ATTR_COLOR, color.element(0).copied().unwrap_or_default())
			.with_attribute(ATTR_DIAMETER, diameter.max(0.))
			.with_attribute(ATTR_HARDNESS, (hardness / 100.).clamp(0., 1.))
			.with_attribute(ATTR_FLOW, (flow / 100.).clamp(0., 1.)),
	)
}
