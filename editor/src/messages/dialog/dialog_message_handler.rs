use super::simple_dialogs::{self, AboutGraphiteDialog, DemoArtworkDialog, LicensesDialog};
use crate::application::GRAPHITE_GIT_COMMIT_DATE;
use crate::messages::dialog::simple_dialogs::{ConfirmRestartDialog, LicensesThirdPartyDialog};
use crate::messages::frontend::utility_types::ExportBounds;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct DialogMessageContext<'a> {
	pub portfolio: &'a PortfolioMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
}

/// Stores the dialogs which require state. These are the ones that have their own message handlers, and are not the ones defined in `simple_dialogs`.
#[derive(Debug, Default, Clone, ExtractField)]
pub struct DialogMessageHandler {
	on_dismiss: Option<Message>,
	export_dialog: ExportDialogMessageHandler,
	new_document_dialog: NewDocumentDialogMessageHandler,
	preferences_dialog: PreferencesDialogMessageHandler,
}

#[message_handler_data]
impl MessageHandler<DialogMessage, DialogMessageContext<'_>> for DialogMessageHandler {
	fn process_message(&mut self, message: DialogMessage, responses: &mut VecDeque<Message>, context: DialogMessageContext) {
		let DialogMessageContext { portfolio, preferences } = context;

		match message {
			DialogMessage::ExportDialog(message) => self.export_dialog.process_message(message, responses, ExportDialogMessageContext { portfolio }),
			DialogMessage::NewDocumentDialog(message) => self.new_document_dialog.process_message(message, responses, ()),
			DialogMessage::PreferencesDialog(message) => self.preferences_dialog.process_message(message, responses, PreferencesDialogMessageContext { preferences }),

			DialogMessage::Dismiss => {
				if let Some(message) = self.on_dismiss.take() {
					responses.add(message);
				}
			}
			DialogMessage::Close => {
				self.on_dismiss = None;
				responses.add(FrontendMessage::DialogClose)
			}
			DialogMessage::CloseAndThen { followups } => {
				for message in followups.into_iter() {
					responses.add(message);
				}

				// This come after followups, so that the followups (which can cause the dialog to open) happen first, then we close it afterwards.
				// If it comes before, the dialog reopens (and appears to not close at all).
				responses.add(DialogMessage::Close);
			}
			DialogMessage::CloseAllDocumentsWithConfirmation => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				let dialog = simple_dialogs::CloseAllDocumentsDialog {
					unsaved_document_names: portfolio.unsaved_document_names(),
				};
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::DisplayDialogError { title, description } => {
				self.on_dismiss = None;
				let dialog = simple_dialogs::ErrorDialog { title, description };
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestAboutGraphiteDialog => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				responses.add(FrontendMessage::TriggerAboutGraphiteLocalizedCommitDate {
					commit_date: GRAPHITE_GIT_COMMIT_DATE.into(),
				});
			}
			DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate {
				localized_commit_date,
				localized_commit_year,
			} => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				let dialog = AboutGraphiteDialog {
					localized_commit_date,
					localized_commit_year,
				};

				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestDemoArtworkDialog => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				let dialog = DemoArtworkDialog;
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestExportDialog => {
				self.on_dismiss = Some(DialogMessage::Close.into());
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

					if let ExportBounds::Artboard(layer) = self.export_dialog.bounds
						&& !self.export_dialog.artboards.contains_key(&layer)
					{
						self.export_dialog.bounds = ExportBounds::AllArtwork;
					}

					self.export_dialog.has_selection = document.network_interface.selected_nodes().selected_layers(document.metadata()).next().is_some();
					self.export_dialog.send_dialog_to_frontend(responses);
				}
			}
			DialogMessage::RequestLicensesDialogWithLocalizedCommitDate { localized_commit_year } => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				let dialog = LicensesDialog { localized_commit_year };
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestLicensesThirdPartyDialogWithLicenseText { license_text } => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				let dialog = LicensesThirdPartyDialog { license_text };
				dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestNewDocumentDialog => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				self.new_document_dialog = NewDocumentDialogMessageHandler {
					name: portfolio.generate_new_document_name(),
					infinite: false,
					dimensions: glam::UVec2::new(1920, 1080),
				};
				self.new_document_dialog.send_dialog_to_frontend(responses);
			}
			DialogMessage::RequestPreferencesDialog => {
				self.on_dismiss = Some(PreferencesDialogMessage::Confirm.into());
				self.preferences_dialog.send_dialog_to_frontend(responses, preferences);
			}
			DialogMessage::RequestConfirmRestartDialog => {
				self.on_dismiss = Some(DialogMessage::Close.into());
				let dialog = ConfirmRestartDialog {
					changed_settings: vec!["Disable UI Acceleration".into()],
				};
				dialog.send_dialog_to_frontend(responses);
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
