use super::new_document_dialog::NewDocumentDialogMessageContext;
use super::simple_dialogs::{self, AboutGraphiteDialog, ComingSoonDialog, DemoArtworkDialog, LicensesDialog};
use crate::messages::dialog::simple_dialogs::LicensesThirdPartyDialog;
use crate::messages::input_mapper::utility_types::input_mouse::ViewportBounds;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct DialogMessageContext<'a> {
	pub portfolio: &'a PortfolioMessageHandler,
	pub viewport_bounds: &'a ViewportBounds,
	pub preferences: &'a PreferencesMessageHandler,
}

/// Stores the dialogs which require state. These are the ones that have their own message handlers, and are not the ones defined in `simple_dialogs`.
#[derive(Debug, Default, Clone, ExtractField)]
pub struct DialogMessageHandler {
	export_dialog: ExportDialogMessageHandler,
	new_document_dialog: NewDocumentDialogMessageHandler,
	preferences_dialog: PreferencesDialogMessageHandler,
}

#[message_handler_data]
impl MessageHandler<DialogMessage, DialogMessageContext<'_>> for DialogMessageHandler {
	fn process_message(&mut self, message: DialogMessage, responses: &mut VecDeque<Message>, context: DialogMessageContext) {
		let DialogMessageContext {
			portfolio,
			preferences,
			viewport_bounds,
		} = context;

		match message {
			DialogMessage::ExportDialog(message) => self.export_dialog.process_message(message, responses, ExportDialogMessageContext { portfolio }),
			DialogMessage::NewDocumentDialog(message) => self.new_document_dialog.process_message(message, responses, NewDocumentDialogMessageContext { viewport_bounds }),
			DialogMessage::PreferencesDialog(message) => self.preferences_dialog.process_message(message, responses, PreferencesDialogMessageContext { preferences }),

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
					let artboards = document
						.metadata()
						.all_layers()
						.filter(|&layer| document.network_interface.is_artboard(&layer.to_node(), &[]))
						.map(|layer| {
							let name = document
								.network_interface
								.node_metadata(&layer.to_node(), &[])
								.map(|node| node.persistent_metadata.display_name.clone())
								.and_then(|name| if name.is_empty() { None } else { Some(name) })
								.unwrap_or_else(|| "Artboard".to_string());
							(layer, name)
						})
						.collect();

					self.export_dialog.artboards = artboards;
					self.export_dialog.has_selection = document.network_interface.selected_nodes().selected_layers(document.metadata()).next().is_some();
					self.export_dialog.send_dialog_to_frontend(responses);
				}
			}
			DialogMessage::RequestLicensesDialogWithLocalizedCommitDate { localized_commit_year } => {
				let dialog = LicensesDialog { localized_commit_year };

				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestLicensesThirdPartyDialogWithLicenseText { license_text } => {
				let dialog = LicensesThirdPartyDialog { license_text };
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
