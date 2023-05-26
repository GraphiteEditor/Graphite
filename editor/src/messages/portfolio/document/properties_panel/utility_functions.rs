use super::utility_types::TransformOp;
use crate::application::generate_uuid;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::assist_widgets::PivotAssist;
use crate::messages::layout::utility_types::widgets::button_widgets::{IconButton, PopoverButton, TextButton};
use crate::messages::layout::utility_types::widgets::input_widgets::{CheckboxInput, ColorInput, NumberInput, NumberInputMode, RadioEntryData, RadioInput, TextInput};
use crate::messages::layout::utility_types::widgets::label_widgets::{IconLabel, TextLabel};
use crate::messages::portfolio::document::node_graph::NodePropertiesContext;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

use document_legacy::document::Document;
use document_legacy::layers::layer_info::{Layer, LayerDataType, LayerDataTypeDiscriminant};
use document_legacy::layers::style::{Fill, Gradient, GradientType, LineCap, LineJoin, RenderData, Stroke, ViewMode};
use graphene_core::raster::color::Color;

use glam::{DAffine2, DVec2};
use std::f64::consts::PI;
use std::sync::Arc;

pub fn apply_transform_operation(layer: &Layer, transform_op: TransformOp, value: f64, render_data: &RenderData) -> [f64; 6] {
	let transformation = match transform_op {
		TransformOp::X => DAffine2::update_x,
		TransformOp::Y => DAffine2::update_y,
		TransformOp::ScaleX | TransformOp::Width => DAffine2::update_scale_x,
		TransformOp::ScaleY | TransformOp::Height => DAffine2::update_scale_y,
		TransformOp::Rotation => DAffine2::update_rotation,
	};

	let scale = match transform_op {
		TransformOp::Width => layer.bounding_transform(render_data).scale_x() / layer.transform.scale_x(),
		TransformOp::Height => layer.bounding_transform(render_data).scale_y() / layer.transform.scale_y(),
		_ => 1.,
	};

	// Apply the operation
	let transform = transformation(layer.transform, value / scale);

	// Return this transform if it is not a dimensions change
	if !matches!(transform_op, TransformOp::ScaleX | TransformOp::Width | TransformOp::ScaleY | TransformOp::Height) {
		return transform.to_cols_array();
	}

	// Find the layerspace pivot
	let pivot = DAffine2::from_translation(layer.transform.transform_point2(layer.layerspace_pivot(render_data)));

	// Find the delta transform
	let mut delta = layer.transform.inverse() * transform;
	if !delta.is_finite() {
		return layer.transform.to_cols_array();
	}

	// Preserve aspect ratio
	if matches!(transform_op, TransformOp::ScaleX | TransformOp::Width) && layer.preserve_aspect {
		let scale_x = layer.transform.scale_x();
		if scale_x != 0. {
			delta = DAffine2::from_scale((1., (value / scale) / scale_x).into()) * delta;
		}
	} else if layer.preserve_aspect {
		let scale_y = layer.transform.scale_y();
		if scale_y != 0. {
			delta = DAffine2::from_scale(((value / scale) / scale_y, 1.).into()) * delta;
		}
	}

	// Transform around pivot
	((pivot * delta * pivot.inverse()) * layer.transform).to_cols_array()
}

pub fn register_artboard_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>, persistent_data: &PersistentData) {
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::IconLabel(IconLabel {
				icon: "NodeArtboard".into(),
				tooltip: "Artboard".into(),
				..Default::default()
			})),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Artboard".into(),
				..TextLabel::default()
			})),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: layer.name.clone().unwrap_or_else(|| "Untitled".to_string()),
				on_update: WidgetCallback::new(|text_input: &TextInput| PropertiesPanelMessage::ModifyName { name: text_input.value.clone() }.into()),
				..Default::default()
			})),
			WidgetHolder::related_separator(),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Options Bar".into(),
				text: "Coming soon".into(),
				..Default::default()
			})),
		],
	}];

	let properties_body = {
		let LayerDataType::Shape(shape) = &layer.data else {
			panic!("Artboards can only be shapes")
		};
		let Fill::Solid(color) = shape.style.fill() else {
			panic!("Artboard must have a solid fill")
		};

		let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::default(), None);
		let pivot = layer.transform.transform_vector2(layer.layerspace_pivot(&render_data));

		vec![LayoutGroup::Section {
			name: "Artboard".into(),
			layout: vec![
				LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Location".into(),
							..TextLabel::default()
						})),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
						WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
						WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						WidgetHolder::unrelated_separator(),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(layer.transform.x() + pivot.x),
							label: "X".into(),
							unit: " px".into(),
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap() - pivot.x,
									transform_op: TransformOp::X,
								}
								.into()
							}),
							..NumberInput::default()
						})),
						WidgetHolder::related_separator(),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(layer.transform.y() + pivot.y),
							label: "Y".into(),
							unit: " px".into(),
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap() - pivot.y,
									transform_op: TransformOp::Y,
								}
								.into()
							}),
							..NumberInput::default()
						})),
					],
				},
				LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Dimensions".into(),
							..TextLabel::default()
						})),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::related_separator(),
						WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
							checked: layer.preserve_aspect,
							icon: "Link".into(),
							tooltip: "Preserve Aspect Ratio".into(),
							on_update: WidgetCallback::new(|input: &CheckboxInput| PropertiesPanelMessage::ModifyPreserveAspect { preserve_aspect: input.checked }.into()),
							..Default::default()
						})),
						WidgetHolder::related_separator(),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(layer.bounding_transform(&render_data).scale_x()),
							label: "W".into(),
							unit: " px".into(),
							is_integer: true,
							min: Some(1.),
							on_update: WidgetCallback::new(|number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap(),
									transform_op: TransformOp::Width,
								}
								.into()
							}),
							..NumberInput::default()
						})),
						WidgetHolder::related_separator(),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(layer.bounding_transform(&render_data).scale_y()),
							label: "H".into(),
							unit: " px".into(),
							is_integer: true,
							min: Some(1.),
							on_update: WidgetCallback::new(|number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap(),
									transform_op: TransformOp::Height,
								}
								.into()
							}),
							..NumberInput::default()
						})),
					],
				},
				LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Background".into(),
							..TextLabel::default()
						})),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
						WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
						WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						WidgetHolder::unrelated_separator(),
						WidgetHolder::new(Widget::ColorInput(ColorInput {
							value: Some(*color),
							on_update: WidgetCallback::new(|text_input: &ColorInput| {
								let fill = if let Some(value) = text_input.value { value } else { Color::TRANSPARENT };
								PropertiesPanelMessage::ModifyFill { fill: Fill::Solid(fill) }.into()
							}),
							..Default::default()
						})),
					],
				},
			],
		}]
	};

	responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
		layout_target: LayoutTarget::PropertiesOptions,
	});
	responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(properties_body)),
		layout_target: LayoutTarget::PropertiesSections,
	});
}

pub fn register_artwork_layer_properties(
	document: &Document,
	layer_path: Vec<document_legacy::LayerId>,
	layer: &Layer,
	responses: &mut VecDeque<Message>,
	persistent_data: &PersistentData,
	node_graph_message_handler: &NodeGraphMessageHandler,
	executor: &mut NodeGraphExecutor,
) {
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			match &layer.data {
				LayerDataType::Folder(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "Folder".into(),
					tooltip: "Folder".into(),
					..Default::default()
				})),
				LayerDataType::Shape(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeShape".into(),
					tooltip: "Shape".into(),
					..Default::default()
				})),
				LayerDataType::Layer(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "Layer".into(),
					tooltip: "Layer".into(),
					..Default::default()
				})),
			},
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: match &layer.data {
					LayerDataType::Layer(_) => "Layer".into(),
					other => LayerDataTypeDiscriminant::from(other).to_string(),
				},
				..TextLabel::default()
			})),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: layer.name.clone().unwrap_or_else(|| "Untitled".to_string()),
				on_update: WidgetCallback::new(|text_input: &TextInput| PropertiesPanelMessage::ModifyName { name: text_input.value.clone() }.into()),
				..Default::default()
			})),
			WidgetHolder::related_separator(),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Options Bar".into(),
				text: "Coming soon".into(),
				..Default::default()
			})),
		],
	}];

	let properties_body = match &layer.data {
		LayerDataType::Shape(shape) => {
			if let Some(fill_layout) = node_section_fill(shape.style.fill()) {
				vec![
					node_section_transform(layer, persistent_data),
					fill_layout,
					node_section_stroke(&shape.style.stroke().unwrap_or_default()),
				]
			} else {
				vec![node_section_transform(layer, persistent_data), node_section_stroke(&shape.style.stroke().unwrap_or_default())]
			}
		}
		LayerDataType::Layer(layer) => {
			let mut properties_sections = Vec::new();

			let mut context = NodePropertiesContext {
				persistent_data,
				document,
				responses,
				nested_path: &node_graph_message_handler.nested_path,
				layer_path: &layer_path,
				executor,
				network: &layer.network,
			};
			node_graph_message_handler.collate_properties(&mut context, &mut properties_sections);

			properties_sections
		}
		LayerDataType::Folder(_) => {
			vec![node_section_transform(layer, persistent_data)]
		}
	};

	responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
		layout_target: LayoutTarget::PropertiesOptions,
	});
	responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(properties_body)),
		layout_target: LayoutTarget::PropertiesSections,
	});
}

pub fn register_document_graph_properties(mut context: NodePropertiesContext, node_graph_message_handler: &NodeGraphMessageHandler) {
	let mut properties_sections = Vec::new();
	node_graph_message_handler.collate_properties(&mut context, &mut properties_sections);
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			IconLabel::new("File").widget_holder(),
			WidgetHolder::unrelated_separator(),
			TextLabel::new("Document graph").widget_holder(),
			WidgetHolder::unrelated_separator(),
			TextInput::new("No layer selected").disabled(true).widget_holder(),
			WidgetHolder::related_separator(),
			PopoverButton::new("Options Bar", "Coming soon").widget_holder(),
		],
	}];

	context.responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
		layout_target: LayoutTarget::PropertiesOptions,
	});
	context.responses.add(LayoutMessage::SendLayout {
		layout: Layout::WidgetLayout(WidgetLayout::new(properties_sections)),
		layout_target: LayoutTarget::PropertiesSections,
	});
}

fn node_section_transform(layer: &Layer, persistent_data: &PersistentData) -> LayoutGroup {
	let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::default(), None);
	let pivot = layer.transform.transform_vector2(layer.layerspace_pivot(&render_data));
	LayoutGroup::Section {
		name: "Transform".into(),
		layout: vec![
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Location".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::PivotAssist(PivotAssist {
						position: layer.pivot.into(),
						on_update: WidgetCallback::new(|pivot_assist: &PivotAssist| PropertiesPanelMessage::SetPivot { new_position: pivot_assist.position }.into()),
						..Default::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.transform.x() + pivot.x),
						label: "X".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.x,
								transform_op: TransformOp::X,
							}
							.into()
						}),
						..NumberInput::default()
					})),
					WidgetHolder::related_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.transform.y() + pivot.y),
						label: "Y".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.y,
								transform_op: TransformOp::Y,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Rotation".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.transform.rotation() * 180. / PI),
						unit: "°".into(),
						mode: NumberInputMode::Range,
						range_min: Some(-180.),
						range_max: Some(180.),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() / 180. * PI,
								transform_op: TransformOp::Rotation,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Scale".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::related_separator(),
					WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
						checked: layer.preserve_aspect,
						icon: "Link".into(),
						tooltip: "Preserve Aspect Ratio".into(),
						on_update: WidgetCallback::new(|input: &CheckboxInput| PropertiesPanelMessage::ModifyPreserveAspect { preserve_aspect: input.checked }.into()),
						..Default::default()
					})),
					WidgetHolder::related_separator(),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.transform.scale_x()),
						label: "X".into(),
						unit: "".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::ScaleX,
							}
							.into()
						}),
						..NumberInput::default()
					})),
					WidgetHolder::related_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.transform.scale_y()),
						label: "Y".into(),
						unit: "".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::ScaleY,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Dimensions".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.bounding_transform(&render_data).scale_x()),
						label: "W".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::Width,
							}
							.into()
						}),
						..NumberInput::default()
					})),
					WidgetHolder::related_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.bounding_transform(&render_data).scale_y()),
						label: "H".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::Height,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
		],
	}
}

fn node_gradient_type(gradient: &Gradient) -> LayoutGroup {
	let selected_index = match gradient.gradient_type {
		GradientType::Linear => 0,
		GradientType::Radial => 1,
	};
	let mut cloned_gradient_linear = gradient.clone();
	cloned_gradient_linear.gradient_type = GradientType::Linear;
	let mut cloned_gradient_radial = gradient.clone();
	cloned_gradient_radial.gradient_type = GradientType::Radial;
	LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Gradient Type".into(),
				..TextLabel::default()
			})),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
			WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
			WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::RadioInput(RadioInput {
				selected_index,
				entries: vec![
					RadioEntryData {
						value: "linear".into(),
						label: "Linear".into(),
						tooltip: "Linear gradient changes colors from one side to the other along a line".into(),
						on_update: WidgetCallback::new(move |_| {
							PropertiesPanelMessage::ModifyFill {
								fill: Fill::Gradient(cloned_gradient_linear.clone()),
							}
							.into()
						}),
						..RadioEntryData::default()
					},
					RadioEntryData {
						value: "radial".into(),
						label: "Radial".into(),
						tooltip: "Radial gradient changes colors from the inside to the outside of a circular area".into(),
						on_update: WidgetCallback::new(move |_| {
							PropertiesPanelMessage::ModifyFill {
								fill: Fill::Gradient(cloned_gradient_radial.clone()),
							}
							.into()
						}),
						..RadioEntryData::default()
					},
				],
				..Default::default()
			})),
		],
	}
}

fn node_gradient_color(gradient: &Gradient, position: usize) -> LayoutGroup {
	let gradient_clone = Arc::new(gradient.clone());
	let gradient_2 = gradient_clone.clone();
	let gradient_3 = gradient_clone.clone();
	let send_fill_message = move |new_gradient: Gradient| PropertiesPanelMessage::ModifyFill { fill: Fill::Gradient(new_gradient) }.into();

	let value = format!("Gradient: {:.0}%", gradient_clone.positions[position].0 * 100.);
	let mut widgets = vec![
		WidgetHolder::new(Widget::TextLabel(TextLabel {
			value,
			tooltip: "Adjustable by dragging the gradient stops in the viewport with the Gradient tool active".into(),
			..TextLabel::default()
		})),
		WidgetHolder::unrelated_separator(),
		WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
		WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
		WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
		WidgetHolder::unrelated_separator(),
		WidgetHolder::new(Widget::ColorInput(ColorInput {
			value: gradient_clone.positions[position].1,
			on_update: WidgetCallback::new(move |text_input: &ColorInput| {
				let mut new_gradient = (*gradient_clone).clone();
				new_gradient.positions[position].1 = text_input.value;
				send_fill_message(new_gradient)
			}),
			..ColorInput::default()
		})),
	];

	let mut skip_separator = false;
	// Remove button
	if gradient.positions.len() != position + 1 && position != 0 {
		let on_update = WidgetCallback::new(move |_| {
			let mut new_gradient = (*gradient_3).clone();
			new_gradient.positions.remove(position);
			send_fill_message(new_gradient)
		});

		skip_separator = true;
		widgets.push(WidgetHolder::related_separator());
		widgets.push(WidgetHolder::new(Widget::IconButton(IconButton {
			icon: "Remove".to_string(),
			tooltip: "Remove this gradient stop".to_string(),
			size: 16,
			on_update,
			..Default::default()
		})));
	}
	// Add button
	if gradient.positions.len() != position + 1 {
		let on_update = WidgetCallback::new(move |_| {
			let mut gradient = (*gradient_2).clone();

			let get_color = |index: usize| match (gradient.positions[index].1, gradient.positions.get(index + 1).and_then(|x| x.1)) {
				(Some(a), Some(b)) => Color::from_rgbaf32((a.r() + b.r()) / 2., (a.g() + b.g()) / 2., (a.b() + b.b()) / 2., ((a.a() + b.a()) / 2.).clamp(0., 1.)),
				(Some(v), _) | (_, Some(v)) => Some(v),
				_ => Some(Color::WHITE),
			};
			let get_pos = |index: usize| (gradient.positions[index].0 + gradient.positions.get(index + 1).map(|v| v.0).unwrap_or(1.)) / 2.;

			gradient.positions.push((get_pos(position), get_color(position)));

			gradient.positions.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

			send_fill_message(gradient)
		});

		if !skip_separator {
			widgets.push(WidgetHolder::related_separator());
		}
		widgets.push(WidgetHolder::new(Widget::IconButton(IconButton {
			icon: "Add".to_string(),
			tooltip: "Add a gradient stop after this".to_string(),
			size: 16,
			on_update,
			..Default::default()
		})));
	}
	LayoutGroup::Row { widgets }
}

fn node_section_fill(fill: &Fill) -> Option<LayoutGroup> {
	let initial_color = if let Fill::Solid(color) = fill { *color } else { Color::BLACK };

	match fill {
		Fill::Solid(_) | Fill::None => Some(LayoutGroup::Section {
			name: "Fill".into(),
			layout: vec![
				LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Color".into(),
							..TextLabel::default()
						})),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
						WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
						WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						WidgetHolder::unrelated_separator(),
						WidgetHolder::new(Widget::ColorInput(ColorInput {
							value: if let Fill::Solid(color) = fill { Some(*color) } else { None },
							on_update: WidgetCallback::new(|text_input: &ColorInput| {
								let fill = if let Some(value) = text_input.value { Fill::Solid(value) } else { Fill::None };
								PropertiesPanelMessage::ModifyFill { fill }.into()
							}),
							..ColorInput::default()
						})),
					],
				},
				LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "".into(),
							..TextLabel::default()
						})),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
						WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
						WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						WidgetHolder::unrelated_separator(),
						WidgetHolder::new(Widget::TextButton(TextButton {
							label: "Use Gradient".into(),
							tooltip: "Change this fill from a solid color to a gradient".into(),
							on_update: WidgetCallback::new(move |_: &TextButton| {
								let (r, g, b, _) = initial_color.components();
								let opposite_color = Color::from_rgbaf32(1. - r, 1. - g, 1. - b, 1.).unwrap();

								PropertiesPanelMessage::ModifyFill {
									fill: Fill::Gradient(Gradient::new(
										DVec2::new(0., 0.5),
										initial_color,
										DVec2::new(1., 0.5),
										opposite_color,
										DAffine2::IDENTITY,
										generate_uuid(),
										GradientType::Linear,
									)),
								}
								.into()
							}),
							..TextButton::default()
						})),
					],
				},
			],
		}),
		Fill::Gradient(gradient) => Some(LayoutGroup::Section {
			name: "Fill".into(),
			layout: {
				let cloned_gradient = gradient.clone();
				let first_color = gradient.positions.get(0).unwrap_or(&(0., None)).1;

				let mut layout = vec![node_gradient_type(gradient)];
				layout.extend((0..gradient.positions.len()).map(|pos| node_gradient_color(gradient, pos)));
				layout.push(LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "".into(),
							..TextLabel::default()
						})),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
						WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
						WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						WidgetHolder::unrelated_separator(),
						WidgetHolder::new(Widget::TextButton(TextButton {
							label: "Invert".into(),
							icon: Some("Swap".into()),
							tooltip: "Reverse the order of each color stop".into(),
							on_update: WidgetCallback::new(move |_: &TextButton| {
								let mut new_gradient = cloned_gradient.clone();
								new_gradient.positions = new_gradient.positions.iter().map(|(distance, color)| (1. - distance, *color)).collect();
								new_gradient.positions.reverse();
								PropertiesPanelMessage::ModifyFill { fill: Fill::Gradient(new_gradient) }.into()
							}),
							..TextButton::default()
						})),
					],
				});
				layout.push(LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "".into(),
							..TextLabel::default()
						})),
						WidgetHolder::unrelated_separator(),
						WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
						WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
						WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						WidgetHolder::unrelated_separator(),
						WidgetHolder::new(Widget::TextButton(TextButton {
							label: "Use Solid Color".into(),
							tooltip: "Change this fill from a gradient to a solid color, keeping the 0% stop color".into(),
							on_update: WidgetCallback::new(move |_: &TextButton| {
								PropertiesPanelMessage::ModifyFill {
									fill: Fill::Solid(first_color.unwrap_or_default()),
								}
								.into()
							}),
							..TextButton::default()
						})),
					],
				});
				layout
			},
		}),
	}
}

fn node_section_stroke(stroke: &Stroke) -> LayoutGroup {
	// We have to make multiple variables because they get moved into different closures.
	let internal_stroke1 = stroke.clone();
	let internal_stroke2 = stroke.clone();
	let internal_stroke3 = stroke.clone();
	let internal_stroke4 = stroke.clone();
	let internal_stroke5 = stroke.clone();
	let internal_stroke6 = stroke.clone();
	let internal_stroke7 = stroke.clone();
	let internal_stroke8 = stroke.clone();
	let internal_stroke9 = stroke.clone();
	let internal_stroke10 = stroke.clone();
	let internal_stroke11 = stroke.clone();

	LayoutGroup::Section {
		name: "Stroke".into(),
		layout: vec![
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Color".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::ColorInput(ColorInput {
						value: stroke.color(),
						on_update: WidgetCallback::new(move |text_input: &ColorInput| {
							internal_stroke1
								.clone()
								.with_color(&text_input.value)
								.map_or(PropertiesPanelMessage::ResendActiveProperties.into(), |stroke| PropertiesPanelMessage::ModifyStroke { stroke }.into())
						}),
						..ColorInput::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Weight".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(stroke.weight()),
						is_integer: false,
						min: Some(0.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke2.clone().with_weight(number_input.value.unwrap()),
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Dash Lengths".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::TextInput(TextInput {
						value: stroke.dash_lengths(),
						centered: true,
						on_update: WidgetCallback::new(move |text_input: &TextInput| {
							internal_stroke3
								.clone()
								.with_dash_lengths(&text_input.value)
								.map_or(PropertiesPanelMessage::ResendActiveProperties.into(), |stroke| PropertiesPanelMessage::ModifyStroke { stroke }.into())
						}),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Dash Offset".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(stroke.dash_offset()),
						is_integer: true,
						min: Some(0.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke4.clone().with_dash_offset(number_input.value.unwrap()),
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Line Cap".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::RadioInput(RadioInput {
						selected_index: stroke.line_cap_index(),
						entries: vec![
							RadioEntryData {
								label: "Butt".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke6.clone().with_line_cap(LineCap::Butt),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Round".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke7.clone().with_line_cap(LineCap::Round),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Square".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke8.clone().with_line_cap(LineCap::Square),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
						],
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Line Join".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::RadioInput(RadioInput {
						selected_index: stroke.line_join_index(),
						entries: vec![
							RadioEntryData {
								label: "Miter".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke9.clone().with_line_join(LineJoin::Miter),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Bevel".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke10.clone().with_line_join(LineJoin::Bevel),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Round".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke11.clone().with_line_join(LineJoin::Round),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
						],
						..Default::default()
					})),
				],
			},
			// TODO: Gray out this row when Line Join isn't set to Miter
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Miter Limit".into(),
						..TextLabel::default()
					})),
					WidgetHolder::unrelated_separator(),
					WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
					WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
					WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(stroke.line_join_miter_limit() as f64),
						is_integer: true,
						min: Some(0.),
						unit: "".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke5.clone().with_line_join_miter_limit(number_input.value.unwrap()),
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
		],
	}
}

trait DAffine2Utils {
	fn scale_x(&self) -> f64;
	fn update_scale_x(self, new_width: f64) -> Self;
	fn scale_y(&self) -> f64;
	fn update_scale_y(self, new_height: f64) -> Self;
	fn x(&self) -> f64;
	fn update_x(self, new_x: f64) -> Self;
	fn y(&self) -> f64;
	fn update_y(self, new_y: f64) -> Self;
	fn rotation(&self) -> f64;
	fn update_rotation(self, new_rotation: f64) -> Self;
}

impl DAffine2Utils for DAffine2 {
	fn scale_x(&self) -> f64 {
		self.transform_vector2((1., 0.).into()).length()
	}

	fn update_scale_x(self, new_width: f64) -> Self {
		let scale_x = self.scale_x();
		if scale_x != 0. {
			self * DAffine2::from_scale((new_width / scale_x, 1.).into())
		} else {
			self
		}
	}

	fn scale_y(&self) -> f64 {
		self.transform_vector2((0., 1.).into()).length()
	}

	fn update_scale_y(self, new_height: f64) -> Self {
		let scale_y = self.scale_y();
		if scale_y != 0. {
			self * DAffine2::from_scale((1., new_height / scale_y).into())
		} else {
			self
		}
	}

	fn x(&self) -> f64 {
		self.translation.x
	}

	fn update_x(mut self, new_x: f64) -> Self {
		self.translation.x = new_x;
		self
	}

	fn y(&self) -> f64 {
		self.translation.y
	}

	fn update_y(mut self, new_y: f64) -> Self {
		self.translation.y = new_y;
		self
	}

	fn rotation(&self) -> f64 {
		if self.scale_x() != 0. {
			let cos = self.matrix2.col(0).x / self.scale_x();
			let sin = self.matrix2.col(0).y / self.scale_x();
			sin.atan2(cos)
		} else if self.scale_y() != 0. {
			let sin = -self.matrix2.col(1).x / self.scale_y();
			let cos = self.matrix2.col(1).y / self.scale_y();
			sin.atan2(cos)
		} else {
			// Rotation information does not exists anymore in the matrix
			// return 0 for user experience.
			0.
		}
	}

	fn update_rotation(self, new_rotation: f64) -> Self {
		let width = self.scale_x();
		let height = self.scale_y();
		let half_width = width / 2.;
		let half_height = height / 2.;

		let angle_translation_offset = |angle: f64| DVec2::new(-half_width * angle.cos() + half_height * angle.sin(), -half_width * angle.sin() - half_height * angle.cos());
		let angle_translation_adjustment = angle_translation_offset(new_rotation) - angle_translation_offset(self.rotation());

		DAffine2::from_scale_angle_translation((width, height).into(), new_rotation, self.translation + angle_translation_adjustment)
	}
}
