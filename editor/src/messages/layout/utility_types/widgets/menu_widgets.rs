use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

use super::input_widgets::InvisibleStandinInput;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, specta::Type)]
pub struct MenuBarEntryChildren(pub Vec<Vec<MenuBarEntry>>);

impl MenuBarEntryChildren {
	pub fn empty() -> Self {
		Self(Vec::new())
	}

	pub fn fill_in_shortcut_actions_with_keys(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub struct MenuBarEntry {
	pub label: String,
	pub icon: Option<String>,
	pub shortcut: Option<ActionKeys>,
	pub action: WidgetHolder,
	pub children: MenuBarEntryChildren,
	pub disabled: bool,
}

impl MenuBarEntry {
	pub fn new_root(label: String, disabled: bool, children: MenuBarEntryChildren) -> Self {
		Self {
			label,
			disabled,
			children,
			..Default::default()
		}
	}

	pub fn create_action(callback: impl Fn(&()) -> Message + 'static + Send + Sync) -> WidgetHolder {
		InvisibleStandinInput::new().on_update(callback).widget_holder()
	}

	pub fn no_action() -> WidgetHolder {
		MenuBarEntry::create_action(|_| Message::NoOp)
	}
}

impl Default for MenuBarEntry {
	fn default() -> Self {
		Self {
			label: "".into(),
			icon: None,
			shortcut: None,
			action: MenuBarEntry::no_action(),
			children: MenuBarEntryChildren::empty(),
			disabled: false,
		}
	}
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct MenuLayout {
	pub layout: Vec<MenuBarEntry>,
}

impl MenuLayout {
	pub fn new(layout: Vec<MenuBarEntry>) -> Self {
		Self { layout }
	}

	pub fn iter(&self) -> impl Iterator<Item = &WidgetHolder> + '_ {
		MenuLayoutIter { stack: self.layout.iter().collect() }
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut WidgetHolder> + '_ {
		MenuLayoutIterMut {
			stack: self.layout.iter_mut().collect(),
		}
	}
}

#[derive(Debug, Default)]
pub struct MenuLayoutIter<'a> {
	pub stack: Vec<&'a MenuBarEntry>,
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
	pub stack: Vec<&'a mut MenuBarEntry>,
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
