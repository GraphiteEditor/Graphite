use std::path::PathBuf;

use graphite_editor::messages::prelude::FrontendMessage;

use super::DesktopWrapperMessageExecutor;
use super::messages::{DesktopFrontendMessage, FileFilter, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn intercept_frontend_message(executor: &mut DesktopWrapperMessageExecutor, message: FrontendMessage) -> Option<FrontendMessage> {
	match message {
		FrontendMessage::RenderOverlays(overlay_context) => {
			executor.respond(DesktopFrontendMessage::UpdateOverlays(overlay_context.take_scene()));
		}
		FrontendMessage::TriggerOpenDocument => {
			executor.respond(DesktopFrontendMessage::OpenFileDialog {
				title: "Open Document".to_string(),
				filters: vec![FileFilter {
					name: "Graphite".to_string(),
					extensions: vec!["graphite".to_string()],
				}],
				context: OpenFileDialogContext::Document,
			});
		}
		FrontendMessage::TriggerImport => {
			executor.respond(DesktopFrontendMessage::OpenFileDialog {
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
				executor.respond(DesktopFrontendMessage::WriteFile { path, content });
			} else {
				executor.respond(DesktopFrontendMessage::SaveFileDialog {
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
			executor.respond(DesktopFrontendMessage::SaveFileDialog {
				title: "Save File".to_string(),
				default_filename: name,
				default_folder: None,
				filters: Vec::new(),
				context: SaveFileDialogContext::Export { content },
			});
		}
		FrontendMessage::TriggerVisitLink { url } => {
			executor.respond(DesktopFrontendMessage::OpenUrl(url));
		}
		m => return Some(m),
	}
	None
}
