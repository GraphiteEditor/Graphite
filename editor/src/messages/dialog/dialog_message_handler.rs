use super::simple_dialogs::{self, AboutGraphiteDialog, ComingSoonDialog};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::prelude::*;

#[derive(Debug, Default, Clone)]
pub struct DialogMessageHandler {
	export_dialog: ExportDialogMessageHandler,
	new_document_dialog: NewDocumentDialogMessageHandler,
	preferences_dialog: PreferencesDialogMessageHandler,
}

impl MessageHandler<DialogMessage, (&PortfolioMessageHandler, &PreferencesMessageHandler)> for DialogMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: DialogMessage, (portfolio, preferences): (&PortfolioMessageHandler, &PreferencesMessageHandler), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			#[remain::unsorted]
			DialogMessage::ExportDialog(message) => self.export_dialog.process_message(message, (), responses),
			#[remain::unsorted]
			DialogMessage::NewDocumentDialog(message) => self.new_document_dialog.process_message(message, (), responses),
			#[remain::unsorted]
			DialogMessage::PreferencesDialog(message) => self.preferences_dialog.process_message(message, preferences, responses),

			DialogMessage::CloseAllDocumentsWithConfirmation => {
				let dialog = simple_dialogs::CloseAllDocumentsDialog;
				dialog.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "Copy".to_string() }.into());
			}
			DialogMessage::CloseDialogAndThen { followups } => {
				responses.push_back(FrontendMessage::DisplayDialogDismiss.into());
				for message in followups.into_iter() {
					responses.push_back(message);
				}
			}
			DialogMessage::DisplayDialogError { title, description } => {
				let dialog = simple_dialogs::ErrorDialog { title, description };
				dialog.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "Warning".to_string() }.into());
			}
			DialogMessage::RequestAboutGraphiteDialog => {
				responses.push_back(
					FrontendMessage::TriggerAboutGraphiteLocalizedCommitDate {
						commit_date: env!("GRAPHITE_GIT_COMMIT_DATE").into(),
					}
					.into(),
				);
			}
			DialogMessage::RequestAboutGraphiteDialogWithLocalizedCommitDate { localized_commit_date } => {
				let about_graphite = AboutGraphiteDialog { localized_commit_date };

				about_graphite.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "GraphiteLogo".to_string() }.into());
			}
			DialogMessage::RequestComingSoonDialog { issue } => {
				let coming_soon = ComingSoonDialog { issue };
				coming_soon.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "Warning".to_string() }.into());
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
					self.export_dialog.register_properties(responses, LayoutTarget::DialogDetails);
					responses.push_back(FrontendMessage::DisplayDialog { icon: "File".to_string() }.into());
				}
			}
			DialogMessage::RequestNewDocumentDialog => {
				self.new_document_dialog = NewDocumentDialogMessageHandler {
					name: portfolio.generate_new_document_name(),
					infinite: false,
					dimensions: glam::UVec2::new(1920, 1080),
				};
				self.new_document_dialog.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "File".to_string() }.into());
			}
			DialogMessage::RequestPreferencesDialog => {
				self.preferences_dialog = PreferencesDialogMessageHandler {};
				self.preferences_dialog.register_properties(responses, LayoutTarget::DialogDetails, preferences);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "Settings".to_string() }.into());
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
