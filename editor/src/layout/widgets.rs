use super::layout_message::LayoutTarget;
use crate::message_prelude::*;
use serde::{Deserialize, Serialize};

use derivative::*;

pub trait PropertyHolder {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::default()
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

pub type SubLayout = Vec<LayoutRow>;

#[remain::sorted]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutRow {
	Row { name: String, widgets: Vec<WidgetHolder> },
	Section { name: String, layout: SubLayout },
}

impl LayoutRow {
	pub fn widgets(&self) -> Vec<WidgetHolder> {
		match &self {
			Self::Row { name: _, widgets } => widgets.to_vec(),
			Self::Section { name: _, layout } => layout.iter().flat_map(|row| row.widgets()).collect(),
		}
	}
}

#[derive(Debug, Default)]
pub struct WidgetIterMut<'a> {
	pub stack: Vec<&'a mut LayoutRow>,
	pub current_slice: Option<&'a mut [WidgetHolder]>,
}

impl<'a> Iterator for WidgetIterMut<'a> {
	type Item = &'a mut WidgetHolder;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some((first, rest)) = self.current_slice.take().map(|slice| slice.split_first_mut()).flatten() {
			self.current_slice = Some(rest);
			return Some(first);
		};

		match self.stack.pop() {
			Some(LayoutRow::Row { name: _, widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutRow::Section { name: _, layout }) => {
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
pub struct WidgetIter<'a> {
	pub stack: Vec<&'a LayoutRow>,
	pub current_slice: Option<&'a [WidgetHolder]>,
}

impl<'a> Iterator for WidgetIter<'a> {
	type Item = &'a WidgetHolder;

	fn next(&mut self) -> Option<Self::Item> {
		if let Some(item) = self.current_slice.map(|slice| slice.first()).flatten() {
			self.current_slice = Some(&self.current_slice.unwrap()[1..]);
			return Some(item);
		}

		match self.stack.pop() {
			Some(LayoutRow::Row { name: _, widgets }) => {
				self.current_slice = Some(widgets);
				self.next()
			}
			Some(LayoutRow::Section { name: _, layout }) => {
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
	pub callback: fn(&T) -> Message,
}

impl<T> WidgetCallback<T> {
	pub fn new(callback: fn(&T) -> Message) -> Self {
		Self { callback }
	}
}

impl<T> Default for WidgetCallback<T> {
	fn default() -> Self {
		Self { callback: |_| Message::NoOp }
	}
}

#[remain::sorted]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Widget {
	IconButton(IconButton),
	NumberInput(NumberInput),
	OptionalInput(OptionalInput),
	PopoverButton(PopoverButton),
	RadioInput(RadioInput),
	Separator(Separator),
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct NumberInput {
	pub value: f64,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<NumberInput>,
	pub min: Option<f64>,
	pub max: Option<f64>,
	#[serde(rename = "isInteger")]
	pub is_integer: bool,
	#[serde(rename = "incrementBehavior")]
	pub increment_behavior: NumberInputIncrementBehavior,
	#[serde(rename = "incrementFactor")]
	pub increment_factor: f64,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub increment_callback_increase: WidgetCallback<NumberInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub increment_callback_decrease: WidgetCallback<NumberInput>,
	pub label: String,
	pub unit: String,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq)]
pub enum NumberInputIncrementBehavior {
	Add,
	Multiply,
	Callback,
}

impl Default for NumberInputIncrementBehavior {
	fn default() -> Self {
		Self::Add
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]

pub struct Separator {
	pub direction: SeparatorDirection,

	#[serde(rename = "type")]
	pub separator_type: SeparatorType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]

pub enum SeparatorDirection {
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]

pub enum SeparatorType {
	Related,
	Unrelated,
	Section,
	List,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct IconButton {
	pub icon: String,
	pub title: String,
	pub size: u32,
	#[serde(rename = "gapAfter")]
	pub gap_after: bool,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<IconButton>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct OptionalInput {
	pub checked: bool,
	pub icon: String,
	pub title: String,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<OptionalInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct PopoverButton {
	pub title: String,
	pub text: String,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct RadioInput {
	pub entries: Vec<RadioEntryData>,

	// use u32 since it will be serialized as a normal JS number
	// TODO(mfish33): Replace with usize when using native UI
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
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}
