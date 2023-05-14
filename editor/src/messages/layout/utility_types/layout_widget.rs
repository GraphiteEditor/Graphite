use super::widgets::assist_widgets::*;
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
use std::sync::Arc;

pub trait PropertyHolder {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}

	fn register_properties(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.properties(),
			layout_target,
		})
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub enum Layout {
	WidgetLayout(WidgetLayout),
	MenuLayout(MenuLayout),
}

/// The new value of the UI, sent as part of a diff.
///
/// An update can represent a single widget or an entire SubLayout, or just a single layout group.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum DiffUpdate {
	#[serde(rename = "subLayout")]
	SubLayout(SubLayout),
	#[serde(rename = "layoutGroup")]
	LayoutGroup(LayoutGroup),
	#[serde(rename = "widget")]
	Widget(WidgetHolder),
}

/// A single change to part of the UI, containing the location of the change and the new value.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub struct WidgetDiff {
	/// A path to the change
	/// e.g. [0, 1, 2] in the properties panel is the first section, second row and third widget.
	/// An empty path [] shows that the entire panel has changed and is sent when the UI is first created.
	#[serde(rename = "widgetPath")]
	pub widget_path: Vec<usize>,
	/// What the specified part of the UI has changed to.
	#[serde(rename = "newValue")]
	pub new_value: DiffUpdate,
}

impl DiffUpdate {
	/// Append the shortcut to the tooltip where applicable
	pub fn apply_shortcut(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) {
		// Function used multiple times later in this code block to convert `ActionKeys::Action` to `ActionKeys::Keys` and append its shortcut to the tooltip
		let apply_shortcut_to_tooltip = |tooltip_shortcut: &mut ActionKeys, tooltip: &mut String| {
			let shortcut_text = tooltip_shortcut.to_keys(action_input_mapping);

			if let ActionKeys::Keys(_keys) = tooltip_shortcut {
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
		let convert_tooltip = |widget_holder: &mut WidgetHolder| {
			// Handle all the widgets that have tooltips
			let mut tooltip_shortcut = match &mut widget_holder.widget {
				Widget::BreadcrumbTrailButtons(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::CheckboxInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::ColorInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::DropdownInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::FontInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::IconButton(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::LayerReferenceInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::NumberInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::OptionalInput(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::ParameterExposeButton(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::PopoverButton(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::TextButton(widget) => Some((&mut widget.tooltip, &mut widget.tooltip_shortcut)),
				Widget::CurveInput(_)
				| Widget::IconLabel(_)
				| Widget::InvisibleStandinInput(_)
				| Widget::PivotAssist(_)
				| Widget::RadioInput(_)
				| Widget::Separator(_)
				| Widget::SwatchPairInput(_)
				| Widget::TextAreaInput(_)
				| Widget::TextInput(_)
				| Widget::TextLabel(_) => None,
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
		};

		match self {
			Self::SubLayout(sub_layout) => sub_layout.iter_mut().flat_map(|group| group.iter_mut()).for_each(convert_tooltip),
			Self::LayoutGroup(layout_group) => layout_group.iter_mut().for_each(convert_tooltip),
			Self::Widget(widget_holder) => convert_tooltip(widget_holder),
		}
	}
}

impl Layout {
	pub fn unwrap_menu_layout(self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) -> MenuLayout {
		if let Self::MenuLayout(mut menu) = self {
			menu.layout
				.iter_mut()
				.for_each(|menu_column| menu_column.children.fill_in_shortcut_actions_with_keys(action_input_mapping));
			menu
		} else {
			panic!("Called unwrap_menu_layout on a widget layout");
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

	/// Diffing updates self (where self is old) based on new, updating the list of modifications as it does so.
	pub fn diff(&mut self, new: Self, widget_path: &mut Vec<usize>, widget_diffs: &mut Vec<WidgetDiff>) {
		match (self, new) {
			// Simply diff the internal layout
			(Self::WidgetLayout(current), Self::WidgetLayout(new)) => current.diff(new, widget_path, widget_diffs),
			(current, Self::WidgetLayout(widget_layout)) => {
				// Update current to the new value
				*current = Self::WidgetLayout(widget_layout.clone());

				// Push an update sublayout value
				let new_value = DiffUpdate::SubLayout(widget_layout.layout);
				let widget_path = widget_path.to_vec();
				widget_diffs.push(WidgetDiff { widget_path, new_value });
			}
			(_, Self::MenuLayout(_)) => panic!("Cannot diff menu layout"),
		}
	}
}

impl Default for Layout {
	fn default() -> Self {
		Self::WidgetLayout(WidgetLayout::default())
	}
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
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

	/// Diffing updates self (where self is old) based on new, updating the list of modifications as it does so.
	pub fn diff(&mut self, new: Self, widget_path: &mut Vec<usize>, widget_diffs: &mut Vec<WidgetDiff>) {
		// Check if the length of items is different
		// TODO: Diff insersion and deletion of items
		if self.layout.len() != new.layout.len() {
			// Update the layout to the new layout
			self.layout = new.layout.clone();

			// Push an update sublayout to the diff
			let new = DiffUpdate::SubLayout(new.layout);
			widget_diffs.push(WidgetDiff {
				widget_path: widget_path.to_vec(),
				new_value: new,
			});
			return;
		}
		// Diff all of the children
		for (index, (current_child, new_child)) in self.layout.iter_mut().zip(new.layout.into_iter()).enumerate() {
			widget_path.push(index);
			current_child.diff(new_child, widget_path, widget_diffs);
			widget_path.pop();
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
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

impl Default for LayoutGroup {
	fn default() -> Self {
		Self::Row { widgets: Vec::new() }
	}
}

impl LayoutGroup {
	/// Applies a tooltip to all widgets in this row or column without a tooltip.
	pub fn with_tooltip(self, tooltip: impl Into<String>) -> Self {
		let (is_col, mut widgets) = match self {
			LayoutGroup::Column { widgets } => (true, widgets),
			LayoutGroup::Row { widgets } => (false, widgets),
			_ => unimplemented!(),
		};
		let tooltip = tooltip.into();
		for widget in &mut widgets {
			let val = match &mut widget.widget {
				Widget::CheckboxInput(x) => &mut x.tooltip,
				Widget::ColorInput(x) => &mut x.tooltip,
				Widget::CurveInput(x) => &mut x.tooltip,
				Widget::DropdownInput(x) => &mut x.tooltip,
				Widget::FontInput(x) => &mut x.tooltip,
				Widget::IconButton(x) => &mut x.tooltip,
				Widget::IconLabel(x) => &mut x.tooltip,
				Widget::LayerReferenceInput(x) => &mut x.tooltip,
				Widget::NumberInput(x) => &mut x.tooltip,
				Widget::OptionalInput(x) => &mut x.tooltip,
				Widget::ParameterExposeButton(x) => &mut x.tooltip,
				Widget::PopoverButton(x) => &mut x.tooltip,
				Widget::TextAreaInput(x) => &mut x.tooltip,
				Widget::TextButton(x) => &mut x.tooltip,
				Widget::TextInput(x) => &mut x.tooltip,
				Widget::TextLabel(x) => &mut x.tooltip,
				Widget::BreadcrumbTrailButtons(x) => &mut x.tooltip,
				Widget::InvisibleStandinInput(_) | Widget::PivotAssist(_) | Widget::RadioInput(_) | Widget::Separator(_) | Widget::SwatchPairInput(_) => continue,
			};
			if val.is_empty() {
				*val = tooltip.clone();
			}
		}
		if is_col {
			Self::Column { widgets }
		} else {
			Self::Row { widgets }
		}
	}

	/// Diffing updates self (where self is old) based on new, updating the list of modifications as it does so.
	pub fn diff(&mut self, new: Self, widget_path: &mut Vec<usize>, widget_diffs: &mut Vec<WidgetDiff>) {
		let is_column = matches!(new, Self::Column { .. });
		match (self, new) {
			(Self::Column { widgets: current_widgets }, Self::Column { widgets: new_widgets }) | (Self::Row { widgets: current_widgets }, Self::Row { widgets: new_widgets }) => {
				// If the lengths are different then resend the entire panel
				// TODO: Diff insersion and deletion of items
				if current_widgets.len() != new_widgets.len() {
					// Update to the new value
					*current_widgets = new_widgets.clone();

					// Push back a LayoutGroup update to the diff
					let new_value = DiffUpdate::LayoutGroup(if is_column { Self::Column { widgets: new_widgets } } else { Self::Row { widgets: new_widgets } });
					let widget_path = widget_path.to_vec();
					widget_diffs.push(WidgetDiff { widget_path, new_value });
					return;
				}
				// Diff all of the children
				for (index, (current_child, new_child)) in current_widgets.iter_mut().zip(new_widgets.into_iter()).enumerate() {
					widget_path.push(index);
					current_child.diff(new_child, widget_path, widget_diffs);
					widget_path.pop();
				}
			}
			(
				Self::Section {
					name: current_name,
					layout: current_layout,
				},
				Self::Section { name: new_name, layout: new_layout },
			) => {
				// If the lengths are different then resend the entire panel
				// TODO: Diff insersion and deletion of items
				if *current_name != new_name || current_layout.len() != new_layout.len() {
					// Update self to reflect new changes
					*current_name = new_name.clone();
					*current_layout = new_layout.clone();

					// Push an update layout group to the diff
					let new_value = DiffUpdate::LayoutGroup(Self::Section { name: new_name, layout: new_layout });
					let widget_path = widget_path.to_vec();
					widget_diffs.push(WidgetDiff { widget_path, new_value });
					return;
				}
				// Diff all of the children
				for (index, (current_child, new_child)) in current_layout.iter_mut().zip(new_layout.into_iter()).enumerate() {
					widget_path.push(index);
					current_child.diff(new_child, widget_path, widget_diffs);
					widget_path.pop();
				}
			}
			(current, new) => {
				*current = new.clone();
				let new_value = DiffUpdate::LayoutGroup(new);
				let widget_path = widget_path.to_vec();
				widget_diffs.push(WidgetDiff { widget_path, new_value });
			}
		}
	}

	pub fn iter_mut(&mut self) -> WidgetIterMut<'_> {
		WidgetIterMut {
			stack: vec![self],
			current_slice: None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct WidgetHolder {
	#[serde(rename = "widgetId")]
	pub widget_id: u64,
	pub widget: Widget,
}

impl WidgetHolder {
	pub fn new(widget: Widget) -> Self {
		Self { widget_id: generate_uuid(), widget }
	}
	pub fn section_separator() -> Self {
		Separator::new(SeparatorDirection::Horizontal, SeparatorType::Section).widget_holder()
	}
	pub fn unrelated_separator() -> Self {
		Separator::new(SeparatorDirection::Horizontal, SeparatorType::Unrelated).widget_holder()
	}
	pub fn related_separator() -> Self {
		Separator::new(SeparatorDirection::Horizontal, SeparatorType::Related).widget_holder()
	}
	pub fn text_widget(text: impl Into<String>) -> Self {
		TextLabel::new(text).widget_holder()
	}
	pub fn bold_text(text: impl Into<String>) -> Self {
		TextLabel::new(text).bold(true).widget_holder()
	}
	/// Diffing updates self (where self is old) based on new, updating the list of modifications as it does so.
	pub fn diff(&mut self, new: Self, widget_path: &mut [usize], widget_diffs: &mut Vec<WidgetDiff>) {
		// If there have been changes to the acutal widget (not just the id)
		if self.widget != new.widget {
			// We should update to the new widget value as well as a new widget id
			*self = new.clone();

			// Push a widget update to the diff
			let new_value = DiffUpdate::Widget(new);
			let widget_path = widget_path.to_vec();
			widget_diffs.push(WidgetDiff { widget_path, new_value });
		} else {
			// Required to update the callback function, which the PartialEq check above skips
			self.widget = new.widget;
		}
	}
}

#[derive(Clone)]
pub struct WidgetCallback<T> {
	pub callback: Arc<dyn Fn(&T) -> Message + 'static + Send + Sync>,
}

impl<T> WidgetCallback<T> {
	pub fn new(callback: impl Fn(&T) -> Message + 'static + Send + Sync) -> Self {
		Self { callback: Arc::new(callback) }
	}
}

impl<T> Default for WidgetCallback<T> {
	fn default() -> Self {
		Self::new(|_| Message::NoOp)
	}
}

#[remain::sorted]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub enum Widget {
	BreadcrumbTrailButtons(BreadcrumbTrailButtons),
	CheckboxInput(CheckboxInput),
	ColorInput(ColorInput),
	CurveInput(CurveInput),
	DropdownInput(DropdownInput),
	FontInput(FontInput),
	IconButton(IconButton),
	IconLabel(IconLabel),
	InvisibleStandinInput(InvisibleStandinInput),
	LayerReferenceInput(LayerReferenceInput),
	NumberInput(NumberInput),
	OptionalInput(OptionalInput),
	ParameterExposeButton(ParameterExposeButton),
	PivotAssist(PivotAssist),
	PopoverButton(PopoverButton),
	RadioInput(RadioInput),
	Separator(Separator),
	SwatchPairInput(SwatchPairInput),
	TextAreaInput(TextAreaInput),
	TextButton(TextButton),
	TextInput(TextInput),
	TextLabel(TextLabel),
}
