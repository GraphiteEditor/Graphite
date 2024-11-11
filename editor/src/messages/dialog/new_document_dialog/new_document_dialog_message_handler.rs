use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use graph_craft::document::NodeId;

use glam::{IVec2, UVec2};

/// A dialog to allow users to set some initial options about a new document.
#[derive(Debug, Clone, Default)]
pub struct NewDocumentDialogMessageHandler {
	pub name: String,
	pub infinite: bool,
	pub dimensions: UVec2,
}

impl MessageHandler<NewDocumentDialogMessage, ()> for NewDocumentDialogMessageHandler {
	fn process_message(&mut self, message: NewDocumentDialogMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			NewDocumentDialogMessage::Name(name) => self.name = name,
			NewDocumentDialogMessage::Infinite(infinite) => self.infinite = infinite,
			NewDocumentDialogMessage::DimensionsX(x) => self.dimensions.x = x as u32,
			NewDocumentDialogMessage::DimensionsY(y) => self.dimensions.y = y as u32,
			NewDocumentDialogMessage::Submit => {
				responses.add(PortfolioMessage::NewDocumentWithName { name: self.name.clone() });

				let create_artboard = !self.infinite && self.dimensions.x > 0 && self.dimensions.y > 0;
				if create_artboard {
					responses.add(Message::StartBuffer);
					responses.add(GraphOperationMessage::NewArtboard {
						id: NodeId::new(),
						artboard: graphene_core::Artboard::new(IVec2::ZERO, self.dimensions.as_ivec2()),
					});
				}

				// TODO: Figure out how to get StartBuffer to work here so we can delete this and use `DocumentMessage::ZoomCanvasToFitAll` instead
				// Currently, it is necessary to use `FrontendMessage::TriggerDelayedZoomCanvasToFitAll` rather than `DocumentMessage::ZoomCanvasToFitAll` because the size of the viewport is not yet populated
				responses.add(Message::StartBuffer);
				responses.add(FrontendMessage::TriggerDelayedZoomCanvasToFitAll);
				responses.add(DocumentMessage::DeselectAllLayers);
			}
		}

		self.send_dialog_to_frontend(responses);
	}

	advertise_actions! {NewDocumentDialogUpdate;}
}

impl DialogLayoutHolder for NewDocumentDialogMessageHandler {
	const ICON: &'static str = "File";
	const TITLE: &'static str = "New Document";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("OK")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![NewDocumentDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Cancel").on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for NewDocumentDialogMessageHandler {
	fn layout(&self) -> Layout {
		let name = vec![
			TextLabel::new("Name").table_align(true).min_width(90).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(&self.name)
				.on_update(|text_input: &TextInput| NewDocumentDialogMessage::Name(text_input.value.clone()).into())
				.min_width(204) // Matches the 100px of both NumberInputs below + the 4px of the Unrelated-type separator
				.widget_holder(),
		];

		let infinite = vec![
			TextLabel::new("Infinite Canvas").table_align(true).min_width(90).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(self.infinite)
				.on_update(|checkbox_input: &CheckboxInput| NewDocumentDialogMessage::Infinite(checkbox_input.checked).into())
				.widget_holder(),
		];

		let scale = vec![
			TextLabel::new("Dimensions").table_align(true).min_width(90).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(self.dimensions.x as f64))
				.label("W")
				.unit(" px")
				.min(0.)
				.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
				.is_integer(true)
				.disabled(self.infinite)
				.min_width(100)
				.on_update(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsX(number_input.value.unwrap()).into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(self.dimensions.y as f64))
				.label("H")
				.unit(" px")
				.min(0.)
				.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
				.is_integer(true)
				.disabled(self.infinite)
				.min_width(100)
				.on_update(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsY(number_input.value.unwrap()).into())
				.widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row { widgets: name },
			LayoutGroup::Row { widgets: infinite },
			LayoutGroup::Row { widgets: scale },
		]))
	}
}
