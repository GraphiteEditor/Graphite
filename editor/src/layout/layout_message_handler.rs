use super::layout_message::LayoutTarget;
use super::widgets::Layout;
use crate::layout::widgets::Widget;
use crate::message_prelude::*;

use graphene::layers::text_layer::Font;

use serde_json::Value;
use std::collections::VecDeque;

#[derive(Debug, Clone, Default)]
pub struct LayoutMessageHandler {
	layouts: [Layout; LayoutTarget::LayoutTargetLength as usize],
}

impl LayoutMessageHandler {
	#[remain::check]
	fn send_layout(&self, layout_target: LayoutTarget, responses: &mut VecDeque<Message>) {
		let layout = &self.layouts[layout_target as usize];
		#[remain::sorted]
		let message = match layout_target {
			LayoutTarget::DialogDetails => FrontendMessage::UpdateDialogDetails {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::DocumentBar => FrontendMessage::UpdateDocumentBarLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::DocumentMode => FrontendMessage::UpdateDocumentModeLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::LayerTreeOptions => FrontendMessage::UpdateLayerTreeOptionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::MenuBar => FrontendMessage::UpdateMenuBarLayout {
				layout_target,
				layout: layout.clone().unwrap_menu_layout().layout,
			},
			LayoutTarget::PropertiesOptions => FrontendMessage::UpdatePropertyPanelOptionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::PropertiesSections => FrontendMessage::UpdatePropertyPanelSectionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::ToolOptions => FrontendMessage::UpdateToolOptionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::ToolShelf => FrontendMessage::UpdateToolShelfLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},
			LayoutTarget::WorkingColors => FrontendMessage::UpdateWorkingColorsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout().layout,
			},

			#[remain::unsorted]
			LayoutTarget::LayoutTargetLength => panic!("`LayoutTargetLength` is not a valid Layout Target and is used for array indexing"),
		};
		responses.push_back(message.into());
	}
}

impl MessageHandler<LayoutMessage, ()> for LayoutMessageHandler {
	#[remain::check]
	fn process_action(&mut self, action: LayoutMessage, _data: (), responses: &mut std::collections::VecDeque<crate::message_prelude::Message>) {
		use LayoutMessage::*;
		#[remain::sorted]
		match action {
			RefreshLayout { layout_target } => {
				self.send_layout(layout_target, responses);
			}
			SendLayout { layout, layout_target } => {
				self.layouts[layout_target as usize] = layout;

				self.send_layout(layout_target, responses);
			}
			UpdateLayout { layout_target, widget_id, value } => {
				let layout = &mut self.layouts[layout_target as usize];
				let widget_holder = layout.iter_mut().find(|widget| widget.widget_id == widget_id);
				if widget_holder.is_none() {
					log::trace!(
						"Could not find widget_id:{} on layout_target:{:?}. This could be an indication of a problem or just a user clicking off of an actively edited layer",
						widget_id,
						layout_target
					);
					return;
				}
				#[remain::sorted]
				match &mut widget_holder.unwrap().widget {
					Widget::CheckboxInput(checkbox_input) => {
						let update_value = value.as_bool().expect("CheckboxInput update was not of type: bool");
						checkbox_input.checked = update_value;
						let callback_message = (checkbox_input.on_update.callback)(checkbox_input);
						responses.push_back(callback_message);
					}
					Widget::ColorInput(color_input) => {
						let update_value = value.as_str().map(String::from);
						color_input.value = update_value;
						let callback_message = (color_input.on_update.callback)(color_input);
						responses.push_back(callback_message);
					}
					Widget::DropdownInput(dropdown_input) => {
						let update_value = value.as_u64().expect("DropdownInput update was not of type: u64");
						dropdown_input.selected_index = Some(update_value as u32);
						let callback_message = (dropdown_input.entries.iter().flatten().nth(update_value as usize).unwrap().on_update.callback)(&());
						responses.push_back(callback_message);
					}
					Widget::FontInput(font_input) => {
						let update_value = value.as_object().expect("FontInput update was not of type: object");
						let font_family_value = update_value.get("fontFamily").expect("FontInput update does not have a fontFamily");
						let font_style_value = update_value.get("fontStyle").expect("FontInput update does not have a fontStyle");

						let font_family = font_family_value.as_str().expect("FontInput update fontFamily was not of type: string");
						let font_style = font_style_value.as_str().expect("FontInput update fontStyle was not of type: string");

						font_input.font_family = font_family.into();
						font_input.font_style = font_style.into();

						responses.push_back(
							PortfolioMessage::LoadFont {
								font: Font::new(font_family.into(), font_style.into()),
								is_default: false,
							}
							.into(),
						);
						let callback_message = (font_input.on_update.callback)(font_input);
						responses.push_back(callback_message);
					}
					Widget::IconButton(icon_button) => {
						let callback_message = (icon_button.on_update.callback)(icon_button);
						responses.push_back(callback_message);
					}
					Widget::IconLabel(_) => {}
					Widget::Invisible(invisible) => {
						let callback_message = (invisible.on_update.callback)(&());
						responses.push_back(callback_message);
					}
					Widget::NumberInput(number_input) => match value {
						Value::Number(num) => {
							let update_value = num.as_f64().unwrap();
							number_input.value = Some(update_value);
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
					Widget::OptionalInput(optional_input) => {
						let update_value = value.as_bool().expect("OptionalInput update was not of type: bool");
						optional_input.checked = update_value;
						let callback_message = (optional_input.on_update.callback)(optional_input);
						responses.push_back(callback_message);
					}
					Widget::PopoverButton(_) => {}
					Widget::RadioInput(radio_input) => {
						let update_value = value.as_u64().expect("RadioInput update was not of type: u64");
						radio_input.selected_index = update_value as u32;
						let callback_message = (radio_input.entries[update_value as usize].on_update.callback)(&());
						responses.push_back(callback_message);
					}
					Widget::Separator(_) => {}
					Widget::SwatchPairInput(_) => {}
					Widget::TextAreaInput(text_area_input) => {
						let update_value = value.as_str().expect("TextAreaInput update was not of type: string");
						text_area_input.value = update_value.into();
						let callback_message = (text_area_input.on_update.callback)(text_area_input);
						responses.push_back(callback_message);
					}
					Widget::TextButton(text_button) => {
						let callback_message = (text_button.on_update.callback)(text_button);
						responses.push_back(callback_message);
					}
					Widget::TextInput(text_input) => {
						let update_value = value.as_str().expect("TextInput update was not of type: string");
						text_input.value = update_value.into();
						let callback_message = (text_input.on_update.callback)(text_input);
						responses.push_back(callback_message);
					}
					Widget::TextLabel(_) => {}
				};
				responses.push_back(RefreshLayout { layout_target }.into());
			}
		}
	}

	fn actions(&self) -> crate::message_prelude::ActionList {
		actions!()
	}
}
