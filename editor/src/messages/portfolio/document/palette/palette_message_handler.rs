use crate::application::generate_uuid;
use graphene_std::vector::style::FillChoice;
use graphene_std::{Color, color::SRGBA8};

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::palette::PaletteMessage;
use crate::messages::prelude::*;
use serde::{Deserialize, Serialize};

// TODO: Ig this should be imported from some preloaded assets file after the
// implementation of import from file feature.
pub const DEFAULT_PALETTES: [(&str, u32, &[SRGBA8]); 2] = [
	(
		"Warm Landscape",
		5,
		&[
			SRGBA8::new_opaque(0x26, 0x46, 0x53),
			SRGBA8::new_opaque(0x2A, 0x9D, 0x8F),
			SRGBA8::new_opaque(0xE9, 0xC4, 0x6A),
			SRGBA8::new_opaque(0xF4, 0xA2, 0x61),
			SRGBA8::new_opaque(0xE7, 0x6F, 0x51),
			SRGBA8::new_opaque(0xD6, 0x28, 0x28),
			SRGBA8::new_opaque(0x00, 0x30, 0x49),
			SRGBA8::new_opaque(0xF7, 0x7F, 0x00),
			SRGBA8::new_opaque(0xFC, 0xBF, 0x49),
			SRGBA8::new_opaque(0xEA, 0xE2, 0xB7),
		],
	),
	(
		"Nordic Frost",
		3,
		&[
			SRGBA8::new_opaque(0x2E, 0x34, 0x40),
			SRGBA8::new_opaque(0x3B, 0x42, 0x52),
			SRGBA8::new_opaque(0x43, 0x4C, 0x5E),
			SRGBA8::new_opaque(0x4C, 0x56, 0x6A),
			SRGBA8::new_opaque(0xD8, 0xDE, 0xE9),
			SRGBA8::new_opaque(0xE5, 0xE9, 0xF0),
			SRGBA8::new_opaque(0xEC, 0xEF, 0xF4),
			SRGBA8::new_opaque(0x88, 0xC0, 0xD0),
			SRGBA8::new_opaque(0x81, 0xA1, 0xC1),
			SRGBA8::new_opaque(0x5E, 0x81, 0xAC),
		],
	),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PaletteId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SwatchId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ColorSwatch {
	pub id: SwatchId,
	pub name: String,
	pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Palette {
	pub id: PaletteId,
	pub name: String,
	// Palette specification where it can be 0 or None in which case the UI will flow
	// the palettes accordingly
	pub columns: Option<u32>,
	pub grid: Vec<Option<SwatchId>>,
	pub swatches: HashMap<SwatchId, ColorSwatch>,
}

#[derive(ExtractField)]
pub struct PaletteMessageContext {
	pub palette_open: bool,
}

#[derive(Debug, Clone, ExtractField)]
pub struct PaletteMessageHandler {
	// At any moment in time at least one palette should be open in the palette section
	pub active_palette: PaletteId,
	pub order: Vec<PaletteId>,
	pub palettes: HashMap<PaletteId, Palette>,
}

impl Default for PaletteMessageHandler {
	fn default() -> Self {
		let mut palettes = HashMap::new();
		let mut order = Vec::new();
		for &(name, columns, colors) in &DEFAULT_PALETTES {
			let mut grid = Vec::new();
			let mut swatches = HashMap::new();
			for color in colors {
				let id = SwatchId { 0: generate_uuid() };
				grid.push(Some(id));
				swatches.insert(
					id,
					ColorSwatch {
						id,
						name: "Some Color".to_string(),
						color: (*color).into(),
					},
				);
			}
			let palette_id = PaletteId { 0: generate_uuid() };
			order.push(palette_id);
			palettes.insert(
				palette_id,
				Palette {
					id: PaletteId { 0: generate_uuid() },
					name: name.to_string(),
					columns: Some(columns),
					grid,
					swatches,
				},
			);
		}
		Self {
			active_palette: order[0],
			order,
			palettes,
		}
	}
}

#[message_handler_data]
impl MessageHandler<PaletteMessage, PaletteMessageContext> for PaletteMessageHandler {
	fn process_message(&mut self, message: PaletteMessage, responses: &mut VecDeque<Message>, context: PaletteMessageContext) {
		let PaletteMessageContext { palette_open } = context;
		match message {
			PaletteMessage::Refresh => {
				if !palette_open {
					return;
				}

				self.refresh_palette(responses);
			}
			PaletteMessage::UploadFromFile => todo!(),
			PaletteMessage::ActivePaletteChanged { id } => todo!(),
			PaletteMessage::PaletteNameChange { id, name } => todo!(),
			PaletteMessage::AddColor { color } => todo!(),
			PaletteMessage::DeleteColor { swatch_id } => todo!(),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PaletteMessage;)
	}
}

impl PaletteMessageHandler {
	pub fn refresh_palette(&self, responses: &mut VecDeque<Message>) {
		// TODO: Add Conditional to load palette from the open document's interface
		// If none is present or it is a new document load the Default.
		if true {
			let mut widgets = Vec::new();

			for palette_id in self.order.iter() {
				let Palette { id: _, name, columns, grid, swatches } = self.palettes.get(palette_id).unwrap();
				widgets.push(LayoutGroup::row(vec![TextLabel::new(name).bold(true).widget_instance()]));

				let mut table_rows = Vec::new();
				if let Some(columns) = columns {
					for chunk in grid.chunks(*columns as usize) {
						let row = chunk
							.iter()
							.map(|&id| {
								if let Some(swatch_id) = id {
									let swatch = swatches.get(&swatch_id).unwrap();
									ColorInput::new(FillChoice::Solid(swatch.color.into()))
										.narrow(true)
										.tooltip_label(swatch.name.clone())
										.widget_instance()
									// .on_update(|input: &ColorInput| {
									// 	let ramp = input.value.as_gradient().cloned().unwrap_or_default();
									// 	GradientToolMessage::UpdateRamp { ramp }.into()
									// })
									// .on_commit(|_| DocumentMessage::AddTransaction.into())
								} else {
									ColorInput::new(FillChoice::None).widget_instance()
								}
							})
							.collect();
						table_rows.push(row);
					}
				} else {
					//TODO: Manage the case when the column size is not determined
				}

				widgets.push(LayoutGroup::table(table_rows, false));
				widgets.push(LayoutGroup::row(vec![Separator::new(SeparatorStyle::Section).widget_instance()]));
			}

			responses.add(LayoutMessage::SendLayout {
				layout: Layout(widgets),
				layout_target: LayoutTarget::Palette,
			});
		} else {
			// TODO: Add attached palettes instead
		}
	}
}
