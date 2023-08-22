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
			DialogMessage::ExportDialog(message) => self.export_dialog.process_message(message, responses, ()),
			#[remain::unsorted]
			DialogMessage::NewDocumentDialog(message) => self.new_document_dialog.process_message(message, responses, ()),
			#[remain::unsorted]
			DialogMessage::PreferencesDialog(message) => self.preferences_dialog.process_message(message, responses, preferences),

			DialogMessage::CloseAllDocumentsWithConfirmation => {
				let dialog = simple_dialogs::CloseAllDocumentsDialog;
				dialog.send_layout(responses, LayoutTarget::DialogDetails);
				responses.add(FrontendMessage::DisplayDialog { icon: "Copy".to_string() });
			}
			DialogMessage::CloseDialogAndThen { followups } => {
				responses.add(FrontendMessage::DisplayDialogDismiss);
				for message in followups.into_iter() {
					responses.add(message);
				}
			}
			DialogMessage::DisplayDialogError { title, description } => {
				let dialog = simple_dialogs::ErrorDialog { title, description };
				dialog.send_layout(responses, LayoutTarget::DialogDetails);
				responses.add(FrontendMessage::DisplayDialog { icon: "Warning".to_string() });
			}
			DialogMessage::RequestAboutGraphiteDialog => {
				responses.add(FrontendMessage::TriggerAboutGraphiteLocalizedCommitDate {
					commit_date: env!("GRAPHITE_GIT_COMMIT_DATE").into(),
				});
			}
			DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate { localized_commit_date } => {
				let about_graphite = AboutGraphiteDialog { localized_commit_date };

				about_graphite.send_layout(responses, LayoutTarget::DialogDetails);
				responses.add(FrontendMessage::DisplayDialog { icon: "GraphiteLogo".to_string() });
			}
			DialogMessage::RequestComingSoonDialog { issue } => {
				let coming_soon = ComingSoonDialog { issue };
				coming_soon.send_layout(responses, LayoutTarget::DialogDetails);
				responses.add(FrontendMessage::DisplayDialog { icon: "Warning".to_string() });
			}
			DialogMessage::RequestDemoArtworkDialog => {
				let demo_artwork_dialog = DemoArtworkDialog;
				demo_artwork_dialog.send_layout(responses, LayoutTarget::DialogDetails);
				responses.add(FrontendMessage::DisplayDialog { icon: "Image".to_string() });
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
						file_name: document.name.clone(),
						scale_factor: 1.,
						artboards,
						has_selection: document.selected_layers().next().is_some(),
						..Default::default()
					};
					self.export_dialog.send_layout(responses, LayoutTarget::DialogDetails);
					responses.add(FrontendMessage::DisplayDialog { icon: "File".to_string() });
				}
			}
			DialogMessage::RequestNewDocumentDialog => {
				self.new_document_dialog = NewDocumentDialogMessageHandler {
					name: portfolio.generate_new_document_name(),
					infinite: false,
					dimensions: glam::UVec2::new(1920, 1080),
				};
				self.new_document_dialog.send_layout(responses, LayoutTarget::DialogDetails);
				responses.add(FrontendMessage::DisplayDialog { icon: "File".to_string() });
			}
			DialogMessage::RequestPreferencesDialog => {
				self.preferences_dialog = PreferencesDialogMessageHandler {};
				self.preferences_dialog.send_layout(responses, LayoutTarget::DialogDetails, preferences);
				responses.add(FrontendMessage::DisplayDialog { icon: "Settings".to_string() });
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
