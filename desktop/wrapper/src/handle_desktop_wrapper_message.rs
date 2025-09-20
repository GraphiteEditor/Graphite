use graphene_std::Color;
use graphene_std::raster::Image;
use graphite_editor::messages::app_window::app_window_message_handler::AppWindowPlatform;
use graphite_editor::messages::prelude::{AppWindowMessage, DocumentMessage, FrontendMessage, PortfolioMessage, PreferencesMessage};

use crate::messages::Platform;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, DesktopWrapperMessage, EditorMessage, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn handle_desktop_wrapper_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: DesktopWrapperMessage) {
	match message {
		DesktopWrapperMessage::FromWeb(message) => {
			dispatcher.queue_editor_message(*message);
		}
		DesktopWrapperMessage::OpenFileDialogResult { path, content, context } => match context {
			OpenFileDialogContext::Document => {
				dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::OpenDocument { path, content });
			}
			OpenFileDialogContext::Import => {
				dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::ImportFile { path, content });
			}
		},
		DesktopWrapperMessage::SaveFileDialogResult { path, context } => match context {
			SaveFileDialogContext::Document { document_id, content } => {
				dispatcher.respond(DesktopFrontendMessage::WriteFile { path: path.clone(), content });
				dispatcher.queue_editor_message(EditorMessage::Portfolio(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: DocumentMessage::SavedDocument { path: Some(path) },
				}));
			}
			SaveFileDialogContext::File { content } => {
				dispatcher.respond(DesktopFrontendMessage::WriteFile { path, content });
			}
		},
		DesktopWrapperMessage::OpenFile { path, content } => {
			let extension = path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
			match extension.as_str() {
				"graphite" => {
					dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::OpenDocument { path, content });
				}
				_ => {
					dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::ImportFile { path, content });
				}
			}
		}
		DesktopWrapperMessage::OpenDocument { path, content } => {
			let Ok(content) = String::from_utf8(content) else {
				tracing::warn!("Document file is invalid: {}", path.display());
				return;
			};

			let message = PortfolioMessage::OpenDocumentFile {
				document_name: None,
				document_path: Some(path),
				document_serialized_content: content,
			};
			dispatcher.queue_editor_message(message.into());
		}
		DesktopWrapperMessage::ImportFile { path, content } => {
			let extension = path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
			match extension.as_str() {
				"svg" => {
					dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::ImportSvg { path, content });
				}
				_ => {
					dispatcher.queue_desktop_wrapper_message(DesktopWrapperMessage::ImportImage { path, content });
				}
			}
		}
		DesktopWrapperMessage::ImportSvg { path, content } => {
			let Ok(content) = String::from_utf8(content) else {
				tracing::warn!("Svg file is invalid: {}", path.display());
				return;
			};

			let message = PortfolioMessage::PasteSvg {
				name: path.file_stem().map(|s| s.to_string_lossy().to_string()),
				svg: content,
				mouse: None,
				parent_and_insert_index: None,
			};
			dispatcher.queue_editor_message(message.into());
		}
		DesktopWrapperMessage::ImportImage { path, content } => {
			let name = path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string());
			let extension = path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
			let Some(image_format) = image::ImageFormat::from_extension(&extension) else {
				tracing::warn!("Unsupported file type: {}", path.display());
				return;
			};
			let reader = image::ImageReader::with_format(std::io::Cursor::new(content), image_format);
			let Ok(image) = reader.decode() else {
				tracing::error!("Failed to decode image: {}", path.display());
				return;
			};
			let width = image.width();
			let height = image.height();

			// TODO: Handle Image formats with more than 8 bits per channel
			let image_data = image.to_rgba8();
			let image = Image::<Color>::from_image_data(image_data.as_raw(), width, height);
			let message = PortfolioMessage::PasteImage {
				name,
				image,
				mouse: None,
				parent_and_insert_index: None,
			};
			dispatcher.queue_editor_message(message.into());
		}
		DesktopWrapperMessage::PollNodeGraphEvaluation => dispatcher.poll_node_graph_evaluation(),
		DesktopWrapperMessage::UpdatePlatform(platform) => {
			let platform = match platform {
				Platform::Windows => AppWindowPlatform::Windows,
				Platform::Mac => AppWindowPlatform::Mac,
				Platform::Linux => AppWindowPlatform::Linux,
			};
			let message = AppWindowMessage::AppWindowUpdatePlatform { platform };
			dispatcher.queue_editor_message(message.into());
		}
		DesktopWrapperMessage::UpdateMaximized { maximized } => {
			let message = FrontendMessage::UpdateMaximized { maximized };
			dispatcher.queue_editor_message(message.into());
		}
		DesktopWrapperMessage::LoadDocument {
			id,
			document,
			to_front,
			select_after_open,
		} => {
			let message = PortfolioMessage::OpenDocumentFileWithId {
				document_id: id,
				document_name: Some(document.name),
				document_path: document.path,
				document_serialized_content: document.content,
				document_is_auto_saved: true,
				document_is_saved: document.is_saved,
				to_front,
				select_after_open,
			};
			dispatcher.queue_editor_message(message.into());
		}
		DesktopWrapperMessage::SelectDocument { id } => {
			let message = PortfolioMessage::SelectDocument { document_id: id };
			dispatcher.queue_editor_message(message.into());
		}
		DesktopWrapperMessage::LoadPreferences { preferences } => {
			let message = PreferencesMessage::Load { preferences };
			dispatcher.queue_editor_message(message.into());
		}
	}
}
