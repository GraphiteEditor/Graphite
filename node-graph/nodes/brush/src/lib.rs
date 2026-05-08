pub mod brush;
mod brush_cache;
pub mod brush_stroke;

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
			Table(LegacyTable),
		}

		Ok(match BrushStrokesFormat::deserialize(deserializer)? {
			BrushStrokesFormat::Strokes(strokes) => strokes,
			BrushStrokesFormat::Table(table) => table.element,
		})
	}
}
