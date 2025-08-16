use graphite_editor::messages::prelude::{DocumentMessage, Message, PortfolioMessage};

use crate::editor_api::messages::{EditorMessage, NativeMessage, OpenFileDialogContext, SaveFileDialogContext};

use super::EditorWrapper;

pub(super) fn handle_editor_message(editor_wrapper: &mut EditorWrapper, message: EditorMessage) {
	match message {
		EditorMessage::FromFrontend(data) => {
			let string = std::str::from_utf8(&data).unwrap();
			match ron::from_str::<Message>(string) {
				Ok(message) => {
					editor_wrapper.queue_message(message);
				}
				Err(e) => {
					tracing::error!("Failed to deserialize message {:?}", e)
				}
			}
		}
		EditorMessage::OpenFileDialogResult { path, content, context } => match context {
			OpenFileDialogContext::Document => match String::from_utf8(content) {
				Ok(content) => {
					editor_wrapper.queue_message(
						PortfolioMessage::OpenDocumentFile {
							document_name: None,
							document_path: Some(path),
							document_serialized_content: content,
						}
						.into(),
					);
				}
				Err(e) => {
					tracing::error!("Failed to deserialize document content: {:?}", e);
				}
			},
			OpenFileDialogContext::Import => {
				let extension = path.extension().and_then(|s| s.to_str());
				let name = path.file_stem().map(|s| s.to_string_lossy().to_string());
				match extension {
					Some("svg") => match String::from_utf8(content) {
						Ok(content) if !content.is_empty() => {
							editor_wrapper.queue_message(
								PortfolioMessage::PasteSvg {
									name,
									svg: content,
									mouse: None,
									parent_and_insert_index: None,
								}
								.into(),
							);
						}
						Ok(_) => {
							tracing::warn!("Svg file is empty: {}", path.display());
						}
						Err(e) => {
							tracing::error!("Failed to deserialize document content: {:?}", e);
						}
					},
					Some(_) => {
						let reader = image::ImageReader::new(std::io::Cursor::new(content));
						match reader.decode() {
							Ok(image) => {
								let width = image.width();
								let height = image.height();
								let image_data = image.to_rgba8();
								let image = graphene_std::raster::Image::<graphene_std::Color>::from_image_data(image_data.as_raw(), width, height);

								editor_wrapper.queue_message(
									PortfolioMessage::PasteImage {
										name,
										image,
										mouse: None,
										parent_and_insert_index: None,
									}
									.into(),
								);
							}
							Err(e) => {
								tracing::error!("Failed to decode image: {}: {}", path.display(), e);
							}
						}
					}
					_ => {
						tracing::warn!("Unsupported file type: {}", path.display());
					}
				}
			}
		},
		EditorMessage::SaveFileDialogResult { path, context } => match context {
			SaveFileDialogContext::Document { document_id, content } => {
				editor_wrapper.respond(NativeMessage::WriteFile { path: path.clone(), content });
				editor_wrapper.queue_message(Message::Portfolio(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: DocumentMessage::SavedDocument { path: Some(path) },
				}));
			}
			SaveFileDialogContext::Export { content } => {
				editor_wrapper.respond(NativeMessage::WriteFile { path, content });
			}
		},
		EditorMessage::PoolNodeGraphEvaluation => editor_wrapper.poll_node_graph_evaluation(),
	}
}
