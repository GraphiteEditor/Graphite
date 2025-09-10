use crate::messages::input_mapper::utility_types::input_keyboard::KeysGroup;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;
use graphene_std::raster::color::Color;
use graphene_std::text::Font;
use graphene_std::vector::style::{FillChoice, GradientStops};
use serde_json::Value;

#[derive(ExtractField)]
pub struct LayoutMessageContext<'a> {
	pub action_input_mapping: &'a dyn Fn(&MessageDiscriminant) -> Option<KeysGroup>,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct LayoutMessageHandler {
	layouts: [Layout; LayoutTarget::LayoutTargetLength as usize],
}

#[message_handler_data]
impl MessageHandler<LayoutMessage, LayoutMessageContext<'_>> for LayoutMessageHandler {
	fn process_message(&mut self, message: LayoutMessage, responses: &mut std::collections::VecDeque<Message>, context: LayoutMessageContext) {
		let action_input_mapping = &context.action_input_mapping;

		match message {
			LayoutMessage::ResendActiveWidget { layout_target, widget_id } => {
				// Find the updated diff based on the specified layout target
				let Some(diff) = (match &self.layouts[layout_target as usize] {
					Layout::MenuLayout(_) => return,
					Layout::WidgetLayout(layout) => Self::get_widget_path(layout, widget_id).map(|(widget, widget_path)| {
						// Create a widget update diff for the relevant id
						let new_value = DiffUpdate::Widget(widget.clone());
						WidgetDiff { widget_path, new_value }
					}),
				}) else {
					return;
				};
				// Resend that diff
				self.send_diff(vec![diff], layout_target, responses, action_input_mapping);
			}
			LayoutMessage::SendLayout { layout, layout_target } => {
				self.diff_and_send_layout_to_frontend(layout_target, layout, responses, action_input_mapping);
			}
			LayoutMessage::WidgetValueCommit { layout_target, widget_id, value } => {
				self.handle_widget_callback(layout_target, widget_id, value, WidgetValueAction::Commit, responses);
			}
			LayoutMessage::WidgetValueUpdate { layout_target, widget_id, value } => {
				self.handle_widget_callback(layout_target, widget_id, value, WidgetValueAction::Update, responses);
				responses.add(LayoutMessage::ResendActiveWidget { layout_target, widget_id });
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(LayoutMessageDiscriminant;)
	}
}

impl LayoutMessageHandler {
	/// Get the widget path for the widget with the specified id
	fn get_widget_path(widget_layout: &WidgetLayout, widget_id: WidgetId) -> Option<(&WidgetHolder, Vec<usize>)> {
		let mut stack = widget_layout.layout.iter().enumerate().map(|(index, val)| (vec![index], val)).collect::<Vec<_>>();
		while let Some((mut widget_path, layout_group)) = stack.pop() {
			match layout_group {
				// Check if any of the widgets in the current column or row have the correct id
				LayoutGroup::Column { widgets } | LayoutGroup::Row { widgets } => {
					for (index, widget) in widgets.iter().enumerate() {
						// Return if this is the correct ID
						if widget.widget_id == widget_id {
							widget_path.push(index);
							return Some((widget, widget_path));
						}

						if let Widget::PopoverButton(popover) = &widget.widget {
							stack.extend(popover.popover_layout.iter().enumerate().map(|(child, val)| ([widget_path.as_slice(), &[index, child]].concat(), val)));
						}
					}
				}
				// A section contains more LayoutGroups which we add to the stack.
				LayoutGroup::Section { layout, .. } => {
					stack.extend(layout.iter().enumerate().map(|(index, val)| ([widget_path.as_slice(), &[index]].concat(), val)));
				}
				LayoutGroup::Table { rows } => {
					for (row_index, row) in rows.iter().enumerate() {
						for (cell_index, cell) in row.iter().enumerate() {
							// Return if this is the correct ID
							if cell.widget_id == widget_id {
								widget_path.push(row_index);
								widget_path.push(cell_index);
								return Some((cell, widget_path));
							}

							if let Widget::PopoverButton(popover) = &cell.widget {
								stack.extend(
									popover
										.popover_layout
										.iter()
										.enumerate()
										.map(|(child, val)| ([widget_path.as_slice(), &[row_index, cell_index, child]].concat(), val)),
								);
							}
						}
					}
				}
			}
		}
		None
	}

	fn handle_widget_callback(&mut self, layout_target: LayoutTarget, widget_id: WidgetId, value: Value, action: WidgetValueAction, responses: &mut std::collections::VecDeque<Message>) {
		let Some(layout) = self.layouts.get_mut(layout_target as usize) else {
			warn!("handle_widget_callback was called referencing an invalid layout. `widget_id: {widget_id}`, `layout_target: {layout_target:?}`",);
			return;
		};

		let Some(widget_holder) = layout.iter_mut().find(|widget| widget.widget_id == widget_id) else {
			warn!("handle_widget_callback was called referencing an invalid widget ID, although the layout target was valid. `widget_id: {widget_id}`, `layout_target: {layout_target:?}`",);
			return;
		};

		match &mut widget_holder.widget {
			Widget::BreadcrumbTrailButtons(breadcrumb_trail_buttons) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (breadcrumb_trail_buttons.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						let Some(update_value) = value.as_u64() else {
							error!("BreadcrumbTrailButtons update was not of type: u64");
							return;
						};
						(breadcrumb_trail_buttons.on_update.callback)(&update_value)
					}
				};
				responses.add(callback_message);
			}
			Widget::CheckboxInput(checkbox_input) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (checkbox_input.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						let Some(update_value) = value.as_bool() else {
							error!("CheckboxInput update was not of type: bool");
							return;
						};
						checkbox_input.checked = update_value;
						(checkbox_input.on_update.callback)(checkbox_input)
					}
				};
				responses.add(callback_message);
			}
			Widget::ColorInput(color_button) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (color_button.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						// Decodes the colors in gamma, not linear
						let decode_color = |color: &serde_json::map::Map<String, serde_json::value::Value>| -> Option<Color> {
							let red = color.get("red").and_then(|x| x.as_f64()).map(|x| x as f32);
							let green = color.get("green").and_then(|x| x.as_f64()).map(|x| x as f32);
							let blue = color.get("blue").and_then(|x| x.as_f64()).map(|x| x as f32);
							let alpha = color.get("alpha").and_then(|x| x.as_f64()).map(|x| x as f32);

							if let (Some(red), Some(green), Some(blue), Some(alpha)) = (red, green, blue, alpha) {
								if let Some(color) = Color::from_rgbaf32(red, green, blue, alpha) {
									return Some(color);
								}
							}
							None
						};

						(|| {
							let Some(update_value) = value.as_object() else {
								warn!("ColorInput update was not of type: object");
								return Message::NoOp;
							};

							// None
							let is_none = update_value.get("none").and_then(|x| x.as_bool());
							if is_none == Some(true) {
								color_button.value = FillChoice::None;
								return (color_button.on_update.callback)(color_button);
							}

							// Solid
							if let Some(color) = decode_color(update_value) {
								color_button.value = FillChoice::Solid(color);
								return (color_button.on_update.callback)(color_button);
							}

							// Gradient
							let gradient = update_value.get("stops").and_then(|x| x.as_array());
							if let Some(stops) = gradient {
								let gradient_stops = stops
									.iter()
									.filter_map(|stop| {
										stop.as_object().and_then(|stop| {
											let position = stop.get("position").and_then(|x| x.as_f64());
											let color = stop.get("color").and_then(|x| x.as_object()).and_then(decode_color);
											if let (Some(position), Some(color)) = (position, color) { Some((position, color)) } else { None }
										})
									})
									.collect::<Vec<_>>();

								color_button.value = FillChoice::Gradient(GradientStops::new(gradient_stops));
								return (color_button.on_update.callback)(color_button);
							}

							warn!("ColorInput update was not able to be parsed with color data: {color_button:?}");
							Message::NoOp
						})()
					}
				};

				responses.add(callback_message);
			}
			Widget::CurveInput(curve_input) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (curve_input.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						let Some(curve) = serde_json::from_value(value).ok() else {
							error!("CurveInput event data could not be deserialized");
							return;
						};
						curve_input.value = curve;
						(curve_input.on_update.callback)(curve_input)
					}
				};

				responses.add(callback_message);
			}
			Widget::DropdownInput(dropdown_input) => {
				let callback_message = match action {
					WidgetValueAction::Commit => {
						let Some(update_value) = value.as_u64() else {
							error!("DropdownInput commit was not of type `u64`, found {value:?}");
							return;
						};
						let Some(entry) = dropdown_input.entries.iter().flatten().nth(update_value as usize) else {
							error!("DropdownInput commit was not able to find entry for index {update_value}");
							return;
						};
						(entry.on_commit.callback)(&())
					}
					WidgetValueAction::Update => {
						let Some(update_value) = value.as_u64() else {
							error!("DropdownInput update was not of type `u64`, found {value:?}");
							return;
						};
						dropdown_input.selected_index = Some(update_value as u32);
						let Some(entry) = dropdown_input.entries.iter().flatten().nth(update_value as usize) else {
							error!("DropdownInput update was not able to find entry for index {update_value}");
							return;
						};
						(entry.on_update.callback)(&())
					}
				};

				responses.add(callback_message);
			}
			Widget::FontInput(font_input) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (font_input.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						let Some(update_value) = value.as_object() else {
							error!("FontInput update was not of type: object");
							return;
						};
						let Some(font_family_value) = update_value.get("fontFamily") else {
							error!("FontInput update does not have a fontFamily");
							return;
						};
						let Some(font_style_value) = update_value.get("fontStyle") else {
							error!("FontInput update does not have a fontStyle");
							return;
						};

						let Some(font_family) = font_family_value.as_str() else {
							error!("FontInput update fontFamily was not of type: string");
							return;
						};
						let Some(font_style) = font_style_value.as_str() else {
							error!("FontInput update fontStyle was not of type: string");
							return;
						};

						font_input.font_family = font_family.into();
						font_input.font_style = font_style.into();

						responses.add(PortfolioMessage::LoadFont {
							font: Font::new(font_family.into(), font_style.into()),
						});
						(font_input.on_update.callback)(font_input)
					}
				};

				responses.add(callback_message);
			}
			Widget::IconButton(icon_button) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (icon_button.on_commit.callback)(&()),
					WidgetValueAction::Update => (icon_button.on_update.callback)(icon_button),
				};

				responses.add(callback_message);
			}
			Widget::ImageButton(image_label) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (image_label.on_commit.callback)(&()),
					WidgetValueAction::Update => (image_label.on_update.callback)(&()),
				};

				responses.add(callback_message);
			}
			Widget::ImageLabel(_) => {}
			Widget::IconLabel(_) => {}
			Widget::InvisibleStandinInput(invisible) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (invisible.on_commit.callback)(&()),
					WidgetValueAction::Update => (invisible.on_update.callback)(&()),
				};

				responses.add(callback_message);
			}
			Widget::NodeCatalog(node_type_input) => match action {
				WidgetValueAction::Commit => {
					let callback_message = (node_type_input.on_commit.callback)(&());
					responses.add(callback_message);
				}
				WidgetValueAction::Update => {
					let Some(value) = value.as_str().map(|s| s.to_string()) else {
						error!("NodeCatalog update was not of type String");
						return;
					};
					let callback_message = (node_type_input.on_update.callback)(&value);
					responses.add(callback_message);
				}
			},
			Widget::NumberInput(number_input) => match action {
				WidgetValueAction::Commit => {
					let callback_message = (number_input.on_commit.callback)(&());
					responses.add(callback_message);
				}
				WidgetValueAction::Update => match value {
					Value::Number(ref num) => {
						let Some(update_value) = num.as_f64() else {
							error!("NumberInput update was not of type: f64, found {value:?}");
							return;
						};
						number_input.value = Some(update_value);
						let callback_message = (number_input.on_update.callback)(number_input);
						responses.add(callback_message);
					}
					// TODO: This crashes when the cursor is in a text box, such as in the Text node, and the transform node is clicked (https://github.com/GraphiteEditor/Graphite/issues/1761)
					Value::String(str) => match str.as_str() {
						"Increment" => responses.add((number_input.increment_callback_increase.callback)(number_input)),
						"Decrement" => responses.add((number_input.increment_callback_decrease.callback)(number_input)),
						_ => panic!("Invalid string found when updating `NumberInput`"),
					},
					_ => {}
				},
			},
			Widget::ParameterExposeButton(parameter_expose_button) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (parameter_expose_button.on_commit.callback)(&()),
					WidgetValueAction::Update => (parameter_expose_button.on_update.callback)(parameter_expose_button),
				};

				responses.add(callback_message);
			}
			Widget::ReferencePointInput(reference_point_input) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (reference_point_input.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						let Some(update_value) = value.as_str() else {
							error!("ReferencePointInput update was not of type: u64");
							return;
						};
						reference_point_input.value = update_value.into();
						(reference_point_input.on_update.callback)(reference_point_input)
					}
				};

				responses.add(callback_message);
			}
			Widget::PopoverButton(_) => {}
			Widget::RadioInput(radio_input) => {
				let Some(update_value) = value.as_u64() else {
					error!("RadioInput update was not of type: u64");
					return;
				};
				radio_input.selected_index = Some(update_value as u32);
				let callback_message = match action {
					WidgetValueAction::Commit => (radio_input.entries[update_value as usize].on_commit.callback)(&()),
					WidgetValueAction::Update => (radio_input.entries[update_value as usize].on_update.callback)(&()),
				};

				responses.add(callback_message);
			}
			Widget::Separator(_) => {}
			Widget::TextAreaInput(text_area_input) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (text_area_input.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						let Some(update_value) = value.as_str() else {
							error!("TextAreaInput update was not of type: string");
							return;
						};
						text_area_input.value = update_value.into();
						(text_area_input.on_update.callback)(text_area_input)
					}
				};

				responses.add(callback_message);
			}
			Widget::TextButton(text_button) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (text_button.on_commit.callback)(&()),
					WidgetValueAction::Update => (text_button.on_update.callback)(text_button),
				};

				responses.add(callback_message);
			}
			Widget::TextInput(text_input) => {
				let callback_message = match action {
					WidgetValueAction::Commit => (text_input.on_commit.callback)(&()),
					WidgetValueAction::Update => {
						let Some(update_value) = value.as_str() else {
							error!("TextInput update was not of type: string");
							return;
						};
						text_input.value = update_value.into();
						(text_input.on_update.callback)(text_input)
					}
				};

				responses.add(callback_message);
			}
			Widget::TextLabel(_) => {}
			Widget::WorkingColorsInput(_) => {}
		};
	}

	/// Diff the update and send to the frontend where necessary
	fn diff_and_send_layout_to_frontend(
		&mut self,
		layout_target: LayoutTarget,
		new_layout: Layout,
		responses: &mut VecDeque<Message>,
		action_input_mapping: &impl Fn(&MessageDiscriminant) -> Option<KeysGroup>,
	) {
		match new_layout {
			Layout::WidgetLayout(_) => {
				let mut widget_diffs = Vec::new();
				self.layouts[layout_target as usize].diff(new_layout, &mut Vec::new(), &mut widget_diffs);

				// Skip sending if no diff.
				if widget_diffs.is_empty() {
					return;
				}

				self.send_diff(widget_diffs, layout_target, responses, action_input_mapping);
			}
			// We don't diff the menu bar layout yet.
			Layout::MenuLayout(_) => {
				// Skip update if the same
				if self.layouts[layout_target as usize] == new_layout {
					return;
				}

				// Update the backend storage
				self.layouts[layout_target as usize] = new_layout;

				// Update the UI
				let Some(layout) = self.layouts[layout_target as usize].clone().as_menu_layout(action_input_mapping).map(|x| x.layout) else {
					error!("Called unwrap_menu_layout on a widget layout");
					return;
				};
				responses.add(FrontendMessage::UpdateMenuBarLayout { layout_target, layout });
			}
		}
	}

	/// Send a diff to the frontend based on the layout target.
	fn send_diff(&self, mut diff: Vec<WidgetDiff>, layout_target: LayoutTarget, responses: &mut VecDeque<Message>, action_input_mapping: &impl Fn(&MessageDiscriminant) -> Option<KeysGroup>) {
		diff.iter_mut().for_each(|diff| diff.new_value.apply_keyboard_shortcut(action_input_mapping));

		let message = match layout_target {
			LayoutTarget::MenuBar => unreachable!("Menu bar is not diffed"),
			LayoutTarget::DialogButtons => FrontendMessage::UpdateDialogButtons { layout_target, diff },
			LayoutTarget::DialogColumn1 => FrontendMessage::UpdateDialogColumn1 { layout_target, diff },
			LayoutTarget::DialogColumn2 => FrontendMessage::UpdateDialogColumn2 { layout_target, diff },
			LayoutTarget::DocumentBar => FrontendMessage::UpdateDocumentBarLayout { layout_target, diff },
			LayoutTarget::DocumentMode => FrontendMessage::UpdateDocumentModeLayout { layout_target, diff },
			LayoutTarget::DataPanel => FrontendMessage::UpdateDataPanelLayout { layout_target, diff },
			LayoutTarget::LayersPanelControlLeftBar => FrontendMessage::UpdateLayersPanelControlBarLeftLayout { layout_target, diff },
			LayoutTarget::LayersPanelControlRightBar => FrontendMessage::UpdateLayersPanelControlBarRightLayout { layout_target, diff },
			LayoutTarget::LayersPanelBottomBar => FrontendMessage::UpdateLayersPanelBottomBarLayout { layout_target, diff },
			LayoutTarget::PropertiesPanel => FrontendMessage::UpdatePropertiesPanelLayout { layout_target, diff },
			LayoutTarget::NodeGraphControlBar => FrontendMessage::UpdateNodeGraphControlBarLayout { layout_target, diff },
			LayoutTarget::ToolOptions => FrontendMessage::UpdateToolOptionsLayout { layout_target, diff },
			LayoutTarget::ToolShelf => FrontendMessage::UpdateToolShelfLayout { layout_target, diff },
			LayoutTarget::WorkingColors => FrontendMessage::UpdateWorkingColorsLayout { layout_target, diff },

			LayoutTarget::LayoutTargetLength => panic!("`LayoutTargetLength` is not a valid Layout Target and is used for array indexing"),
		};
		responses.add(message);
	}
}

enum WidgetValueAction {
	Commit,
	Update,
}
