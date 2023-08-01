use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use document_legacy::LayerId;

/// A dialog to allow users to customize their file export.
#[derive(Debug, Clone, Default)]
pub struct ExportDialogMessageHandler {
	pub file_name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub transparent_background: bool,
	pub artboards: HashMap<LayerId, String>,
	pub has_selection: bool,
}

impl MessageHandler<ExportDialogMessage, ()> for ExportDialogMessageHandler {
	fn process_message(&mut self, message: ExportDialogMessage, responses: &mut VecDeque<Message>, _data: ()) {
		match message {
			ExportDialogMessage::FileName(name) => self.file_name = name,
			ExportDialogMessage::FileType(export_type) => self.file_type = export_type,
			ExportDialogMessage::ScaleFactor(factor) => self.scale_factor = factor,
			ExportDialogMessage::TransparentBackground(transparent_background) => self.transparent_background = transparent_background,
			ExportDialogMessage::ExportBounds(export_area) => self.bounds = export_area,

			ExportDialogMessage::Submit => responses.add_front(DocumentMessage::ExportDocument {
				file_name: self.file_name.clone(),
				file_type: self.file_type,
				scale_factor: self.scale_factor,
				bounds: self.bounds,
				transparent_background: self.file_type != FileType::Jpg && self.transparent_background,
			}),
		}

		self.register_properties(responses, LayoutTarget::DialogDetails);
	}

	advertise_actions! {ExportDialogUpdate;}
}

impl PropertyHolder for ExportDialogMessageHandler {
	fn properties(&self) -> Layout {
		let file_name = vec![
			TextLabel::new("File Name").table_align(true).widget_holder(),
			WidgetHolder::unrelated_separator(),
			TextInput::new(&self.file_name)
				.on_update(|text_input: &TextInput| ExportDialogMessage::FileName(text_input.value.clone()).into())
				.widget_holder(),
		];

		let entries = [(FileType::Png, "PNG"), (FileType::Jpg, "JPG"), (FileType::Svg, "SVG")]
			.into_iter()
			.map(|(val, name)| RadioEntryData::new(name).on_update(move |_| ExportDialogMessage::FileType(val).into()))
			.collect();

		let export_type = vec![
			TextLabel::new("File Type").table_align(true).widget_holder(),
			WidgetHolder::unrelated_separator(),
			RadioInput::new(entries).selected_index(self.file_type as u32).widget_holder(),
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
			.map(|(val, name, disabled)| DropdownEntryData::new(name).on_update(move |_| ExportDialogMessage::ExportBounds(val).into()).disabled(disabled))
			.collect()];

		let export_area = vec![
			TextLabel::new("Bounds").table_align(true).widget_holder(),
			WidgetHolder::unrelated_separator(),
			DropdownInput::new(entries).selected_index(Some(index as u32)).widget_holder(),
		];

		let transparent_background = vec![
			TextLabel::new("Transparency").table_align(true).widget_holder(),
			WidgetHolder::unrelated_separator(),
			CheckboxInput::new(self.transparent_background)
				.disabled(self.file_type == FileType::Jpg)
				.on_update(move |value: &CheckboxInput| ExportDialogMessage::TransparentBackground(value.checked).into())
				.widget_holder(),
		];

		let resolution = vec![
			TextLabel::new("Scale Factor").table_align(true).widget_holder(),
			WidgetHolder::unrelated_separator(),
			NumberInput::new(Some(self.scale_factor))
				.unit(" ")
				.min(0.)
				.disabled(self.file_type == FileType::Svg)
				.on_update(|number_input: &NumberInput| ExportDialogMessage::ScaleFactor(number_input.value.unwrap()).into())
				.widget_holder(),
		];

		let button_widgets = vec![
			TextButton::new("Export")
				.min_width(96)
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![ExportDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Cancel").min_width(96).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Export").bold(true).widget_holder()],
			},
			LayoutGroup::Row { widgets: file_name },
			LayoutGroup::Row { widgets: export_type },
			LayoutGroup::Row { widgets: resolution },
			LayoutGroup::Row { widgets: export_area },
			LayoutGroup::Row { widgets: transparent_background },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
