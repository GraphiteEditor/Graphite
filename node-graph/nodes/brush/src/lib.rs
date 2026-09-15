use core_types::attribute::{Attr, BrushColor, Diameter, Flow, Hardness};
use core_types::registry::types::Percentage;
use core_types::{Color, Ctx};
use graphic_types::Graphic;

pub mod brush;
mod brush_cache;
pub mod brush_stroke;

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

pub mod migrations {
	use crate::brush_stroke::BrushStroke;

	// TODO: Eventually remove this document upgrade code
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
