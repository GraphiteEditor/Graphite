use super::simple_dialogs::{self, AboutGraphiteDialog, ComingSoonDialog, DemoArtworkDialog, LicensesDialog};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::is_layer_fed_by_node_of_name;

/// Stores the dialogs which require state. These are the ones that have their own message handlers, and are not the ones defined in `simple_dialogs`.
#[derive(Debug, Default, Clone)]
pub struct DialogMessageHandler {
	export_dialog: ExportDialogMessageHandler,
	new_document_dialog: NewDocumentDialogMessageHandler,
	preferences_dialog: PreferencesDialogMessageHandler,
}

pub struct DialogData<'a> {
	pub portfolio: &'a PortfolioMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
}

impl MessageHandler<DialogMessage, DialogData<'_>> for DialogMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: DialogMessage, responses: &mut VecDeque<Message>, DialogData { portfolio, preferences }: DialogData) {
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
					let mut index = 0;
					let artboards = document
						.metadata
						.all_layers()
						.filter(|&layer| is_layer_fed_by_node_of_name(layer, &document.network, "Artboard"))
						.map(|layer| {
							(
								layer,
								format!("Artboard: {}", {
									index += 1;
									format!("Untitled {index}")
								}),
							)
						})
						.collect();

					self.export_dialog = ExportDialogMessageHandler {
						scale_factor: 1.,
						artboards,
						has_selection: document.selected_nodes.selected_layers(document.metadata()).next().is_some(),
						..Default::default()
					};
					self.export_dialog.send_dialog_to_frontend(responses);
				}
			}
			DialogMessage::RequestLicensesDialogWithLocalizedCommitDate { localized_commit_year } => {
				let dialog = LicensesDialog { localized_commit_year };

				dialog.send_dialog_to_frontend(responses);
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
