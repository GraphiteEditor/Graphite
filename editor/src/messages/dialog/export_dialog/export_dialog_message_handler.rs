use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

/// A dialog to allow users to customize their file export.
#[derive(Debug, Clone, Default)]
pub struct ExportDialogMessageHandler {
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub transparent_background: bool,
	pub artboards: HashMap<LayerNodeIdentifier, String>,
	pub has_selection: bool,
}

impl MessageHandler<ExportDialogMessage, &PortfolioMessageHandler> for ExportDialogMessageHandler {
	fn process_message(&mut self, message: ExportDialogMessage, responses: &mut VecDeque<Message>, portfolio: &PortfolioMessageHandler) {
		match message {
			ExportDialogMessage::FileType(export_type) => self.file_type = export_type,
			ExportDialogMessage::ScaleFactor(factor) => self.scale_factor = factor,
			ExportDialogMessage::TransparentBackground(transparent_background) => self.transparent_background = transparent_background,
			ExportDialogMessage::ExportBounds(export_area) => self.bounds = export_area,

			ExportDialogMessage::Submit => responses.add_front(PortfolioMessage::SubmitDocumentExport {
				file_name: portfolio.active_document().map(|document| document.name.clone()).unwrap_or_default(),
				file_type: self.file_type,
				scale_factor: self.scale_factor,
				bounds: self.bounds,
				transparent_background: self.file_type != FileType::Jpg && self.transparent_background,
			}),
		}

		self.send_dialog_to_frontend(responses);
	}

	advertise_actions! {ExportDialogUpdate;}
}

impl DialogLayoutHolder for ExportDialogMessageHandler {
	const ICON: &'static str = "File";
	const TITLE: &'static str = "Export";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("Export")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![ExportDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Cancel").on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for ExportDialogMessageHandler {
	fn layout(&self) -> Layout {
		let entries = [(FileType::Png, "PNG"), (FileType::Jpg, "JPG"), (FileType::Svg, "SVG")]
			.into_iter()
			.map(|(val, name)| RadioEntryData::new(name).on_update(move |_| ExportDialogMessage::FileType(val).into()))
			.collect();

		let export_type = vec![
			TextLabel::new("File Type").table_align(true).min_width(100).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries).selected_index(Some(self.file_type as u32)).widget_holder(),
		];

		let resolution = vec![
			TextLabel::new("Scale Factor").table_align(true).min_width(100).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(self.scale_factor))
				.unit("")
				.min(0.)
				.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
				.disabled(self.file_type == FileType::Svg)
				.on_update(|number_input: &NumberInput| ExportDialogMessage::ScaleFactor(number_input.value.unwrap()).into())
				.min_width(200)
				.widget_holder(),
		];

		let artboards = self.artboards.iter().map(|(&layer, name)| (ExportBounds::Artboard(layer), name.to_string(), false));
		let mut export_area_options = vec![
			(ExportBounds::AllArtwork, "All Artwork".to_string(), false),
			(ExportBounds::Selection, "Selection".to_string(), !self.has_selection),
		];
		export_area_options.extend(artboards);
		let index = export_area_options.iter().position(|(val, _, _)| val == &self.bounds).unwrap();
		let entries = vec![export_area_options
			.into_iter()
			.map(|(val, name, disabled)| MenuListEntry::new(name).on_update(move |_| ExportDialogMessage::ExportBounds(val).into()).disabled(disabled))
			.collect()];

		let export_area = vec![
			TextLabel::new("Bounds").table_align(true).min_width(100).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(entries).selected_index(Some(index as u32)).widget_holder(),
		];

		let transparent_background = vec![
			TextLabel::new("Transparency").table_align(true).min_width(100).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(self.transparent_background)
				.disabled(self.file_type == FileType::Jpg)
				.on_update(move |value: &CheckboxInput| ExportDialogMessage::TransparentBackground(value.checked).into())
				.widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row { widgets: export_type },
			LayoutGroup::Row { widgets: resolution },
			LayoutGroup::Row { widgets: export_area },
			LayoutGroup::Row { widgets: transparent_background },
		]))
	}
}
