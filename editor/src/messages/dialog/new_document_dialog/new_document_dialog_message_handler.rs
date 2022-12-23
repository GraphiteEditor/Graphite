use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::input_widgets::{CheckboxInput, NumberInput, TextInput};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::prelude::*;

use glam::UVec2;

/// A dialog to allow users to set some initial options about a new document.
#[derive(Debug, Clone, Default)]
pub struct NewDocumentDialogMessageHandler {
	pub name: String,
	pub infinite: bool,
	pub dimensions: UVec2,
}

impl MessageHandler<NewDocumentDialogMessage, ()> for NewDocumentDialogMessageHandler {
	fn process_message(&mut self, message: NewDocumentDialogMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			NewDocumentDialogMessage::Name(name) => self.name = name,
			NewDocumentDialogMessage::Infinite(infinite) => self.infinite = infinite,
			NewDocumentDialogMessage::DimensionsX(x) => self.dimensions.x = x as u32,
			NewDocumentDialogMessage::DimensionsY(y) => self.dimensions.y = y as u32,

			NewDocumentDialogMessage::Submit => {
				responses.push_back(PortfolioMessage::NewDocumentWithName { name: self.name.clone() }.into());

				if !self.infinite && self.dimensions.x > 0 && self.dimensions.y > 0 {
					responses.push_back(
						ArtboardMessage::AddArtboard {
							id: None,
							position: (0., 0.),
							size: (self.dimensions.x as f64, self.dimensions.y as f64),
						}
						.into(),
					);
					responses.push_back(DocumentMessage::ZoomCanvasToFitAll.into());
				}
			}
		}

		self.register_properties(responses, LayoutTarget::DialogDetails);
	}

	advertise_actions! {NewDocumentDialogUpdate;}
}

impl PropertyHolder for NewDocumentDialogMessageHandler {
	fn properties(&self) -> Layout {
		let title = vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
			value: "New document".into(),
			bold: true,
			..Default::default()
		}))];

		let name = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Name".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: self.name.clone(),
				on_update: widget_callback!(|text_input: &TextInput| NewDocumentDialogMessage::Name(text_input.value.clone()).into()),
				..Default::default()
			})),
		];

		let infinite = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Infinite Canvas".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
				checked: self.infinite,
				on_update: widget_callback!(|checkbox_input: &CheckboxInput| NewDocumentDialogMessage::Infinite(checkbox_input.checked).into()),
				..Default::default()
			})),
		];

		let scale = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Dimensions".into(),
				table_align: true,
				..TextLabel::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				label: "W".into(),
				unit: " px".into(),
				value: Some(self.dimensions.x as f64),
				min: Some(0.),
				is_integer: true,
				disabled: self.infinite,
				min_width: 100,
				on_update: widget_callback!(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsX(number_input.value.unwrap()).into()),
				..NumberInput::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				label: "H".into(),
				unit: " px".into(),
				value: Some(self.dimensions.y as f64),
				min: Some(0.),
				is_integer: true,
				disabled: self.infinite,
				min_width: 100,
				on_update: widget_callback!(|number_input: &NumberInput| NewDocumentDialogMessage::DimensionsY(number_input.value.unwrap()).into()),
				..NumberInput::default()
			})),
		];

		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "OK".to_string(),
				min_width: 96,
				emphasized: true,
				on_update: widget_callback!(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![NewDocumentDialogMessage::Submit.into()],
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Cancel".to_string(),
				min_width: 96,
				on_update: widget_callback!(|_| FrontendMessage::DisplayDialogDismiss.into()),
				..Default::default()
			})),
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
