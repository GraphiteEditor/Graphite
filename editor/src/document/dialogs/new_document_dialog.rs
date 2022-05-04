use crate::{
	layout::{layout_message::LayoutTarget, widgets::*},
	message_prelude::*,
};

use glam::UVec2;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct NewDocument {
	pub name: String,
	pub infinite: bool,
	pub dimensions: UVec2,
}

impl PropertyHolder for NewDocument {
	fn properties(&self) -> WidgetLayout {
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
			})),
		];

		let infinite = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Infinite".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::OptionalInput(OptionalInput {
				checked: self.infinite,
				icon: "Checkmark".to_string(),
				on_update: WidgetCallback::new(|optional_input: &OptionalInput| NewDocumentDialogUpdate::Infinite(optional_input.checked).into()),
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
				value: self.dimensions.x as f64,
				label: "W".into(),
				unit: " px".into(),
				disabled: self.infinite,
				is_integer: true,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| NewDocumentDialogUpdate::DimensionsX(number_input.value).into()),
				..NumberInput::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: self.dimensions.y as f64,
				label: "H".into(),
				unit: " px".into(),
				disabled: self.infinite,
				is_integer: true,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| NewDocumentDialogUpdate::DimensionsY(number_input.value).into()),
				..NumberInput::default()
			})),
		];

		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "OK".to_string(),
				min_width: 96,
				emphasized: true,
				on_update: WidgetCallback::new(|_| {
					PortfolioMessage::CloseDialogAndThen {
						followup: Box::new(NewDocumentDialogUpdate::Submit.into()),
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Cancel".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(|_| FrontendMessage::TriggerDismissDialog.into()),
				..Default::default()
			})),
		];

		WidgetLayout::new(vec![
			LayoutRow::Row { widgets: name },
			LayoutRow::Row { widgets: infinite },
			LayoutRow::Row { widgets: scale },
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Vertical,
					separator_type: SeparatorType::Unrelated,
				}))],
			},
			LayoutRow::Row { widgets: button_widgets },
		])
	}
}

#[impl_message(Message, PortfolioMessage, NewDocumentDialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum NewDocumentDialogUpdate {
	Name(String),
	Infinite(bool),
	DimensionsX(f64),
	DimensionsY(f64),

	Submit,
	BufferArtboard,
	AddArtboard,
	FitCanvas,
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

				responses.push_back(NewDocumentDialogUpdate::BufferArtboard.into());
			}
			NewDocumentDialogUpdate::BufferArtboard => {
				if !self.infinite {
					responses.push_back(NewDocumentDialogUpdate::AddArtboard.into());
				}
			}
			NewDocumentDialogUpdate::AddArtboard => {
				responses.push_back(
					ArtboardMessage::AddArtboard {
						id: None,
						position: (0., 0.),
						size: (self.dimensions.x as f64, self.dimensions.y as f64),
					}
					.into(),
				);
				responses.push_back(NewDocumentDialogUpdate::FitCanvas.into());
			}
			NewDocumentDialogUpdate::FitCanvas => {
				responses.push_back(DocumentMessage::ZoomCanvasToFitAll.into());
			}
		}

		self.register_properties(responses, LayoutTarget::DialogDetails);
	}

	advertise_actions! {NewDocumentDialogUpdate;}
}
