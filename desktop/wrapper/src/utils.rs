#[cfg(target_os = "macos")]
pub(crate) mod menu {
	use base64::engine::Engine;
	use base64::engine::general_purpose::STANDARD as BASE64;

	use graphite_editor::messages::input_mapper::utility_types::input_keyboard::{Key, LabeledKeyOrMouseMotion, LabeledShortcut};
	use graphite_editor::messages::input_mapper::utility_types::misc::ActionShortcut;
	use graphite_editor::messages::layout::LayoutMessage;
	use graphite_editor::messages::tool::tool_messages::tool_prelude::{Layout, LayoutGroup, LayoutTarget, MenuListEntry, Widget, WidgetId};

	use crate::messages::{EditorMessage, KeyCode, MenuItem, Modifiers, Shortcut};

	pub(crate) fn convert_menu_bar_layout_to_menu_items(Layout(layout): &Layout) -> Vec<MenuItem> {
		let layout_group = match layout.as_slice() {
			[layout_group] => layout_group,
			_ => panic!("Menu bar layout is supposed to have exactly one layout group"),
		};
		let LayoutGroup::Row { widgets } = layout_group else {
			panic!("Menu bar layout group is supposed to be a row");
		};
		widgets
			.into_iter()
			.map(|widget| {
				let text_button = match widget.widget.as_ref() {
					Widget::TextButton(text_button) => text_button,
					_ => panic!("Menu bar layout top-level widgets are supposed to be text buttons"),
				};

				MenuItem::SubMenu {
					id: widget.widget_id.to_string(),
					text: text_button.label.clone(),
					enabled: !text_button.disabled,
					items: convert_menu_bar_entry_children_to_menu_items(&text_button.menu_list_children, widget.widget_id.0, Vec::new()),
				}
			})
			.collect::<Vec<MenuItem>>()
	}

	pub(crate) fn parse_item_path(id: String) -> Option<EditorMessage> {
		let mut id_parts = id.split(':');
		let widget_id = id_parts.next()?.parse::<u64>().ok()?;

		let value = id_parts
			.map(|part| {
				let bytes = BASE64.decode(part).ok()?;
				String::from_utf8(bytes).ok()
			})
			.collect::<Option<Vec<String>>>()?;
		let value = serde_json::to_value(value).ok()?;

		Some(
			LayoutMessage::WidgetValueUpdate {
				layout_target: LayoutTarget::MenuBar,
				widget_id: WidgetId(widget_id),
				value,
			}
			.into(),
		)
	}

	fn item_path_to_string(widget_id: u64, path: Vec<String>) -> String {
		let path = path.into_iter().map(|element| BASE64.encode(element)).collect::<Vec<_>>().join(":");
		format!("{widget_id}:{path}")
	}

	fn convert_menu_bar_layout_to_menu_item(entry: &MenuListEntry, root_widget_id: u64, mut path: Vec<String>) -> MenuItem {
		let MenuListEntry {
			value,
			label,
			icon,
			disabled,
			tooltip_shortcut,
			children,
			..
		}: &MenuListEntry = entry;
		path.push(value.clone());
		let id = item_path_to_string(root_widget_id, path.clone());
		let text = label.clone();
		let enabled = !*disabled;

		if !children.is_empty() {
			let items = convert_menu_bar_entry_children_to_menu_items(&children, root_widget_id, path.clone());
			return MenuItem::SubMenu { id, text, enabled, items };
		}

		let shortcut = match tooltip_shortcut {
			Some(ActionShortcut::Shortcut(LabeledShortcut(shortcut))) => convert_labeled_keys_to_shortcut(shortcut),
			_ => None,
		};

		match icon.as_str() {
			"CheckboxChecked" => {
				return MenuItem::Checkbox {
					id,
					text,
					enabled,
					shortcut,
					checked: true,
				};
			}
			"CheckboxUnchecked" => {
				return MenuItem::Checkbox {
					id,
					text,
					enabled,
					shortcut,
					checked: false,
				};
			}
			_ => {}
		}

		MenuItem::Action { id, text, shortcut, enabled }
	}

	fn convert_menu_bar_entry_children_to_menu_items(children: &[Vec<MenuListEntry>], root_widget_id: u64, path: Vec<String>) -> Vec<MenuItem> {
		let mut items = Vec::new();
		for (i, section) in children.iter().enumerate() {
			for entry in section.iter() {
				items.push(convert_menu_bar_layout_to_menu_item(entry, root_widget_id, path.clone()));
			}
			if i != children.len() - 1 {
				items.push(MenuItem::Separator);
			}
		}
		items
	}

	fn convert_labeled_keys_to_shortcut(labeled_keys: &Vec<LabeledKeyOrMouseMotion>) -> Option<Shortcut> {
		let mut key: Option<KeyCode> = None;
		let mut modifiers = Modifiers::default();
		for labeled_key in labeled_keys {
			let LabeledKeyOrMouseMotion::Key(labeled_key) = labeled_key else {
				// Return None for shortcuts that include mouse motion because we can't show them in native menu
				return None;
			};
			match labeled_key.key() {
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
				Key::FakeKeyPlus => key = Some(KeyCode::Equal),
				_ => key = None,
			}
		}
		key.map(|key| Shortcut { key, modifiers })
	}
}
