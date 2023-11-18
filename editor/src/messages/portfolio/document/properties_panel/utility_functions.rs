use super::utility_types::TransformOp;
use crate::application::generate_uuid;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::NodePropertiesContext;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

use document_legacy::document::Document;
use document_legacy::layers::layer_info::{Layer, LayerDataType};
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
				LayerDataType::Folder(_) => IconLabel::new("Folder").tooltip("Folder").widget_holder(),
				LayerDataType::Shape(_) => IconLabel::new("NodeShape").tooltip("Shape").widget_holder(),
				LayerDataType::Layer(_) => IconLabel::new("Layer").tooltip("Layer").widget_holder(),
			},
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(layer.name.clone().unwrap_or_else(|| "Untitled Layer".to_string()))
				.on_update(|text_input: &TextInput| PropertiesPanelMessage::ModifyName { name: text_input.value.clone() }.into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			PopoverButton::new("Additional Options", "Coming soon").widget_holder(),
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
				nested_path: &node_graph_message_handler.network,
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

pub fn register_document_graph_properties(mut context: NodePropertiesContext, node_graph_message_handler: &NodeGraphMessageHandler, document_name: &str) {
	let mut properties_sections = Vec::new();
	node_graph_message_handler.collate_properties(&mut context, &mut properties_sections);
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			IconLabel::new("File").tooltip("Document").widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(document_name)
				.on_update(|text_input| DocumentMessage::RenameDocument { new_name: text_input.value.clone() }.into())
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			PopoverButton::new("Additional Options", "Coming soon").widget_holder(),
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
					TextLabel::new("Location").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					PivotInput::new(layer.pivot.into())
						.on_update(|pivot_input: &PivotInput| PropertiesPanelMessage::SetPivot { new_position: pivot_input.position }.into())
						.widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.transform.x() + pivot.x))
						.label("X")
						.unit(" px")
						.min(-((1u64 << std::f64::MANTISSA_DIGITS) as f64))
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.x,
								transform_op: TransformOp::X,
							}
							.into()
						})
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(layer.transform.y() + pivot.y))
						.label("Y")
						.unit(" px")
						.min(-((1u64 << std::f64::MANTISSA_DIGITS) as f64))
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.y,
								transform_op: TransformOp::Y,
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Rotation").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.transform.rotation() * 180. / PI))
						.unit("°")
						.mode(NumberInputMode::Range)
						.range_min(Some(-180.))
						.range_max(Some(180.))
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() / 180. * PI,
								transform_op: TransformOp::Rotation,
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Scale").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					CheckboxInput::new(layer.preserve_aspect)
						.icon("Link")
						.tooltip("Preserve Aspect Ratio")
						.on_update(|input: &CheckboxInput| PropertiesPanelMessage::ModifyPreserveAspect { preserve_aspect: input.checked }.into())
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.transform.scale_x()))
						.label("X")
						.unit("")
						.min(0.)
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::ScaleX,
							}
							.into()
						})
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(layer.transform.scale_y()))
						.label("Y")
						.unit("")
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::ScaleY,
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Dimensions").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(layer.bounding_transform(&render_data).scale_x()))
						.label("W")
						.unit(" px")
						.max((1u64 << f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::Width,
							}
							.into()
						})
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(layer.bounding_transform(&render_data).scale_y()))
						.label("H")
						.unit(" px")
						.max((1u64 << f64::MANTISSA_DIGITS) as f64)
						.on_update(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::Height,
							}
							.into()
						})
						.widget_holder(),
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
			TextLabel::new("Gradient Type").widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
			Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
			Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(vec![
				RadioEntryData::new("linear")
					.label("Linear")
					.tooltip("Linear gradient changes colors from one side to the other along a line")
					.on_update(move |_| {
						PropertiesPanelMessage::ModifyFill {
							fill: Fill::Gradient(cloned_gradient_linear.clone()),
						}
						.into()
					}),
				RadioEntryData::new("radial")
					.label("Radial")
					.tooltip("Radial gradient changes colors from the inside to the outside of a circular area")
					.on_update(move |_| {
						PropertiesPanelMessage::ModifyFill {
							fill: Fill::Gradient(cloned_gradient_radial.clone()),
						}
						.into()
					}),
			])
			.selected_index(Some(selected_index))
			.widget_holder(),
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
		TextLabel::new(value)
			.tooltip("Adjustable by dragging the gradient stops in the viewport with the Gradient tool active")
			.widget_holder(),
		Separator::new(SeparatorType::Unrelated).widget_holder(),
		Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
		Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
		Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
		Separator::new(SeparatorType::Unrelated).widget_holder(),
		ColorButton::new(gradient_clone.positions[position].1)
			.on_update(move |text_input: &ColorButton| {
				let mut new_gradient = (*gradient_clone).clone();
				new_gradient.positions[position].1 = text_input.value;
				send_fill_message(new_gradient)
			})
			.widget_holder(),
	];

	let mut skip_separator = false;
	// Remove button
	if gradient.positions.len() != position + 1 && position != 0 {
		let on_update = move |_: &IconButton| {
			let mut new_gradient = (*gradient_3).clone();
			new_gradient.positions.remove(position);
			send_fill_message(new_gradient)
		};

		skip_separator = true;
		widgets.push(Separator::new(SeparatorType::Related).widget_holder());
		widgets.push(IconButton::new("Remove", 16).tooltip("Remove this gradient stop").on_update(on_update).widget_holder());
	}
	// Add button
	if gradient.positions.len() != position + 1 {
		let on_update = move |_: &IconButton| {
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
		};

		if !skip_separator {
			widgets.push(Separator::new(SeparatorType::Related).widget_holder());
		}
		widgets.push(IconButton::new("Add", 16).tooltip("Add a gradient stop after this").on_update(on_update).widget_holder());
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
						TextLabel::new("Color").widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						ColorButton::new(if let Fill::Solid(color) = fill { Some(*color) } else { None })
							.on_update(|text_input: &ColorButton| {
								let fill = if let Some(value) = text_input.value { Fill::Solid(value) } else { Fill::None };
								PropertiesPanelMessage::ModifyFill { fill }.into()
							})
							.widget_holder(),
					],
				},
				LayoutGroup::Row {
					widgets: vec![
						TextLabel::new("").widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						TextButton::new("Use Gradient")
							.tooltip("Change this fill from a solid color to a gradient")
							.on_update(move |_: &TextButton| {
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
							})
							.widget_holder(),
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
						TextLabel::new("").widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						TextButton::new("Invert")
							.icon(Some("Swap".into()))
							.tooltip("Reverse the order of each color stop")
							.on_update(move |_: &TextButton| {
								let mut new_gradient = cloned_gradient.clone();
								new_gradient.positions = new_gradient.positions.iter().map(|(distance, color)| (1. - distance, *color)).collect();
								new_gradient.positions.reverse();
								PropertiesPanelMessage::ModifyFill { fill: Fill::Gradient(new_gradient) }.into()
							})
							.widget_holder(),
					],
				});
				layout.push(LayoutGroup::Row {
					widgets: vec![
						TextLabel::new("").widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
						Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						TextButton::new("Use Solid Color")
							.tooltip("Change this fill from a gradient to a solid color, keeping the 0% stop color")
							.on_update(move |_: &TextButton| {
								PropertiesPanelMessage::ModifyFill {
									fill: Fill::Solid(first_color.unwrap_or_default()),
								}
								.into()
							})
							.widget_holder(),
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
					TextLabel::new("Color").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					ColorButton::new(stroke.color())
						.on_update(move |text_input: &ColorButton| {
							internal_stroke1
								.clone()
								.with_color(&text_input.value)
								.map_or(PropertiesPanelMessage::ResendActiveProperties.into(), |stroke| PropertiesPanelMessage::ModifyStroke { stroke }.into())
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Weight").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(stroke.weight()))
						.is_integer(false)
						.min(0.)
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.unit(" px")
						.on_update(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke2.clone().with_weight(number_input.value.unwrap()),
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Dash Lengths").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					TextInput::new(stroke.dash_lengths())
						.centered(true)
						.on_update(move |text_input: &TextInput| {
							internal_stroke3
								.clone()
								.with_dash_lengths(&text_input.value)
								.map_or(PropertiesPanelMessage::ResendActiveProperties.into(), |stroke| PropertiesPanelMessage::ModifyStroke { stroke }.into())
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Dash Offset").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(stroke.dash_offset()))
						.is_integer(true)
						.min(0.)
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.unit(" px")
						.on_update(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke4.clone().with_dash_offset(number_input.value.unwrap()),
							}
							.into()
						})
						.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Line Cap").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					RadioInput::new(vec![
						RadioEntryData::new("Butt").on_update(move |_| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke6.clone().with_line_cap(LineCap::Butt),
							}
							.into()
						}),
						RadioEntryData::new("Round").on_update(move |_| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke7.clone().with_line_cap(LineCap::Round),
							}
							.into()
						}),
						RadioEntryData::new("Square").on_update(move |_| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke8.clone().with_line_cap(LineCap::Square),
							}
							.into()
						}),
					])
					.selected_index(Some(stroke.line_cap_index()))
					.widget_holder(),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Line Join").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					RadioInput::new(vec![
						RadioEntryData::new("Miter").on_update(move |_| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke9.clone().with_line_join(LineJoin::Miter),
							}
							.into()
						}),
						RadioEntryData::new("Bevel").on_update(move |_| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke10.clone().with_line_join(LineJoin::Bevel),
							}
							.into()
						}),
						RadioEntryData::new("Round").on_update(move |_| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke11.clone().with_line_join(LineJoin::Round),
							}
							.into()
						}),
					])
					.selected_index(Some(stroke.line_join_index()))
					.widget_holder(),
				],
			},
			// TODO: Gray out this row when Line Join isn't set to Miter
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new("Miter Limit").widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: These three separators add up to 24px,
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: which is the width of the Assist area.
					Separator::new(SeparatorType::Unrelated).widget_holder(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(stroke.line_join_miter_limit() as f64))
						.is_integer(true)
						.min(0.)
						.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
						.unit("")
						.on_update(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke5.clone().with_line_join_miter_limit(number_input.value.unwrap()),
							}
							.into()
						})
						.widget_holder(),
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
