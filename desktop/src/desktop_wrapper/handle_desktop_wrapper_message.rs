use graphene_std::Color;
use graphene_std::raster::Image;
use graphite_editor::messages::prelude::{DocumentMessage, Message, PortfolioMessage};

use super::DesktopWrapperMessageExecutor;
use super::messages::{DesktopFrontendMessage, DesktopWrapperMessage, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn handle_desktop_wrapper_message(executor: &mut DesktopWrapperMessageExecutor, message: DesktopWrapperMessage) {
	match message {
		DesktopWrapperMessage::FromWeb(data) => {
			let string = std::str::from_utf8(&data).unwrap();
			match ron::from_str::<Message>(string) {
				Ok(message) => {
					executor.queue_message(message);
				}
				Err(e) => {
					tracing::error!("Failed to deserialize message {:?}", e)
				}
			}
		}
		DesktopWrapperMessage::OpenFileDialogResult { path, content, context } => match context {
			OpenFileDialogContext::Document => {
				executor.queue(DesktopWrapperMessage::OpenDocument { path, content });
			}
			OpenFileDialogContext::Import => {
				executor.queue(DesktopWrapperMessage::ImportFile { path, content });
			}
		},
		DesktopWrapperMessage::SaveFileDialogResult { path, context } => match context {
			SaveFileDialogContext::Document { document_id, content } => {
				executor.respond(DesktopFrontendMessage::WriteFile { path: path.clone(), content });
				executor.queue_message(Message::Portfolio(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: DocumentMessage::SavedDocument { path: Some(path) },
				}));
			}
			SaveFileDialogContext::Export { content } => {
				executor.respond(DesktopFrontendMessage::WriteFile { path, content });
			}
		},
		DesktopWrapperMessage::OpenFile { path, content } => {
			let extension = path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
			match extension.as_str() {
				"graphite" => {
					executor.queue(DesktopWrapperMessage::OpenDocument { path, content });
				}
				_ => {
					executor.queue(DesktopWrapperMessage::ImportFile { path, content });
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
			executor.queue_message(message.into());
		}
		DesktopWrapperMessage::ImportFile { path, content } => {
			let extension = path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
			match extension.as_str() {
				"svg" => {
					executor.queue(DesktopWrapperMessage::ImportSvg { path, content });
				}
				_ => {
					executor.queue(DesktopWrapperMessage::ImportImage { path, content });
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
			executor.queue_message(message.into());
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
			let image_data = image.to_rgba8();
			let image = Image::<Color>::from_image_data(image_data.as_raw(), width, height);
			let message = PortfolioMessage::PasteImage {
				name,
				image,
				mouse: None,
				parent_and_insert_index: None,
			};
			executor.queue_message(message.into());
		}
		DesktopWrapperMessage::PollNodeGraphEvaluation => executor.poll_node_graph_evaluation(),
	}
}
