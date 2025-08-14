use std::path::PathBuf;

use graphite_editor::messages::prelude::FrontendMessage;

use crate::editor_api::messages::{FileFilter, NativeMessage, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn intercept_frontend_message(message: FrontendMessage, responses: &mut Vec<NativeMessage>) -> Option<FrontendMessage> {
	match message {
		FrontendMessage::RenderOverlays(overlay_context) => {
			responses.push(NativeMessage::UpdateOverlays(overlay_context.take_scene()));
		}
		FrontendMessage::TriggerOpenDocument => {
			responses.push(NativeMessage::OpenFileDialog {
				title: "Open Document".to_string(),
				filters: vec![FileFilter {
					name: "Graphite".to_string(),
					extensions: vec!["graphite".to_string()],
				}],
				context: OpenFileDialogContext::Document,
			});
		}
		FrontendMessage::TriggerImport => {
			responses.push(NativeMessage::OpenFileDialog {
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
			responses.push(NativeMessage::SaveFileDialog {
				title: "Save Document".to_string(),
				default_filename: name,
				default_folder: path.and_then(|p| p.parent().map(PathBuf::from)),
				content,
				context: SaveFileDialogContext::Document { document_id },
			});
		}
		FrontendMessage::TriggerSaveFile { name, content } => {
			responses.push(NativeMessage::SaveFileDialog {
				title: "Save File".to_string(),
				default_filename: name,
				default_folder: None,
				content,
				context: SaveFileDialogContext::Export,
			});
		}
		FrontendMessage::TriggerVisitLink { url } => {
			responses.push(NativeMessage::OpenUrl(url));
		}
		m => return Some(m),
	}
	None
}
