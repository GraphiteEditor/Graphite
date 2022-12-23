use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::input_widgets::{DropdownEntryData, DropdownInput, NumberInput, RadioEntryData, RadioInput, TextInput};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::prelude::*;

use graphene::LayerId;

/// A dialog to allow users to customize their file export.
#[derive(Debug, Clone, Default)]
pub struct ExportDialogMessageHandler {
	pub file_name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub artboards: HashMap<LayerId, String>,
	pub has_selection: bool,
}

impl MessageHandler<ExportDialogMessage, ()> for ExportDialogMessageHandler {
	fn process_message(&mut self, message: ExportDialogMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			ExportDialogMessage::FileName(name) => self.file_name = name,
			ExportDialogMessage::FileType(export_type) => self.file_type = export_type,
			ExportDialogMessage::ScaleFactor(x) => self.scale_factor = x,
			ExportDialogMessage::ExportBounds(export_area) => self.bounds = export_area,

			ExportDialogMessage::Submit => responses.push_front(
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

impl PropertyHolder for ExportDialogMessageHandler {
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
				on_update: widget_callback!(|text_input: &TextInput| ExportDialogMessage::FileName(text_input.value.clone()).into()),
				..Default::default()
			})),
		];

		let entries = [(FileType::Png, "PNG"), (FileType::Jpg, "JPG"), (FileType::Svg, "SVG")]
			.into_iter()
			.map(|(val, name)| RadioEntryData {
				label: name.into(),
				on_update: widget_callback!(move |_| ExportDialogMessage::FileType(val).into()),
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
				..Default::default()
			})),
		];

		let artboards = self.artboards.iter().map(|(&val, name)| (ExportBounds::Artboard(val), name.to_string(), false));
		let mut export_area_options = vec![
			(ExportBounds::AllArtwork, "All Artwork".to_string(), false),
			(ExportBounds::Selection, "Selection".to_string(), !self.has_selection),
		];
		export_area_options.extend(artboards);
		let index = export_area_options.iter().position(|(val, _, _)| val == &self.bounds).unwrap();
		let entries = vec![export_area_options
			.into_iter()
			.map(|(val, name, disabled)| DropdownEntryData {
				label: name,
				on_update: widget_callback!(move |_| ExportDialogMessage::ExportBounds(val).into()),
				disabled,
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
				on_update: widget_callback!(|number_input: &NumberInput| ExportDialogMessage::ScaleFactor(number_input.value.unwrap()).into()),
				..NumberInput::default()
			})),
		];

		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Export".to_string(),
				min_width: 96,
				emphasized: true,
				on_update: widget_callback!(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![ExportDialogMessage::Submit.into()],
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
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Export".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row { widgets: file_name },
			LayoutGroup::Row { widgets: export_type },
			LayoutGroup::Row { widgets: resolution },
			LayoutGroup::Row { widgets: export_area },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
