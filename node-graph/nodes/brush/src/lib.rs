use core_types::list::{ATTR_BRUSH_STYLE, Item, List};
use core_types::{Color, Ctx};
use graphic_types::Graphic;

pub mod airbrush;

pub use brush_types::*;

#[node_macro::node(category("Raster: Brush"))]
fn brush_strokes(_: impl Ctx, strokes: List<Stroke>, color: List<Color>, #[default(20.)] diameter: f64, #[default(0.8)] hardness: f64, #[default(1.)] flow: f64) -> List<Graphic> {
	let style = BrushStyle {
		color: color.element(0).copied().unwrap_or_default(),
		diameter: diameter.max(0.),
		hardness: hardness.clamp(0., 1.),
		flow: flow.clamp(0., 1.),
	};
	List::new_from_item(Item::new_from_element(Graphic::from(strokes)).with_attribute(ATTR_BRUSH_STYLE, style))
}
