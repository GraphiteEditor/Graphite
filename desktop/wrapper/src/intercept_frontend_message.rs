use graphite_editor::messages::frontend::utility_types::ExportAnimationFrame;
#[cfg(target_os = "macos")]
use graphite_editor::messages::layout::utility_types::layout_widget::LayoutTarget;
use graphite_editor::messages::prelude::FrontendMessage;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, FileFilter, OpenFileDialogContext, SaveFileDialogContext};

pub(super) fn intercept_frontend_message(dispatcher: &mut DesktopWrapperMessageDispatcher, message: FrontendMessage) -> Option<FrontendMessage> {
	match message {
		FrontendMessage::RenderOverlays { context } => {
			dispatcher.respond(DesktopFrontendMessage::UpdateOverlays(context.take_scene()));
		}
		FrontendMessage::TriggerOpen => {
			dispatcher.respond(DesktopFrontendMessage::OpenFileDialog {
				title: "Open Document".to_string(),
				filters: vec![],
				multiple: true,
				context: OpenFileDialogContext::Open,
			});
		}
		FrontendMessage::TriggerImport => {
			dispatcher.respond(DesktopFrontendMessage::OpenFileDialog {
				title: "Import File".to_string(),
				filters: vec![],
				multiple: false,
				context: OpenFileDialogContext::Import,
			});
		}
		FrontendMessage::TriggerSaveDocument {
			document_id,
			name,
			path,
			folder,
			content,
		} => {
			let content = content.into_vec();
			if let Some(path) = path {
				dispatcher.respond(DesktopFrontendMessage::WriteFile { path, content });
			} else {
				dispatcher.respond(DesktopFrontendMessage::SaveFileDialog {
					title: "Save Document".to_string(),
					default_filename: name,
					default_folder: folder,
					filters: vec![FileFilter {
						name: "Graphite".to_string(),
						extensions: vec!["graphite".to_string()],
					}],
					context: SaveFileDialogContext::Document { document_id, content },
				});
			}
		}
		FrontendMessage::TriggerSaveFile { name, folder, content } => {
			let content = content.into_vec();
			dispatcher.respond(DesktopFrontendMessage::SaveFileDialog {
				title: "Save File".to_string(),
				default_filename: name,
				default_folder: folder,
				filters: Vec::new(),
				context: SaveFileDialogContext::File { content },
			});
		}
		FrontendMessage::TriggerExportAnimation {
			name,
			extension,
			mime,
			size,
			folder,
			frames,
		} => {
			// Materialize each frame to bytes; SVG strings are encoded as UTF-8.
			// Raster-needs-canvas-rasterize frames can't be encoded here without a Rust SVG rasterizer,
			// so fall through to the frontend zip path in that case.
			// TODO: This fallback is inconsistent with the desktop folder-save flow for the rest of the animation
			// export — desktop users will see a .zip download instead of a folder. Once SVG rasterization moves to
			// Rust (resvg), this fallback can be removed and all frame paths can save into the chosen folder.
			let mut needs_frontend_rasterization = false;
			let mut materialized = Vec::with_capacity(frames.len());
			// Dynamic zero-pad width so files keep sorting in playback order beyond 9,999 frames.
			let pad_width = frames.len().to_string().len().max(4);
			let safe_base = graphite_editor::messages::frontend::utility_types::sanitize_filename_component(&name);
			for (index, frame) in frames.iter().enumerate() {
				let filename = format!("{safe_base}_{:0pad$}.{extension}", index + 1, pad = pad_width);
				let bytes = match frame {
					ExportAnimationFrame::Svg(svg) if extension == "svg" => svg.as_bytes().to_vec(),
					ExportAnimationFrame::Bytes(bytes) => bytes.to_vec(),
					ExportAnimationFrame::Svg(_) => {
						needs_frontend_rasterization = true;
						break;
					}
				};
				materialized.push((filename, bytes));
			}

			if needs_frontend_rasterization {
				return Some(FrontendMessage::TriggerExportAnimation {
					name,
					extension,
					mime,
					size,
					folder,
					frames,
				});
			}

			// The dialog name is the folder the frames go into (analogous to the .zip on web).
			dispatcher.respond(DesktopFrontendMessage::SaveFileDialog {
				title: "Save Animation Frames Folder As".to_string(),
				default_filename: safe_base,
				default_folder: folder,
				filters: Vec::new(),
				context: SaveFileDialogContext::MultipleFiles {
					files: materialized,
					expected_extension: extension,
				},
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
		FrontendMessage::TriggerPersistenceReadState => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceReadState);
		}
		FrontendMessage::TriggerPersistenceWriteState { state } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceWriteState { state });
		}
		FrontendMessage::TriggerPersistenceReadDocument { document_id } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceReadDocument { id: document_id });
		}
		FrontendMessage::TriggerPersistenceDeleteDocument { document_id } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceDeleteDocument { id: document_id });
		}
		FrontendMessage::TriggerPersistenceWriteDocument { document_id, document } => {
			dispatcher.respond(DesktopFrontendMessage::PersistenceWriteDocument {
				id: document_id,
				document_serialized_content: document,
			});
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
		FrontendMessage::UpdateLayout {
			layout_target: LayoutTarget::MenuBar,
			diff,
		} => {
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
		FrontendMessage::WindowPointerLock => {
			dispatcher.respond(DesktopFrontendMessage::PointerLock);
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
		FrontendMessage::WindowFullscreen => {
			dispatcher.respond(DesktopFrontendMessage::WindowFullscreen);
		}
		FrontendMessage::WindowDrag => {
			dispatcher.respond(DesktopFrontendMessage::WindowDrag);
		}
		FrontendMessage::WindowFocus => {
			dispatcher.respond(DesktopFrontendMessage::WindowFocus);
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
		FrontendMessage::WindowRestart => {
			dispatcher.respond(DesktopFrontendMessage::Restart);
		}
		FrontendMessage::TriggerDisplayThirdPartyLicensesDialog => {
			dispatcher.respond(DesktopFrontendMessage::LoadThirdPartyLicenses);
		}
		m => return Some(m),
	}
	None
}
