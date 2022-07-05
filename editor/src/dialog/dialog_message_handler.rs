use super::*;
use crate::document::PortfolioMessageHandler;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;

#[derive(Debug, Default, Clone)]
pub struct DialogMessageHandler {
	export_dialog: Export,
	new_document_dialog: NewDocument,
}

impl MessageHandler<DialogMessage, &PortfolioMessageHandler> for DialogMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: DialogMessage, portfolio: &PortfolioMessageHandler, responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			#[remain::unsorted]
			DialogMessage::ExportDialog(message) => self.export_dialog.process_action(message, (), responses),
			#[remain::unsorted]
			DialogMessage::NewDocumentDialog(message) => self.new_document_dialog.process_action(message, (), responses),

			DialogMessage::CloseAllDocumentsWithConfirmation => {
				let dialog = dialogs::CloseAllDocuments;
				dialog.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "Copy".to_string() }.into());
			}
			DialogMessage::CloseDialogAndThen { followup } => {
				responses.push_back(FrontendMessage::DisplayDialogDismiss.into());
				responses.push_back(*followup);
			}
			DialogMessage::DisplayDialogError { title, description } => {
				let dialog = dialogs::Error { title, description };
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
				let about_graphite = AboutGraphite { localized_commit_date };

				about_graphite.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "GraphiteLogo".to_string() }.into());
			}
			DialogMessage::RequestComingSoonDialog { issue } => {
				let coming_soon = ComingSoon { issue };
				coming_soon.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "Warning".to_string() }.into());
			}
			DialogMessage::RequestExportDialog => {
				let artboard_handler = &portfolio.active_document().artboard_message_handler;
				let mut index = 0;
				let artboards = artboard_handler
					.artboard_ids
					.iter()
					.rev()
					.filter_map(|&artboard| artboard_handler.artboards_graphene_document.layer(&[artboard]).ok().map(|layer| (artboard, layer)))
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

				self.export_dialog = Export {
					file_name: portfolio.active_document().name.clone(),
					scale_factor: 1.,
					artboards,
					has_selection: portfolio.active_document().selected_layers().next().is_some(),
					..Default::default()
				};
				self.export_dialog.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "File".to_string() }.into());
			}
			DialogMessage::RequestNewDocumentDialog => {
				self.new_document_dialog = NewDocument {
					name: portfolio.generate_new_document_name(),
					infinite: true,
					dimensions: glam::UVec2::new(1920, 1080),
				};
				self.new_document_dialog.register_properties(responses, LayoutTarget::DialogDetails);
				responses.push_back(FrontendMessage::DisplayDialog { icon: "File".to_string() }.into());
			}
		}
	}

	advertise_actions!(DialogMessageDiscriminant;RequestNewDocumentDialog,RequestExportDialog,CloseAllDocumentsWithConfirmation);
}
