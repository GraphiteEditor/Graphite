use std::path::PathBuf;

use graphite_editor::messages::prelude::FrontendMessage;

use crate::editor_api::messages::{FileFilter, NativeMessage, OpenFileDialogContext, SaveFileDialogContext};

use super::EditorMessageExecutor;

pub(super) fn intercept_frontend_message(executor: &mut EditorMessageExecutor, message: FrontendMessage) -> Option<FrontendMessage> {
	match message {
		FrontendMessage::RenderOverlays(overlay_context) => {
			executor.respond(NativeMessage::UpdateOverlays(overlay_context.take_scene()));
		}
		FrontendMessage::TriggerOpenDocument => {
			executor.respond(NativeMessage::OpenFileDialog {
				title: "Open Document".to_string(),
				filters: vec![FileFilter {
					name: "Graphite".to_string(),
					extensions: vec!["graphite".to_string()],
				}],
				context: OpenFileDialogContext::Document,
			});
		}
		FrontendMessage::TriggerImport => {
			executor.respond(NativeMessage::OpenFileDialog {
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
				executor.respond(NativeMessage::WriteFile { path, content });
			} else {
				executor.respond(NativeMessage::SaveFileDialog {
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
			executor.respond(NativeMessage::SaveFileDialog {
				title: "Save File".to_string(),
				default_filename: name,
				default_folder: None,
				filters: Vec::new(),
				context: SaveFileDialogContext::Export { content },
			});
		}
		FrontendMessage::TriggerVisitLink { url } => {
			executor.respond(NativeMessage::OpenUrl(url));
		}
		m => return Some(m),
	}
	None
}
