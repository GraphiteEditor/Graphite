use super::utility_types::misc::LayoutTarget;
use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::layout::utility_types::layout_widget::Layout;
use crate::messages::layout::utility_types::layout_widget::Widget;
use crate::messages::prelude::*;

use graphene::color::Color;
use graphene::layers::text_layer::Font;
use graphene::LayerId;

use serde_json::Value;
use std::ops::Not;

#[derive(Debug, Clone, Default)]
pub struct LayoutMessageHandler {
	layouts: [Layout; LayoutTarget::LayoutTargetLength as usize],
}

impl<F: Fn(&MessageDiscriminant) -> Vec<KeysGroup>> MessageHandler<LayoutMessage, F> for LayoutMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: LayoutMessage, data: F, responses: &mut std::collections::VecDeque<Message>) {
		let action_input_mapping = data;

		use LayoutMessage::*;
		#[remain::sorted]
		match message {
			RefreshLayout { layout_target } => {
				self.send_layout(layout_target, responses, &action_input_mapping);
			}
			SendLayout { layout, layout_target } => {
				self.layouts[layout_target as usize] = layout;

				self.send_layout(layout_target, responses, &action_input_mapping);
			}
			UpdateLayout { layout_target, widget_id, value } => {
				// Look up the layout
				let layout = if let Some(layout) = self.layouts.get_mut(layout_target as usize) {
					layout
				} else {
					warn!(
						"UpdateLayout was called referencing an invalid layout. `widget_id: {}`, `layout_target: {:?}`",
						widget_id, layout_target
					);
					return;
				};

				let widget_holder = if let Some(widget_holder) = layout.iter_mut().find(|widget| widget.widget_id == widget_id) {
					widget_holder
				} else {
					warn!(
						"UpdateLayout was called referencing an invalid widget ID, although the layout target was valid. `widget_id: {}`, `layout_target: {:?}`",
						widget_id, layout_target
					);
					return;
				};

				#[remain::sorted]
				match &mut widget_holder.widget {
					Widget::CheckboxInput(checkbox_input) => {
						let update_value = value.as_bool().expect("CheckboxInput update was not of type: bool");
						checkbox_input.checked = update_value;
						let callback_message = (checkbox_input.on_update.callback)(checkbox_input);
						responses.push_back(callback_message);
					}
					Widget::ColorInput(color_input) => {
						let update_value = value.as_object().expect("ColorInput update was not of type: object");
						let parsed_color = (|| {
							let is_none = update_value.get("none")?.as_bool()?;

							if !is_none {
								Some(Some(Color::from_rgbaf32(
									update_value.get("red")?.as_f64()? as f32,
									update_value.get("green")?.as_f64()? as f32,
									update_value.get("blue")?.as_f64()? as f32,
									update_value.get("alpha")?.as_f64()? as f32,
								)?))
							} else {
								Some(None)
							}
						})()
						.unwrap_or_else(|| panic!("ColorInput update was not able to be parsed with color data: {:?}", color_input));
						color_input.value = parsed_color;
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
					Widget::InvisibleStandinInput(invisible) => {
						let callback_message = (invisible.on_update.callback)(&());
						responses.push_back(callback_message);
					}
					Widget::LayerReferenceInput(layer_reference_input) => {
						let update_value = value.is_null().not().then(|| {
							value
								.as_str()
								.expect("LayerReferenceInput update was not of type: string")
								.split(',')
								.map(|id| id.parse::<LayerId>().unwrap())
								.collect::<Vec<_>>()
						});
						layer_reference_input.value = update_value;
						let callback_message = (layer_reference_input.on_update.callback)(layer_reference_input);
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
					Widget::PivotAssist(pivot_assist) => {
						let update_value = value.as_str().expect("RadioInput update was not of type: u64");
						pivot_assist.position = update_value.into();
						let callback_message = (pivot_assist.on_update.callback)(pivot_assist);
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

	fn actions(&self) -> ActionList {
		actions!()
	}
}

impl LayoutMessageHandler {
	#[remain::check]
	fn send_layout(&self, layout_target: LayoutTarget, responses: &mut VecDeque<Message>, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) {
		let layout = &self.layouts[layout_target as usize];
		#[remain::sorted]
		let message = match layout_target {
			LayoutTarget::DialogDetails => FrontendMessage::UpdateDialogDetails {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::DocumentBar => FrontendMessage::UpdateDocumentBarLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::DocumentMode => FrontendMessage::UpdateDocumentModeLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::LayerTreeOptions => FrontendMessage::UpdateLayerTreeOptionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::MenuBar => FrontendMessage::UpdateMenuBarLayout {
				layout_target,
				layout: layout.clone().unwrap_menu_layout(action_input_mapping).layout,
			},
			LayoutTarget::PropertiesOptions => FrontendMessage::UpdatePropertyPanelOptionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::PropertiesSections => FrontendMessage::UpdatePropertyPanelSectionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::ToolOptions => FrontendMessage::UpdateToolOptionsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::ToolShelf => FrontendMessage::UpdateToolShelfLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},
			LayoutTarget::WorkingColors => FrontendMessage::UpdateWorkingColorsLayout {
				layout_target,
				layout: layout.clone().unwrap_widget_layout(action_input_mapping).layout,
			},

			#[remain::unsorted]
			LayoutTarget::LayoutTargetLength => panic!("`LayoutTargetLength` is not a valid Layout Target and is used for array indexing"),
		};
		responses.push_back(message.into());
	}
}
