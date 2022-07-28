use super::layout_message::LayoutTarget;
use crate::input::input_mapper::FutureKeyMapping;
use crate::input::keyboard::Key;
use crate::message_prelude::*;
use crate::Color;

use derivative::*;
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Layout {
	WidgetLayout(WidgetLayout),
	MenuLayout(MenuLayout),
}

impl Layout {
	pub fn unwrap_widget_layout(self) -> WidgetLayout {
		if let Layout::WidgetLayout(widget_layout) = self {
			widget_layout
		} else {
			panic!("Tried to unwrap layout as WidgetLayout. Got {:?}", self)
		}
	}

	pub fn unwrap_menu_layout(self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<Vec<Key>>) -> MenuLayout {
		if let Layout::MenuLayout(mut menu_layout) = self {
			for menu_column in &mut menu_layout.layout {
				menu_column.children.realize_future_key_mappings(action_input_mapping);
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

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct MenuEntryGroups(pub Vec<Vec<MenuEntry>>);

impl MenuEntryGroups {
	pub fn empty() -> Self {
		Self(Vec::new())
	}

	pub fn realize_future_key_mappings(&mut self, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<Vec<Key>>) {
		let entries = self.0.iter_mut().flatten();
		for entry in entries {
			if let Some(future_key_mapping) = &mut entry.shortcut {
				future_key_mapping.realize(action_input_mapping);
			}

			// Recursively do this for the children also
			entry.children.realize_future_key_mappings(action_input_mapping);
		}
	}
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MenuEntry {
	pub label: String,
	pub icon: Option<String>,
	pub children: MenuEntryGroups,
	pub action: WidgetHolder,
	pub shortcut: Option<FutureKeyMapping>,
}

impl MenuEntry {
	pub fn create_action(callback: impl Fn(&()) -> Message + 'static) -> WidgetHolder {
		WidgetHolder::new(Widget::Invisible(Invisible {
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

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct MenuColumn {
	pub label: String,
	pub children: MenuEntryGroups,
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
	Invisible(Invisible),
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

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct CheckboxInput {
	pub checked: bool,

	pub icon: String,

	pub tooltip: String,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<CheckboxInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct ColorInput {
	pub value: Option<String>,

	pub label: Option<String>,

	#[serde(rename = "noTransparency")]
	#[derivative(Default(value = "true"))]
	pub no_transparency: bool,

	pub disabled: bool,

	pub tooltip: String,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<ColorInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct DropdownInput {
	pub entries: DropdownInputEntries,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: Option<u32>,

	#[serde(rename = "drawIcon")]
	pub draw_icon: bool,

	#[derivative(Default(value = "true"))]
	pub interactive: bool,

	pub disabled: bool,
	//
	// Callbacks
	// `on_update` exists on the `DropdownEntryData`, not this parent `DropdownInput`
}

pub type DropdownInputEntries = Vec<Vec<DropdownEntryData>>;

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct DropdownEntryData {
	pub value: String,

	pub label: String,

	pub icon: String,

	pub shortcut: Vec<String>,

	#[serde(rename = "shortcutRequiresLock")]
	pub shortcut_requires_lock: bool,

	pub disabled: bool,

	pub children: DropdownInputEntries,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct FontInput {
	#[serde(rename = "fontFamily")]
	pub font_family: String,

	#[serde(rename = "fontStyle")]
	pub font_style: String,

	#[serde(rename = "isStyle")]
	pub is_style_picker: bool,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<FontInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct IconButton {
	pub icon: String,

	pub size: u32, // TODO: Convert to an `IconSize` enum

	pub active: bool,

	pub tooltip: String,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<IconButton>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, Default, PartialEq, Eq)]
pub struct IconLabel {
	pub icon: String,

	#[serde(rename = "iconStyle")]
	pub icon_style: IconStyle,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, Default, PartialEq, Eq)]
pub enum IconStyle {
	#[default]
	Normal,
	Node,
}

/// This widget allows for the flexible use of the layout system.
/// In a custom layout, one can define a widget that is just used to trigger code on the backend.
/// This is used in MenuLayout to pipe the triggering of messages from the frontend to backend.
#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct Invisible {
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct NumberInput {
	pub label: String,

	pub value: Option<f64>,

	pub min: Option<f64>,

	pub max: Option<f64>,

	#[serde(rename = "isInteger")]
	pub is_integer: bool,

	#[serde(rename = "displayDecimalPlaces")]
	#[derivative(Default(value = "3"))]
	pub display_decimal_places: u32,

	pub unit: String,

	#[serde(rename = "unitIsHiddenWhenEditing")]
	#[derivative(Default(value = "true"))]
	pub unit_is_hidden_when_editing: bool,

	#[serde(rename = "incrementBehavior")]
	pub increment_behavior: NumberInputIncrementBehavior,

	#[serde(rename = "incrementFactor")]
	#[derivative(Default(value = "1."))]
	pub increment_factor: f64,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<NumberInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub increment_callback_increase: WidgetCallback<NumberInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub increment_callback_decrease: WidgetCallback<NumberInput>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
pub enum NumberInputIncrementBehavior {
	#[default]
	Add,
	Multiply,
	Callback,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct OptionalInput {
	pub checked: bool,

	pub icon: String,

	pub tooltip: String,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<OptionalInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct PopoverButton {
	pub icon: Option<String>,

	// Body
	pub header: String,

	pub text: String,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct RadioInput {
	pub entries: Vec<RadioEntryData>,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: u32,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct RadioEntryData {
	pub value: String,

	pub label: String,

	pub icon: String,

	pub tooltip: String,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Separator {
	pub direction: SeparatorDirection,

	#[serde(rename = "type")]
	pub separator_type: SeparatorType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeparatorDirection {
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeparatorType {
	Related,
	Unrelated,
	Section,
	List,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct SwatchPairInput {
	pub primary: Color,

	pub secondary: Color,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextAreaInput {
	pub value: String,

	pub label: Option<String>,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextAreaInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct TextButton {
	pub label: String,

	pub emphasized: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextButton>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextInput {
	pub value: String,

	pub label: Option<String>,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, PartialEq, Eq, Default)]
pub struct TextLabel {
	pub bold: bool,

	pub italic: bool,

	#[serde(rename = "tableAlign")]
	pub table_align: bool,

	pub multiline: bool,

	// Body
	pub value: String,
}
