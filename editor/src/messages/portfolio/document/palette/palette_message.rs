use std::path::PathBuf;

use crate::messages::prelude::*;
use graphene_std::color::SRGBA8;

#[impl_message(Message, DocumentMessage, Palette)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PaletteMessage {
	ActivePaletteChanged {
		id: u64,
	},
	PaletteNameChange {
		id: u64,
		name: String,
	},
	AddColor {
		color: SRGBA8,
	},
	DeleteColor {
		swatch_id: u64,
	},
	Refresh,
	InsertPaletteGPL {
		palette_name: Option<String>,
		content: Vec<u8>,
	},
	InsertPaletteKPL {
		palette_name: Option<String>,
		palette_path: Option<PathBuf>,
		content: Vec<u8>,
	},
}
