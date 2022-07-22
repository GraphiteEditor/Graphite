use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::*;
use crate::message_prelude::*;

use glam::UVec2;
use serde::{Deserialize, Serialize};

/// A dialog to allow users to set some initial options about a new document.
#[derive(Debug, Clone, Default)]
pub struct NewDocument {
	pub name: String,
	pub infinite: bool,
	pub dimensions: UVec2,
}

impl PropertyHolder for NewDocument {
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
				on_update: WidgetCallback::new(|text_input: &TextInput| NewDocumentDialogUpdate::Name(text_input.value.clone()).into()),
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
				icon: "Checkmark".to_string(),
				on_update: WidgetCallback::new(|checkbox_input: &CheckboxInput| NewDocumentDialogUpdate::Infinite(checkbox_input.checked).into()),
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
				value: Some(self.dimensions.x as f64),
				label: "W".into(),
				unit: " px".into(),
				disabled: self.infinite,
				is_integer: true,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| NewDocumentDialogUpdate::DimensionsX(number_input.value.unwrap()).into()),
				..NumberInput::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some(self.dimensions.y as f64),
				label: "H".into(),
				unit: " px".into(),
				disabled: self.infinite,
				is_integer: true,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| NewDocumentDialogUpdate::DimensionsY(number_input.value.unwrap()).into()),
				..NumberInput::default()
			})),
		];

		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "OK".to_string(),
				min_width: 96,
				emphasized: true,
				on_update: WidgetCallback::new(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![NewDocumentDialogUpdate::Submit.into()],
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Cancel".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogDismiss.into()),
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

#[impl_message(Message, DialogMessage, NewDocumentDialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum NewDocumentDialogUpdate {
	Name(String),
	Infinite(bool),
	DimensionsX(f64),
	DimensionsY(f64),

	Submit,
}

impl MessageHandler<NewDocumentDialogUpdate, ()> for NewDocument {
	fn process_action(&mut self, action: NewDocumentDialogUpdate, _data: (), responses: &mut VecDeque<Message>) {
		match action {
			NewDocumentDialogUpdate::Name(name) => self.name = name,
			NewDocumentDialogUpdate::Infinite(infinite) => self.infinite = infinite,
			NewDocumentDialogUpdate::DimensionsX(x) => self.dimensions.x = x as u32,
			NewDocumentDialogUpdate::DimensionsY(y) => self.dimensions.y = y as u32,

			NewDocumentDialogUpdate::Submit => {
				responses.push_back(PortfolioMessage::NewDocumentWithName { name: self.name.clone() }.into());

				if !self.infinite && self.dimensions.x != 0 && self.dimensions.y != 0 {
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
