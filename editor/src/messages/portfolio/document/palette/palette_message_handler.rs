use graphene_std::vector::style::FillChoice;

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::palette::PaletteMessage;
use crate::messages::prelude::*;

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

				// TODO: Attach strings for selected active color/palette
				// Make use of context for getting info if there exists an
				// active attached color from a palette.
				PaletteMessageHandler::refresh_palette(palette_open, responses);
			}
			PaletteMessage::Upload => todo!(),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PaletteMessage;)
	}
}

impl PaletteMessageHandler {
	pub fn refresh_palette(palette_open: bool, responses: &mut VecDeque<Message>) {
		if !palette_open {
			return;
		}

		let widgets = vec![
			LayoutGroup::row(vec![TextLabel::new("Khaki Palette").multiline(true).center_align(false).widget_instance()]),
			LayoutGroup::table(
				vec![vec![
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					ColorInput::new(FillChoice::None).widget_instance(),
					ColorInput::new(FillChoice::None).widget_instance(),
					ColorInput::new(FillChoice::None).widget_instance(),
					ColorInput::new(FillChoice::None).widget_instance(),
					ColorInput::new(FillChoice::None).widget_instance(),
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				]],
				true,
			),
		];
		responses.add(LayoutMessage::SendLayout {
			layout: Layout(widgets),
			layout_target: LayoutTarget::Palette,
		});
	}
}
