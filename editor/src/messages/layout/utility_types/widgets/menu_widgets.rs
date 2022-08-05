use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::layout_widget::WidgetHolder;
use crate::messages::layout::utility_types::layout_widget::{Widget, WidgetCallback};
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

use super::input_widgets::InvisibleStandinInput;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MenuEntryGroups(pub Vec<Vec<MenuEntry>>);

impl MenuEntryGroups {
	pub fn empty() -> Self {
		Self(Vec::new())
	}

	pub fn fill_in_shortcut_actions_with_keys(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<Vec<Key>>) {
		let entries = self.0.iter_mut().flatten();

		for entry in entries {
			if let Some(action_keys) = &mut entry.shortcut {
				action_keys.to_keys(action_input_mapping);
			}

			// Recursively do this for the children also
			entry.children.fill_in_shortcut_actions_with_keys(action_input_mapping);
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuEntry {
	pub label: String,
	pub icon: Option<String>,
	pub children: MenuEntryGroups,
	pub action: WidgetHolder,
	pub shortcut: Option<ActionKeys>,
}

impl MenuEntry {
	pub fn create_action(callback: impl Fn(&()) -> Message + 'static) -> WidgetHolder {
		WidgetHolder::new(Widget::InvisibleStandinInput(InvisibleStandinInput {
			on_update: WidgetCallback::new(callback),
		}))
	}

	pub fn no_action() -> WidgetHolder {
		MenuEntry::create_action(|_| Message::NoOp)
	}
}

impl Default for MenuEntry {
	fn default() -> Self {
		Self {
			action: MenuEntry::create_action(|_| DialogMessage::RequestComingSoonDialog { issue: None }.into()),
			label: "".into(),
			icon: None,
			children: MenuEntryGroups::empty(),
			shortcut: None,
		}
	}
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuColumn {
	pub label: String,
	pub children: MenuEntryGroups,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuLayout {
	pub layout: Vec<MenuColumn>,
}

impl MenuLayout {
	pub fn new(layout: Vec<MenuColumn>) -> Self {
		Self { layout }
	}

	pub fn iter(&self) -> impl Iterator<Item = &WidgetHolder> + '_ {
		MenuLayoutIter {
			stack: self.layout.iter().flat_map(|column| column.children.0.iter()).flat_map(|group| group.iter()).collect(),
		}
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut WidgetHolder> + '_ {
		MenuLayoutIterMut {
			stack: self.layout.iter_mut().flat_map(|column| column.children.0.iter_mut()).flat_map(|group| group.iter_mut()).collect(),
		}
	}
}

#[derive(Debug, Default)]
pub struct MenuLayoutIter<'a> {
	pub stack: Vec<&'a MenuEntry>,
}

impl<'a> Iterator for MenuLayoutIter<'a> {
	type Item = &'a WidgetHolder;

	fn next(&mut self) -> Option<Self::Item> {
		match self.stack.pop() {
			Some(menu_entry) => {
				let more_entries = menu_entry.children.0.iter().flat_map(|entry| entry.iter());
				self.stack.extend(more_entries);

				Some(&menu_entry.action)
			}
			None => None,
		}
	}
}

pub struct MenuLayoutIterMut<'a> {
	pub stack: Vec<&'a mut MenuEntry>,
}

impl<'a> Iterator for MenuLayoutIterMut<'a> {
	type Item = &'a mut WidgetHolder;

	fn next(&mut self) -> Option<Self::Item> {
		match self.stack.pop() {
			Some(menu_entry) => {
				let more_entries = menu_entry.children.0.iter_mut().flat_map(|entry| entry.iter_mut());
				self.stack.extend(more_entries);

				Some(&mut menu_entry.action)
			}
			None => None,
		}
	}
}
