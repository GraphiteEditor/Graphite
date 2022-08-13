use super::widgets::button_widgets::*;
use super::widgets::input_widgets::*;
use super::widgets::label_widgets::*;
use super::widgets::menu_widgets::MenuLayout;
use crate::application::generate_uuid;
use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub trait PropertyHolder {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}

	fn register_properties(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget) {
		responses.push_back(
			LayoutMessage::SendLayout {
				layout: self.properties(),
				layout_target,
			}
			.into(),
		)
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Layout {
	WidgetLayout(WidgetLayout),
	MenuLayout(MenuLayout),
}

impl Layout {
	pub fn unwrap_widget_layout(self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) -> WidgetLayout {
		if let Layout::WidgetLayout(mut widget_layout) = self {
			// Function used multiple times later in this code block to convert `ActionKeys::Action` to `ActionKeys::Keys` and append its shortcut to the tooltip
			let apply_shortcut_to_tooltip = |tooltip_shortcut: &mut ActionKeys, tooltip: &mut String| {
				tooltip_shortcut.to_keys(action_input_mapping);

				if let ActionKeys::Keys(keys) = tooltip_shortcut {
					let shortcut_text = keys.to_string();

					if !shortcut_text.is_empty() {
						if !tooltip.is_empty() {
							tooltip.push(' ');
						}
						tooltip.push('(');
						tooltip.push_str(&shortcut_text);
						tooltip.push(')');
					}
				}
			};

			// Go through each widget to convert `ActionKeys::Action` to `ActionKeys::Keys` and append the key combination to the widget tooltip
			for widget_holder in &mut widget_layout.iter_mut() {
				// Handle all the widgets that have tooltips
				let mut tooltip_shortcut = match &mut widget_holder.widget {
					Widget::CheckboxInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
					Widget::ColorInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
					Widget::IconButton(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
					Widget::OptionalInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
					_ => None,
				};
				if let Some((tooltip, Some(tooltip_shortcut))) = &mut tooltip_shortcut {
					apply_shortcut_to_tooltip(tooltip_shortcut, tooltip);
				}

				// Handle RadioInput separately because its tooltips are children of the widget
				if let Widget::RadioInput(radio_input) = &mut widget_holder.widget {
					for radio_entry_data in &mut radio_input.entries {
						if let RadioEntryData {
							tooltip,
							tooltip_shortcut: Some(tooltip_shortcut),
							..
						} = radio_entry_data
						{
							apply_shortcut_to_tooltip(tooltip_shortcut, tooltip);
						}
					}
				}
			}

			widget_layout
		} else {
			panic!("Tried to unwrap layout as WidgetLayout. Got {:?}", self)
		}
	}

	pub fn unwrap_menu_layout(self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) -> MenuLayout {
		if let Layout::MenuLayout(mut menu_layout) = self {
			for menu_column in &mut menu_layout.layout {
				menu_column.children.fill_in_shortcut_actions_with_keys(action_input_mapping);
			}

			menu_layout
		} else {
			panic!("Tried to unwrap layout as MenuLayout. Got {:?}", self)
		}
	}

	pub fn iter(&self) -> Box<dyn Iterator<Item = &WidgetHolder> + '_> {
		match self {
			Layout::MenuLayout(menu_layout) => Box::new(menu_layout.iter()),
			Layout::WidgetLayout(widget_layout) => Box::new(widget_layout.iter()),
		}
	}

	pub fn iter_mut(&mut self) -> Box<dyn Iterator<Item = &mut WidgetHolder> + '_> {
		match self {
			Layout::MenuLayout(menu_layout) => Box::new(menu_layout.iter_mut()),
			Layout::WidgetLayout(widget_layout) => Box::new(widget_layout.iter_mut()),
		}
	}
}

impl Default for Layout {
	fn default() -> Self {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct WidgetLayout {
	pub layout: SubLayout,
}

impl WidgetLayout {
	pub fn new(layout: SubLayout) -> Self {
		Self { layout }
	}

	pub fn iter(&self) -> WidgetIter<'_> {
		WidgetIter {
			stack: self.layout.iter().collect(),
			current_slice: None,
		}
	}

	pub fn iter_mut(&mut self) -> WidgetIterMut<'_> {
		WidgetIterMut {
			stack: self.layout.iter_mut().collect(),
			current_slice: None,
		}
	}
}

#[derive(Debug, Default)]
pub struct WidgetIter<'a> {
	pub stack: Vec<&'a LayoutGroup>,
	pub current_slice: Option<&'a [WidgetHolder]>,
}

impl<'a> Iterator for WidgetIter<'a> {
	type Item = &'a WidgetHolder;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(item) = self.current_slice.and_then(|slice| slice.first()) {
			self.current_slice = Some(&self.current_slice.unwrap()[1..]);
			return Some(item);
		}

		match self.stack.pop() {
			Some(LayoutGroup::Column { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Row { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Section { name: _, layout }) => {
				for layout_row in layout {
					self.stack.push(layout_row);
				}
				self.next()
			}
			None => None,
		}
	}
}

#[derive(Debug, Default)]
pub struct WidgetIterMut<'a> {
	pub stack: Vec<&'a mut LayoutGroup>,
	pub current_slice: Option<&'a mut [WidgetHolder]>,
}

impl<'a> Iterator for WidgetIterMut<'a> {
	type Item = &'a mut WidgetHolder;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some((first, rest)) = self.current_slice.take().and_then(|slice| slice.split_first_mut()) {
			self.current_slice = Some(rest);
			return Some(first);
		};

		match self.stack.pop() {
			Some(LayoutGroup::Column { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Row { widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutGroup::Section { name: _, layout }) => {
				for layout_row in layout {
					self.stack.push(layout_row);
				}
				self.next()
			}
			None => None,
		}
	}
}

pub type SubLayout = Vec<LayoutGroup>;

#[remain::sorted]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutGroup {
	#[serde(rename = "column")]
	Column {
		#[serde(rename = "columnWidgets")]
		widgets: Vec<WidgetHolder>,
	},
	#[serde(rename = "row")]
	Row {
		#[serde(rename = "rowWidgets")]
		widgets: Vec<WidgetHolder>,
	},
	#[serde(rename = "section")]
	Section { name: String, layout: SubLayout },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetHolder {
	#[serde(rename = "widgetId")]
	pub widget_id: u64,
	pub widget: Widget,
}

impl WidgetHolder {
	pub fn new(widget: Widget) -> Self {
		Self { widget_id: generate_uuid(), widget }
	}
}

#[derive(Clone)]
pub struct WidgetCallback<T> {
	pub callback: Rc<dyn Fn(&T) -> Message + 'static>,
}

impl<T> WidgetCallback<T> {
	pub fn new(callback: impl Fn(&T) -> Message + 'static) -> Self {
		Self { callback: Rc::new(callback) }
	}
}

impl<T> Default for WidgetCallback<T> {
	fn default() -> Self {
		Self::new(|_| Message::NoOp)
	}
}

#[remain::sorted]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Widget {
	CheckboxInput(CheckboxInput),
	ColorInput(ColorInput),
	DropdownInput(DropdownInput),
	FontInput(FontInput),
	IconButton(IconButton),
	IconLabel(IconLabel),
	InvisibleStandinInput(InvisibleStandinInput),
	NumberInput(NumberInput),
	OptionalInput(OptionalInput),
	PopoverButton(PopoverButton),
	RadioInput(RadioInput),
	Separator(Separator),
	SwatchPairInput(SwatchPairInput),
	TextAreaInput(TextAreaInput),
	TextButton(TextButton),
	TextInput(TextInput),
	TextLabel(TextLabel),
}
