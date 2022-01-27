use std::{
	collections::{HashMap, VecDeque},
	ops::DerefMut,
};

use serde_json::Value;

use crate::{
	layout::widgets::Widget,
	message_prelude::{FrontendMessage, Message, MessageHandler},
};

use super::{layout_message::LayoutTarget, widgets::WidgetLayout, LayoutMessage};

#[derive(Debug, Clone, Default)]
pub struct LayoutMessageHandler {
	layouts: HashMap<LayoutTarget, WidgetLayout>,
}

impl LayoutMessageHandler {
	fn send_layout(&self, layout_target: LayoutTarget, responses: &mut VecDeque<Message>) {
		let widget_layout = self.layouts.get(&layout_target).unwrap();
		let message = match layout_target {
			LayoutTarget::ToolOptions => FrontendMessage::UpdateToolOptionsLayout {
				layout_target,
				layout: widget_layout.layout.clone(),
			},
			LayoutTarget::DocumentBar => FrontendMessage::UpdateDocumentBarLayout {
				layout_target,
				layout: widget_layout.layout.clone(),
			},
		};
		responses.push_back(message.into());
	}
}

impl MessageHandler<LayoutMessage, ()> for LayoutMessageHandler {
	fn process_action(&mut self, action: LayoutMessage, _data: (), responses: &mut std::collections::VecDeque<crate::message_prelude::Message>) {
		use LayoutMessage::*;
		match action {
			SendLayout { layout, layout_target } => {
				self.layouts.insert(layout_target.clone(), layout);

				self.send_layout(layout_target, responses);
			}
			UpdateLayout { layout_target, widget_id, value } => {
				let layout = self.layouts.get(&layout_target).expect("Received invalid layout_id from the frontend");
				let widget = layout.widget_lookup.get(&widget_id).expect("Received invalid widget_id from the frontend");
				match (**widget).borrow_mut().deref_mut() {
					Widget::NumberInput(number_input) => match value {
						Value::Number(num) => {
							let update_value = num.as_f64().unwrap();
							number_input.value = update_value;
							let callback_message = (number_input.on_update.callback)(number_input);
							responses.push_back(callback_message);
						}
						Value::String(str) => match str.as_str() {
							"Increment" => responses.push_back((number_input.increment_callback_increase.callback)(number_input)),
							"Decrement" => responses.push_back((number_input.increment_callback_decrease.callback)(number_input)),
							_ => {
								panic!("Invalid string found when updating `NumberInput`")
							}
						},
						_ => panic!("Invalid type found when updating `NumberInput`"),
					},
					Widget::Separator(_) => {}
					Widget::IconButton(icon_button) => {
						let callback_message = (icon_button.on_update.callback)(icon_button);
						responses.push_back(callback_message);
					}
					Widget::PopoverButton(_) => {}
					Widget::OptionalInput(optional_input) => {
						let update_value = value.as_bool().expect("OptionalInput update was not of type: bool");
						optional_input.checked = update_value;
						let callback_message = (optional_input.on_update.callback)(optional_input);
						responses.push_back(callback_message);
					}
					Widget::RadioInput(radio_input) => {
						let update_value = value.as_u64().expect("OptionalInput update was not of type: u64");
						radio_input.selected_index = update_value as u32;
						let callback_message = (radio_input.entries[update_value as usize].on_update.callback)(&());
						responses.push_back(callback_message);
					}
				};
				self.send_layout(layout_target, responses);
			}
			WidgetDefaultMarker => {
				panic!("Please ensure that all widgets have an `on_update` property")
			}
		}
	}

	fn actions(&self) -> crate::message_prelude::ActionList {
		actions!()
	}
}
