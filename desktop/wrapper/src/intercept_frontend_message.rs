use std::path::PathBuf;

use graphite_editor::messages::input_mapper::utility_types::input_keyboard::{LayoutKey, LayoutKeysGroup};
use graphite_editor::messages::input_mapper::utility_types::misc::ActionKeys;
use graphite_editor::messages::layout::utility_types::widgets::menu_widgets::MenuBarEntry;
use graphite_editor::messages::prelude::FrontendMessage;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{Code, DesktopFrontendMessage, Document, FileFilter, MenuItem, Modifiers, OpenFileDialogContext, SaveFileDialogContext, Shortcut};

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
		FrontendMessage::UpdateMenuBarLayout { layout_target, layout } => {
			fn shortcut_from_layout_keys(layout_keys: &Vec<LayoutKey>) -> Option<Shortcut> {
				let mut key: Option<Code> = None;
				let mut modifiers = Modifiers::default();
				for layout_key in layout_keys {
					use graphite_editor::messages::input_mapper::utility_types::input_keyboard::Key;
					match layout_key.key {
						Key::Shift => modifiers |= Modifiers::SHIFT,
						Key::Control => modifiers |= Modifiers::CONTROL,
						Key::Alt => modifiers |= Modifiers::ALT,
						Key::Meta => modifiers |= Modifiers::META,
						Key::Command => modifiers |= Modifiers::ALT,
						Key::Accel => modifiers |= Modifiers::META,
						Key::Digit0 => key = Some(Code::Digit0),
						Key::Digit1 => key = Some(Code::Digit1),
						Key::Digit2 => key = Some(Code::Digit2),
						Key::Digit3 => key = Some(Code::Digit3),
						Key::Digit4 => key = Some(Code::Digit4),
						Key::Digit5 => key = Some(Code::Digit5),
						Key::Digit6 => key = Some(Code::Digit6),
						Key::Digit7 => key = Some(Code::Digit7),
						Key::Digit8 => key = Some(Code::Digit8),
						Key::Digit9 => key = Some(Code::Digit9),
						Key::KeyA => key = Some(Code::KeyA),
						Key::KeyB => key = Some(Code::KeyB),
						Key::KeyC => key = Some(Code::KeyC),
						Key::KeyD => key = Some(Code::KeyD),
						Key::KeyE => key = Some(Code::KeyE),
						Key::KeyF => key = Some(Code::KeyF),
						Key::KeyG => key = Some(Code::KeyG),
						Key::KeyH => key = Some(Code::KeyH),
						Key::KeyI => key = Some(Code::KeyI),
						Key::KeyJ => key = Some(Code::KeyJ),
						Key::KeyK => key = Some(Code::KeyK),
						Key::KeyL => key = Some(Code::KeyL),
						Key::KeyM => key = Some(Code::KeyM),
						Key::KeyN => key = Some(Code::KeyN),
						Key::KeyO => key = Some(Code::KeyO),
						Key::KeyP => key = Some(Code::KeyP),
						Key::KeyQ => key = Some(Code::KeyQ),
						Key::KeyR => key = Some(Code::KeyR),
						Key::KeyS => key = Some(Code::KeyS),
						Key::KeyT => key = Some(Code::KeyT),
						Key::KeyU => key = Some(Code::KeyU),
						Key::KeyV => key = Some(Code::KeyV),
						Key::KeyW => key = Some(Code::KeyW),
						Key::KeyX => key = Some(Code::KeyX),
						Key::KeyY => key = Some(Code::KeyY),
						Key::KeyZ => key = Some(Code::KeyZ),
						Key::Backquote => key = Some(Code::Backquote),
						Key::Backslash => key = Some(Code::Backslash),
						Key::BracketLeft => key = Some(Code::BracketLeft),
						Key::BracketRight => key = Some(Code::BracketRight),
						Key::Comma => key = Some(Code::Comma),
						Key::Equal => key = Some(Code::Equal),
						Key::Minus => key = Some(Code::Minus),
						Key::Period => key = Some(Code::Period),
						Key::Quote => key = Some(Code::Quote),
						Key::Semicolon => key = Some(Code::Semicolon),
						Key::Slash => key = Some(Code::Slash),
						Key::Backspace => key = Some(Code::Backspace),
						Key::CapsLock => key = Some(Code::CapsLock),
						Key::ContextMenu => key = Some(Code::ContextMenu),
						Key::Enter => key = Some(Code::Enter),
						Key::Space => key = Some(Code::Space),
						Key::Tab => key = Some(Code::Tab),
						Key::Delete => key = Some(Code::Delete),
						Key::End => key = Some(Code::End),
						Key::Help => key = Some(Code::Help),
						Key::Home => key = Some(Code::Home),
						Key::Insert => key = Some(Code::Insert),
						Key::PageDown => key = Some(Code::PageDown),
						Key::PageUp => key = Some(Code::PageUp),
						Key::ArrowDown => key = Some(Code::ArrowDown),
						Key::ArrowLeft => key = Some(Code::ArrowLeft),
						Key::ArrowRight => key = Some(Code::ArrowRight),
						Key::ArrowUp => key = Some(Code::ArrowUp),
						Key::NumLock => key = Some(Code::NumLock),
						Key::NumpadAdd => key = Some(Code::NumpadAdd),
						Key::NumpadHash => key = Some(Code::NumpadHash),
						Key::NumpadMultiply => key = Some(Code::NumpadMultiply),
						Key::NumpadParenLeft => key = Some(Code::NumpadParenLeft),
						Key::NumpadParenRight => key = Some(Code::NumpadParenRight),
						Key::Escape => key = Some(Code::Escape),
						Key::F1 => key = Some(Code::F1),
						Key::F2 => key = Some(Code::F2),
						Key::F3 => key = Some(Code::F3),
						Key::F4 => key = Some(Code::F4),
						Key::F5 => key = Some(Code::F5),
						Key::F6 => key = Some(Code::F6),
						Key::F7 => key = Some(Code::F7),
						Key::F8 => key = Some(Code::F8),
						Key::F9 => key = Some(Code::F9),
						Key::F10 => key = Some(Code::F10),
						Key::F11 => key = Some(Code::F11),
						Key::F12 => key = Some(Code::F12),
						Key::F13 => key = Some(Code::F13),
						Key::F14 => key = Some(Code::F14),
						Key::F15 => key = Some(Code::F15),
						Key::F16 => key = Some(Code::F16),
						Key::F17 => key = Some(Code::F17),
						Key::F18 => key = Some(Code::F18),
						Key::F19 => key = Some(Code::F19),
						Key::F20 => key = Some(Code::F20),
						Key::F21 => key = Some(Code::F21),
						Key::F22 => key = Some(Code::F22),
						Key::F23 => key = Some(Code::F23),
						Key::F24 => key = Some(Code::F24),
						Key::Fn => key = Some(Code::Fn),
						Key::FnLock => key = Some(Code::FnLock),
						Key::PrintScreen => key = Some(Code::PrintScreen),
						Key::ScrollLock => key = Some(Code::ScrollLock),
						Key::Pause => key = Some(Code::Pause),
						Key::Unidentified => key = Some(Code::Unidentified),
						_ => key = None,
					}
				}
				if let Some(key) = key { Some(Shortcut { key, modifiers }) } else { None }
			}

			fn create_menu_item(
				MenuBarEntry {
					label,
					icon,
					shortcut,
					action,
					children,
					disabled,
				}: &MenuBarEntry,
			) -> Option<MenuItem> {
				let id = action.widget_id.0;
				let text = if label.is_empty() {
					return None;
				} else {
					label.clone()
				};
				let enabled = !*disabled;

				if !children.0.is_empty() {
					let items = items_from_children(&children.0);
					return Some(MenuItem::SubMenu { id, text, enabled, items });
				}

				let shortcut = match shortcut {
					Some(ActionKeys::Keys(LayoutKeysGroup(keys))) => {
						if let Some(shortcut) = shortcut_from_layout_keys(&keys) {
							Some(shortcut)
						} else {
							None
						}
					}
					_ => None,
				};

				// TODO: Find a better way to determine if this is a checkbox
				match icon.as_deref() {
					Some("CheckboxChecked") => {
						return Some(MenuItem::Checkbox {
							id,
							text,
							enabled,
							shortcut,
							checked: true,
						});
					}
					Some("CheckboxUnchecked") => {
						return Some(MenuItem::Checkbox {
							id,
							text,
							enabled,
							shortcut,
							checked: false,
						});
					}
					_ => {}
				}

				Some(MenuItem::Action { id, text, shortcut, enabled })
			}

			fn items_from_children(children: &Vec<Vec<MenuBarEntry>>) -> Vec<MenuItem> {
				let mut items = Vec::new();
				for (i, section) in children.iter().enumerate() {
					for entry in section.iter() {
						if let Some(item) = create_menu_item(entry) {
							items.push(item);
						}
					}
					if i != children.len() - 1 {
						items.push(MenuItem::Separator);
					}
				}
				items
			}

			let entries: Vec<MenuItem> = layout.iter().filter_map(|entry| create_menu_item(entry)).collect();

			dispatcher.respond(DesktopFrontendMessage::UpdateMenu { entries });

			return Some(FrontendMessage::UpdateMenuBarLayout { layout, layout_target });
		}
		m => return Some(m),
	}
	None
}
