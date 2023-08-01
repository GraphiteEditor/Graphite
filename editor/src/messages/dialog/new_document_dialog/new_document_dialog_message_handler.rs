use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use graphene_core::uuid::generate_uuid;

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

				if !self.infinite && self.dimensions.x > 0 && self.dimensions.y > 0 {
					let id = generate_uuid();
					responses.add(ArtboardMessage::AddArtboard {
						id: Some(id),
						position: (0., 0.),
						size: (self.dimensions.x as f64, self.dimensions.y as f64),
					});
					responses.add(GraphOperationMessage::NewArtboard {
						id,
						artboard: graphene_core::Artboard::new(IVec2::ZERO, self.dimensions.as_ivec2()),
					});
					responses.add(DocumentMessage::ZoomCanvasToFitAll);
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::UpdateNewNodeGraph);
			}
		}

		self.register_properties(responses, LayoutTarget::DialogDetails);
	}

	advertise_actions! {NewDocumentDialogUpdate;}
}

impl PropertyHolder for NewDocumentDialogMessageHandler {
	fn properties(&self) -> Layout {
		let title = vec![TextLabel::new("New document").bold(true).widget_holder()];

		let name = vec![
			TextLabel::new("Name").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(&self.name)
				.on_update(|text_input: &TextInput| NewDocumentDialogMessage::Name(text_input.value.clone()).into())
				.widget_holder(),
		];

		let infinite = vec![
			TextLabel::new("Infinite Canvas").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(self.infinite)
				.on_update(|checkbox_input: &CheckboxInput| NewDocumentDialogMessage::Infinite(checkbox_input.checked).into())
				.widget_holder(),
		];

		let scale = vec![
			TextLabel::new("Dimensions").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(self.dimensions.x as f64))
				.label("W")
				.unit(" px")
				.min(0.)
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
				.is_integer(true)
				.disabled(self.infinite)
				.min_width(100)
				.on_update(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsY(number_input.value.unwrap()).into())
				.widget_holder(),
		];

		let button_widgets = vec![
			TextButton::new("OK")
				.min_width(96)
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![NewDocumentDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Cancel").min_width(96).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row { widgets: title },
			LayoutGroup::Row { widgets: name },
			LayoutGroup::Row { widgets: infinite },
			LayoutGroup::Row { widgets: scale },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
