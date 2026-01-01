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
		FrontendMessage::TriggerVisitLink { url } => {
			dispatcher.respond(DesktopFrontendMessage::OpenUrl(url));
		}
		FrontendMessage::UpdateViewportPhysicalBounds { x, y, width, height } => {
			dispatcher.respond(DesktopFrontendMessage::UpdateViewportPhysicalBounds { x, y, width, height });
		}
		FrontendMessage::UpdateUIScale { scale } => {
			dispatcher.respond(DesktopFrontendMessage::UpdateUIScale { scale });
			return Some(FrontendMessage::UpdateUIScale { scale });
		}
		FrontendMessage::TriggerPersistenceWriteDocument { document_id, document, details } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceWriteDocument {
				id: document_id,
				document: Document {
					name: details.name,
					path: details.path,
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
		FrontendMessage::TriggerOpenLaunchDocuments => {
			dispatcher.respond(DesktopFrontendMessage::OpenLaunchDocuments);
		}
		FrontendMessage::TriggerSavePreferences { preferences } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceWritePreferences { preferences });
		}
		FrontendMessage::TriggerLoadPreferences => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceLoadPreferences);
		}
		#[cfg(target_os = "macos")]
		FrontendMessage::UpdateMenuBarLayout { diff } => {
			use graphite_editor::messages::tool::tool_messages::tool_prelude::{DiffUpdate, WidgetDiff};
			match diff.as_slice() {
				[
					WidgetDiff {
						widget_path,
						new_value: DiffUpdate::Layout(layout),
					},
				] if widget_path.is_empty() => {
					let entries = crate::utils::menu::convert_menu_bar_layout_to_menu_items(layout);
					dispatcher.respond(DesktopFrontendMessage::UpdateMenu { entries });
				}
				_ => {}
			}
		}
		FrontendMessage::TriggerClipboardRead => {
			dispatcher.respond(DesktopFrontendMessage::ClipboardRead);
		}
		FrontendMessage::TriggerClipboardWrite { content } => {
			dispatcher.respond(DesktopFrontendMessage::ClipboardWrite { content });
		}
		FrontendMessage::WindowClose => {
			dispatcher.respond(DesktopFrontendMessage::WindowClose);
		}
		FrontendMessage::WindowMinimize => {
			dispatcher.respond(DesktopFrontendMessage::WindowMinimize);
		}
		FrontendMessage::WindowMaximize => {
			dispatcher.respond(DesktopFrontendMessage::WindowMaximize);
		}
		FrontendMessage::WindowDrag => {
			dispatcher.respond(DesktopFrontendMessage::WindowDrag);
		}
		FrontendMessage::WindowHide => {
			dispatcher.respond(DesktopFrontendMessage::WindowHide);
		}
		FrontendMessage::WindowHideOthers => {
			dispatcher.respond(DesktopFrontendMessage::WindowHideOthers);
		}
		FrontendMessage::WindowShowAll => {
			dispatcher.respond(DesktopFrontendMessage::WindowShowAll);
		}
		m => return Some(m),
	}
	None
}
