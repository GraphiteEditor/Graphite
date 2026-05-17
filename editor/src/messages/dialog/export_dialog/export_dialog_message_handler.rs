use crate::messages::frontend::utility_types::{AnimationExport, ExportBounds, FileType};
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
	pub artboards: HashMap<LayerNodeIdentifier, String>,
	pub has_selection: bool,
	pub animated: bool,
	pub fps: f64,
	pub start_seconds: f64,
	pub end_seconds: f64,
}

impl Default for ExportDialogMessageHandler {
	fn default() -> Self {
		Self {
			file_type: Default::default(),
			scale_factor: 1.,
			bounds: Default::default(),
			artboards: Default::default(),
			has_selection: false,
			animated: false,
			fps: 30.,
			start_seconds: 0.,
			end_seconds: 1.,
		}
	}
}

impl ExportDialogMessageHandler {
	fn total_frames(&self) -> u32 {
		let duration = (self.end_seconds - self.start_seconds).max(0.);
		((duration * self.fps).round() as i64).max(1) as u32
	}
}

#[message_handler_data]
impl MessageHandler<ExportDialogMessage, ExportDialogMessageContext<'_>> for ExportDialogMessageHandler {
	fn process_message(&mut self, message: ExportDialogMessage, responses: &mut VecDeque<Message>, context: ExportDialogMessageContext) {
		let ExportDialogMessageContext { portfolio } = context;

		match message {
			ExportDialogMessage::FileType { file_type } => self.file_type = file_type,
			ExportDialogMessage::ScaleFactor { factor } => self.scale_factor = factor,
			ExportDialogMessage::ExportBounds { bounds } => self.bounds = bounds,
			ExportDialogMessage::Animated { animated } => self.animated = animated,
			ExportDialogMessage::Fps { fps } => self.fps = fps.max(0.001),
			ExportDialogMessage::StartSeconds { start } => {
				self.start_seconds = start.max(0.);
				if self.end_seconds < self.start_seconds {
					self.end_seconds = self.start_seconds;
				}
			}
			ExportDialogMessage::EndSeconds { end } => self.end_seconds = end.max(self.start_seconds),

			ExportDialogMessage::Submit => {
				// Fall back to "All Artwork" if "Selection" was chosen but nothing is currently selected
				let bounds = if !self.has_selection && self.bounds == ExportBounds::Selection {
					ExportBounds::AllArtwork
				} else {
					self.bounds
				};

				let artboard_name = match bounds {
					ExportBounds::Artboard(layer) => self.artboards.get(&layer).cloned(),
					_ => None,
				};

				let animation = self.animated.then(|| AnimationExport {
					fps: self.fps,
					start_seconds: self.start_seconds,
					total_frames: self.total_frames(),
				});

				responses.add_front(PortfolioMessage::SubmitDocumentExport {
					name: portfolio.active_document().map(|document| document.name.clone()).unwrap_or_default(),
					file_type: self.file_type,
					scale_factor: self.scale_factor,
					bounds,
					artboard_name,
					artboard_count: self.artboards.len(),
					animation,
				})
			}
		}

		self.send_dialog_to_frontend(responses);
	}

	advertise_actions!(ExportDialogUpdate;
	);
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
			TextButton::new("Cancel").on_update(|_| FrontendMessage::DialogClose.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for ExportDialogMessageHandler {
	fn layout(&self) -> Layout {
		let entries = [(FileType::Png, "PNG"), (FileType::Jpg, "JPG"), (FileType::Svg, "SVG")]
			.into_iter()
			.map(|(file_type, name)| {
				RadioEntryData::new(format!("{file_type:?}"))
					.label(name)
					.on_update(move |_| ExportDialogMessage::FileType { file_type }.into())
			})
			.collect();

		let export_type = vec![
			TextLabel::new("File Type").table_align(true).min_width(100).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			RadioInput::new(entries).selected_index(Some(self.file_type as u32)).widget_instance(),
		];

		let resolution = vec![
			TextLabel::new("Scale Factor").table_align(true).min_width(100).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			NumberInput::new(Some(self.scale_factor))
				.unit("")
				.min(0.)
				.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
				.increment_step(0.5)
				.disabled(self.file_type == FileType::Svg)
				.on_update(|number_input: &NumberInput| ExportDialogMessage::ScaleFactor { factor: number_input.value.unwrap() }.into())
				.min_width(200)
				.widget_instance(),
		];

		let standard_bounds = vec![
			(ExportBounds::AllArtwork, "All Artwork".to_string(), false),
			(ExportBounds::Selection, "Selection".to_string(), !self.has_selection),
		];
		let artboards = self.artboards.iter().map(|(&layer, name)| (ExportBounds::Artboard(layer), name.to_string(), false)).collect();
		let choices = [standard_bounds, artboards];

		// Fall back to "All Artwork" if "Selection" was chosen but nothing is currently selected
		let current_bounds = if !self.has_selection && self.bounds == ExportBounds::Selection {
			ExportBounds::AllArtwork
		} else {
			self.bounds
		};
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
			TextLabel::new("Bounds").table_align(true).min_width(100).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			DropdownInput::new(entries).selected_index(Some(index as u32)).widget_instance(),
		];

		let animation_checkbox_id = CheckboxId::new();
		let animation_toggle = vec![
			TextLabel::new("Animation").table_align(true).min_width(100).for_checkbox(animation_checkbox_id).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			CheckboxInput::new(self.animated)
				.on_update(|checkbox_input: &CheckboxInput| ExportDialogMessage::Animated { animated: checkbox_input.checked }.into())
				.for_label(animation_checkbox_id)
				.widget_instance(),
		];

		let mut layout_groups = vec![
			LayoutGroup::row(export_type),
			LayoutGroup::row(resolution),
			LayoutGroup::row(export_area),
			LayoutGroup::row(animation_toggle),
		];

		if self.animated {
			let fps_row = vec![
				TextLabel::new("FPS").table_align(true).min_width(100).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(self.fps))
					.unit(" fps")
					.min(0.001)
					.max(1000.)
					.increment_step(1.)
					.on_update(|number_input: &NumberInput| ExportDialogMessage::Fps { fps: number_input.value.unwrap() }.into())
					.min_width(200)
					.widget_instance(),
			];

			let start_row = vec![
				TextLabel::new("Start").table_align(true).min_width(100).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(self.start_seconds))
					.unit(" sec")
					.min(0.)
					.increment_step(0.1)
					.on_update(|number_input: &NumberInput| ExportDialogMessage::StartSeconds { start: number_input.value.unwrap() }.into())
					.min_width(200)
					.widget_instance(),
			];

			let end_row = vec![
				TextLabel::new("End").table_align(true).min_width(100).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(self.end_seconds))
					.unit(" sec")
					.min(self.start_seconds)
					.increment_step(0.1)
					.on_update(|number_input: &NumberInput| ExportDialogMessage::EndSeconds { end: number_input.value.unwrap() }.into())
					.min_width(200)
					.widget_instance(),
			];

			let frame_count = self.total_frames();
			let frames_row = vec![
				TextLabel::new("Frames").table_align(true).min_width(100).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				TextLabel::new(format!("{frame_count} frame{}", if frame_count == 1 { "" } else { "s" })).widget_instance(),
			];

			layout_groups.push(LayoutGroup::row(fps_row));
			layout_groups.push(LayoutGroup::row(start_row));
			layout_groups.push(LayoutGroup::row(end_row));
			layout_groups.push(LayoutGroup::row(frames_row));
		}

		Layout(layout_groups)
	}
}
