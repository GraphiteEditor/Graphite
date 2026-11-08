use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct ExportDialogMessageContext<'a> {
	pub portfolio: &'a PortfolioMessageHandler,
}

/// A dialog to allow users to customize their file export.
#[derive(Debug, Clone, ExtractField)]
pub struct ExportDialogMessageHandler {
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	/// Whether the bounds were picked in the dialog, rather than left at the default of the file type, which a change of file type replaces.
	pub bounds_chosen: bool,
	/// Each artboard with its name, from the top of the Layers panel down.
	pub artboards: Vec<(LayerNodeIdentifier, String)>,
	pub has_selection: bool,
}

impl Default for ExportDialogMessageHandler {
	fn default() -> Self {
		Self {
			file_type: Default::default(),
			scale_factor: 1.,
			bounds: Default::default(),
			bounds_chosen: false,
			artboards: Default::default(),
			has_selection: false,
		}
	}
}

#[message_handler_data]
impl MessageHandler<ExportDialogMessage, ExportDialogMessageContext<'_>> for ExportDialogMessageHandler {
	fn process_message(&mut self, message: ExportDialogMessage, responses: &mut VecDeque<Message>, context: ExportDialogMessageContext) {
		let ExportDialogMessageContext { portfolio } = context;

		match message {
			ExportDialogMessage::FileType { file_type } => {
				self.file_type = file_type;
				if !self.bounds_chosen {
					self.bounds = self.default_bounds();
				}
			}
			ExportDialogMessage::ScaleFactor { factor } => self.scale_factor = factor,
			ExportDialogMessage::ExportBounds { bounds } => {
				self.bounds = bounds;
				self.bounds_chosen = true;
			}

			ExportDialogMessage::Submit => {
				let bounds = self.current_bounds();

				let artboard_name = match bounds {
					ExportBounds::Artboard(layer) => self.artboards.iter().find(|(artboard, _)| *artboard == layer).map(|(_, name)| name.clone()),
					_ => None,
				};
				responses.add_front(PortfolioMessage::SubmitDocumentExport {
					name: portfolio.active_document().map(|document| document.name.clone()).unwrap_or_default(),
					file_type: self.file_type,
					scale_factor: self.scale_factor,
					bounds,
					artboard_name,
					artboard_count: self.artboards.len(),
				})
			}
		}

		self.send_dialog_to_frontend(responses);
	}

	advertise_actions!(ExportDialogUpdate;
	);
}

impl ExportDialogMessageHandler {
	/// The bounds a file type starts with: each artboard as its own page for a PDF, or otherwise all the artwork.
	pub fn default_bounds(&self) -> ExportBounds {
		if self.file_type == FileType::Pdf { ExportBounds::AllArtboards } else { ExportBounds::AllArtwork }
	}

	/// The chosen bounds, falling back to the default and then "All Artwork" in place of a choice that isn't currently available.
	fn current_bounds(&self) -> ExportBounds {
		let available = |bounds: &ExportBounds| match bounds {
			ExportBounds::Selection => self.has_selection,
			ExportBounds::AllArtboards => self.file_type == FileType::Pdf && !self.artboards.is_empty(),
			ExportBounds::AllArtwork | ExportBounds::Artboard(_) => true,
		};
		[self.bounds, self.default_bounds()].into_iter().find(available).unwrap_or(ExportBounds::AllArtwork)
	}
}

impl DialogLayoutHolder for ExportDialogMessageHandler {
	const ICON: &'static str = "File";
	const TITLE: &'static str = "Export";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("Export")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseAndThen {
						followups: vec![ExportDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Cancel").on_update(|_| DialogMessage::Close.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for ExportDialogMessageHandler {
	fn layout(&self) -> Layout {
		// The vector types, then the raster ones
		let file_types = [
			vec![(FileType::Svg, "SVG"), (FileType::Pdf, "PDF")],
			vec![
				(FileType::Png, "PNG"),
				(FileType::Jpg, "JPG"),
				(FileType::Webp, "WEBP"),
				(FileType::Tiff, "TIFF"),
				(FileType::Bmp, "BMP"),
				(FileType::Tga, "TGA"),
				(FileType::Ico, "ICO"),
			],
		];
		let selected_index = file_types.iter().flatten().position(|(file_type, _)| *file_type == self.file_type);
		let entries = file_types
			.into_iter()
			.map(|section| {
				section
					.into_iter()
					.map(|(file_type, name)| {
						MenuListEntry::new(format!("{file_type:?}"))
							.label(name)
							.on_commit(move |_| ExportDialogMessage::FileType { file_type }.into())
					})
					.collect()
			})
			.collect();

		let export_type = vec![
			TextLabel::new("File Type").table_align(true).sizing(WidgetSizing::fixed(100)).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			DropdownInput::new(entries)
				.selected_index(selected_index.map(|index| index as u32))
				.sizing(WidgetSizing::default().min_width(200))
				.widget_instance(),
		];

		let resolution = vec![
			TextLabel::new("Scale Factor").table_align(true).sizing(WidgetSizing::fixed(100)).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			NumberInput::new(Some(self.scale_factor))
				.unit("")
				.min(0.)
				.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
				.increment_step(0.5)
				.disabled(matches!(self.file_type, FileType::Svg | FileType::Pdf))
				.on_update(|number_input: &NumberInput| ExportDialogMessage::ScaleFactor { factor: number_input.value.unwrap() }.into())
				.sizing(WidgetSizing::default().min_width(200))
				.widget_instance(),
		];

		let standard_bounds = vec![
			(ExportBounds::AllArtwork, "All Artwork".to_string(), false),
			(ExportBounds::Selection, "Selection".to_string(), !self.has_selection),
		];
		// Only a PDF can hold every artboard, each as its own page
		let all_artboards = (!self.artboards.is_empty()).then(|| (ExportBounds::AllArtboards, "All Artboards".to_string(), self.file_type != FileType::Pdf));
		let artboards = self.artboards.iter().map(|(layer, name)| (ExportBounds::Artboard(*layer), name.to_string(), false));
		let choices = [standard_bounds, all_artboards.into_iter().chain(artboards).collect()];

		let current_bounds = self.current_bounds();
		let index = choices.iter().flatten().position(|(bounds, _, _)| *bounds == current_bounds).unwrap_or(0);

		let mut entries = choices
			.into_iter()
			.map(|choice| {
				choice
					.into_iter()
					.map(|(bounds, name, disabled)| {
						MenuListEntry::new(format!("{bounds:?}"))
							.label(name)
							.on_commit(move |_| ExportDialogMessage::ExportBounds { bounds }.into())
							.disabled(disabled)
					})
					.collect::<Vec<_>>()
			})
			.collect::<Vec<_>>();

		if entries[1].is_empty() {
			entries.remove(1);
		}

		let export_area = vec![
			TextLabel::new("Bounds").table_align(true).sizing(WidgetSizing::fixed(100)).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			DropdownInput::new(entries).selected_index(Some(index as u32)).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(export_type), LayoutGroup::row(resolution), LayoutGroup::row(export_area)])
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::EditorTestUtils;
	use graphene_std::uuid::NodeId;

	#[test]
	fn a_pdf_defaults_to_every_artboard_as_a_page() {
		let editor = EditorTestUtils::create();
		let portfolio = editor.portfolio_message_handler();
		let mut dialog = ExportDialogMessageHandler {
			artboards: vec![(LayerNodeIdentifier::new_unchecked(NodeId(1)), "Artboard".to_string())],
			has_selection: true,
			..Default::default()
		};
		let send = |dialog: &mut ExportDialogMessageHandler, message: ExportDialogMessage| dialog.process_message(message, &mut VecDeque::new(), ExportDialogMessageContext { portfolio });

		// Bounds left at the default follow the file type between PDF and a single image
		send(&mut dialog, ExportDialogMessage::FileType { file_type: FileType::Pdf });
		assert_eq!(dialog.current_bounds(), ExportBounds::AllArtboards);
		send(&mut dialog, ExportDialogMessage::FileType { file_type: FileType::Png });
		assert_eq!(dialog.current_bounds(), ExportBounds::AllArtwork);

		// A choice of other bounds stays through a change of file type
		send(&mut dialog, ExportDialogMessage::ExportBounds { bounds: ExportBounds::Selection });
		send(&mut dialog, ExportDialogMessage::FileType { file_type: FileType::Pdf });
		assert_eq!(dialog.current_bounds(), ExportBounds::Selection);

		// Even a choice of the bounds another file type defaults to
		send(&mut dialog, ExportDialogMessage::ExportBounds { bounds: ExportBounds::AllArtwork });
		send(&mut dialog, ExportDialogMessage::FileType { file_type: FileType::Png });
		send(&mut dialog, ExportDialogMessage::FileType { file_type: FileType::Pdf });
		assert_eq!(dialog.current_bounds(), ExportBounds::AllArtwork);

		// Without artboards, a PDF falls back to one page of all the artwork
		send(&mut dialog, ExportDialogMessage::ExportBounds { bounds: ExportBounds::AllArtboards });
		dialog.artboards.clear();
		assert_eq!(dialog.current_bounds(), ExportBounds::AllArtwork);
	}
}
