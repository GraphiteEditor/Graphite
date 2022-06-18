use std::collections::HashMap;

use crate::frontend::utility_types::{ExportBounds, FileType};
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::*;
use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

/// A dialog to allow users to customise their file export.
#[derive(Debug, Clone, Default)]
pub struct Export {
	pub file_name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub artboards: HashMap<LayerId, String>,
}

impl PropertyHolder for Export {
	fn properties(&self) -> Layout {
		let file_name = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "File Name".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: self.file_name.clone(),
				on_update: WidgetCallback::new(|text_input: &TextInput| ExportDialogUpdate::FileName(text_input.value.clone()).into()),
			})),
		];

		let entries = [(FileType::Svg, "SVG"), (FileType::Png, "PNG"), (FileType::Jpg, "JPG")]
			.into_iter()
			.map(|(val, name)| RadioEntryData {
				label: name.into(),
				on_update: WidgetCallback::new(move |_| ExportDialogUpdate::FileType(val).into()),
				..RadioEntryData::default()
			})
			.collect();

		let export_type = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "File Type".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::RadioInput(RadioInput {
				selected_index: self.file_type as u32,
				entries,
			})),
		];

		let artboards = self.artboards.iter().map(|(&val, name)| (ExportBounds::Artboard(val), name.to_string()));
		let mut export_area_options = vec![(ExportBounds::AllArtwork, "All Artwork".to_string())];
		export_area_options.extend(artboards);
		let index = export_area_options.iter().position(|(val, _)| val == &self.bounds).unwrap();
		let entries = vec![export_area_options
			.into_iter()
			.map(|(val, name)| DropdownEntryData {
				label: name,
				on_update: WidgetCallback::new(move |_| ExportDialogUpdate::ExportBounds(val).into()),
				..Default::default()
			})
			.collect()];

		let export_area = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Bounds".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::DropdownInput(DropdownInput {
				selected_index: Some(index as u32),
				entries,
				..Default::default()
			})),
		];

		let resolution = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Scale Factor".into(),
				table_align: true,
				..TextLabel::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some(self.scale_factor),
				label: "".into(),
				unit: " ".into(),
				min: Some(0.),
				disabled: self.file_type == FileType::Svg,
				on_update: WidgetCallback::new(|number_input: &NumberInput| ExportDialogUpdate::ScaleFactor(number_input.value.unwrap()).into()),
				..NumberInput::default()
			})),
		];

		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Export".to_string(),
				min_width: 96,
				emphasized: true,
				on_update: WidgetCallback::new(|_| {
					DialogMessage::CloseDialogAndThen {
						followup: Box::new(ExportDialogUpdate::Submit.into()),
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
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Export".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row { widgets: file_name },
			LayoutRow::Row { widgets: export_type },
			LayoutRow::Row { widgets: resolution },
			LayoutRow::Row { widgets: export_area },
			LayoutRow::Row { widgets: button_widgets },
		]))
	}
}

#[impl_message(Message, DialogMessage, ExportDialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum ExportDialogUpdate {
	FileName(String),
	FileType(FileType),
	ScaleFactor(f64),
	ExportBounds(ExportBounds),

	Submit,
}

impl MessageHandler<ExportDialogUpdate, ()> for Export {
	fn process_action(&mut self, action: ExportDialogUpdate, _data: (), responses: &mut VecDeque<Message>) {
		match action {
			ExportDialogUpdate::FileName(name) => self.file_name = name,
			ExportDialogUpdate::FileType(export_type) => self.file_type = export_type,
			ExportDialogUpdate::ScaleFactor(x) => self.scale_factor = x,
			ExportDialogUpdate::ExportBounds(export_area) => self.bounds = export_area,

			ExportDialogUpdate::Submit => responses.push_front(
				DocumentMessage::ExportDocument {
					file_name: self.file_name.clone(),
					file_type: self.file_type,
					scale_factor: self.scale_factor,
					bounds: self.bounds,
				}
				.into(),
			),
		}

		self.register_properties(responses, LayoutTarget::DialogDetails);
	}

	advertise_actions! {ExportDialogUpdate;}
}
