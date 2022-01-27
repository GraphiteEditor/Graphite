use std::rc::Rc;
use std::{
	cell::RefCell,
	collections::{HashMap, VecDeque},
};

use serde::{Deserialize, Serialize};

use crate::message_prelude::{generate_uuid, Message};

use super::{layout_message::LayoutTarget, LayoutMessage};

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
	#[serde(skip)]
	pub widget_lookup: HashMap<u64, Rc<RefCell<Widget>>>,
}

impl WidgetLayout {
	pub fn new(layout: SubLayout) -> Self {
		let widget_lookup: HashMap<u64, Rc<RefCell<Widget>>> = layout.iter().flat_map(|row| row.widgets()).map(|holder| (holder.widget_id, holder.widget)).collect();
		Self { layout, widget_lookup }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WidgetHolder {
	pub widget_id: u64,
	pub widget: Rc<RefCell<Widget>>,
}

impl WidgetHolder {
	pub fn new(widget: Widget) -> Self {
		Self {
			widget_id: generate_uuid(),
			widget: Rc::new(RefCell::new(widget)),
		}
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
	None,
}

impl Default for NumberInputIncrementBehavior {
	fn default() -> Self {
		Self::None
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
