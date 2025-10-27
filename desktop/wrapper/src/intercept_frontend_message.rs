use std::path::PathBuf;

use graphite_editor::messages::input_mapper::utility_types::input_keyboard::{Key, LayoutKey, LayoutKeysGroup};
use graphite_editor::messages::input_mapper::utility_types::misc::ActionKeys;
use graphite_editor::messages::layout::utility_types::widgets::menu_widgets::MenuBarEntry;
use graphite_editor::messages::prelude::FrontendMessage;

use super::DesktopWrapperMessageDispatcher;
use super::messages::{DesktopFrontendMessage, Document, FileFilter, KeyCode, MenuItem, Modifiers, OpenFileDialogContext, SaveFileDialogContext, Shortcut};

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
			let entries = convert_menu_bar_entries_to_menu_items(&layout);
			dispatcher.respond(DesktopFrontendMessage::UpdateMenu { entries });

			return Some(FrontendMessage::UpdateMenuBarLayout { layout, layout_target });
		}
		m => return Some(m),
	}
	None
}

fn convert_menu_bar_entries_to_menu_items(layout: &Vec<MenuBarEntry>) -> Vec<MenuItem> {
	layout.iter().filter_map(|entry| convert_menu_bar_entry_to_menu_item(entry)).collect()
}

fn convert_menu_bar_entry_to_menu_item(
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
		let items = convert_menu_bar_entry_children_to_menu_items(&children.0);
		return Some(MenuItem::SubMenu { id, text, enabled, items });
	}

	let shortcut = match shortcut {
		Some(ActionKeys::Keys(LayoutKeysGroup(keys))) => {
			if let Some(shortcut) = convert_layout_keys_to_shortcut(&keys) {
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

fn convert_menu_bar_entry_children_to_menu_items(children: &Vec<Vec<MenuBarEntry>>) -> Vec<MenuItem> {
	let mut items = Vec::new();
	for (i, section) in children.iter().enumerate() {
		for entry in section.iter() {
			if let Some(item) = convert_menu_bar_entry_to_menu_item(entry) {
				items.push(item);
			}
		}
		if i != children.len() - 1 {
			items.push(MenuItem::Separator);
		}
	}
	items
}

fn convert_layout_keys_to_shortcut(layout_keys: &Vec<LayoutKey>) -> Option<Shortcut> {
	let mut key: Option<KeyCode> = None;
	let mut modifiers = Modifiers::default();
	for layout_key in layout_keys {
		match layout_key.key {
			Key::Shift => modifiers |= Modifiers::SHIFT,
			Key::Control => modifiers |= Modifiers::CONTROL,
			Key::Alt => modifiers |= Modifiers::ALT,
			Key::Meta => modifiers |= Modifiers::META,
			Key::Command => modifiers |= Modifiers::ALT,
			Key::Accel => modifiers |= Modifiers::META,
			Key::Digit0 => key = Some(KeyCode::Digit0),
			Key::Digit1 => key = Some(KeyCode::Digit1),
			Key::Digit2 => key = Some(KeyCode::Digit2),
			Key::Digit3 => key = Some(KeyCode::Digit3),
			Key::Digit4 => key = Some(KeyCode::Digit4),
			Key::Digit5 => key = Some(KeyCode::Digit5),
			Key::Digit6 => key = Some(KeyCode::Digit6),
			Key::Digit7 => key = Some(KeyCode::Digit7),
			Key::Digit8 => key = Some(KeyCode::Digit8),
			Key::Digit9 => key = Some(KeyCode::Digit9),
			Key::KeyA => key = Some(KeyCode::KeyA),
			Key::KeyB => key = Some(KeyCode::KeyB),
			Key::KeyC => key = Some(KeyCode::KeyC),
			Key::KeyD => key = Some(KeyCode::KeyD),
			Key::KeyE => key = Some(KeyCode::KeyE),
			Key::KeyF => key = Some(KeyCode::KeyF),
			Key::KeyG => key = Some(KeyCode::KeyG),
			Key::KeyH => key = Some(KeyCode::KeyH),
			Key::KeyI => key = Some(KeyCode::KeyI),
			Key::KeyJ => key = Some(KeyCode::KeyJ),
			Key::KeyK => key = Some(KeyCode::KeyK),
			Key::KeyL => key = Some(KeyCode::KeyL),
			Key::KeyM => key = Some(KeyCode::KeyM),
			Key::KeyN => key = Some(KeyCode::KeyN),
			Key::KeyO => key = Some(KeyCode::KeyO),
			Key::KeyP => key = Some(KeyCode::KeyP),
			Key::KeyQ => key = Some(KeyCode::KeyQ),
			Key::KeyR => key = Some(KeyCode::KeyR),
			Key::KeyS => key = Some(KeyCode::KeyS),
			Key::KeyT => key = Some(KeyCode::KeyT),
			Key::KeyU => key = Some(KeyCode::KeyU),
			Key::KeyV => key = Some(KeyCode::KeyV),
			Key::KeyW => key = Some(KeyCode::KeyW),
			Key::KeyX => key = Some(KeyCode::KeyX),
			Key::KeyY => key = Some(KeyCode::KeyY),
			Key::KeyZ => key = Some(KeyCode::KeyZ),
			Key::Backquote => key = Some(KeyCode::Backquote),
			Key::Backslash => key = Some(KeyCode::Backslash),
			Key::BracketLeft => key = Some(KeyCode::BracketLeft),
			Key::BracketRight => key = Some(KeyCode::BracketRight),
			Key::Comma => key = Some(KeyCode::Comma),
			Key::Equal => key = Some(KeyCode::Equal),
			Key::Minus => key = Some(KeyCode::Minus),
			Key::Period => key = Some(KeyCode::Period),
			Key::Quote => key = Some(KeyCode::Quote),
			Key::Semicolon => key = Some(KeyCode::Semicolon),
			Key::Slash => key = Some(KeyCode::Slash),
			Key::Backspace => key = Some(KeyCode::Backspace),
			Key::CapsLock => key = Some(KeyCode::CapsLock),
			Key::ContextMenu => key = Some(KeyCode::ContextMenu),
			Key::Enter => key = Some(KeyCode::Enter),
			Key::Space => key = Some(KeyCode::Space),
			Key::Tab => key = Some(KeyCode::Tab),
			Key::Delete => key = Some(KeyCode::Delete),
			Key::End => key = Some(KeyCode::End),
			Key::Help => key = Some(KeyCode::Help),
			Key::Home => key = Some(KeyCode::Home),
			Key::Insert => key = Some(KeyCode::Insert),
			Key::PageDown => key = Some(KeyCode::PageDown),
			Key::PageUp => key = Some(KeyCode::PageUp),
			Key::ArrowDown => key = Some(KeyCode::ArrowDown),
			Key::ArrowLeft => key = Some(KeyCode::ArrowLeft),
			Key::ArrowRight => key = Some(KeyCode::ArrowRight),
			Key::ArrowUp => key = Some(KeyCode::ArrowUp),
			Key::NumLock => key = Some(KeyCode::NumLock),
			Key::NumpadAdd => key = Some(KeyCode::NumpadAdd),
			Key::NumpadHash => key = Some(KeyCode::NumpadHash),
			Key::NumpadMultiply => key = Some(KeyCode::NumpadMultiply),
			Key::NumpadParenLeft => key = Some(KeyCode::NumpadParenLeft),
			Key::NumpadParenRight => key = Some(KeyCode::NumpadParenRight),
			Key::Escape => key = Some(KeyCode::Escape),
			Key::F1 => key = Some(KeyCode::F1),
			Key::F2 => key = Some(KeyCode::F2),
			Key::F3 => key = Some(KeyCode::F3),
			Key::F4 => key = Some(KeyCode::F4),
			Key::F5 => key = Some(KeyCode::F5),
			Key::F6 => key = Some(KeyCode::F6),
			Key::F7 => key = Some(KeyCode::F7),
			Key::F8 => key = Some(KeyCode::F8),
			Key::F9 => key = Some(KeyCode::F9),
			Key::F10 => key = Some(KeyCode::F10),
			Key::F11 => key = Some(KeyCode::F11),
			Key::F12 => key = Some(KeyCode::F12),
			Key::F13 => key = Some(KeyCode::F13),
			Key::F14 => key = Some(KeyCode::F14),
			Key::F15 => key = Some(KeyCode::F15),
			Key::F16 => key = Some(KeyCode::F16),
			Key::F17 => key = Some(KeyCode::F17),
			Key::F18 => key = Some(KeyCode::F18),
			Key::F19 => key = Some(KeyCode::F19),
			Key::F20 => key = Some(KeyCode::F20),
			Key::F21 => key = Some(KeyCode::F21),
			Key::F22 => key = Some(KeyCode::F22),
			Key::F23 => key = Some(KeyCode::F23),
			Key::F24 => key = Some(KeyCode::F24),
			Key::Fn => key = Some(KeyCode::Fn),
			Key::FnLock => key = Some(KeyCode::FnLock),
			Key::PrintScreen => key = Some(KeyCode::PrintScreen),
			Key::ScrollLock => key = Some(KeyCode::ScrollLock),
			Key::Pause => key = Some(KeyCode::Pause),
			Key::Unidentified => key = Some(KeyCode::Unidentified),
			_ => key = None,
		}
	}
	if let Some(key) = key { Some(Shortcut { key, modifiers }) } else { None }
}
