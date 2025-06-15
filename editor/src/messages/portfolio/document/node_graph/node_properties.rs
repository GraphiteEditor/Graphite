#![allow(clippy::too_many_arguments)]

use super::document_node_definitions::{NODE_OVERRIDES, NodePropertiesContext};
use super::utility_types::FrontendGraphDataType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::*;
use choice::enum_choice;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, IVec2, UVec2};
use graph_craft::Type;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput};
use graphene_core::raster::curve::Curve;
use graphene_core::raster::image::RasterDataTable;
use graphene_core::raster::{
	BlendMode, CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, LuminanceCalculation, NoiseType, RedGreenBlue, RedGreenBlueAlpha, RelativeAbsolute,
	SelectiveColorChoice,
};
use graphene_core::text::Font;
use graphene_core::vector::generator_nodes::grid;
use graphene_core::vector::misc::CentroidType;
use graphene_core::vector::style::{GradientType, LineCap, LineJoin};
use graphene_std::animation::RealTimeMode;
use graphene_std::ops::XY;
use graphene_std::transform::{Footprint, ReferencePoint};
use graphene_std::vector::VectorDataTable;
use graphene_std::vector::misc::ArcType;
use graphene_std::vector::misc::{BooleanOperation, GridType};
use graphene_std::vector::style::{Fill, FillChoice, FillType, GradientStops};
use graphene_std::{GraphicGroupTable, NodeInputDecleration};

pub(crate) fn string_properties(text: &str) -> Vec<LayoutGroup> {
	let widget = TextLabel::new(text).widget_holder();
	vec![LayoutGroup::Row { widgets: vec![widget] }]
}

fn optionally_update_value<T>(value: impl Fn(&T) -> Option<TaggedValue> + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	move |input_value: &T| match value(input_value) {
		Some(value) => NodeGraphMessage::SetInputValue { node_id, input_index, value }.into(),
		_ => Message::NoOp,
	}
}

pub fn update_value<T>(value: impl Fn(&T) -> TaggedValue + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	optionally_update_value(move |v| Some(value(v)), node_id, input_index)
}

pub fn commit_value<T>(_: &T) -> Message {
	DocumentMessage::AddTransaction.into()
}

pub fn expose_widget(node_id: NodeId, index: usize, data_type: FrontendGraphDataType, exposed: bool) -> WidgetHolder {
	ParameterExposeButton::new()
		.exposed(exposed)
		.data_type(data_type)
		.tooltip(if exposed {
			"Stop exposing this parameter as a node input in the graph"
		} else {
			"Expose this parameter as a node input in the graph"
		})
		.on_update(move |_parameter| {
			Message::Batched(Box::new([
				NodeGraphMessage::ExposeInput {
					input_connector: InputConnector::node(node_id, index),
					set_to_exposed: !exposed,
					start_transaction: true,
				}
				.into(),
				DocumentMessage::GraphViewOverlay { open: true }.into(),
				NavigationMessage::FitViewportToSelection.into(),
				DocumentMessage::ZoomCanvasTo100Percent.into(),
			]))
		})
		.widget_holder()
}

// TODO: Remove this when we have proper entry row formatting that includes room for Assists.
pub fn add_blank_assist(widgets: &mut Vec<WidgetHolder>) {
	widgets.extend_from_slice(&[
		// Custom CSS specific to the Properties panel converts this Section separator into the width of an assist (24px).
		Separator::new(SeparatorType::Section).widget_holder(),
		// This last one is the separator after the 24px assist.
		Separator::new(SeparatorType::Unrelated).widget_holder(),
	]);
}

pub fn start_widgets(parameter_widgets_info: ParameterWidgetsInfo, data_type: FrontendGraphDataType) -> Vec<WidgetHolder> {
	start_widgets_exposable(parameter_widgets_info, data_type, true)
}

pub fn start_widgets_exposable(parameter_widgets_info: ParameterWidgetsInfo, data_type: FrontendGraphDataType, exposable: bool) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo {
		document_node,
		node_id,
		index,
		name,
		description,
		blank_assist,
	} = parameter_widgets_info;

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	let description = if description != "TODO" { description } else { "" };
	let mut widgets = Vec::with_capacity(6);
	if exposable {
		widgets.push(expose_widget(node_id, index, data_type, input.is_exposed()));
	}
	widgets.push(TextLabel::new(name).tooltip(description).widget_holder());
	if blank_assist {
		add_blank_assist(&mut widgets);
	}

	widgets
}

pub(crate) fn property_from_type(
	node_id: NodeId,
	index: usize,
	ty: &Type,
	number_options: (Option<f64>, Option<f64>, Option<(f64, f64)>),
	unit: Option<&str>,
	display_decimal_places: Option<u32>,
	step: Option<f64>,
	context: &mut NodePropertiesContext,
) -> Result<Vec<LayoutGroup>, Vec<LayoutGroup>> {
	let Some(network) = context.network_interface.nested_network(context.selection_network_path) else {
		log::warn!("A widget failed to be built for node {node_id}, index {index} because the network could not be determined");
		return Err(vec![]);
	};
	let Some(document_node) = network.nodes.get(&node_id) else {
		log::warn!("A widget failed to be built for node {node_id}, index {index} because the document node does not exist");
		return Err(vec![]);
	};

	let name = context.network_interface.input_name(node_id, index, context.selection_network_path).unwrap_or_default();
	let description = context.network_interface.input_description(node_id, index, context.selection_network_path).unwrap_or_default();

	let (mut number_min, mut number_max, range) = number_options;
	let mut number_input = NumberInput::default();
	if let Some((range_start, range_end)) = range {
		number_min = Some(range_start);
		number_max = Some(range_end);
		number_input = number_input.mode_range().min(range_start).max(range_end);
	}
	if let Some(unit) = unit {
		number_input = number_input.unit(unit);
	}
	if let Some(display_decimal_places) = display_decimal_places {
		number_input = number_input.display_decimal_places(display_decimal_places);
	}
	if let Some(step) = step {
		number_input = number_input.step(step);
	}

	let min = |x: f64| number_min.unwrap_or(x);
	let max = |x: f64| number_max.unwrap_or(x);

	let default_info = ParameterWidgetsInfo::new(document_node, node_id, index, name, description, true);

	let mut extra_widgets = vec![];
	let widgets = match ty {
		Type::Concrete(concrete_type) => {
			match concrete_type.alias.as_ref().map(|x| x.as_ref()) {
				// Aliased types (ambiguous values)
				Some("Percentage") => number_widget(default_info, number_input.percentage().min(min(0.)).max(max(100.))).into(),
				Some("SignedPercentage") => number_widget(default_info, number_input.percentage().min(min(-100.)).max(max(100.))).into(),
				Some("Angle") => number_widget(default_info, number_input.mode_range().min(min(-180.)).max(max(180.)).unit(unit.unwrap_or("°"))).into(),
				Some("Multiplier") => number_widget(default_info, number_input.unit(unit.unwrap_or("x"))).into(),
				Some("PixelLength") => number_widget(default_info, number_input.min(min(0.)).unit(unit.unwrap_or(" px"))).into(),
				Some("Length") => number_widget(default_info, number_input.min(min(0.))).into(),
				Some("Fraction") => number_widget(default_info, number_input.mode_range().min(min(0.)).max(max(1.))).into(),
				Some("IntegerCount") => number_widget(default_info, number_input.int().min(min(1.))).into(),
				Some("SeedValue") => number_widget(default_info, number_input.int().min(min(0.))).into(),
				Some("Resolution") => coordinate_widget(default_info, "W", "H", unit.unwrap_or(" px"), Some(64.)),
				Some("PixelSize") => coordinate_widget(default_info, "X", "Y", unit.unwrap_or(" px"), None),

				// For all other types, use TypeId-based matching
				_ => {
					use std::any::TypeId;
					match concrete_type.id {
						// ===============
						// PRIMITIVE TYPES
						// ===============
						Some(x) if x == TypeId::of::<f64>() => number_widget(default_info, number_input.min(min(f64::NEG_INFINITY)).max(max(f64::INFINITY))).into(),
						Some(x) if x == TypeId::of::<u32>() => number_widget(default_info, number_input.int().min(min(0.)).max(max(f64::from(u32::MAX)))).into(),
						Some(x) if x == TypeId::of::<u64>() => number_widget(default_info, number_input.int().min(min(0.))).into(),
						Some(x) if x == TypeId::of::<bool>() => bool_widget(default_info, CheckboxInput::default()).into(),
						Some(x) if x == TypeId::of::<String>() => text_widget(default_info).into(),
						Some(x) if x == TypeId::of::<DVec2>() => coordinate_widget(default_info, "X", "Y", "", None),
						Some(x) if x == TypeId::of::<UVec2>() => coordinate_widget(default_info, "X", "Y", "", Some(0.)),
						Some(x) if x == TypeId::of::<IVec2>() => coordinate_widget(default_info, "X", "Y", "", None),
						// ==========================
						// PRIMITIVE COLLECTION TYPES
						// ==========================
						Some(x) if x == TypeId::of::<Vec<f64>>() => array_of_number_widget(default_info, TextInput::default()).into(),
						Some(x) if x == TypeId::of::<Vec<DVec2>>() => array_of_coordinates_widget(default_info, TextInput::default()).into(),
						// ====================
						// GRAPHICAL DATA TYPES
						// ====================
						Some(x) if x == TypeId::of::<VectorDataTable>() => vector_data_widget(default_info).into(),
						Some(x) if x == TypeId::of::<RasterDataTable<Color>>() => raster_widget(default_info).into(),
						Some(x) if x == TypeId::of::<GraphicGroupTable>() => group_widget(default_info).into(),
						// ============
						// STRUCT TYPES
						// ============
						Some(x) if x == TypeId::of::<Color>() => color_widget(default_info, ColorInput::default().allow_none(false)),
						Some(x) if x == TypeId::of::<Option<Color>>() => color_widget(default_info, ColorInput::default().allow_none(true)),
						Some(x) if x == TypeId::of::<GradientStops>() => color_widget(default_info, ColorInput::default().allow_none(false)),
						Some(x) if x == TypeId::of::<Font>() => font_widget(default_info),
						Some(x) if x == TypeId::of::<Curve>() => curve_widget(default_info),
						Some(x) if x == TypeId::of::<Footprint>() => footprint_widget(default_info, &mut extra_widgets),
						// ===============================
						// MANUALLY IMPLEMENTED ENUM TYPES
						// ===============================
						Some(x) if x == TypeId::of::<ReferencePoint>() => reference_point_widget(default_info, false).into(),
						Some(x) if x == TypeId::of::<BlendMode>() => blend_mode_widget(default_info),
						// =========================
						// AUTO-GENERATED ENUM TYPES
						// =========================
						Some(x) if x == TypeId::of::<FillType>() => enum_choice::<FillType>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<GradientType>() => enum_choice::<GradientType>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<RealTimeMode>() => enum_choice::<RealTimeMode>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<RedGreenBlue>() => enum_choice::<RedGreenBlue>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<RedGreenBlueAlpha>() => enum_choice::<RedGreenBlueAlpha>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<XY>() => enum_choice::<XY>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<NoiseType>() => enum_choice::<NoiseType>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<FractalType>() => enum_choice::<FractalType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if x == TypeId::of::<CellularDistanceFunction>() => enum_choice::<CellularDistanceFunction>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if x == TypeId::of::<CellularReturnType>() => enum_choice::<CellularReturnType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if x == TypeId::of::<DomainWarpType>() => enum_choice::<DomainWarpType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if x == TypeId::of::<RelativeAbsolute>() => enum_choice::<RelativeAbsolute>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if x == TypeId::of::<GridType>() => enum_choice::<GridType>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<LineCap>() => enum_choice::<LineCap>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<LineJoin>() => enum_choice::<LineJoin>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<ArcType>() => enum_choice::<ArcType>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<BooleanOperation>() => enum_choice::<BooleanOperation>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<CentroidType>() => enum_choice::<CentroidType>().for_socket(default_info).property_row(),
						Some(x) if x == TypeId::of::<LuminanceCalculation>() => enum_choice::<LuminanceCalculation>().for_socket(default_info).property_row(),
						// =====
						// OTHER
						// =====
						_ => {
							let mut widgets = start_widgets(default_info, FrontendGraphDataType::General);
							widgets.extend_from_slice(&[
								Separator::new(SeparatorType::Unrelated).widget_holder(),
								TextLabel::new("-")
									.tooltip(format!(
										"This data can only be supplied through the node graph because no widget exists for its type:\n\
										{}",
										concrete_type.name
									))
									.widget_holder(),
							]);
							return Err(vec![widgets.into()]);
						}
					}
				}
			}
		}
		Type::Generic(_) => vec![TextLabel::new("Generic type (not supported)").widget_holder()].into(),
		Type::Fn(_, out) => return property_from_type(node_id, index, out, number_options, unit, display_decimal_places, step, context),
		Type::Future(out) => return property_from_type(node_id, index, out, number_options, unit, display_decimal_places, step, context),
	};

	extra_widgets.push(widgets);

	Ok(extra_widgets)
}

pub fn text_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(x.clone())
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	}
	widgets
}

pub fn text_area_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextAreaInput::new(x.clone())
				.on_update(update_value(|x: &TextAreaInput| TaggedValue::String(x.value.clone()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	}
	widgets
}

pub fn bool_widget(parameter_widgets_info: ParameterWidgetsInfo, checkbox_input: CheckboxInput) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(x)) = input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			checkbox_input
				.checked(x)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	}
	widgets
}

pub fn reference_point_widget(parameter_widgets_info: ParameterWidgetsInfo, disabled: bool) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::ReferencePoint(reference_point)) = input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(reference_point != ReferencePoint::None)
				.on_update(update_value(
					move |x: &CheckboxInput| TaggedValue::ReferencePoint(if x.checked { ReferencePoint::Center } else { ReferencePoint::None }),
					node_id,
					index,
				))
				.disabled(disabled)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			ReferencePointInput::new(reference_point)
				.on_update(update_value(move |x: &ReferencePointInput| TaggedValue::ReferencePoint(x.value), node_id, index))
				.disabled(disabled)
				.widget_holder(),
		])
	}
	widgets
}

pub fn footprint_widget(parameter_widgets_info: ParameterWidgetsInfo, extra_widgets: &mut Vec<LayoutGroup>) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut location_widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);
	location_widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let mut scale_widgets = vec![TextLabel::new("").widget_holder()];
	add_blank_assist(&mut scale_widgets);
	scale_widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let mut resolution_widgets = vec![TextLabel::new("").widget_holder()];
	add_blank_assist(&mut resolution_widgets);
	resolution_widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return Vec::new().into();
	};
	if let Some(&TaggedValue::Footprint(footprint)) = input.as_non_exposed_value() {
		let top_left = footprint.transform.transform_point2(DVec2::ZERO);
		let bounds = footprint.scale();
		let oversample = footprint.resolution.as_dvec2() / bounds;

		location_widgets.extend_from_slice(&[
			NumberInput::new(Some(top_left.x))
				.label("X")
				.unit(" px")
				.on_update(update_value(
					move |x: &NumberInput| {
						let (offset, scale) = {
							let diff = DVec2::new(top_left.x - x.value.unwrap_or_default(), 0.);
							(top_left - diff, bounds)
						};

						let footprint = Footprint {
							transform: DAffine2::from_scale_angle_translation(scale, 0., offset),
							resolution: (oversample * scale).as_uvec2(),
							..footprint
						};

						TaggedValue::Footprint(footprint)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(top_left.y))
				.label("Y")
				.unit(" px")
				.on_update(update_value(
					move |x: &NumberInput| {
						let (offset, scale) = {
							let diff = DVec2::new(0., top_left.y - x.value.unwrap_or_default());
							(top_left - diff, bounds)
						};

						let footprint = Footprint {
							transform: DAffine2::from_scale_angle_translation(scale, 0., offset),
							resolution: (oversample * scale).as_uvec2(),
							..footprint
						};

						TaggedValue::Footprint(footprint)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_holder(),
		]);

		scale_widgets.extend_from_slice(&[
			NumberInput::new(Some(bounds.x))
				.label("W")
				.unit(" px")
				.on_update(update_value(
					move |x: &NumberInput| {
						let (offset, scale) = (top_left, DVec2::new(x.value.unwrap_or_default(), bounds.y));

						let footprint = Footprint {
							transform: DAffine2::from_scale_angle_translation(scale, 0., offset),
							resolution: (oversample * scale).as_uvec2(),
							..footprint
						};

						TaggedValue::Footprint(footprint)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(bounds.y))
				.label("H")
				.unit(" px")
				.on_update(update_value(
					move |x: &NumberInput| {
						let (offset, scale) = (top_left, DVec2::new(bounds.x, x.value.unwrap_or_default()));

						let footprint = Footprint {
							transform: DAffine2::from_scale_angle_translation(scale, 0., offset),
							resolution: (oversample * scale).as_uvec2(),
							..footprint
						};

						TaggedValue::Footprint(footprint)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_holder(),
		]);

		resolution_widgets.push(
			NumberInput::new(Some((footprint.resolution.as_dvec2() / bounds).x * 100.))
				.label("Resolution")
				.unit("%")
				.on_update(update_value(
					move |x: &NumberInput| {
						let resolution = (bounds * x.value.unwrap_or(100.) / 100.).as_uvec2().max((1, 1).into()).min((4000, 4000).into());

						let footprint = Footprint { resolution, ..footprint };
						TaggedValue::Footprint(footprint)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_holder(),
		);
	}

	let widgets = vec![
		LayoutGroup::Row { widgets: location_widgets },
		LayoutGroup::Row { widgets: scale_widgets },
		LayoutGroup::Row { widgets: resolution_widgets },
	];
	let (last, rest) = widgets.split_last().expect("Footprint widget should return multiple rows");
	*extra_widgets = rest.to_vec();
	last.clone()
}

pub fn coordinate_widget(parameter_widgets_info: ParameterWidgetsInfo, x: &str, y: &str, unit: &str, min: Option<f64>) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::Number);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	match input.as_non_exposed_value() {
		Some(&TaggedValue::DVec2(dvec2)) => {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(dvec2.x))
					.label(x)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), dvec2.y)), node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(dvec2.y))
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(dvec2.x, input.value.unwrap())), node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
			]);
		}
		Some(&TaggedValue::IVec2(ivec2)) => {
			let update_x = move |input: &NumberInput| TaggedValue::IVec2(IVec2::new(input.value.unwrap() as i32, ivec2.y));
			let update_y = move |input: &NumberInput| TaggedValue::IVec2(IVec2::new(ivec2.x, input.value.unwrap() as i32));
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(ivec2.x as f64))
					.int()
					.label(x)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(update_x, node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(ivec2.y as f64))
					.int()
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(update_y, node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
			]);
		}
		Some(&TaggedValue::UVec2(uvec2)) => {
			let update_x = move |input: &NumberInput| TaggedValue::UVec2(UVec2::new(input.value.unwrap() as u32, uvec2.y));
			let update_y = move |input: &NumberInput| TaggedValue::UVec2(UVec2::new(uvec2.x, input.value.unwrap() as u32));
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(uvec2.x as f64))
					.int()
					.label(x)
					.unit(unit)
					.min(min.unwrap_or(0.))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(update_x, node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(uvec2.y as f64))
					.int()
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(0.))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(update_y, node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
			]);
		}
		Some(&TaggedValue::F64(value)) => {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(value))
					.label(x)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), value)), node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(value))
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(value, input.value.unwrap())), node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
			]);
		}
		_ => {}
	}

	LayoutGroup::Row { widgets }
}

pub fn array_of_number_widget(parameter_widgets_info: ParameterWidgetsInfo, text_input: TextInput) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::Number);

	let from_string = |string: &str| {
		string
			.split(&[',', ' '])
			.filter(|x| !x.is_empty())
			.map(str::parse::<f64>)
			.collect::<Result<Vec<_>, _>>()
			.ok()
			.map(TaggedValue::VecF64)
	};

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::VecF64(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			text_input
				.value(x.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
				.on_update(optionally_update_value(move |x: &TextInput| from_string(&x.value), node_id, index))
				.widget_holder(),
		])
	}
	widgets
}

pub fn array_of_coordinates_widget(parameter_widgets_info: ParameterWidgetsInfo, text_props: TextInput) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::Number);

	let from_string = |string: &str| {
		string
			.split(|c: char| !c.is_alphanumeric() && !matches!(c, '.' | '+' | '-'))
			.filter(|x| !x.is_empty())
			.map(|x| x.parse::<f64>().ok())
			.collect::<Option<Vec<_>>>()
			.map(|numbers| numbers.chunks_exact(2).map(|values| DVec2::new(values[0], values[1])).collect())
			.map(TaggedValue::VecDVec2)
	};

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::VecDVec2(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			text_props
				.value(x.iter().map(|v| format!("({}, {})", v.x, v.y)).collect::<Vec<_>>().join(", "))
				.on_update(optionally_update_value(move |x: &TextInput| from_string(&x.value), node_id, index))
				.widget_holder(),
		])
	}
	widgets
}

pub fn font_inputs(parameter_widgets_info: ParameterWidgetsInfo) -> (Vec<WidgetHolder>, Option<Vec<WidgetHolder>>) {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut first_widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);
	let mut second_widgets = None;

	let from_font_input = |font: &FontInput| TaggedValue::Font(Font::new(font.font_family.clone(), font.font_style.clone()));

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return (vec![], None);
	};
	if let Some(TaggedValue::Font(font)) = &input.as_non_exposed_value() {
		first_widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			FontInput::new(font.font_family.clone(), font.font_style.clone())
				.on_update(update_value(from_font_input, node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]);

		let mut second_row = vec![TextLabel::new("").widget_holder()];
		add_blank_assist(&mut second_row);
		second_row.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			FontInput::new(font.font_family.clone(), font.font_style.clone())
				.is_style_picker(true)
				.on_update(update_value(from_font_input, node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]);
		second_widgets = Some(second_row);
	}
	(first_widgets, second_widgets)
}

pub fn vector_data_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::VectorData);

	widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
	widgets.push(TextLabel::new("Vector data is supplied through the node graph").widget_holder());

	widgets
}

pub fn raster_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::Raster);

	widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
	widgets.push(TextLabel::new("Raster data is supplied through the node graph").widget_holder());

	widgets
}

pub fn group_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::Group);

	widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
	widgets.push(TextLabel::new("Group data is supplied through the node graph").widget_holder());

	widgets
}

pub fn number_widget(parameter_widgets_info: ParameterWidgetsInfo, number_props: NumberInput) -> Vec<WidgetHolder> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::Number);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	match input.as_non_exposed_value() {
		Some(&TaggedValue::F64(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]),
		Some(&TaggedValue::U32(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::U32((x.value.unwrap()) as u32), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]),
		Some(&TaggedValue::U64(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::U64((x.value.unwrap()) as u64), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]),
		Some(&TaggedValue::OptionalF64(x)) => {
			// TODO: Don't wipe out the previously set value (setting it back to the default of 100) when reenabling this checkbox back to Some from None
			let toggle_enabled = move |checkbox_input: &CheckboxInput| TaggedValue::OptionalF64(if checkbox_input.checked { Some(100.) } else { None });
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				// The checkbox toggles if the value is Some or None
				CheckboxInput::new(x.is_some())
					.on_update(update_value(toggle_enabled, node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				number_props
					.value(x)
					.on_update(update_value(move |x: &NumberInput| TaggedValue::OptionalF64(x.value), node_id, index))
					.disabled(x.is_none())
					.on_commit(commit_value)
					.widget_holder(),
			]);
		}
		Some(&TaggedValue::DVec2(dvec2)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
			// We use an arbitrary `y` instead of an arbitrary `x` here because the "Grid" node's "Spacing" value's height should be used from rectangular mode when transferred to "Y Spacing" in isometric mode
				.value(Some(dvec2.y))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]),
		_ => {}
	}

	widgets
}

// TODO: Auto-generate this enum dropdown menu widget
pub fn blend_mode_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::BlendMode(blend_mode)) = input.as_non_exposed_value() {
		let entries = BlendMode::list_svg_subset()
			.iter()
			.map(|category| {
				category
					.iter()
					.map(|blend_mode| {
						MenuListEntry::new(format!("{blend_mode:?}"))
							.label(blend_mode.to_string())
							.on_update(update_value(move |_| TaggedValue::BlendMode(*blend_mode), node_id, index))
							.on_commit(commit_value)
					})
					.collect()
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(entries)
				.selected_index(blend_mode.index_in_list_svg_subset().map(|index| index as u32))
				.widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Formula used for blending")
}

pub fn color_widget(parameter_widgets_info: ParameterWidgetsInfo, color_button: ColorInput) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);

	// Return early with just the label if the input is exposed to the graph, meaning we don't want to show the color picker widget in the Properties panel
	let NodeInput::Value { tagged_value, exposed: false } = &document_node.inputs[index] else {
		return LayoutGroup::Row { widgets };
	};

	// Add a separator
	widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	// Add the color input
	match &**tagged_value {
		TaggedValue::Color(color) => widgets.push(
			color_button
				.value(FillChoice::Solid(*color))
				.on_update(update_value(|x: &ColorInput| TaggedValue::Color(x.value.as_solid().unwrap_or_default()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		),
		TaggedValue::OptionalColor(color) => widgets.push(
			color_button
				.value(match color {
					Some(color) => FillChoice::Solid(*color),
					None => FillChoice::None,
				})
				.on_update(update_value(|x: &ColorInput| TaggedValue::OptionalColor(x.value.as_solid()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		),
		TaggedValue::GradientStops(x) => widgets.push(
			color_button
				.value(FillChoice::Gradient(x.clone()))
				.on_update(update_value(
					|x: &ColorInput| TaggedValue::GradientStops(x.value.as_gradient().cloned().unwrap_or_default()),
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_holder(),
		),
		_ => {}
	}

	LayoutGroup::Row { widgets }
}

pub fn font_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let (font_widgets, style_widgets) = font_inputs(parameter_widgets_info);
	font_widgets.into_iter().chain(style_widgets.unwrap_or_default()).collect::<Vec<_>>().into()
}

pub fn curve_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info, FrontendGraphDataType::General);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(TaggedValue::Curve(curve)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CurveInput::new(curve.clone())
				.on_update(update_value(|x: &CurveInput| TaggedValue::Curve(x.value.clone()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	}
	LayoutGroup::Row { widgets }
}

pub fn get_document_node<'a>(node_id: NodeId, context: &'a NodePropertiesContext<'a>) -> Result<&'a DocumentNode, String> {
	let network = context
		.network_interface
		.nested_network(context.selection_network_path)
		.ok_or("network not found in get_document_node")?;
	network.nodes.get(&node_id).ok_or(format!("node {node_id} not found in get_document_node"))
}

pub fn query_node_and_input_info<'a>(node_id: NodeId, input_index: usize, context: &'a NodePropertiesContext<'a>) -> Result<(&'a DocumentNode, &'a str, &'a str), String> {
	let node_id2 = node_id.clone();
	let document_node = get_document_node(node_id2, context)?;
	let input_name = context.network_interface.input_name(node_id, input_index, context.selection_network_path).unwrap_or_else(|| {
		log::warn!("input name not found in query_node_and_input_info");
		""
	});
	let input_description = context.network_interface.input_description(node_id, input_index, context.selection_network_path).unwrap_or_default();
	Ok((document_node, input_name, input_description))
}

pub fn query_noise_pattern_state(node_id: NodeId, context: &NodePropertiesContext) -> Result<(bool, bool, bool, bool, bool, bool), String> {
	let document_node = get_document_node(node_id, context)?;
	let current_noise_type = document_node.inputs.iter().find_map(|input| match input.as_value() {
		Some(&TaggedValue::NoiseType(noise_type)) => Some(noise_type),
		_ => None,
	});
	let current_fractal_type = document_node.inputs.iter().find_map(|input| match input.as_value() {
		Some(&TaggedValue::FractalType(fractal_type)) => Some(fractal_type),
		_ => None,
	});
	let current_domain_warp_type = document_node.inputs.iter().find_map(|input| match input.as_value() {
		Some(&TaggedValue::DomainWarpType(domain_warp_type)) => Some(domain_warp_type),
		_ => None,
	});
	let fractal_active = current_fractal_type != Some(FractalType::None);
	let coherent_noise_active = current_noise_type != Some(NoiseType::WhiteNoise);
	let cellular_noise_active = current_noise_type == Some(NoiseType::Cellular);
	let ping_pong_active = current_fractal_type == Some(FractalType::PingPong);
	let domain_warp_active = current_domain_warp_type != Some(DomainWarpType::None);
	let domain_warp_only_fractal_type_wrongly_active =
		!domain_warp_active && (current_fractal_type == Some(FractalType::DomainWarpIndependent) || current_fractal_type == Some(FractalType::DomainWarpProgressive));

	Ok((
		fractal_active,
		coherent_noise_active,
		cellular_noise_active,
		ping_pong_active,
		domain_warp_active,
		domain_warp_only_fractal_type_wrongly_active,
	))
}

pub fn query_assign_colors_randomize(node_id: NodeId, context: &NodePropertiesContext) -> Result<bool, String> {
	let document_node = get_document_node(node_id, context)?;
	// This is safe since the node is a proto node and the implementation cannot be changed.
	let randomize_index = 5;
	Ok(match document_node.inputs.get(randomize_index).and_then(|input| input.as_value()) {
		Some(TaggedValue::Bool(randomize_enabled)) => *randomize_enabled,
		_ => false,
	})
}

pub(crate) fn brightness_contrast_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in brightness_contrast_properties: {err}");
			return Vec::new();
		}
	};

	// Use Classic
	let use_classic_index = 3;
	let use_classic = bool_widget(ParameterWidgetsInfo::from_index(document_node, node_id, use_classic_index, true, context), CheckboxInput::default());
	let use_classic_value = match document_node.inputs[use_classic_index].as_value() {
		Some(TaggedValue::Bool(use_classic_choice)) => *use_classic_choice,
		_ => false,
	};

	// Brightness
	let brightness_index = 1;
	let brightness = number_widget(
		ParameterWidgetsInfo::from_index(document_node, node_id, brightness_index, true, context),
		NumberInput::default()
			.unit("%")
			.mode_range()
			.display_decimal_places(2)
			.range_min(Some(if use_classic_value { -100. } else { -150. }))
			.range_max(Some(if use_classic_value { 100. } else { 150. })),
	);

	// Contrast
	let contrast_index = 2;
	let contrast = number_widget(
		ParameterWidgetsInfo::from_index(document_node, node_id, contrast_index, true, context),
		NumberInput::default()
			.unit("%")
			.mode_range()
			.display_decimal_places(2)
			.range_min(Some(if use_classic_value { -100. } else { -50. }))
			.range_max(Some(100.)),
	);

	let layout = vec![
		LayoutGroup::Row { widgets: brightness },
		LayoutGroup::Row { widgets: contrast },
		LayoutGroup::Row { widgets: use_classic },
	];

	layout
}

pub(crate) fn channel_mixer_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in channel_mixer_properties: {err}");
			return Vec::new();
		}
	};

	// Monochrome
	let monochrome_index = 1;
	let is_monochrome = bool_widget(ParameterWidgetsInfo::from_index(document_node, node_id, monochrome_index, true, context), CheckboxInput::default());
	let is_monochrome_value = match document_node.inputs[monochrome_index].as_value() {
		Some(TaggedValue::Bool(monochrome_choice)) => *monochrome_choice,
		_ => false,
	};

	// Output channel choice
	let output_channel_index = 18;
	let output_channel = enum_choice::<RedGreenBlue>()
		.for_socket(ParameterWidgetsInfo::from_index(document_node, node_id, output_channel_index, true, context))
		.exposable(false)
		.property_row();
	let output_channel_value = match &document_node.inputs[output_channel_index].as_value() {
		Some(TaggedValue::RedGreenBlue(choice)) => choice,
		_ => {
			warn!("Channel Mixer node properties panel could not be displayed.");
			return vec![];
		}
	};

	// Output Channel modes
	let (red_output_index, green_output_index, blue_output_index, constant_output_index) = match (is_monochrome_value, output_channel_value) {
		(true, _) => (2, 3, 4, 5),
		(false, RedGreenBlue::Red) => (6, 7, 8, 9),
		(false, RedGreenBlue::Green) => (10, 11, 12, 13),
		(false, RedGreenBlue::Blue) => (14, 15, 16, 17),
	};
	let number_input = NumberInput::default().mode_range().min(-200.).max(200.).unit("%");
	let red = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, red_output_index, true, context), number_input.clone());
	let green = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, green_output_index, true, context), number_input.clone());
	let blue = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, blue_output_index, true, context), number_input.clone());
	let constant = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, constant_output_index, true, context), number_input);

	// Monochrome
	let mut layout = vec![LayoutGroup::Row { widgets: is_monochrome }];
	// Output channel choice
	if !is_monochrome_value {
		layout.push(output_channel);
	}
	// Channel values
	layout.extend([
		LayoutGroup::Row { widgets: red },
		LayoutGroup::Row { widgets: green },
		LayoutGroup::Row { widgets: blue },
		LayoutGroup::Row { widgets: constant },
	]);
	layout
}

pub(crate) fn selective_color_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in selective_color_properties: {err}");
			return Vec::new();
		}
	};
	// Colors choice
	let colors_index = 38;

	let colors = enum_choice::<SelectiveColorChoice>()
		.for_socket(ParameterWidgetsInfo::from_index(document_node, node_id, colors_index, true, context))
		.exposable(false)
		.property_row();

	let colors_choice_index = match &document_node.inputs[colors_index].as_value() {
		Some(TaggedValue::SelectiveColorChoice(choice)) => choice,
		_ => {
			warn!("Selective Color node properties panel could not be displayed.");
			return vec![];
		}
	};

	// CMYK
	let (c_index, m_index, y_index, k_index) = match colors_choice_index {
		SelectiveColorChoice::Reds => (2, 3, 4, 5),
		SelectiveColorChoice::Yellows => (6, 7, 8, 9),
		SelectiveColorChoice::Greens => (10, 11, 12, 13),
		SelectiveColorChoice::Cyans => (14, 15, 16, 17),
		SelectiveColorChoice::Blues => (18, 19, 20, 21),
		SelectiveColorChoice::Magentas => (22, 23, 24, 25),
		SelectiveColorChoice::Whites => (26, 27, 28, 29),
		SelectiveColorChoice::Neutrals => (30, 31, 32, 33),
		SelectiveColorChoice::Blacks => (34, 35, 36, 37),
	};
	let number_input = NumberInput::default().mode_range().min(-100.).max(100.).unit("%");
	let cyan = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, c_index, true, context), number_input.clone());
	let magenta = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, m_index, true, context), number_input.clone());
	let yellow = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, y_index, true, context), number_input.clone());
	let black = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, k_index, true, context), number_input);

	// Mode
	let mode_index = 1;
	let mode = enum_choice::<RelativeAbsolute>()
		.for_socket(ParameterWidgetsInfo::from_index(document_node, node_id, mode_index, true, context))
		.property_row();

	vec![
		// Colors choice
		colors,
		// CMYK
		LayoutGroup::Row { widgets: cyan },
		LayoutGroup::Row { widgets: magenta },
		LayoutGroup::Row { widgets: yellow },
		LayoutGroup::Row { widgets: black },
		// Mode
		mode,
	]
}

#[cfg(feature = "gpu")]
pub(crate) fn _gpu_map_properties(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<LayoutGroup> {
	let map = text_widget(parameter_widgets_info);

	vec![LayoutGroup::Row { widgets: map }]
}

pub(crate) fn grid_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let grid_type_index = grid::GridTypeInput::INDEX;
	let spacing_index = grid::SpacingInput::<f64>::INDEX;
	let angles_index = grid::AnglesInput::INDEX;
	let rows_index = grid::RowsInput::INDEX;
	let columns_index = grid::ColumnsInput::INDEX;

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in exposure_properties: {err}");
			return Vec::new();
		}
	};
	let grid_type = enum_choice::<GridType>()
		.for_socket(ParameterWidgetsInfo::from_index(document_node, node_id, grid_type_index, true, context))
		.property_row();

	let mut widgets = vec![grid_type];

	let Some(grid_type_input) = document_node.inputs.get(grid_type_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::GridType(grid_type)) = grid_type_input.as_non_exposed_value() {
		match grid_type {
			GridType::Rectangular => {
				let spacing = coordinate_widget(ParameterWidgetsInfo::from_index(document_node, node_id, spacing_index, true, context), "W", "H", " px", Some(0.));
				widgets.push(spacing);
			}
			GridType::Isometric => {
				let spacing = LayoutGroup::Row {
					widgets: number_widget(
						ParameterWidgetsInfo::from_index(document_node, node_id, spacing_index, true, context),
						NumberInput::default().label("H").min(0.).unit(" px"),
					),
				};
				let angles = coordinate_widget(ParameterWidgetsInfo::from_index(document_node, node_id, angles_index, true, context), "", "", "°", None);
				widgets.extend([spacing, angles]);
			}
		}
	}

	let rows = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, rows_index, true, context), NumberInput::default().min(1.));
	let columns = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, columns_index, true, context), NumberInput::default().min(1.));

	widgets.extend([LayoutGroup::Row { widgets: rows }, LayoutGroup::Row { widgets: columns }]);

	widgets
}

pub(crate) fn exposure_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in exposure_properties: {err}");
			return Vec::new();
		}
	};
	let exposure = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, 1, true, context), NumberInput::default().min(-20.).max(20.));
	let offset = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, 2, true, context), NumberInput::default().min(-0.5).max(0.5));
	let gamma_correction = number_widget(
		ParameterWidgetsInfo::from_index(document_node, node_id, 3, true, context),
		NumberInput::default().min(0.01).max(9.99).increment_step(0.1),
	);

	vec![
		LayoutGroup::Row { widgets: exposure },
		LayoutGroup::Row { widgets: offset },
		LayoutGroup::Row { widgets: gamma_correction },
	]
}

pub(crate) fn rectangle_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in rectangle_properties: {err}");
			return Vec::new();
		}
	};
	let size_x_index = 1;
	let size_y_index = 2;
	let corner_rounding_type_index = 3;
	let corner_radius_index = 4;
	let clamped_index = 5;

	// Size X
	let size_x = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, size_x_index, true, context), NumberInput::default());

	// Size Y
	let size_y = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, size_y_index, true, context), NumberInput::default());

	// Corner Radius
	let mut corner_radius_row_1 = start_widgets(
		ParameterWidgetsInfo::from_index(document_node, node_id, corner_radius_index, true, context),
		FrontendGraphDataType::Number,
	);
	corner_radius_row_1.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let mut corner_radius_row_2 = vec![Separator::new(SeparatorType::Unrelated).widget_holder()];
	corner_radius_row_2.push(TextLabel::new("").widget_holder());
	add_blank_assist(&mut corner_radius_row_2);

	let Some(input) = document_node.inputs.get(corner_rounding_type_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(is_individual)) = input.as_non_exposed_value() {
		// Values
		let Some(input) = document_node.inputs.get(corner_radius_index) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		let uniform_val = match input.as_non_exposed_value() {
			Some(TaggedValue::F64(x)) => *x,
			Some(TaggedValue::F64Array4(x)) => x[0],
			_ => 0.,
		};
		let individual_val = match input.as_non_exposed_value() {
			Some(&TaggedValue::F64Array4(x)) => x,
			Some(&TaggedValue::F64(x)) => [x; 4],
			_ => [0.; 4],
		};

		// Uniform/individual radio input widget
		let uniform = RadioEntryData::new("Uniform")
			.label("Uniform")
			.on_update(move |_| {
				Message::Batched(Box::new([
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: corner_rounding_type_index,
						value: TaggedValue::Bool(false),
					}
					.into(),
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: corner_radius_index,
						value: TaggedValue::F64(uniform_val),
					}
					.into(),
				]))
			})
			.on_commit(commit_value);
		let individual = RadioEntryData::new("Individual")
			.label("Individual")
			.on_update(move |_| {
				Message::Batched(Box::new([
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: corner_rounding_type_index,
						value: TaggedValue::Bool(true),
					}
					.into(),
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: corner_radius_index,
						value: TaggedValue::F64Array4(individual_val),
					}
					.into(),
				]))
			})
			.on_commit(commit_value);
		let radio_input = RadioInput::new(vec![uniform, individual]).selected_index(Some(is_individual as u32)).widget_holder();
		corner_radius_row_1.push(radio_input);

		// Radius value input widget
		let input_widget = if is_individual {
			let from_string = |string: &str| {
				string
					.split(&[',', ' '])
					.filter(|x| !x.is_empty())
					.map(str::parse::<f64>)
					.collect::<Result<Vec<f64>, _>>()
					.ok()
					.map(|v| {
						let arr: Box<[f64; 4]> = v.into_boxed_slice().try_into().unwrap_or_default();
						*arr
					})
					.map(TaggedValue::F64Array4)
			};
			TextInput::default()
				.value(individual_val.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
				.on_update(optionally_update_value(move |x: &TextInput| from_string(&x.value), node_id, corner_radius_index))
				.widget_holder()
		} else {
			NumberInput::default()
				.value(Some(uniform_val))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, corner_radius_index))
				.on_commit(commit_value)
				.widget_holder()
		};
		corner_radius_row_2.push(input_widget);
	}

	// Clamped
	let clamped = bool_widget(ParameterWidgetsInfo::from_index(document_node, node_id, clamped_index, true, context), CheckboxInput::default());

	vec![
		LayoutGroup::Row { widgets: size_x },
		LayoutGroup::Row { widgets: size_y },
		LayoutGroup::Row { widgets: corner_radius_row_1 },
		LayoutGroup::Row { widgets: corner_radius_row_2 },
		LayoutGroup::Row { widgets: clamped },
	]
}

pub(crate) fn node_no_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let text = if context.network_interface.is_layer(&node_id, context.selection_network_path) {
		"Layer has no properties"
	} else {
		"Node has no properties"
	};
	string_properties(text)
}

pub(crate) fn generate_node_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> LayoutGroup {
	let mut layout = Vec::new();

	if let Some(properties_override) = context
		.network_interface
		.reference(&node_id, context.selection_network_path)
		.cloned()
		.unwrap_or_default()
		.as_ref()
		.and_then(|reference| super::document_node_definitions::resolve_document_node_type(reference))
		.and_then(|definition| definition.properties)
		.and_then(|properties| NODE_OVERRIDES.get(properties))
	{
		layout = properties_override(node_id, context);
	} else {
		let number_of_inputs = context.network_interface.number_of_inputs(&node_id, context.selection_network_path);
		for input_index in 1..number_of_inputs {
			let row = context.call_widget_override(&node_id, input_index).unwrap_or_else(|| {
				let Some(implementation) = context.network_interface.implementation(&node_id, context.selection_network_path) else {
					log::error!("Could not get implementation for node {node_id}");
					return Vec::new();
				};

				let mut number_options = (None, None, None);
				let mut display_decimal_places = None;
				let mut step = None;
				let mut unit_suffix = None;
				let input_type = match implementation {
					DocumentNodeImplementation::ProtoNode(proto_node_identifier) => 'early_return: {
						if let Some(field) = graphene_core::registry::NODE_METADATA
							.lock()
							.unwrap()
							.get(&proto_node_identifier.name.clone().into_owned())
							.and_then(|metadata| metadata.fields.get(input_index))
						{
							number_options = (field.number_min, field.number_max, field.number_mode_range);
							display_decimal_places = field.number_display_decimal_places;
							unit_suffix = field.unit;
							step = field.number_step;
							if let Some(ref default) = field.default_type {
								break 'early_return default.clone();
							}
						}

						let Some(implementations) = &interpreted_executor::node_registry::NODE_REGISTRY.get(proto_node_identifier) else {
							log::error!("Could not get implementation for protonode {proto_node_identifier:?}");
							return Vec::new();
						};

						let mut input_types = implementations
							.keys()
							.filter_map(|item| item.inputs.get(input_index))
							.filter(|ty| property_from_type(node_id, input_index, ty, number_options, unit_suffix, display_decimal_places, step, context).is_ok())
							.collect::<Vec<_>>();
						input_types.sort_by_key(|ty| ty.type_name());
						let input_type = input_types.first().cloned();

						let Some(input_type) = input_type else {
							return Vec::new();
						};

						input_type.clone()
					}
					_ => context.network_interface.input_type(&InputConnector::node(node_id, input_index), context.selection_network_path).0,
				};

				property_from_type(node_id, input_index, &input_type, number_options, unit_suffix, display_decimal_places, step, context).unwrap_or_else(|value| value)
			});

			layout.extend(row);
		}
	}

	if layout.is_empty() {
		layout = node_no_properties(node_id, context);
	}
	let name = context
		.network_interface
		.reference(&node_id, context.selection_network_path)
		.cloned()
		.unwrap_or_default() // If there is an error getting the reference, default to empty string
		.or_else(|| {
			// If there is no reference, try to get the proto node name
			context.network_interface.implementation(&node_id, context.selection_network_path).and_then(|implementation|{
				if let DocumentNodeImplementation::ProtoNode(protonode) = implementation {
					Some(protonode.name.clone().into_owned())
				} else {
					None
				}
			})
		})
		.unwrap_or("Custom Node".to_string());
	let description = context.network_interface.description(&node_id, context.selection_network_path);
	let visible = context.network_interface.is_visible(&node_id, context.selection_network_path);
	let pinned = context.network_interface.is_pinned(&node_id, context.selection_network_path);
	LayoutGroup::Section {
		name,
		description,
		visible,
		pinned,
		id: node_id.0,
		layout,
	}
}

/// Fill Node Widgets LayoutGroup
pub(crate) fn fill_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in fill_properties: {err}");
			return Vec::new();
		}
	};
	let fill_index = 1;
	let backup_color_index = 2;
	let backup_gradient_index = 3;

	let mut widgets_first_row = start_widgets(ParameterWidgetsInfo::from_index(document_node, node_id, fill_index, true, context), FrontendGraphDataType::General);

	let (fill, backup_color, backup_gradient) = if let (Some(TaggedValue::Fill(fill)), &Some(&TaggedValue::OptionalColor(backup_color)), Some(TaggedValue::Gradient(backup_gradient))) = (
		&document_node.inputs[fill_index].as_value(),
		&document_node.inputs[backup_color_index].as_value(),
		&document_node.inputs[backup_gradient_index].as_value(),
	) {
		(fill, backup_color, backup_gradient)
	} else {
		return vec![LayoutGroup::Row { widgets: widgets_first_row }];
	};
	let fill2 = fill.clone();
	let backup_color_fill: Fill = backup_color.into();
	let backup_gradient_fill: Fill = backup_gradient.clone().into();

	widgets_first_row.push(Separator::new(SeparatorType::Unrelated).widget_holder());
	widgets_first_row.push(
		ColorInput::default()
			.value(fill.clone().into())
			.on_update(move |x: &ColorInput| {
				Message::Batched(Box::new([
					match &fill2 {
						Fill::None => NodeGraphMessage::SetInputValue {
							node_id,
							input_index: backup_color_index,
							value: TaggedValue::OptionalColor(None),
						}
						.into(),
						Fill::Solid(color) => NodeGraphMessage::SetInputValue {
							node_id,
							input_index: backup_color_index,
							value: TaggedValue::OptionalColor(Some(*color)),
						}
						.into(),
						Fill::Gradient(gradient) => NodeGraphMessage::SetInputValue {
							node_id,
							input_index: backup_gradient_index,
							value: TaggedValue::Gradient(gradient.clone()),
						}
						.into(),
					},
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: fill_index,
						value: TaggedValue::Fill(x.value.to_fill(fill2.as_gradient())),
					}
					.into(),
				]))
			})
			.on_commit(commit_value)
			.widget_holder(),
	);
	let mut widgets = vec![LayoutGroup::Row { widgets: widgets_first_row }];

	let fill_type_switch = {
		let mut row = vec![TextLabel::new("").widget_holder()];
		match fill {
			Fill::Solid(_) | Fill::None => add_blank_assist(&mut row),
			Fill::Gradient(gradient) => {
				let reverse_button = IconButton::new("Reverse", 24)
					.tooltip("Reverse the gradient color stops")
					.on_update(update_value(
						{
							let gradient = gradient.clone();
							move |_| {
								let mut gradient = gradient.clone();
								gradient.stops = gradient.stops.reversed();
								TaggedValue::Fill(Fill::Gradient(gradient))
							}
						},
						node_id,
						fill_index,
					))
					.widget_holder();
				row.push(Separator::new(SeparatorType::Unrelated).widget_holder());
				row.push(reverse_button);
			}
		}

		let entries = vec![
			RadioEntryData::new("solid")
				.label("Solid")
				.on_update(update_value(move |_| TaggedValue::Fill(backup_color_fill.clone()), node_id, fill_index))
				.on_commit(commit_value),
			RadioEntryData::new("gradient")
				.label("Gradient")
				.on_update(update_value(move |_| TaggedValue::Fill(backup_gradient_fill.clone()), node_id, fill_index))
				.on_commit(commit_value),
		];

		row.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries).selected_index(Some(if fill.as_gradient().is_some() { 1 } else { 0 })).widget_holder(),
		]);

		LayoutGroup::Row { widgets: row }
	};
	widgets.push(fill_type_switch);

	if let Fill::Gradient(gradient) = fill.clone() {
		let mut row = vec![TextLabel::new("").widget_holder()];
		match gradient.gradient_type {
			GradientType::Linear => add_blank_assist(&mut row),
			GradientType::Radial => {
				let orientation = if (gradient.end.x - gradient.start.x).abs() > f64::EPSILON * 1e6 {
					gradient.end.x > gradient.start.x
				} else {
					(gradient.start.x + gradient.start.y) < (gradient.end.x + gradient.end.y)
				};
				let reverse_radial_gradient_button = IconButton::new(if orientation { "ReverseRadialGradientToRight" } else { "ReverseRadialGradientToLeft" }, 24)
					.tooltip("Reverse which end the gradient radiates from")
					.on_update(update_value(
						{
							let gradient = gradient.clone();
							move |_| {
								let mut gradient = gradient.clone();
								std::mem::swap(&mut gradient.start, &mut gradient.end);
								TaggedValue::Fill(Fill::Gradient(gradient))
							}
						},
						node_id,
						fill_index,
					))
					.widget_holder();
				row.push(Separator::new(SeparatorType::Unrelated).widget_holder());
				row.push(reverse_radial_gradient_button);
			}
		}

		let new_gradient1 = gradient.clone();
		let new_gradient2 = gradient.clone();

		let entries = vec![
			RadioEntryData::new("Linear")
				.label("Linear")
				.on_update(update_value(
					move |_| {
						let mut new_gradient = new_gradient1.clone();
						new_gradient.gradient_type = GradientType::Linear;
						TaggedValue::Fill(Fill::Gradient(new_gradient))
					},
					node_id,
					fill_index,
				))
				.on_commit(commit_value),
			RadioEntryData::new("Radial")
				.label("Radial")
				.on_update(update_value(
					move |_| {
						let mut new_gradient = new_gradient2.clone();
						new_gradient.gradient_type = GradientType::Radial;
						TaggedValue::Fill(Fill::Gradient(new_gradient))
					},
					node_id,
					fill_index,
				))
				.on_commit(commit_value),
		];

		row.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries).selected_index(Some(gradient.gradient_type as u32)).widget_holder(),
		]);

		widgets.push(LayoutGroup::Row { widgets: row });
	}

	widgets
}

pub fn stroke_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in fill_properties: {err}");
			return Vec::new();
		}
	};
	let color_index = 1;
	let weight_index = 2;
	let dash_lengths_index = 3;
	let dash_offset_index = 4;
	let line_cap_index = 5;
	let line_join_index = 6;
	let miter_limit_index = 7;

	let color = color_widget(ParameterWidgetsInfo::from_index(document_node, node_id, color_index, true, context), ColorInput::default());
	let weight = number_widget(
		ParameterWidgetsInfo::from_index(document_node, node_id, weight_index, true, context),
		NumberInput::default().unit(" px").min(0.),
	);

	let dash_lengths_val = match &document_node.inputs[dash_lengths_index].as_value() {
		Some(TaggedValue::VecF64(x)) => x,
		_ => &vec![],
	};
	let dash_lengths = array_of_number_widget(
		ParameterWidgetsInfo::from_index(document_node, node_id, dash_lengths_index, true, context),
		TextInput::default().centered(true),
	);
	let number_input = NumberInput::default().unit(" px").disabled(dash_lengths_val.is_empty());
	let dash_offset = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, dash_offset_index, true, context), number_input);
	let line_cap = enum_choice::<LineCap>()
		.for_socket(ParameterWidgetsInfo::from_index(document_node, node_id, line_cap_index, true, context))
		.property_row();
	let line_join = enum_choice::<LineJoin>()
		.for_socket(ParameterWidgetsInfo::from_index(document_node, node_id, line_join_index, true, context))
		.property_row();
	let line_join_val = match &document_node.inputs[line_join_index].as_value() {
		Some(TaggedValue::LineJoin(x)) => x,
		_ => &LineJoin::Miter,
	};
	let miter_limit = number_widget(
		ParameterWidgetsInfo::from_index(document_node, node_id, miter_limit_index, true, context),
		NumberInput::default().min(0.).disabled(line_join_val != &LineJoin::Miter),
	);

	vec![
		color,
		LayoutGroup::Row { widgets: weight },
		LayoutGroup::Row { widgets: dash_lengths },
		LayoutGroup::Row { widgets: dash_offset },
		line_cap,
		line_join,
		LayoutGroup::Row { widgets: miter_limit },
	]
}

pub fn offset_path_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in offset_path_properties: {err}");
			return Vec::new();
		}
	};
	let distance_index = 1;
	let line_join_index = 2;
	let miter_limit_index = 3;

	let number_input = NumberInput::default().unit(" px");
	let distance = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, distance_index, true, context), number_input);

	let line_join = enum_choice::<LineJoin>()
		.for_socket(ParameterWidgetsInfo::from_index(document_node, node_id, line_join_index, true, context))
		.property_row();
	let line_join_val = match &document_node.inputs[line_join_index].as_value() {
		Some(TaggedValue::LineJoin(x)) => x,
		_ => &LineJoin::Miter,
	};

	let number_input = NumberInput::default().min(0.).disabled(line_join_val != &LineJoin::Miter);
	let miter_limit = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, miter_limit_index, true, context), number_input);

	vec![LayoutGroup::Row { widgets: distance }, line_join, LayoutGroup::Row { widgets: miter_limit }]
}

pub fn math_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in offset_path_properties: {err}");
			return Vec::new();
		}
	};

	let expression_index = 1;
	let operation_b_index = 2;

	let expression = (|| {
		let mut widgets = start_widgets(
			ParameterWidgetsInfo::from_index(document_node, node_id, expression_index, true, context),
			FrontendGraphDataType::General,
		);

		let Some(input) = document_node.inputs.get(expression_index) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				TextInput::new(x.clone())
					.centered(true)
					.on_update(update_value(
						|x: &TextInput| {
							TaggedValue::String({
								let mut expression = x.value.trim().to_string();

								if ["+", "-", "*", "/", "^", "%"].iter().any(|&infix| infix == expression) {
									expression = format!("A {} B", expression);
								} else if expression == "^" {
									expression = String::from("A^B");
								}

								expression
							})
						},
						node_id,
						expression_index,
					))
					.on_commit(commit_value)
					.widget_holder(),
			])
		}
		widgets
	})();
	let operand_b = number_widget(ParameterWidgetsInfo::from_index(document_node, node_id, operation_b_index, true, context), NumberInput::default());
	let operand_a_hint = vec![TextLabel::new("(Operand A is the primary input)").widget_holder()];

	vec![
		LayoutGroup::Row { widgets: expression }.with_tooltip(r#"A math expression that may incorporate "A" and/or "B", such as "sqrt(A + B) - B^2""#),
		LayoutGroup::Row { widgets: operand_b }.with_tooltip(r#"The value of "B" when calculating the expression"#),
		LayoutGroup::Row { widgets: operand_a_hint }.with_tooltip(r#""A" is fed by the value from the previous node in the primary data flow, or it is 0 if disconnected"#),
	]
}

pub struct ParameterWidgetsInfo<'a> {
	document_node: &'a DocumentNode,
	node_id: NodeId,
	index: usize,
	name: &'a str,
	description: &'a str,
	blank_assist: bool,
}

impl<'a> ParameterWidgetsInfo<'a> {
	pub fn new(document_node: &'a DocumentNode, node_id: NodeId, index: usize, name: &'a str, description: &'a str, blank_assist: bool) -> ParameterWidgetsInfo<'a> {
		ParameterWidgetsInfo {
			document_node,
			node_id,
			index,
			name,
			description,
			blank_assist,
		}
	}

	pub fn from_index(document_node: &'a DocumentNode, node_id: NodeId, index: usize, blank_assist: bool, context: &'a NodePropertiesContext) -> ParameterWidgetsInfo<'a> {
		let name = context.network_interface.input_name(node_id, index, context.selection_network_path).unwrap_or_default();
		let description = context.network_interface.input_description(node_id, index, context.selection_network_path).unwrap_or_default();

		Self {
			document_node,
			node_id,
			index,
			name,
			description,
			blank_assist,
		}
	}
}

pub mod choice {
	use super::ParameterWidgetsInfo;
	use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
	use crate::messages::tool::tool_messages::tool_prelude::*;
	use graph_craft::document::value::TaggedValue;
	use graphene_core::registry::{ChoiceTypeStatic, ChoiceWidgetHint};
	use std::marker::PhantomData;

	pub trait WidgetFactory {
		type Value: Clone + 'static;

		fn disabled(self, disabled: bool) -> Self;

		fn build<U, C>(&self, current: Self::Value, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetHolder
		where
			U: Fn(&Self::Value) -> Message + 'static + Send + Sync,
			C: Fn(&()) -> Message + 'static + Send + Sync;

		fn description(&self) -> Option<&str>;
	}

	pub fn enum_choice<E: ChoiceTypeStatic>() -> EnumChoice<E> {
		EnumChoice {
			disabled: false,
			phantom: PhantomData,
		}
	}

	pub struct EnumChoice<E> {
		disabled: bool,
		phantom: PhantomData<E>,
	}

	impl<E: ChoiceTypeStatic + 'static> EnumChoice<E> {
		pub fn for_socket(self, parameter_info: ParameterWidgetsInfo) -> ForSocket<Self> {
			ForSocket {
				widget_factory: self,
				parameter_info,
				exposable: true,
			}
		}

		/// Not yet implemented!
		pub fn for_value(self, _current: E) -> ForValue<Self> {
			todo!()
		}

		pub fn disabled(self, disabled: bool) -> Self {
			Self { disabled, ..self }
		}

		/// Not yet implemented!
		pub fn into_menu_entries(self, _action: impl Fn(E) -> Message + 'static + Send + Sync) -> Vec<Vec<MenuBarEntry>> {
			todo!()
		}

		fn dropdown_menu<U, C>(&self, current: E, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetHolder
		where
			U: Fn(&E) -> Message + 'static + Send + Sync,
			C: Fn(&()) -> Message + 'static + Send + Sync,
		{
			let items = E::list()
				.into_iter()
				.map(|group| {
					group
						.into_iter()
						.map(|(item, metadata)| {
							let item = item.clone();
							let updater = updater_factory();
							let committer = committer_factory();
							MenuListEntry::new(metadata.name.as_ref())
								.label(metadata.label.as_ref())
								.on_update(move |_| updater(&item))
								.on_commit(committer)
						})
						.collect()
				})
				.collect();
			DropdownInput::new(items).disabled(self.disabled).selected_index(Some(current.as_u32())).widget_holder()
		}

		fn radio_buttons<U, C>(&self, current: E, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetHolder
		where
			U: Fn(&E) -> Message + 'static + Send + Sync,
			C: Fn(&()) -> Message + 'static + Send + Sync,
		{
			let items = E::list()
				.into_iter()
				.map(|group| group.into_iter())
				.flatten()
				.map(|(item, var_meta)| {
					let item = item.clone();
					let updater = updater_factory();
					let committer = committer_factory();
					let entry = RadioEntryData::new(var_meta.name.as_ref()).on_update(move |_| updater(&item)).on_commit(committer);
					match (var_meta.icon.as_deref(), var_meta.docstring.as_deref()) {
						(None, None) => entry.label(var_meta.label.as_ref()),
						(None, Some(doc)) => entry.label(var_meta.label.as_ref()).tooltip(doc),
						(Some(icon), None) => entry.icon(icon).tooltip(var_meta.label.as_ref()),
						(Some(icon), Some(doc)) => entry.icon(icon).tooltip(format!("{}\n\n{}", var_meta.label, doc)),
					}
				})
				.collect();
			RadioInput::new(items).selected_index(Some(current.as_u32())).widget_holder()
		}
	}

	impl<E: ChoiceTypeStatic + 'static> WidgetFactory for EnumChoice<E> {
		type Value = E;

		fn disabled(self, disabled: bool) -> Self {
			Self { disabled, ..self }
		}

		fn description(&self) -> Option<&str> {
			E::DESCRIPTION
		}

		fn build<U, C>(&self, current: Self::Value, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetHolder
		where
			U: Fn(&Self::Value) -> Message + 'static + Send + Sync,
			C: Fn(&()) -> Message + 'static + Send + Sync,
		{
			match E::WIDGET_HINT {
				ChoiceWidgetHint::Dropdown => self.dropdown_menu(current, updater_factory, committer_factory),
				ChoiceWidgetHint::RadioButtons => self.radio_buttons(current, updater_factory, committer_factory),
			}
		}
	}

	pub struct ForSocket<'p, W> {
		widget_factory: W,
		parameter_info: ParameterWidgetsInfo<'p>,
		exposable: bool,
	}

	impl<'p, W> ForSocket<'p, W>
	where
		W: WidgetFactory,
		W::Value: Clone,
		for<'a> &'a W::Value: TryFrom<&'a TaggedValue>,
		TaggedValue: From<W::Value>,
	{
		pub fn disabled(self, disabled: bool) -> Self {
			Self {
				widget_factory: self.widget_factory.disabled(disabled),
				..self
			}
		}

		pub fn exposable(self, exposable: bool) -> Self {
			Self { exposable, ..self }
		}

		pub fn property_row(self) -> LayoutGroup {
			let ParameterWidgetsInfo { document_node, node_id, index, .. } = self.parameter_info;

			let mut widgets = super::start_widgets_exposable(self.parameter_info, FrontendGraphDataType::General, self.exposable);

			let Some(input) = document_node.inputs.get(index) else {
				log::warn!("A widget failed to be built because its node's input index is invalid.");
				return LayoutGroup::Row { widgets: vec![] };
			};

			let input: Option<W::Value> = input.as_non_exposed_value().and_then(|v| <&W::Value as TryFrom<&TaggedValue>>::try_from(v).ok()).map(Clone::clone);

			if let Some(current) = input {
				let committer = || super::commit_value;
				let updater = || super::update_value(move |v: &W::Value| TaggedValue::from(v.clone()), node_id, index);
				let widget = self.widget_factory.build(current, updater, committer);
				widgets.extend_from_slice(&[Separator::new(SeparatorType::Unrelated).widget_holder(), widget]);
			}

			let mut row = LayoutGroup::Row { widgets };
			if let Some(desc) = self.widget_factory.description() {
				row = row.with_tooltip(desc);
			}
			row
		}
	}

	pub struct ForValue<W>(PhantomData<W>);
}
