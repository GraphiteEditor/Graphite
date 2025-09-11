use crate::messages::prelude::*;
use crate::messages::{input_mapper::utility_types::input_mouse::ViewportBounds, layout::utility_types::widget_prelude::*};
use glam::{IVec2, UVec2};
use graph_craft::document::NodeId;

#[derive(ExtractField)]
pub struct NewDocumentDialogMessageContext<'a> {
	pub viewport_bounds: &'a ViewportBounds,
}

/// A dialog to allow users to set some initial options about a new document.
#[derive(Debug, Clone, Default, ExtractField)]
pub struct NewDocumentDialogMessageHandler {
	pub name: String,
	pub infinite: bool,
	pub dimensions: UVec2,
}

#[message_handler_data]
impl<'a> MessageHandler<NewDocumentDialogMessage, NewDocumentDialogMessageContext<'a>> for NewDocumentDialogMessageHandler {
	fn process_message(&mut self, message: NewDocumentDialogMessage, responses: &mut VecDeque<Message>, context: NewDocumentDialogMessageContext<'a>) {
		match message {
			NewDocumentDialogMessage::Name { name } => self.name = name,
			NewDocumentDialogMessage::Infinite { infinite } => self.infinite = infinite,
			NewDocumentDialogMessage::DimensionsX { width } => self.dimensions.x = width as u32,
			NewDocumentDialogMessage::DimensionsY { height } => self.dimensions.y = height as u32,
			NewDocumentDialogMessage::Submit => {
				responses.add(PortfolioMessage::NewDocumentWithName { name: self.name.clone() });

				let create_artboard = !self.infinite && self.dimensions.x > 0 && self.dimensions.y > 0;
				if create_artboard {
					responses.add(GraphOperationMessage::NewArtboard {
						id: NodeId::new(),
						artboard: graphene_std::Artboard::new(IVec2::ZERO, self.dimensions.as_ivec2()),
					});
					responses.add(NavigationMessage::CanvasPan { delta: self.dimensions.as_dvec2() });
					responses.add(NodeGraphMessage::RunDocumentGraph);
					// If we already have bounds, we won't receive a viewport bounds update so we just fabricate one ourselves
					if *context.viewport_bounds != ViewportBounds::default() {
						responses.add(InputPreprocessorMessage::BoundsOfViewports {
							bounds_of_viewports: vec![context.viewport_bounds.clone()],
						});
					}
					responses.add(DeferMessage::AfterNavigationReady {
						messages: vec![DocumentMessage::ZoomCanvasToFitAll.into(), DocumentMessage::DeselectAllLayers.into()],
					});
				}

				responses.add(DocumentMessage::MarkAsSaved);
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
			TextLabel::new("Name").table_align(true).min_width("90px").widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(&self.name)
				.on_update(|text_input: &TextInput| NewDocumentDialogMessage::Name { name: text_input.value.clone() }.into())
				.min_width(204) // Matches the 100px of both NumberInputs below + the 4px of the Unrelated-type separator
				.widget_holder(),
		];

		let checkbox_id = CheckboxId::new();
		let infinite = vec![
			TextLabel::new("Infinite Canvas").table_align(true).min_width("90px").for_checkbox(checkbox_id).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(self.infinite)
				.on_update(|checkbox_input: &CheckboxInput| NewDocumentDialogMessage::Infinite { infinite: checkbox_input.checked }.into())
				.for_label(checkbox_id)
				.widget_holder(),
		];

		let scale = vec![
			TextLabel::new("Dimensions").table_align(true).min_width("90px").widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(self.dimensions.x as f64))
				.label("W")
				.unit(" px")
				.min(0.)
				.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
				.is_integer(true)
				.disabled(self.infinite)
				.min_width(100)
				.on_update(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsX { width: number_input.value.unwrap() }.into())
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
				.on_update(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsY { height: number_input.value.unwrap() }.into())
				.widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row { widgets: name },
			LayoutGroup::Row { widgets: infinite },
			LayoutGroup::Row { widgets: scale },
		]))
	}
}
