use std::path::PathBuf;

use graphite_editor::messages::prelude::FrontendMessage;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, Document, FileFilter, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn intercept_frontend_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: FrontendMessage) -> Option<FrontendMessage> {
	match message {
		FrontendMessage::RenderOverlays { context } => {
			dispatcher.respond(DesktopFrontendMessage::UpdateOverlays(context.take_scene()));
		}
		FrontendMessage::TriggerOpenDocument => {
			dispatcher.respond(DesktopFrontendMessage::OpenFileDialog {
				title: "Open Document".to_string(),
				filters: vec![FileFilter {
					name: "Graphite".to_string(),
					extensions: vec!["graphite".to_string()],
				}],
				context: OpenFileDialogContext::Document,
			});
		}
		FrontendMessage::TriggerImport => {
			dispatcher.respond(DesktopFrontendMessage::OpenFileDialog {
				title: "Import File".to_string(),
				filters: vec![
					FileFilter {
						name: "Svg".to_string(),
						extensions: vec!["svg".to_string()],
					},
					FileFilter {
						name: "Image".to_string(),
						extensions: vec!["png".to_string(), "jpg".to_string(), "jpeg".to_string(), "bmp".to_string()],
					},
				],
				context: OpenFileDialogContext::Import,
			});
		}
		FrontendMessage::TriggerSaveDocument { document_id, name, path, content } => {
			if let Some(path) = path {
				dispatcher.respond(DesktopFrontendMessage::WriteFile { path, content });
			} else {
				dispatcher.respond(DesktopFrontendMessage::SaveFileDialog {
					title: "Save Document".to_string(),
					default_filename: name,
					default_folder: path.and_then(|p| p.parent().map(PathBuf::from)),
					filters: vec![FileFilter {
						name: "Graphite".to_string(),
						extensions: vec!["graphite".to_string()],
					}],
					context: SaveFileDialogContext::Document { document_id, content },
				});
			}
		}
		FrontendMessage::TriggerSaveFile { name, content } => {
			dispatcher.respond(DesktopFrontendMessage::SaveFileDialog {
				title: "Save File".to_string(),
				default_filename: name,
				default_folder: None,
				filters: Vec::new(),
				context: SaveFileDialogContext::File { content },
			});
		}
		FrontendMessage::TriggerExportImageBuffer {
			data,
			width,
			height,
			transparent,
			file_type,
			name,
		} => {
			let Some(image) = image::RgbaImage::from_raw(width, height, data) else {
				tracing::error!("Failed to create image buffer for export");
				return None;
			};

			let mut encoded = Vec::new();
			let mut cursor = std::io::Cursor::new(&mut encoded);

			use graphite_editor::messages::frontend::utility_types::FileType;
			use image::buffer::ConvertBuffer;
			match file_type {
				FileType::Png => {
					let result = if transparent {
						image.write_to(&mut cursor, image::ImageFormat::Png)
						// TODO: find out why this is never triggered and fix it
					} else {
						let image: image::RgbImage = image.convert();
						image.write_to(&mut cursor, image::ImageFormat::Png)
					};
					if let Err(err) = result {
						tracing::error!("Failed to encode PNG: {err}");
						return None;
					}
				}
				FileType::Jpg => {
					let image: image::RgbImage = image.convert();
					let result = image.write_to(&mut cursor, image::ImageFormat::Jpeg);
					if let Err(err) = result {
						tracing::error!("Failed to encode JPG: {err}");
						return None;
					}
				}
				FileType::Svg => {
					tracing::error!("SVG cannot be exported from an image buffer");
					return None;
				}
			}
			let file_extension = match file_type {
				FileType::Png => "png",
				FileType::Jpg => "jpg",
				FileType::Svg => unreachable!(),
			};

			let default_filename = if name.ends_with(&format!(".{file_extension}")) {
				name
			} else {
				format!("{name}.{file_extension}")
			};

			dispatcher.respond(DesktopFrontendMessage::SaveFileDialog {
				title: "Export".to_string(),
				default_filename,
				default_folder: None,
				filters: Vec::new(),
				context: SaveFileDialogContext::File { content: encoded },
			});
		}
		FrontendMessage::TriggerVisitLink { url } => {
			dispatcher.respond(DesktopFrontendMessage::OpenUrl(url));
		}
		FrontendMessage::DragWindow => {
			dispatcher.respond(DesktopFrontendMessage::DragWindow);
		}
		FrontendMessage::CloseWindow => {
			dispatcher.respond(DesktopFrontendMessage::CloseWindow);
		}
		FrontendMessage::TriggerMinimizeWindow => {
			dispatcher.respond(DesktopFrontendMessage::MinimizeWindow);
		}
		FrontendMessage::TriggerMaximizeWindow => {
			dispatcher.respond(DesktopFrontendMessage::MaximizeWindow);
		}
		FrontendMessage::TriggerPersistenceWriteDocument { document_id, document, details } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceWriteDocument {
				id: document_id,
				document: Document {
					name: details.name,
					path: None,
					content: document,
					is_saved: details.is_saved,
				},
			});
		}
		FrontendMessage::TriggerPersistenceRemoveDocument { document_id } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceDeleteDocument { id: document_id });
		}
		FrontendMessage::UpdateActiveDocument { document_id } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceUpdateCurrentDocument { id: document_id });

			// Forward this to update the UI
			return Some(FrontendMessage::UpdateActiveDocument { document_id });
		}
		FrontendMessage::UpdateOpenDocumentsList { open_documents } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceUpdateDocumentsList {
				ids: open_documents.iter().map(|document| document.id).collect(),
			});

			// Forward this to update the UI
			return Some(FrontendMessage::UpdateOpenDocumentsList { open_documents });
		}
		FrontendMessage::TriggerLoadFirstAutoSaveDocument => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceLoadCurrentDocument);
		}
		FrontendMessage::TriggerLoadRestAutoSaveDocuments => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceLoadRemainingDocuments);
		}
		FrontendMessage::TriggerSavePreferences { preferences } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceWritePreferences { preferences });
		}
		FrontendMessage::TriggerLoadPreferences => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceLoadPreferences);
		}
		m => return Some(m),
	}
	None
}
