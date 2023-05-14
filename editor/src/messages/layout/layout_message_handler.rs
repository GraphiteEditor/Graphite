use super::utility_types::layout_widget::{LayoutGroup, WidgetDiff, WidgetHolder};
use super::utility_types::misc::LayoutTarget;
use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::layout::utility_types::layout_widget::{DiffUpdate, Widget};
use crate::messages::layout::utility_types::layout_widget::{Layout, WidgetLayout};
use crate::messages::prelude::*;

use document_legacy::LayerId;
use graphene_core::raster::color::Color;
use graphene_core::text::Font;

use serde_json::Value;
use std::ops::Not;

#[derive(Debug, Clone, Default)]
pub struct LayoutMessageHandler {
	layouts: [Layout; LayoutTarget::LayoutTargetLength as usize],
}

impl LayoutMessageHandler {
	/// Get the widget path for the widget with the specified id
	fn get_widget_path(widget_layout: &WidgetLayout, id: u64) -> Option<(&WidgetHolder, Vec<usize>)> {
		let mut stack = widget_layout.layout.iter().enumerate().map(|(index, val)| (vec![index], val)).collect::<Vec<_>>();
		while let Some((mut widget_path, group)) = stack.pop() {
			match group {
				// Check if any of the widgets in the current column or row have the correct id
				LayoutGroup::Column { widgets } | LayoutGroup::Row { widgets } => {
					for (index, widget) in widgets.iter().enumerate() {
						// Return if this is the correct ID
						if widget.widget_id == id {
							widget_path.push(index);
							return Some((widget, widget_path));
						}
					}
				}
				// A section contains more LayoutGroups which we add to the stack.
				LayoutGroup::Section { layout, .. } => {
					stack.extend(layout.iter().enumerate().map(|(index, val)| ([widget_path.as_slice(), &[index]].concat(), val)));
				}
			}
		}
		None
	}
}

impl<F: Fn(&MessageDiscriminant) -> Vec<KeysGroup>> MessageHandler<LayoutMessage, F> for LayoutMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: LayoutMessage, responses: &mut std::collections::VecDeque<Message>, action_input_mapping: F) {
		use LayoutMessage::*;
		#[remain::sorted]
		match message {
			ResendActiveWidget { layout_target, dirty_id } => {
				// Find the updated diff based on the specified layout target
				let Some(diff) = (match &self.layouts[layout_target as usize] {
					Layout::MenuLayout(_) => return,
					Layout::WidgetLayout(layout) => Self::get_widget_path(layout, dirty_id).map(|(widget, widget_path)| {
						// Create a widget update diff for the relevant id
						let new_value = DiffUpdate::Widget(widget.clone());
						WidgetDiff { widget_path, new_value }
					}),
				}) else {
					return;
				};
				// Resend that diff
				self.send_diff(vec![diff], layout_target, responses, &action_input_mapping);
			}
			SendLayout { layout, layout_target } => self.send_layout(layout_target, layout, responses, &action_input_mapping),
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
					Widget::BreadcrumbTrailButtons(breadcrumb_trail_buttons) => {
						let update_value = value.as_u64().expect("BreadcrumbTrailButtons update was not of type: u64");

						let callback_message = (breadcrumb_trail_buttons.on_update.callback)(&update_value);
						responses.add(callback_message);
					}
					Widget::CheckboxInput(checkbox_input) => {
						let update_value = value.as_bool().expect("CheckboxInput update was not of type: bool");
						checkbox_input.checked = update_value;
						let callback_message = (checkbox_input.on_update.callback)(checkbox_input);
						responses.add(callback_message);
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
						.unwrap_or_else(|| panic!("ColorInput update was not able to be parsed with color data: {color_input:?}"));
						color_input.value = parsed_color;
						let callback_message = (color_input.on_update.callback)(color_input);
						responses.add(callback_message);
					}
					Widget::CurveInput(curve_input) => {
						let curve = serde_json::from_value(value).expect("CurveInput event data could not be deserialized");
						curve_input.value = curve;
						let callback_message = (curve_input.on_update.callback)(curve_input);
						info!("callback message: {callback_message:?}");
						responses.add(callback_message);
					}
					Widget::DropdownInput(dropdown_input) => {
						let update_value = value.as_u64().expect("DropdownInput update was not of type: u64");
						dropdown_input.selected_index = Some(update_value as u32);
						let callback_message = (dropdown_input.entries.iter().flatten().nth(update_value as usize).unwrap().on_update.callback)(&());
						responses.add(callback_message);
					}
					Widget::FontInput(font_input) => {
						let update_value = value.as_object().expect("FontInput update was not of type: object");
						let font_family_value = update_value.get("fontFamily").expect("FontInput update does not have a fontFamily");
						let font_style_value = update_value.get("fontStyle").expect("FontInput update does not have a fontStyle");

						let font_family = font_family_value.as_str().expect("FontInput update fontFamily was not of type: string");
						let font_style = font_style_value.as_str().expect("FontInput update fontStyle was not of type: string");

						font_input.font_family = font_family.into();
						font_input.font_style = font_style.into();

						responses.add(PortfolioMessage::LoadFont {
							font: Font::new(font_family.into(), font_style.into()),
							is_default: false,
						});
						let callback_message = (font_input.on_update.callback)(font_input);
						responses.add(callback_message);
					}
					Widget::IconButton(icon_button) => {
						let callback_message = (icon_button.on_update.callback)(icon_button);
						responses.add(callback_message);
					}
					Widget::IconLabel(_) => {}
					Widget::InvisibleStandinInput(invisible) => {
						let callback_message = (invisible.on_update.callback)(&());
						responses.add(callback_message);
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
						responses.add(callback_message);
					}
					Widget::NumberInput(number_input) => match value {
						Value::Number(num) => {
							let update_value = num.as_f64().unwrap();
							number_input.value = Some(update_value);
							let callback_message = (number_input.on_update.callback)(number_input);
							responses.add(callback_message);
						}
						Value::String(str) => match str.as_str() {
							"Increment" => responses.add((number_input.increment_callback_increase.callback)(number_input)),
							"Decrement" => responses.add((number_input.increment_callback_decrease.callback)(number_input)),
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
						responses.add(callback_message);
					}
					Widget::ParameterExposeButton(parameter_expose_button) => {
						let callback_message = (parameter_expose_button.on_update.callback)(parameter_expose_button);
						responses.add(callback_message);
					}
					Widget::PivotAssist(pivot_assist) => {
						let update_value = value.as_str().expect("RadioInput update was not of type: u64");
						pivot_assist.position = update_value.into();
						let callback_message = (pivot_assist.on_update.callback)(pivot_assist);
						responses.add(callback_message);
					}
					Widget::PopoverButton(_) => {}
					Widget::RadioInput(radio_input) => {
						let update_value = value.as_u64().expect("RadioInput update was not of type: u64");
						radio_input.selected_index = update_value as u32;
						let callback_message = (radio_input.entries[update_value as usize].on_update.callback)(&());
						responses.add(callback_message);
					}
					Widget::Separator(_) => {}
					Widget::SwatchPairInput(_) => {}
					Widget::TextAreaInput(text_area_input) => {
						let update_value = value.as_str().expect("TextAreaInput update was not of type: string");
						text_area_input.value = update_value.into();
						let callback_message = (text_area_input.on_update.callback)(text_area_input);
						responses.add(callback_message);
					}
					Widget::TextButton(text_button) => {
						let callback_message = (text_button.on_update.callback)(text_button);
						responses.add(callback_message);
					}
					Widget::TextInput(text_input) => {
						let update_value = value.as_str().expect("TextInput update was not of type: string");
						text_input.value = update_value.into();
						let callback_message = (text_input.on_update.callback)(text_input);
						responses.add(callback_message);
					}
					Widget::TextLabel(_) => {}
				};
				responses.add(ResendActiveWidget { layout_target, dirty_id: widget_id });
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!()
	}
}

impl LayoutMessageHandler {
	/// Diff the update and send to the frontend where necessary
	fn send_layout(&mut self, layout_target: LayoutTarget, new_layout: Layout, responses: &mut VecDeque<Message>, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) {
		// We don't diff the menu bar layout yet.
		if matches!(new_layout, Layout::MenuLayout(_)) {
			// Skip update if the same
			if self.layouts[layout_target as usize] == new_layout {
				return;
			}
			// Update the backend storage
			self.layouts[layout_target as usize] = new_layout;
			// Update the UI
			responses.add(FrontendMessage::UpdateMenuBarLayout {
				layout_target,
				layout: self.layouts[layout_target as usize].clone().unwrap_menu_layout(action_input_mapping).layout,
			});
			return;
		}

		let mut widget_diffs = Vec::new();
		self.layouts[layout_target as usize].diff(new_layout, &mut Vec::new(), &mut widget_diffs);
		// Skip sending if no diff.
		if widget_diffs.is_empty() {
			return;
		}

		self.send_diff(widget_diffs, layout_target, responses, action_input_mapping);
	}

	/// Send a diff to the frontend based on the layout target.
	#[remain::check]
	fn send_diff(&self, mut diff: Vec<WidgetDiff>, layout_target: LayoutTarget, responses: &mut VecDeque<Message>, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Vec<KeysGroup>) {
		diff.iter_mut().for_each(|diff| diff.new_value.apply_shortcut(action_input_mapping));

		trace!("{layout_target:?} diff {diff:#?}");

		#[remain::sorted]
		let message = match layout_target {
			LayoutTarget::DialogDetails => FrontendMessage::UpdateDialogDetails { layout_target, diff },
			LayoutTarget::DocumentBar => FrontendMessage::UpdateDocumentBarLayout { layout_target, diff },
			LayoutTarget::DocumentMode => FrontendMessage::UpdateDocumentModeLayout { layout_target, diff },
			LayoutTarget::LayerTreeOptions => FrontendMessage::UpdateLayerTreeOptionsLayout { layout_target, diff },
			LayoutTarget::MenuBar => unreachable!("Menu bar is not diffed"),
			LayoutTarget::NodeGraphBar => FrontendMessage::UpdateNodeGraphBarLayout { layout_target, diff },
			LayoutTarget::PropertiesOptions => FrontendMessage::UpdatePropertyPanelOptionsLayout { layout_target, diff },
			LayoutTarget::PropertiesSections => FrontendMessage::UpdatePropertyPanelSectionsLayout { layout_target, diff },
			LayoutTarget::ToolOptions => FrontendMessage::UpdateToolOptionsLayout { layout_target, diff },
			LayoutTarget::ToolShelf => FrontendMessage::UpdateToolShelfLayout { layout_target, diff },
			LayoutTarget::WorkingColors => FrontendMessage::UpdateWorkingColorsLayout { layout_target, diff },

			#[remain::unsorted]
			LayoutTarget::LayoutTargetLength => panic!("`LayoutTargetLength` is not a valid Layout Target and is used for array indexing"),
		};
		responses.add(message);
	}
}
