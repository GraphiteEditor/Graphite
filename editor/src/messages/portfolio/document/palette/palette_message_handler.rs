use graphene_std::color::SRGBA8;
use graphene_std::vector::style::FillChoice;

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::palette::PaletteMessage;
use crate::messages::prelude::*;

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

#[derive(ExtractField)]
pub struct PaletteMessageContext {
	pub palette_open: bool,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct PaletteMessageHandler {}

#[message_handler_data]
impl MessageHandler<PaletteMessage, PaletteMessageContext> for PaletteMessageHandler {
	fn process_message(&mut self, message: PaletteMessage, responses: &mut VecDeque<Message>, context: PaletteMessageContext) {
		let PaletteMessageContext { palette_open } = context;
		match message {
			PaletteMessage::Refresh => {
				if !palette_open {
					return;
				}

				PaletteMessageHandler::refresh_palette(responses);
			}
			PaletteMessage::Upload => todo!(),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PaletteMessage;)
	}
}

impl PaletteMessageHandler {
	pub fn refresh_palette(responses: &mut VecDeque<Message>) {
		// TODO: Add Conditional to load palette from the open document's interface
		// If none is present or it is a new document load the Default.
		let mut widgets = Vec::new();

		for &(name, columns, colors) in &DEFAULT_PALETTES {
			widgets.push(LayoutGroup::row(vec![TextLabel::new(name).bold(true).widget_instance()]));

			let mut table_rows = Vec::new();
			for chunk in colors.chunks(columns as usize) {
				let row = chunk.iter().map(|&color| ColorInput::new(FillChoice::Solid(color)).widget_instance()).collect();
				table_rows.push(row);
			}

			widgets.push(LayoutGroup::table(table_rows, false));
			widgets.push(LayoutGroup::row(vec![Separator::new(SeparatorStyle::Section).widget_instance()]));
		}

		responses.add(LayoutMessage::SendLayout {
			layout: Layout(widgets),
			layout_target: LayoutTarget::Palette,
		});
	}
}
