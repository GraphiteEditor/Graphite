use core_types::list::{ATTR_COLOR, ATTR_DIAMETER, ATTR_FLOW, ATTR_HARDNESS, Item, List};
use core_types::registry::types::Percentage;
use core_types::{Color, Ctx};
use graphic_types::Graphic;

pub mod brush;
mod brush_cache;
pub mod brush_stroke;

pub use brush_types::*;

#[node_macro::node(category("Raster: Brush"))]
fn brush_strokes(
	_: impl Ctx,
	strokes: List<Stroke>,
	color: List<Color>,
	#[default(40.)] diameter: f64,
	#[default(0.)] hardness: Percentage,
	#[default(100.)] flow: Percentage,
) -> List<Graphic> {
	List::new_from_item(
		Item::new_from_element(Graphic::from(strokes))
			.with_attribute(ATTR_COLOR, color.element(0).copied().unwrap_or_default())
			.with_attribute(ATTR_DIAMETER, diameter.max(0.))
			.with_attribute(ATTR_HARDNESS, (hardness / 100.).clamp(0., 1.))
			.with_attribute(ATTR_FLOW, (flow / 100.).clamp(0., 1.)),
	)
}

pub mod migrations {
	use crate::brush_stroke::BrushStroke;

	// TODO: Eventually remove this migration document upgrade code
	pub fn migrate_to_brush_strokes<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<BrushStroke>, D::Error> {
		use serde::Deserialize;

		#[derive(serde::Deserialize)]
		struct LegacyTable {
			#[serde(alias = "instances", alias = "instance")]
			element: Vec<BrushStroke>,
		}

		#[derive(serde::Deserialize)]
		#[serde(untagged)]
		enum BrushStrokesFormat {
			Strokes(Vec<BrushStroke>),
			List(LegacyTable),
		}

		Ok(match BrushStrokesFormat::deserialize(deserializer)? {
			BrushStrokesFormat::Strokes(strokes) => strokes,
			BrushStrokesFormat::List(list) => list.element,
		})
	}
}
