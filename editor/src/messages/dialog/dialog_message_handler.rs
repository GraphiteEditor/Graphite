use super::simple_dialogs::{self, AboutGraphiteDialog, ComingSoonDialog, DemoArtworkDialog};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// Stores the dialogs which require state. These are the ones that have their own message handlers, and are not the ones defined in `simple_dialogs`.
#[derive(Debug, Default, Clone)]
pub struct DialogMessageHandler {
	export_dialog: ExportDialogMessageHandler,
	new_document_dialog: NewDocumentDialogMessageHandler,
	preferences_dialog: PreferencesDialogMessageHandler,
}

impl MessageHandler<DialogMessage, (&PortfolioMessageHandler, &PreferencesMessageHandler)> for DialogMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: DialogMessage, responses: &mut VecDeque<Message>, (portfolio, preferences): (&PortfolioMessageHandler, &PreferencesMessageHandler)) {
		#[remain::sorted]
		match message {
			#[remain::unsorted]
			DialogMessage::ExportDialog(message) => self.export_dialog.process_message(message, responses, portfolio),
			#[remain::unsorted]
			DialogMessage::NewDocumentDialog(message) => self.new_document_dialog.process_message(message, responses, ()),
			#[remain::unsorted]
			DialogMessage::PreferencesDialog(message) => self.preferences_dialog.process_message(message, responses, preferences),

			DialogMessage::CloseAllDocumentsWithConfirmation => {
				let dialog = simple_dialogs::CloseAllDocumentsDialog {
					unsaved_document_names: portfolio.unsaved_document_names(),
				};
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::CloseDialogAndThen { followups } => {
				for message in followups.into_iter() {
					responses.add(message);
				}

				// This come after followups, so that the followups (which can cause the dialog to open) happen first, then we close it afterwards.
				// If it comes before, the dialog reopens (and appears to not close at all).
				responses.add(FrontendMessage::DisplayDialogDismiss);
			}
			DialogMessage::DisplayDialogError { title, description } => {
				let dialog = simple_dialogs::ErrorDialog { title, description };
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestAboutGraphiteDialog => {
				responses.add(FrontendMessage::TriggerAboutGraphiteLocalizedCommitDate {
					commit_date: env!("GRAPHITE_GIT_COMMIT_DATE").into(),
				});
			}
			DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate {
				localized_commit_date,
				localized_commit_year,
			} => {
				let dialog = AboutGraphiteDialog {
					localized_commit_date,
					localized_commit_year,
				};

				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestComingSoonDialog { issue } => {
				let dialog = ComingSoonDialog { issue };
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestDemoArtworkDialog => {
				let dialog = DemoArtworkDialog;
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestExportDialog => {
				if let Some(document) = portfolio.active_document() {
					let artboard_handler = &document.artboard_message_handler;
					let mut index = 0;
					let artboards = artboard_handler
						.artboard_ids
						.iter()
						.rev()
						.filter_map(|&artboard| artboard_handler.artboards_document.layer(&[artboard]).ok().map(|layer| (artboard, layer)))
						.map(|(artboard, layer)| {
							(
								artboard,
								format!(
									"Artboard: {}",
									layer.name.clone().unwrap_or_else(|| {
										index += 1;
										format!("Untitled {index}")
									})
								),
							)
						})
						.collect();

					self.export_dialog = ExportDialogMessageHandler {
						scale_factor: 1.,
						artboards,
						has_selection: document.selected_layers().next().is_some(),
						..Default::default()
					};
					self.export_dialog.send_dialog_to_frontend(responses);
				}
			}
			DialogMessage::RequestNewDocumentDialog => {
				self.new_document_dialog = NewDocumentDialogMessageHandler {
					name: portfolio.generate_new_document_name(),
					infinite: false,
					dimensions: glam::UVec2::new(1920, 1080),
				};
				self.new_document_dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestPreferencesDialog => {
				self.preferences_dialog = PreferencesDialogMessageHandler {};
				self.preferences_dialog.send_dialog_to_frontend(responses, preferences);
			}
		}
	}

	advertise_actions!(DialogMessageDiscriminant;
		CloseAllDocumentsWithConfirmation,
		RequestExportDialog,
		RequestNewDocumentDialog,
		RequestPreferencesDialog,
	);
}
