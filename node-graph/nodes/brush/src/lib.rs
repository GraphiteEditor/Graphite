pub mod brush;
mod brush_cache;
pub mod brush_stroke;

pub mod migrations {
	use crate::brush_stroke::BrushStroke;
	use core_types::table::Table;

	// TODO: Eventually remove this migration document upgrade code
	pub fn migrate_to_brush_strokes<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<BrushStroke>, D::Error> {
		use serde::Deserialize;

		#[derive(serde::Serialize, serde::Deserialize)]
		#[serde(untagged)]
		enum BrushStrokesFormat {
			Strokes(Vec<BrushStroke>),
			Table(Table<BrushStroke>),
		}

		Ok(match BrushStrokesFormat::deserialize(deserializer)? {
			BrushStrokesFormat::Strokes(strokes) => strokes,
			BrushStrokesFormat::Table(table) => table.iter_element_values().cloned().collect(),
		})
	}
}
