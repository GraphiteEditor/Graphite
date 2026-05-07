pub mod brush;
pub mod brush_cache;
pub mod brush_stroke;

pub mod migrations {
	use crate::brush_stroke::BrushStroke;
	use core_types::table::{Table, TableRow};

	// TODO: Eventually remove this migration document upgrade code
	pub fn migrate_brush_strokes_to_table<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Table<BrushStroke>, D::Error> {
		use serde::Deserialize;

		#[derive(serde::Serialize, serde::Deserialize)]
		#[serde(untagged)]
		enum BrushStrokeTableFormat {
			BrushStrokes(Vec<BrushStroke>),
			BrushStrokeTable(Table<BrushStroke>),
		}

		Ok(match BrushStrokeTableFormat::deserialize(deserializer)? {
			BrushStrokeTableFormat::BrushStrokes(strokes) => strokes.into_iter().map(TableRow::new_from_element).collect(),
			BrushStrokeTableFormat::BrushStrokeTable(table) => table,
		})
	}
}
