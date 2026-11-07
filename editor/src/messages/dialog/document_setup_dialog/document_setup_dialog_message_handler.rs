use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;
use graphene_std::units::DEFAULT_PIXELS_PER_INCH;

/// A dialog to allow users to change the settings of the active document.
#[derive(Debug, Clone, ExtractField)]
pub struct DocumentSetupDialogMessageHandler {
	pub pixels_per_inch: f64,
}

impl Default for DocumentSetupDialogMessageHandler {
	fn default() -> Self {
		Self {
			pixels_per_inch: DEFAULT_PIXELS_PER_INCH,
		}
	}
}

#[message_handler_data]
impl MessageHandler<DocumentSetupDialogMessage, ()> for DocumentSetupDialogMessageHandler {
	fn process_message(&mut self, message: DocumentSetupDialogMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			DocumentSetupDialogMessage::Resolution { pixels_per_inch } => self.pixels_per_inch = pixels_per_inch,

			DocumentSetupDialogMessage::Submit => {
				// The settings apply as one undo step, which ending the transaction drops if nothing changed
				responses.add(DocumentMessage::StartTransaction);
				responses.add(DocumentMessage::SetPixelsPerInch {
					pixels_per_inch: self.pixels_per_inch,
				});
				responses.add(DocumentMessage::EndTransaction);
			}
		}

		self.send_dialog_to_frontend(responses);
	}

	advertise_actions!(DocumentSetupDialogUpdate;
	);
}

impl DialogLayoutHolder for DocumentSetupDialogMessageHandler {
	const ICON: &'static str = "File";
	const TITLE: &'static str = "Document Setup";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("OK")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseAndThen {
						followups: vec![DocumentSetupDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Cancel").on_update(|_| DialogMessage::Close.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for DocumentSetupDialogMessageHandler {
	fn layout(&self) -> Layout {
		let resolution = resolution_row(self.pixels_per_inch, |pixels_per_inch| DocumentSetupDialogMessage::Resolution { pixels_per_inch }.into());

		Layout(vec![LayoutGroup::row(resolution)])
	}
}

/// The labeled field for a document's resolution in pixels per inch, shared by the dialogs that set it.
pub fn resolution_row(pixels_per_inch: f64, on_update: impl Fn(f64) -> Message + Send + Sync + 'static) -> Vec<WidgetInstance> {
	let description = "How many pixels make an inch, which sizes the lengths typed in physical units like millimeters and inches, and the physical size of exported files.";

	vec![
		TextLabel::new("Resolution")
			.table_align(true)
			.sizing(WidgetSizing::fixed(90))
			.tooltip_label("Resolution")
			.tooltip_description(description)
			.widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		NumberInput::new(Some(pixels_per_inch))
			.unit(" PPI")
			.min(1.)
			.max(100_000.)
			.tooltip_label("Resolution")
			.tooltip_description(description)
			// Matches the width of the New Document dialog's Name field
			.sizing(WidgetSizing::default().min_width(204))
			.on_update(move |number_input: &NumberInput| on_update(number_input.value.unwrap_or(DEFAULT_PIXELS_PER_INCH)))
			.widget_instance(),
	]
}

#[cfg(test)]
mod tests {
	use crate::test_utils::test_prelude::*;

	fn pixels_per_inch(editor: &EditorTestUtils) -> f64 {
		editor.active_document().network_interface.pixels_per_inch()
	}

	async fn open_and_set_resolution(editor: &mut EditorTestUtils, pixels_per_inch: f64) {
		editor.handle_message(DialogMessage::RequestDocumentSetupDialog).await;
		editor.handle_message(DocumentSetupDialogMessage::Resolution { pixels_per_inch }).await;
	}

	async fn confirm(editor: &mut EditorTestUtils) {
		editor
			.handle_message(DialogMessage::CloseAndThen {
				followups: vec![DocumentSetupDialogMessage::Submit.into()],
			})
			.await;
	}

	#[tokio::test]
	async fn confirming_a_resolution_applies_it_as_one_undo_step() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 100., 100.).await;
		let layer_count = editor.active_document().metadata().all_layers().count();

		open_and_set_resolution(&mut editor, 300.).await;
		assert_eq!(pixels_per_inch(&editor), 96., "the dialog applies nothing before it is confirmed");
		confirm(&mut editor).await;
		assert_eq!(pixels_per_inch(&editor), 300.);

		// Undoing reverts only the resolution, leaving the rectangle drawn before it
		editor.handle_message(DocumentMessage::Undo).await;
		assert_eq!(pixels_per_inch(&editor), 96.);
		assert_eq!(editor.active_document().metadata().all_layers().count(), layer_count);

		editor.handle_message(DocumentMessage::Redo).await;
		assert_eq!(pixels_per_inch(&editor), 300.);
	}

	#[tokio::test]
	async fn canceling_or_confirming_an_unchanged_resolution_adds_no_undo_step() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		open_and_set_resolution(&mut editor, 300.).await;
		confirm(&mut editor).await;

		open_and_set_resolution(&mut editor, 150.).await;
		editor.handle_message(DialogMessage::Close).await;
		assert_eq!(pixels_per_inch(&editor), 300., "canceling discards the dialog's resolution");

		// Reopening starts from the document's resolution, so confirming it unchanged has nothing to apply
		editor.handle_message(DialogMessage::RequestDocumentSetupDialog).await;
		confirm(&mut editor).await;
		assert_eq!(pixels_per_inch(&editor), 300.);

		editor.handle_message(DocumentMessage::Undo).await;
		assert_eq!(pixels_per_inch(&editor), 96., "the one undo step is the resolution's first change");
	}
}
