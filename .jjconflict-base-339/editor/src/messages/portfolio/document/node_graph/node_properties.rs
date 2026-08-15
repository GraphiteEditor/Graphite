#![allow(clippy::too_many_arguments)]

use super::document_node_definitions::{NODE_OVERRIDES, NodePropertiesContext};
use super::utility_types::FrontendGraphDataType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface};
use crate::messages::portfolio::fonts::utility_types::FontCatalogStyle;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use choice::enum_choice;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput};
use graph_craft::{Type, concrete};
use graphene_std::Graphic;
use graphene_std::NodeInputDecleration;
use graphene_std::animation::RealTimeMode;
use graphene_std::brush::brush_stroke::BrushTrace;
use graphene_std::color::SRGBA8;
use graphene_std::extract_xy::XY;
use graphene_std::list::List;
use graphene_std::raster::{
	BlendMode, CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, LuminanceCalculation, NoiseType, RedGreenBlue, RedGreenBlueAlpha, RelativeAbsolute,
	SelectiveColorChoice,
};
use graphene_std::raster_types::Image;
use graphene_std::text::{Font, TextAlign};
use graphene_std::text_nodes::StringCapitalization;
use graphene_std::transform::{Footprint, ReferencePoint, ScaleType, Transform};
use graphene_std::vector::misc::BooleanOperation;
use graphene_std::vector::misc::{
	ArcType, BoxCorners, CentroidType, ExtrudeJoiningAlgorithm, GridType, InterpolationDistribution, MergeByDistanceAlgorithm, PointSpacingType, RowsOrColumns, SpiralType,
};
use graphene_std::vector::style::{
	DashPattern, FillChoiceUI, Gradient, GradientSpreadMethod, GradientType, GradientUI, PaintOrder, StrokeAlign, StrokeCap, StrokeJoin, build_transform_with_y_preservation,
};
use graphene_std::vector::{QRCodeErrorCorrectionLevel, VectorModification};

pub(crate) fn string_properties(text: &str) -> Vec<LayoutGroup> {
	let widget = TextLabel::new(text).widget_instance();
	vec![LayoutGroup::row(vec![widget])]
}

fn optionally_update_value<T>(value: impl Fn(&T) -> Option<TaggedValue> + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	move |input_value: &T| match value(input_value) {
		Some(value) => NodeGraphMessage::SetInputValue { node_id, input_index, value }.into(),
		None => Message::NoOp,
	}
}

pub fn update_value<T>(value: impl Fn(&T) -> TaggedValue + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	optionally_update_value(move |v| Some(value(v)), node_id, input_index)
}

pub fn commit_value<T>(_: &T) -> Message {
	DocumentMessage::AddTransaction.into()
}

pub fn expose_widget(node_id: NodeId, index: usize, data_type: FrontendGraphDataType, exposed: bool) -> WidgetInstance {
	ParameterExposeButton::new()
		.exposed(exposed)
		.data_type(data_type)
		.tooltip_description(if exposed {
			"Stop exposing this parameter as a node input in the graph."
		} else {
			"Expose this parameter as a node input in the graph."
		})
		.on_update(move |_parameter| Message::Batched {
			messages: Box::new([NodeGraphMessage::ExposeInput {
				input_connector: InputConnector::node(node_id, index),
				set_to_exposed: !exposed,
				start_transaction: true,
			}
			.into()]),
		})
		.widget_instance()
}

// TODO: Remove this when we have proper entry row formatting that includes room for Assists.
pub fn add_blank_assist(widgets: &mut Vec<WidgetInstance>) {
	widgets.extend_from_slice(&[
		// Custom CSS specific to the Properties panel converts this Section separator into the width of an assist (24px).
		Separator::new(SeparatorStyle::Section).widget_instance(),
		// This last one is the separator after the 24px assist.
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
	]);
}

pub fn jump_to_source_widget(input: &NodeInput, network_interface: &NodeNetworkInterface, selection_network_path: &[NodeId]) -> WidgetInstance {
	match input {
		NodeInput::Node { node_id: source_id, .. } => {
			let source_id = *source_id;
			let node_name = network_interface.implementation_name(&source_id, selection_network_path);
			TextButton::new(format!("From Graph ({})", node_name))
				.tooltip_description("Click to select the node producing this parameter's data.")
				.on_update(move |_| NodeGraphMessage::SelectedNodesSet { nodes: vec![source_id] }.into())
				.widget_instance()
		}
		_ => TextLabel::new("From Graph (Disconnected)")
			.tooltip_description(
				"
				This parameter is exposed as an input in the node graph, but not currently receiving data from any node.\n\
				\n\
				In the graph, drag a wire out from a compatible output connector of another node, and feed it into the input connector of this exposed node parameter. Alternatively, un-expose this parameter by clicking the triangle directly to the left of here.
				"
				.trim(),
			)
			.widget_instance(),
	}
}

pub fn start_widgets(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo {
		document_node,
		node_id,
		index,
		name,
		description,
		input_type,
		blank_assist,
		exposable,
		network_interface,
		selection_network_path,
		..
	} = parameter_widgets_info;

	let Some(document_node) = document_node else {
		log::warn!("A widget failed to be built because its document node is invalid.");
		return vec![];
	};

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	let mut widgets = Vec::with_capacity(6);
	if exposable {
		widgets.push(expose_widget(node_id, index, input_type, input.is_exposed()));
	}
	widgets.push(TextLabel::new(name).tooltip_description(description).widget_instance());

	if blank_assist || input.is_exposed() {
		add_blank_assist(&mut widgets);
	}

	if input.is_exposed() {
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.push(jump_to_source_widget(input, network_interface, selection_network_path));
	}

	widgets
}

/// The numeric bounds and widget mode of a number parameter, sourced from the node's field metadata.
#[derive(Clone, Copy, Default)]
pub(crate) struct NumberOptions {
	pub soft_min: Option<f64>,
	pub soft_max: Option<f64>,
	pub hard_min: Option<f64>,
	pub hard_max: Option<f64>,
	pub slider: bool,
}

pub(crate) fn property_from_type(
	node_id: NodeId,
	index: usize,
	ty: &Type,
	number_options: NumberOptions,
	unit: Option<&str>,
	display_decimal_places: Option<u32>,
	step: Option<f64>,
	context: &mut NodePropertiesContext,
) -> Result<Vec<LayoutGroup>, Vec<LayoutGroup>> {
	let NumberOptions {
		soft_min,
		soft_max,
		hard_min,
		hard_max,
		slider,
	} = number_options;
	let mut number_input = NumberInput::default();
	if slider {
		number_input = number_input.mode_range();
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

	// Applies the parameter's typing clamp and slider extent to the widget, given the type's own default bounds.
	// Per end: the clamp is the hard bound (or unbounded if only a soft bound is given, since soft is a suggested
	// extent rather than a limit), and the slider extent is the soft bound, each falling back to the hard bound
	// and then to the type default when unspecified. An end with any explicit bound ignores the type default.
	let bounded = |number_input: NumberInput, type_min: f64, type_max: f64| {
		let clamp_min = hard_min.unwrap_or(if soft_min.is_some() { f64::NEG_INFINITY } else { type_min });
		let clamp_max = hard_max.unwrap_or(if soft_max.is_some() { f64::INFINITY } else { type_max });
		let extent_min = soft_min.or(hard_min).unwrap_or(type_min);
		let extent_max = soft_max.or(hard_max).unwrap_or(type_max);

		number_input
			.min(clamp_min)
			.max(clamp_max)
			.range_min(Some(extent_min).filter(|bound| bound.is_finite()))
			.range_max(Some(extent_max).filter(|bound| bound.is_finite()))
	};

	let default_info = ParameterWidgetsInfo::new(node_id, index, true, context);

	// A type with no widget can only be supplied through the graph, labeled with a placeholder row
	let unsupported_widgets = |default_info: ParameterWidgetsInfo, type_label: String| {
		let is_exposed = default_info.is_exposed();

		let mut widgets = start_widgets(default_info);
		if !is_exposed {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				TextLabel::new("-")
					.tooltip_label(type_label)
					.tooltip_description("This data can only be supplied through the node graph because no widget exists for its type.")
					.widget_instance(),
			]);
		}

		vec![LayoutGroup::from(widgets)]
	};

	let mut extra_widgets = vec![];
	let widgets = match ty {
		Type::Concrete(concrete_type) => {
			match concrete_type.alias.as_ref().map(|x| x.as_ref()) {
				// Aliased types (ambiguous values)
				Some("Percentage") | Some("PercentageF32") => number_widget(default_info, bounded(number_input.percentage(), 0., 100.)).into(),
				Some("SignedPercentage") | Some("SignedPercentageF32") => number_widget(default_info, bounded(number_input.percentage(), -100., 100.)).into(),
				Some("Angle") | Some("AngleF32") => number_widget(default_info, bounded(number_input.mode_range(), -180., 180.).unit(unit.unwrap_or("°"))).into(),
				Some("Multiplier") => number_widget(default_info, bounded(number_input, f64::NEG_INFINITY, f64::INFINITY).unit(unit.unwrap_or("x"))).into(),
				Some("PixelLength") => number_widget(default_info, bounded(number_input, 0., f64::INFINITY).unit(unit.unwrap_or(" px"))).into(),
				Some("Length") => number_widget(default_info, bounded(number_input, 0., f64::INFINITY)).into(),
				Some("Fraction") => number_widget(default_info, bounded(number_input.mode_range(), 0., 1.)).into(),
				Some("Progression") => progression_widget(default_info, bounded(number_input, 0., f64::INFINITY)).into(),
				Some("SignedInteger") => number_widget(default_info, bounded(number_input.int(), f64::NEG_INFINITY, f64::INFINITY)).into(),
				Some("SeedValue") => number_widget(default_info, bounded(number_input.int(), 0., f64::INFINITY)).into(),
				Some("PixelSize") => vec2_widget(default_info, "X", "Y", unit.unwrap_or(" px"), None, false),
				Some("TextArea") => text_area_widget(default_info).into(),

				// For all other types, use TypeId-based matching
				_ => {
					use std::any::TypeId;

					// The compiler peels a rank-0 `Item` cell to its element before this arm runs, so widgets dispatch on the bare element `T`
					fn id_is<T: 'static>(id: TypeId) -> bool {
						id == TypeId::of::<T>()
					}

					match concrete_type.id {
						// ===============
						// PRIMITIVE TYPES
						// ===============
						Some(x) if id_is::<f64>(x) || id_is::<f32>(x) => number_widget(default_info, bounded(number_input, f64::NEG_INFINITY, f64::INFINITY)).into(),
						Some(x) if id_is::<u32>(x) => number_widget(default_info, bounded(number_input.int(), 0., f64::from(u32::MAX))).into(),
						Some(x) if id_is::<u64>(x) => number_widget(default_info, bounded(number_input.int(), 0., f64::INFINITY)).into(),
						Some(x) if id_is::<bool>(x) => bool_widget(default_info, CheckboxInput::default()).into(),
						Some(x) if id_is::<String>(x) => text_widget(default_info).into(),
						Some(x) if id_is::<DVec2>(x) => vec2_widget(default_info, "X", "Y", "", None, false),
						Some(x) if id_is::<DAffine2>(x) => transform_widget(default_info, &mut extra_widgets),
						Some(x) if id_is::<Color>(x) => color_widget(default_info, ColorInput::default().allow_none(false)),
						Some(x) if id_is::<Gradient>(x) => color_widget(default_info, ColorInput::default().allow_none(false)),
						Some(x) if id_is::<BrushTrace>(x) => brush_strokes_widget(default_info).into(),
						// ============
						// STRUCT TYPES
						// ============
						Some(x) if id_is::<Font>(x) => font_widget(default_info),
						Some(x) if id_is::<Footprint>(x) => footprint_widget(default_info, &mut extra_widgets),
						Some(x) if id_is::<Box<VectorModification>>(x) => vector_modification_widget(default_info).into(),
						Some(x) if id_is::<Image<Color>>(x) => image_data_widget(default_info).into(),
						// ===============================
						// MANUALLY IMPLEMENTED ENUM TYPES
						// ===============================
						Some(x) if id_is::<ReferencePoint>(x) => reference_point_widget(default_info, false).into(),
						Some(x) if id_is::<BlendMode>(x) => blend_mode_widget(default_info),
						// =========================
						// AUTO-GENERATED ENUM TYPES
						// =========================
						Some(x) if id_is::<GradientType>(x) => enum_choice::<GradientType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<GradientSpreadMethod>(x) => enum_choice::<GradientSpreadMethod>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RealTimeMode>(x) => enum_choice::<RealTimeMode>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RedGreenBlue>(x) => enum_choice::<RedGreenBlue>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RedGreenBlueAlpha>(x) => enum_choice::<RedGreenBlueAlpha>().for_socket(default_info).property_row(),
						Some(x) if id_is::<XY>(x) => enum_choice::<XY>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StringCapitalization>(x) => enum_choice::<StringCapitalization>().for_socket(default_info).property_row(),
						Some(x) if id_is::<NoiseType>(x) => enum_choice::<NoiseType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<FractalType>(x) => enum_choice::<FractalType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<CellularDistanceFunction>(x) => enum_choice::<CellularDistanceFunction>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<CellularReturnType>(x) => enum_choice::<CellularReturnType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<DomainWarpType>(x) => enum_choice::<DomainWarpType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<RelativeAbsolute>(x) => enum_choice::<RelativeAbsolute>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<GridType>(x) => enum_choice::<GridType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StrokeCap>(x) => enum_choice::<StrokeCap>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StrokeJoin>(x) => enum_choice::<StrokeJoin>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StrokeAlign>(x) => enum_choice::<StrokeAlign>().for_socket(default_info).property_row(),
						Some(x) if id_is::<PaintOrder>(x) => enum_choice::<PaintOrder>().for_socket(default_info).property_row(),
						Some(x) if id_is::<ArcType>(x) => enum_choice::<ArcType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RowsOrColumns>(x) => enum_choice::<RowsOrColumns>().for_socket(default_info).property_row(),
						Some(x) if id_is::<TextAlign>(x) => enum_choice::<TextAlign>().for_socket(default_info).property_row(),
						Some(x) if id_is::<MergeByDistanceAlgorithm>(x) => enum_choice::<MergeByDistanceAlgorithm>().for_socket(default_info).property_row(),
						Some(x) if id_is::<ExtrudeJoiningAlgorithm>(x) => enum_choice::<ExtrudeJoiningAlgorithm>().for_socket(default_info).property_row(),
						Some(x) if id_is::<PointSpacingType>(x) => enum_choice::<PointSpacingType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<BooleanOperation>(x) => enum_choice::<BooleanOperation>().for_socket(default_info).property_row(),
						Some(x) if id_is::<CentroidType>(x) => enum_choice::<CentroidType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<LuminanceCalculation>(x) => enum_choice::<LuminanceCalculation>().for_socket(default_info).property_row(),
						Some(x) if id_is::<QRCodeErrorCorrectionLevel>(x) => enum_choice::<QRCodeErrorCorrectionLevel>().for_socket(default_info).property_row(),
						Some(x) if id_is::<ScaleType>(x) => enum_choice::<ScaleType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<InterpolationDistribution>(x) => enum_choice::<InterpolationDistribution>().for_socket(default_info).property_row(),
						// =====
						// OTHER
						// =====
						_ => return Err(unsupported_widgets(default_info, concrete_type.to_string())),
					}
				}
			}
		}
		Type::Item(element) => return property_from_type(node_id, index, element, number_options, unit, display_decimal_places, step, context),
		Type::List(element) => match element.as_ref() {
			Type::Concrete(element_type) if element_type.name == std::any::type_name::<f64>() => array_of_number_widget(default_info, TextInput::default()).into(),
			_ => return Err(unsupported_widgets(default_info, ty.to_string())),
		},
		Type::Generic(_) => vec![TextLabel::new("Generic Type (Not Supported)").widget_instance()].into(),
		Type::Fn(_, out) => return property_from_type(node_id, index, out, number_options, unit, display_decimal_places, step, context),
		Type::Future(out) => return property_from_type(node_id, index, out, number_options, unit, display_decimal_places, step, context),
	};

	extra_widgets.push(widgets);

	Ok(extra_widgets)
}

pub fn text_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(x.clone())
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

pub fn text_area_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextAreaInput::new(x.clone())
				.on_update(update_value(|x: &TextAreaInput| TaggedValue::String(x.value.clone()), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

pub fn bool_widget(parameter_widgets_info: ParameterWidgetsInfo, checkbox_input: CheckboxInput) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(x)) = input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			checkbox_input
				.checked(x)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

pub fn reference_point_widget(parameter_widgets_info: ParameterWidgetsInfo, disabled: bool) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::ReferencePoint(reference_point)) = input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			CheckboxInput::new(reference_point != ReferencePoint::None)
				.on_update(update_value(
					move |x: &CheckboxInput| TaggedValue::ReferencePoint(if x.checked { ReferencePoint::Center } else { ReferencePoint::None }),
					node_id,
					index,
				))
				.disabled(disabled)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			ReferencePointInput::new(reference_point)
				.on_update(update_value(move |x: &ReferencePointInput| TaggedValue::ReferencePoint(x.value), node_id, index))
				.disabled(disabled)
				.widget_instance(),
		])
	}
	widgets
}

pub fn vector_modification_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id: _, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return widgets };
	let Some(input) = document_node.inputs.get(index) else { return widgets };

	if let Some(TaggedValue::VectorModification(modification)) = input.as_non_exposed_value() {
		let label = modification.summary_label();
		let tooltip = modification.summary_tooltip();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextLabel::new(label).tooltip_label("Summary of Differential Edits").tooltip_description(tooltip).widget_instance(),
		]);
	}

	widgets
}

pub fn brush_strokes_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id: _, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return widgets };
	let Some(input) = document_node.inputs.get(index) else { return widgets };

	if let Some(TaggedValue::BrushStrokes(strokes)) = input.as_non_exposed_value() {
		let stroke_count = strokes.len();
		let sample_count: usize = strokes.iter().map(|s| s.trace.len()).sum();
		let label = if stroke_count == 0 {
			"Empty".to_string()
		} else {
			format!(
				"{stroke_count} {} / {sample_count} {}",
				if stroke_count == 1 { "Stroke" } else { "Strokes" },
				if sample_count == 1 { "Sample" } else { "Samples" }
			)
		};

		widgets.extend_from_slice(&[Separator::new(SeparatorStyle::Unrelated).widget_instance(), TextLabel::new(label).widget_instance()]);
	}

	widgets
}

pub fn image_data_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id: _, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return widgets };
	let Some(input) = document_node.inputs.get(index) else { return widgets };

	if let Some(TaggedValue::ImageData(image)) = input.as_non_exposed_value() {
		let label = format!("{} x {}", image.width, image.height);

		widgets.extend_from_slice(&[Separator::new(SeparatorStyle::Unrelated).widget_instance(), TextLabel::new(label).widget_instance()]);
	}

	widgets
}

pub fn footprint_widget(parameter_widgets_info: ParameterWidgetsInfo, extra_widgets: &mut Vec<LayoutGroup>) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut location_widgets = start_widgets(parameter_widgets_info);
	location_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	let mut scale_widgets = vec![TextLabel::new("").widget_instance()];
	add_blank_assist(&mut scale_widgets);
	scale_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	let mut resolution_widgets = vec![TextLabel::new("").widget_instance()];
	add_blank_assist(&mut resolution_widgets);
	resolution_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	let Some(document_node) = document_node else { return LayoutGroup::default() };
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
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
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
				.widget_instance(),
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
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
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
				.widget_instance(),
		]);

		resolution_widgets.push(
			NumberInput::new(Some((footprint.resolution.as_dvec2() / bounds).x * 100.))
				.label("Resolution")
				.mode_range()
				.min(0.)
				.range_min(Some(1.))
				.range_max(Some(100.))
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
				.widget_instance(),
		);
	}

	let widgets = [LayoutGroup::row(location_widgets), LayoutGroup::row(scale_widgets), LayoutGroup::row(resolution_widgets)];
	let (last, rest) = widgets.split_last().expect("Footprint widget should return multiple rows");
	*extra_widgets = rest.to_vec();
	last.clone()
}

pub fn transform_widget(parameter_widgets_info: ParameterWidgetsInfo, extra_widgets: &mut Vec<LayoutGroup>) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut location_widgets = start_widgets(parameter_widgets_info);
	location_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	let mut rotation_widgets = vec![TextLabel::new("").widget_instance()];
	add_blank_assist(&mut rotation_widgets);
	rotation_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	let mut scale_widgets = vec![TextLabel::new("").widget_instance()];
	add_blank_assist(&mut scale_widgets);
	scale_widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	let Some(document_node) = document_node else { return LayoutGroup::default() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return Vec::new().into();
	};

	let widgets = if let Some(&TaggedValue::DAffine2(transform)) = input.as_non_exposed_value() {
		let translation = transform.translation;
		let (rotation, scale, skew) = transform.decompose_rotation_scale_skew();
		let skew_matrix = DAffine2::from_cols_array(&[1., 0., skew, 1., 0., 0.]);

		location_widgets.extend_from_slice(&[
			NumberInput::new(Some(translation.x))
				.label("X")
				.unit(" px")
				.on_update(update_value(
					move |x: &NumberInput| {
						let mut transform = transform;
						transform.translation.x = x.value.unwrap_or(transform.translation.x);
						TaggedValue::DAffine2(transform)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(Some(translation.y))
				.label("Y")
				.unit(" px")
				.on_update(update_value(
					move |y: &NumberInput| {
						let mut transform = transform;
						transform.translation.y = y.value.unwrap_or(transform.translation.y);
						TaggedValue::DAffine2(transform)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_instance(),
		]);

		rotation_widgets.extend_from_slice(&[NumberInput::new(Some(rotation.to_degrees()))
			.unit("°")
			.mode(NumberInputMode::Range)
			.range_min(Some(-180.))
			.range_max(Some(180.))
			.on_update(update_value(
				move |r: &NumberInput| {
					let transform = DAffine2::from_scale_angle_translation(scale, r.value.map(|r| r.to_radians()).unwrap_or(rotation), translation) * skew_matrix;
					TaggedValue::DAffine2(transform)
				},
				node_id,
				index,
			))
			.on_commit(commit_value)
			.widget_instance()]);

		scale_widgets.extend_from_slice(&[
			NumberInput::new(Some(scale.x))
				.label("W")
				.unit("x")
				.on_update(update_value(
					move |w: &NumberInput| {
						let transform = DAffine2::from_scale_angle_translation(DVec2::new(w.value.unwrap_or(scale.x), scale.y), rotation, translation) * skew_matrix;
						TaggedValue::DAffine2(transform)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(Some(scale.y))
				.label("H")
				.unit("x")
				.on_update(update_value(
					move |h: &NumberInput| {
						let transform = DAffine2::from_scale_angle_translation(DVec2::new(scale.x, h.value.unwrap_or(scale.y)), rotation, translation) * skew_matrix;
						TaggedValue::DAffine2(transform)
					},
					node_id,
					index,
				))
				.on_commit(commit_value)
				.widget_instance(),
		]);

		vec![LayoutGroup::row(location_widgets), LayoutGroup::row(rotation_widgets), LayoutGroup::row(scale_widgets)]
	} else {
		vec![LayoutGroup::row(location_widgets)]
	};

	if let Some((last, rest)) = widgets.split_last() {
		*extra_widgets = rest.to_vec();
		last.clone()
	} else {
		LayoutGroup::default()
	}
}

pub fn vec2_widget(parameter_widgets_info: ParameterWidgetsInfo, x: &str, y: &str, unit: &str, min: Option<f64>, is_integer: bool) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return LayoutGroup::default() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::row(vec![]);
	};
	match input.as_non_exposed_value() {
		Some(&TaggedValue::DVec2(dvec2)) => {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(dvec2.x))
					.label(x)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.is_integer(is_integer)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), dvec2.y)), node_id, index))
					.on_commit(commit_value)
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				NumberInput::new(Some(dvec2.y))
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.is_integer(is_integer)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(dvec2.x, input.value.unwrap())), node_id, index))
					.on_commit(commit_value)
					.widget_instance(),
			]);
		}
		Some(&TaggedValue::F64(value)) => {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(value))
					.label(x)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.is_integer(is_integer)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), value)), node_id, index))
					.on_commit(commit_value)
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				NumberInput::new(Some(value))
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.is_integer(is_integer)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(value, input.value.unwrap())), node_id, index))
					.on_commit(commit_value)
					.widget_instance(),
			]);
		}
		_ => {}
	}

	LayoutGroup::row(widgets)
}

pub fn array_of_number_widget(parameter_widgets_info: ParameterWidgetsInfo, text_input: TextInput) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let from_string = |string: &str| {
		string
			.split(&[',', ' '])
			.filter(|x| !x.is_empty())
			.map(str::parse::<f64>)
			.collect::<Result<Vec<_>, _>>()
			.ok()
			.map(TaggedValue::F64Array)
	};

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::F64Array(values)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			text_input
				.value(values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
				.on_update(optionally_update_value(move |x: &TextInput| from_string(&x.value), node_id, index))
				.widget_instance(),
		])
	}
	widgets
}

pub fn dash_pattern_widget(parameter_widgets_info: ParameterWidgetsInfo, text_input: TextInput) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::DashPattern(pattern)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			text_input
				.value(pattern.0.iter_element_values().map(|length| length.to_string()).collect::<Vec<_>>().join(", "))
				.on_update(optionally_update_value(
					move |input: &TextInput| Some(TaggedValue::DashPattern(DashPattern::from(input.value.as_str()))),
					node_id,
					index,
				))
				.widget_instance(),
		])
	}
	widgets
}

pub fn font_inputs(parameter_widgets_info: ParameterWidgetsInfo) -> (Vec<WidgetInstance>, Option<Vec<WidgetInstance>>) {
	pub fn assign_font_message(node_id: NodeId, font: Font) -> Message {
		let resource_id = ResourceId::new();
		Message::Batched {
			messages: Box::new([
				DocumentMessage::Resource(ResourceMessage::AddFont { resource_id, font }).into(),
				NodeGraphMessage::SetInputValue {
					node_id,
					input_index: graphene_std::text::text::FontInput::INDEX,
					value: TaggedValue::Resource(resource_id),
				}
				.into(),
			]),
		}
	}

	let ParameterWidgetsInfo {
		document_node,
		node_id,
		index,
		resources,
		fonts,
		..
	} = parameter_widgets_info;

	let mut first_widgets = start_widgets(parameter_widgets_info);
	let mut second_widgets = None;

	let Some(document_node) = document_node else { return (Vec::new(), None) };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return (vec![], None);
	};

	// A freshly added node carries the empty-resource `TypeDefault` placeholder until a font is chosen
	let font = match input.as_non_exposed_value() {
		Some(TaggedValue::Resource(resource_id)) => fonts.id_font(resources, *resource_id).unwrap_or_default(),
		Some(TaggedValue::TypeDefault(_)) => Font::default(),
		_ => return (first_widgets, second_widgets),
	};
	{
		first_widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			DropdownInput::new(vec![
				fonts
					.font_catalog
					.iter()
					.map(|family| {
						let FontCatalogStyle { weight, italic, .. } = FontCatalogStyle::from_named_style(&font.font_style, "");
						let new_font = Font::new(family.name.clone(), family.closest_style(weight, italic).to_named_style());
						let commit_font = new_font.clone();
						MenuListEntry::new(family.name.clone())
							.label(family.name.clone())
							.font(family.closest_style(400, false).preview_url(&family.name))
							.on_update(move |_| assign_font_message(node_id, new_font.clone()))
							.on_commit(move |_| {
								DeferMessage::AfterGraphRun {
									messages: vec![assign_font_message(node_id, commit_font.clone()), commit_value(&())],
								}
								.into()
							})
					})
					.collect::<Vec<_>>(),
			])
			.selected_index(fonts.font_catalog.iter().position(|family| family.name == font.font_family).map(|i| i as u32))
			.virtual_scrolling(true)
			.widget_instance(),
		]);

		let mut second_row = vec![TextLabel::new("").widget_instance()];
		add_blank_assist(&mut second_row);
		second_row.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			DropdownInput::new({
				fonts
					.font_catalog
					.iter()
					.find(|family| family.name == font.font_family)
					.map(|family| {
						let build_entry = |style: &FontCatalogStyle| {
							let font_style = style.to_named_style();
							let font_family = font.font_family.clone();
							let new_font = Font::new(font_family, font_style.clone());
							MenuListEntry::new(font_style.clone())
								.label(font_style)
								.on_update(move |_| assign_font_message(node_id, new_font.clone()))
								.on_commit(commit_value)
						};

						vec![
							family.styles.iter().filter(|style| !style.italic).map(build_entry).collect::<Vec<_>>(),
							family.styles.iter().filter(|style| style.italic).map(build_entry).collect::<Vec<_>>(),
						]
					})
					.filter(|styles| !styles.is_empty())
					.unwrap_or_default()
			})
			.selected_index(
				fonts
					.font_catalog
					.iter()
					.find(|family| family.name == font.font_family)
					.and_then(|family| {
						let not_italic = family.styles.iter().filter(|style| !style.italic);
						let italic = family.styles.iter().filter(|style| style.italic);
						not_italic.chain(italic).position(|style| style.to_named_style() == font.font_style)
					})
					.map(|i| i as u32),
			)
			.widget_instance(),
		]);
		second_widgets = Some(second_row);
	}
	(first_widgets, second_widgets)
}

// Two number fields beside one another, the first for the fractional part (decimals, range mode) and the second for the whole part (integers, increment mode)
pub fn progression_widget(parameter_widgets_info: ParameterWidgetsInfo, number_props: NumberInput) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::F64(x)) = input.as_non_exposed_value() {
		let whole_part = x.trunc();
		let fractional_part = x.fract();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.clone()
				.label("Progress")
				.mode_range()
				.min(0.)
				.max(0.99999)
				.value(Some(fractional_part))
				.on_update(update_value(move |input: &NumberInput| TaggedValue::F64(whole_part + input.value.unwrap()), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			TextLabel::new("+").widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			number_props
				.label("Element #")
				.mode_increment()
				.min(0.)
				.is_integer(true)
				.value(Some(whole_part))
				.on_update(update_value(move |input: &NumberInput| TaggedValue::F64(input.value.unwrap() + fractional_part), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

/// `parameter_widgets_info` is for the f64 parameter. `bool_input_index` is the input index of the bool parameter for the checkbox.
pub fn optional_f64_widget(parameter_widgets_info: ParameterWidgetsInfo, bool_input_index: usize, number_props: NumberInput) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo {
		document_node,
		node_id,
		index: number_input_index,
		..
	} = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(number_input) = document_node.inputs.get(number_input_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	let Some(bool_input) = document_node.inputs.get(bool_input_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let (Some(&TaggedValue::Bool(enabled)), Some(&TaggedValue::F64(number))) = (bool_input.as_non_exposed_value(), number_input.as_non_exposed_value()) {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			// The checkbox toggles if the value is Some or None
			CheckboxInput::new(enabled)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, bool_input_index))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.value(Some(number))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap_or_default()), node_id, number_input_index))
				.disabled(!enabled)
				.on_commit(commit_value)
				.widget_instance(),
		]);
	}

	widgets
}

pub fn number_widget(parameter_widgets_info: ParameterWidgetsInfo, number_props: NumberInput) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return Vec::new() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	match input.as_non_exposed_value() {
		Some(&TaggedValue::F64(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.value(Some(x))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		Some(&TaggedValue::F32(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F32(x.value.unwrap() as f32), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		Some(&TaggedValue::U32(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::U32((x.value.unwrap()) as u32), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		Some(&TaggedValue::U64(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::U64((x.value.unwrap()) as u64), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		Some(&TaggedValue::DVec2(dvec2)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
			// We use an arbitrary `y` instead of an arbitrary `x` here because the "Grid" node's "Spacing" value's height should be used from rectangular mode when transferred to "Y Spacing" in isometric mode
				.value(Some(dvec2.y))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		_ => {}
	}

	widgets
}

// TODO: Auto-generate this enum dropdown menu widget
pub fn blend_mode_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);
	let Some(document_node) = document_node else { return LayoutGroup::default() };
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::row(vec![]);
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
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			DropdownInput::new(entries)
				.selected_index(blend_mode.index_in_list_svg_subset().map(|index| index as u32))
				.widget_instance(),
		]);
	}
	LayoutGroup::row(widgets).with_tooltip_description("Formula used for blending.")
}

pub fn color_widget(parameter_widgets_info: ParameterWidgetsInfo, color_button: ColorInput) -> LayoutGroup {
	let ParameterWidgetsInfo { document_node, node_id, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(parameter_widgets_info);

	let Some(document_node) = document_node else { return LayoutGroup::default() };
	// Return early with just the label if the input is exposed to the graph, meaning we don't want to show the color picker widget in the Properties panel
	let NodeInput::Value { tagged_value, exposed: false } = &document_node.inputs[index] else {
		return LayoutGroup::row(widgets);
	};

	// Add a separator
	widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	// Add the color input
	let widget_value = match &**tagged_value {
		TaggedValue::Color(color) => FillChoiceUI::Solid(SRGBA8::from(*color)),
		TaggedValue::Gradient(stops) => FillChoiceUI::Gradient(GradientUI::from(stops)),
		value if value.is_no_paint() => FillChoiceUI::None,
		x => {
			warn!("Color {x:?}");
			return LayoutGroup::row(widgets);
		}
	};

	// A paint input (`allow_none`) stores the pick as a plain color, gradient, or no-paint type default,
	// while a plain color or gradient input always keeps its own value type
	let on_update: fn(&ColorInput) -> TaggedValue = if color_button.allow_none {
		|input| match &input.value {
			FillChoiceUI::None => TaggedValue::no_paint(),
			FillChoiceUI::Solid(srgba) => TaggedValue::Color(Color::from(*srgba)),
			FillChoiceUI::Gradient(gradient_ui) => TaggedValue::Gradient(Gradient::from(gradient_ui)),
		}
	} else if matches!(&**tagged_value, TaggedValue::Gradient(_)) {
		|input| TaggedValue::Gradient(input.value.as_gradient().map(Gradient::from).unwrap_or_default())
	} else {
		|input| TaggedValue::Color(input.value.as_solid().map(Color::from).unwrap_or(Color::TRANSPARENT))
	};

	widgets.push(
		color_button
			.value(widget_value)
			.on_update(update_value(on_update, node_id, index))
			.on_commit(commit_value)
			.widget_instance(),
	);

	LayoutGroup::row(widgets)
}

pub fn font_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let (font_widgets, style_widgets) = font_inputs(parameter_widgets_info);
	font_widgets.into_iter().chain(style_widgets.unwrap_or_default()).collect::<Vec<_>>().into()
}

pub fn get_document_node<'a>(node_id: NodeId, context: &'a NodePropertiesContext<'a>) -> Result<&'a DocumentNode, String> {
	let network = context
		.network_interface
		.nested_network(context.selection_network_path)
		.ok_or("network not found in get_document_node")?;
	network.nodes.get(&node_id).ok_or(format!("node {node_id} not found in get_document_node"))
}

pub fn query_node_and_input_info<'a>(node_id: NodeId, input_index: usize, context: &'a mut NodePropertiesContext<'a>) -> Result<(&'a DocumentNode, String, String), String> {
	let (name, description) = context.network_interface.displayed_input_name_and_description(&node_id, input_index, context.selection_network_path);
	let document_node = get_document_node(node_id, context)?;

	Ok((document_node, name, description))
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
	use graphene_std::vector::assign_colors::*;

	let document_node = get_document_node(node_id, context)?;
	// This is safe since the node is a proto node and the implementation cannot be changed.
	Ok(match document_node.inputs.get(RandomizeInput::INDEX).and_then(|input| input.as_value()) {
		Some(TaggedValue::Bool(randomize_enabled)) => *randomize_enabled,
		_ => false,
	})
}

/// 2-stop black-to-white gradient track for spectrum sliders that map a value to a grayscale axis.
fn bw_track() -> Gradient {
	Gradient {
		position: vec![0., 1.],
		midpoint: vec![0.5, 0.5],
		color: vec![Color::BLACK, Color::WHITE],
	}
}

/// 3-stop black-to-color-to-white gradient track for spectrum sliders that map a value to a hue's full luminance range.
fn color_track(color: Color) -> Gradient {
	Gradient {
		position: vec![0., 0.5, 1.],
		midpoint: vec![0.5; 3],
		color: vec![Color::BLACK, color, Color::WHITE],
	}
}

pub(crate) fn brightness_contrast_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::brightness_contrast::*;

	// Use Classic toggle changes the brightness range
	let use_classic_value = get_document_node(node_id, context)
		.ok()
		.and_then(|document_node| document_node.inputs.get(UseClassicInput::INDEX).and_then(|input| input.as_value()))
		.and_then(|tagged| if let TaggedValue::Bool(value) = tagged { Some(*value) } else { None });
	let includes_use_classic = use_classic_value.is_some();
	let use_classic_value = use_classic_value.unwrap_or(false);

	let brightness_min = if use_classic_value { -100. } else { -150. };
	let brightness_max = if use_classic_value { 100. } else { 150. };

	let brightness = spectrum_slider_row(
		node_id,
		context,
		BrightnessInput::INDEX,
		bw_track(),
		Color::WHITE,
		brightness_min,
		brightness_max,
		0.,
		NumberInput::default().mode_increment().unit("%").min(brightness_min).max(brightness_max),
	);

	let contrast_min = if use_classic_value { -100. } else { -50. };
	let zero_position = -contrast_min / (100. - contrast_min);
	let contrast_track = Gradient {
		position: vec![0., zero_position, 1.],
		midpoint: vec![0.5; 3],
		color: vec![Color::from_rgbf32_unchecked(0.5, 0.5, 0.5), Color::BLACK, Color::from_rgbf32_unchecked(0.5, 0.5, 0.5)],
	};
	let contrast = spectrum_slider_row(
		node_id,
		context,
		ContrastInput::INDEX,
		contrast_track,
		Color::WHITE,
		contrast_min,
		100.,
		0.,
		NumberInput::default().mode_increment().unit("%").min(contrast_min).max(100.),
	);

	let mut layout = vec![brightness, contrast];
	if includes_use_classic {
		// TODO: When we no longer use this function in the temporary "Brightness/Contrast Classic" node, remove this conditional pushing and just always include this
		let use_classic = bool_widget(ParameterWidgetsInfo::new(node_id, UseClassicInput::INDEX, true, context), CheckboxInput::default());
		layout.push(LayoutGroup::row(use_classic));
	}

	layout
}

pub(crate) fn levels_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::levels::*;

	// (input index, marker handle color, default percentage for double-click reset)
	let input_range_params = [
		(ShadowsInput::INDEX, Color::BLACK, 0.),
		(MidtonesInput::INDEX, Color::from_rgbf32_unchecked(0.5, 0.5, 0.5), 50.),
		(HighlightsInput::INDEX, Color::WHITE, 100.),
	];
	let output_range_params = [(OutputMinimumsInput::INDEX, Color::BLACK, 0.), (OutputMaximumsInput::INDEX, Color::WHITE, 100.)];

	let mut layout = Vec::with_capacity(5);
	build_shared_spectrum_section(node_id, context, &input_range_params, &mut layout);
	build_shared_spectrum_section(node_id, context, &output_range_params, &mut layout);
	layout
}

/// Append a section of related percentage parameters as rows: a shared black-to-white spectrum (with one marker per non-exposed parameter) sits on the first non-exposed row
/// alongside its 60px number input, and the remaining non-exposed rows show only their 60px number input. Exposed parameters render as the standard exposed-row display.
/// Marker positions are clamped to non-decreasing display order so they never visually cross even if the underlying values do.
fn build_shared_spectrum_section(node_id: NodeId, context: &mut NodePropertiesContext, params: &[(usize, Color, f64)], layout: &mut Vec<LayoutGroup>) {
	// Snapshot exposure and values before the mutable-borrow loop
	let exposure_and_value: Vec<(bool, f64)> = match get_document_node(node_id, context) {
		Ok(document_node) => params
			.iter()
			.map(|&(input_index, _, _)| {
				let input = document_node.inputs.get(input_index);
				let exposed = input.is_some_and(|input| input.is_exposed());
				let percent = input
					.and_then(|input| input.as_value())
					.and_then(|tagged| if let TaggedValue::F32(value) = tagged { Some(*value as f64) } else { None })
					.unwrap_or(0.);
				(exposed, percent)
			})
			.collect(),
		Err(err) => {
			log::error!("Could not get document node in build_shared_spectrum_section: {err}");
			return;
		}
	};

	// Build markers for all non-exposed params
	let mut marker_input_indices = Vec::new();
	let mut marker_default_percents = Vec::new();
	let mut marker_positions = Vec::new();
	let mut handle_colors = Vec::new();
	for (i, &(input_index, handle_color, default_percent)) in params.iter().enumerate() {
		let (exposed, percent) = exposure_and_value[i];
		if exposed {
			continue;
		}
		marker_positions.push((percent / 100.).clamp(0., 1.));
		marker_input_indices.push(input_index);
		marker_default_percents.push(default_percent);
		handle_colors.push(handle_color);
	}

	// Enforce non-decreasing order so markers never visually cross, matching the node's algorithm where shadows takes precedence
	for i in 1..marker_positions.len() {
		marker_positions[i] = marker_positions[i].max(marker_positions[i - 1]);
	}

	let spectrum_markers: Vec<SpectrumMarker> = marker_positions
		.iter()
		.zip(&handle_colors)
		.map(|(&position, &handle_color)| SpectrumMarker::new(position, 0.5, handle_color))
		.collect();

	// Build the shared spectrum widget (placed on the first non-exposed row)
	let spectrum_widget = (!spectrum_markers.is_empty()).then(|| {
		SpectrumInput::new(GradientUI::from(&bw_track()))
			.markers(spectrum_markers)
			.show_midpoints(false)
			.allow_insert(false)
			.allow_delete(false)
			.allow_reorder(false)
			.narrow(true)
			.on_update({
				let marker_input_indices = marker_input_indices.clone();
				let marker_default_percents = marker_default_percents.clone();
				let marker_positions = marker_positions.clone();
				move |update: &SpectrumInputUpdate| {
					let (input_index, percent) = match update {
						SpectrumInputUpdate::MoveMarker { index, position } => match marker_input_indices.get(*index as usize) {
							Some(&input_index) => (input_index, *position * 100.),
							None => return Message::NoOp,
						},
						SpectrumInputUpdate::ResetMarker { index } => {
							let i = *index as usize;
							let Some(&input_index) = marker_input_indices.get(i) else { return Message::NoOp };
							let Some(&default_percent) = marker_default_percents.get(i) else { return Message::NoOp };
							// Falls back to midpoint between neighbors if the default would cross one
							let left = if i == 0 { 0. } else { marker_positions[i - 1] };
							let right = marker_positions.get(i + 1).copied().unwrap_or(1.);
							let default_position = default_percent / 100.;
							let new_position = if (left..=right).contains(&default_position) { default_position } else { (left + right) / 2. };
							(input_index, new_position * 100.)
						}
						_ => return Message::NoOp,
					};
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index,
						value: TaggedValue::F32(percent.clamp(0., 100.) as f32),
					}
					.into()
				}
			})
			.on_commit(commit_value)
			.widget_instance()
	});
	let spectrum_owner = marker_input_indices.first().copied();

	let number_input = NumberInput::default().mode_increment().unit("%").min(0.).max(100.);

	// One row per parameter: first non-exposed carries the shared spectrum, others get just a number input
	for (i, &(input_index, _, _)) in params.iter().enumerate() {
		let (exposed, current) = exposure_and_value[i];

		if exposed {
			let row = number_widget(ParameterWidgetsInfo::new(node_id, input_index, true, context), number_input.clone());
			layout.push(LayoutGroup::row(row));
		} else {
			let mut row = start_widgets(ParameterWidgetsInfo::new(node_id, input_index, true, context));
			row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

			if Some(input_index) == spectrum_owner
				&& let Some(spectrum) = &spectrum_widget
			{
				row.push(spectrum.clone());
				row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			}

			row.push(
				number_input
					.clone()
					.value(Some(current))
					.min_width(60)
					.max_width(60)
					.display_decimal_places(0)
					.on_update(update_value(move |widget: &NumberInput| TaggedValue::F32(widget.value.unwrap_or(0.) as f32), node_id, input_index))
					.on_commit(commit_value)
					.widget_instance(),
			);
			layout.push(LayoutGroup::row(row));
		}
	}
}

pub(crate) fn hue_saturation_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::hue_saturation::*;

	// Current hue position on the rainbow track, used for the saturation track's right-end color
	let current_hue_shift = get_document_node(node_id, context)
		.ok()
		.and_then(|document_node| document_node.inputs.get(HueShiftInput::INDEX).and_then(|input| input.as_value()))
		.and_then(|tagged| if let TaggedValue::F32(value) = tagged { Some(*value) } else { None })
		.unwrap_or(0.);
	// The rainbow has cyan at position 0.5 (hue_shift=0), so offset by +180 to align
	let marker_hue = ((current_hue_shift + 180.) / 360.).rem_euclid(1.);
	let saturated_current_hue = Color::from_hsva(marker_hue, 1., 1., 1.);

	// Hue: cyclic rainbow
	let hue_track = Gradient {
		position: vec![0., 1. / 6., 2. / 6., 3. / 6., 4. / 6., 5. / 6., 1.],
		midpoint: vec![0.5; 7],
		color: vec![Color::RED, Color::YELLOW, Color::GREEN, Color::CYAN, Color::BLUE, Color::MAGENTA, Color::RED],
	};
	// Saturation: gray to the fully saturated current hue
	let saturation_track = Gradient {
		position: vec![0., 1.],
		midpoint: vec![0.5, 0.5],
		color: vec![Color::from_rgbf32_unchecked(0.5, 0.5, 0.5), saturated_current_hue],
	};
	// Lightness: black to white
	let lightness_track = bw_track();

	vec![
		spectrum_slider_row(
			node_id,
			context,
			HueShiftInput::INDEX,
			hue_track,
			Color::WHITE,
			-180.,
			180.,
			0.,
			NumberInput::default().mode_increment().unit("°").min(-180.).max(180.),
		),
		spectrum_slider_row(
			node_id,
			context,
			SaturationShiftInput::INDEX,
			saturation_track,
			Color::WHITE,
			-100.,
			100.,
			0.,
			NumberInput::default().mode_increment().unit("%").min(-100.).max(100.),
		),
		spectrum_slider_row(
			node_id,
			context,
			LightnessShiftInput::INDEX,
			lightness_track,
			Color::WHITE,
			-100.,
			100.,
			0.,
			NumberInput::default().mode_increment().unit("%").min(-100.).max(100.),
		),
	]
}

/// Build a row with a single-marker `SpectrumInput` and a 60px `NumberInput`. The marker maps `value_min..value_max` to position 0..1, and double-click resets to `default_value`.
fn spectrum_slider_row(
	node_id: NodeId,
	context: &mut NodePropertiesContext,
	input_index: usize,
	track: Gradient,
	handle_color: Color,
	value_min: f64,
	value_max: f64,
	default_value: f64,
	number_input: NumberInput,
) -> LayoutGroup {
	let mut row = start_widgets(ParameterWidgetsInfo::new(node_id, input_index, true, context));

	let current = get_document_node(node_id, context)
		.ok()
		.and_then(|document_node| document_node.inputs.get(input_index))
		.and_then(|input| input.as_non_exposed_value())
		.and_then(|tagged| if let TaggedValue::F32(value) = tagged { Some(*value as f64) } else { None });

	// Only add the spectrum and number widgets when the input is not exposed
	if let Some(current) = current {
		let value_range = value_max - value_min;
		let position = ((current - value_min) / value_range).clamp(0., 1.);
		let default_position = ((default_value - value_min) / value_range).clamp(0., 1.);

		row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

		let position_to_value = move |position: f64| value_min + position * value_range;
		row.push(
			SpectrumInput::new(GradientUI::from(&track))
				.markers(vec![SpectrumMarker::new(position, 0.5, handle_color)])
				.show_midpoints(false)
				.allow_insert(false)
				.allow_delete(false)
				.allow_reorder(false)
				.narrow(true)
				.on_update(move |update: &SpectrumInputUpdate| {
					let new_position = match update {
						SpectrumInputUpdate::MoveMarker { index: 0, position } => *position,
						SpectrumInputUpdate::ResetMarker { index: 0 } => default_position,
						_ => return Message::NoOp,
					};
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index,
						value: TaggedValue::F32(position_to_value(new_position).clamp(value_min, value_max) as f32),
					}
					.into()
				})
				.on_commit(commit_value)
				.widget_instance(),
		);
		row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		row.push(
			number_input
				.value(Some(current))
				.min_width(60)
				.max_width(60)
				.display_decimal_places(0)
				.on_update(update_value(move |widget: &NumberInput| TaggedValue::F32(widget.value.unwrap_or(0.) as f32), node_id, input_index))
				.on_commit(commit_value)
				.widget_instance(),
		);
	}

	LayoutGroup::row(row)
}

pub(crate) fn threshold_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::threshold::*;

	let params: &[(usize, Color, f64)] = &[(MinLuminanceInput::INDEX, Color::BLACK, 50.), (MaxLuminanceInput::INDEX, Color::WHITE, 100.)];

	let mut layout = Vec::with_capacity(3);
	build_shared_spectrum_section(node_id, context, params, &mut layout);

	let luminance_calc = {
		let mut info = ParameterWidgetsInfo::new(node_id, LuminanceCalcInput::INDEX, true, context);
		info.exposable = false;
		enum_choice::<LuminanceCalculation>().for_socket(info).property_row()
	};
	layout.push(luminance_calc);

	layout
}

pub(crate) fn vibrance_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::vibrance::*;

	let track = Gradient {
		position: vec![0., 1.],
		midpoint: vec![0.5, 0.5],
		color: vec![Color::from_rgbf32_unchecked(0.5, 0.5, 0.5), Color::RED],
	};
	vec![spectrum_slider_row(
		node_id,
		context,
		VibranceInput::INDEX,
		track,
		Color::WHITE,
		-100.,
		100.,
		0.,
		NumberInput::default().mode_increment().unit("%").min(-100.).max(100.),
	)]
}

pub(crate) fn black_and_white_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::black_and_white::*;

	let number_input = NumberInput::default().mode_increment().unit("%").min(-200.).max(300.);

	let tint = color_widget(ParameterWidgetsInfo::new(node_id, TintInput::INDEX, true, context), ColorInput::default());

	let mut layout = vec![tint];
	let params: &[(usize, Color, f64)] = &[
		(RedsInput::INDEX, Color::RED, 40.),
		(YellowsInput::INDEX, Color::YELLOW, 60.),
		(GreensInput::INDEX, Color::GREEN, 40.),
		(CyansInput::INDEX, Color::CYAN, 60.),
		(BluesInput::INDEX, Color::BLUE, 20.),
		(MagentasInput::INDEX, Color::MAGENTA, 80.),
	];
	for &(input_index, color, default) in params {
		layout.push(spectrum_slider_row(
			node_id,
			context,
			input_index,
			color_track(color),
			Color::WHITE,
			-200.,
			300.,
			default,
			number_input.clone(),
		));
	}

	layout
}

pub(crate) fn channel_mixer_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::channel_mixer::*;

	let is_monochrome = bool_widget(ParameterWidgetsInfo::new(node_id, MonochromeInput::INDEX, true, context), CheckboxInput::default());
	let mut parameter_info = ParameterWidgetsInfo::new(node_id, OutputChannelInput::INDEX, true, context);
	parameter_info.exposable = false;
	let output_channel = enum_choice::<RedGreenBlue>().for_socket(parameter_info).property_row();

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in channel_mixer_properties: {err}");
			return Vec::new();
		}
	};
	// Monochrome
	let is_monochrome_value = match document_node.inputs[MonochromeInput::INDEX].as_value() {
		Some(TaggedValue::Bool(monochrome_choice)) => *monochrome_choice,
		_ => false,
	};
	// Output channel choice
	let output_channel_value = match &document_node.inputs[OutputChannelInput::INDEX].as_value() {
		Some(TaggedValue::RedGreenBlue(choice)) => choice,
		_ => {
			warn!("Channel Mixer node properties panel could not be displayed.");
			return vec![];
		}
	};

	// Input indices and defaults depend on monochrome toggle and output channel selection
	let (indices, defaults) = match (is_monochrome_value, output_channel_value) {
		(true, _) => (
			[MonochromeRInput::INDEX, MonochromeGInput::INDEX, MonochromeBInput::INDEX, MonochromeCInput::INDEX],
			[40., 40., 20., 0.],
		),
		(false, RedGreenBlue::Red) => ([RedRInput::INDEX, RedGInput::INDEX, RedBInput::INDEX, RedCInput::INDEX], [100., 0., 0., 0.]),
		(false, RedGreenBlue::Green) => ([GreenRInput::INDEX, GreenGInput::INDEX, GreenBInput::INDEX, GreenCInput::INDEX], [0., 100., 0., 0.]),
		(false, RedGreenBlue::Blue) => ([BlueRInput::INDEX, BlueGInput::INDEX, BlueBInput::INDEX, BlueCInput::INDEX], [0., 0., 100., 0.]),
	};

	let number_input = NumberInput::default().mode_increment().unit("%").min(-200.).max(200.);
	let tracks = [color_track(Color::RED), color_track(Color::GREEN), color_track(Color::BLUE), bw_track()];

	let mut layout = vec![LayoutGroup::row(is_monochrome)];
	if !is_monochrome_value {
		layout.push(output_channel);
	}
	for (i, (&input_index, &default)) in indices.iter().zip(defaults.iter()).enumerate() {
		layout.push(spectrum_slider_row(
			node_id,
			context,
			input_index,
			tracks[i].clone(),
			Color::WHITE,
			-200.,
			200.,
			default,
			number_input.clone(),
		));
	}

	layout
}

pub(crate) fn selective_color_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::selective_color::*;

	let mut default_info = ParameterWidgetsInfo::new(node_id, ColorsInput::INDEX, true, context);
	default_info.exposable = false;
	let colors = enum_choice::<SelectiveColorChoice>().for_socket(default_info).property_row();

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in selective_color_properties: {err}");
			return Vec::new();
		}
	};
	// Colors choice
	let colors_choice = match &document_node.inputs[ColorsInput::INDEX].as_value() {
		Some(TaggedValue::SelectiveColorChoice(choice)) => choice,
		_ => {
			warn!("Selective Color node properties panel could not be displayed.");
			return vec![];
		}
	};
	// CMYK
	let indices = match colors_choice {
		SelectiveColorChoice::Reds => [RCInput::INDEX, RMInput::INDEX, RYInput::INDEX, RKInput::INDEX],
		SelectiveColorChoice::Yellows => [YCInput::INDEX, YMInput::INDEX, YYInput::INDEX, YKInput::INDEX],
		SelectiveColorChoice::Greens => [GCInput::INDEX, GMInput::INDEX, GYInput::INDEX, GKInput::INDEX],
		SelectiveColorChoice::Cyans => [CCInput::INDEX, CMInput::INDEX, CYInput::INDEX, CKInput::INDEX],
		SelectiveColorChoice::Blues => [BCInput::INDEX, BMInput::INDEX, BYInput::INDEX, BKInput::INDEX],
		SelectiveColorChoice::Magentas => [MCInput::INDEX, MMInput::INDEX, MYInput::INDEX, MKInput::INDEX],
		SelectiveColorChoice::Whites => [WCInput::INDEX, WMInput::INDEX, WYInput::INDEX, WKInput::INDEX],
		SelectiveColorChoice::Neutrals => [NCInput::INDEX, NMInput::INDEX, NYInput::INDEX, NKInput::INDEX],
		SelectiveColorChoice::Blacks => [KCInput::INDEX, KMInput::INDEX, KYInput::INDEX, KKInput::INDEX],
	};

	let tracks = [color_track(Color::CYAN), color_track(Color::MAGENTA), color_track(Color::YELLOW), bw_track()];
	let number_input = NumberInput::default().mode_increment().unit("%").min(-100.).max(100.);

	// Mode
	let mode = enum_choice::<RelativeAbsolute>()
		.for_socket(ParameterWidgetsInfo::new(node_id, ModeInput::INDEX, true, context))
		.property_row();

	let mut layout = vec![colors];
	for (i, &input_index) in indices.iter().enumerate() {
		layout.push(spectrum_slider_row(
			node_id,
			context,
			input_index,
			tracks[i].clone(),
			Color::WHITE,
			-100.,
			100.,
			0.,
			number_input.clone(),
		));
	}
	layout.push(mode);

	layout
}

pub(crate) fn grid_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::generator_nodes::grid::*;

	let grid_type = enum_choice::<GridType>()
		.for_socket(ParameterWidgetsInfo::new(node_id, GridTypeInput::INDEX, true, context))
		.property_row();

	let mut widgets = vec![grid_type];

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in grid_properties: {err}");
			return Vec::new();
		}
	};
	let Some(grid_type_input) = document_node.inputs.get(GridTypeInput::INDEX) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::GridType(grid_type)) = grid_type_input.as_non_exposed_value() {
		match grid_type {
			GridType::Rectangular => {
				let spacing = vec2_widget(ParameterWidgetsInfo::new(node_id, SpacingInput::<f64>::INDEX, true, context), "W", "H", " px", Some(0.), false);
				widgets.push(spacing);
			}
			GridType::Isometric => {
				let spacing = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, SpacingInput::<f64>::INDEX, true, context),
					NumberInput::default().label("H").min(0.).unit(" px"),
				));
				let angles = vec2_widget(ParameterWidgetsInfo::new(node_id, AnglesInput::INDEX, true, context), "", "", "°", None, false);
				widgets.extend([spacing, angles]);
			}
		}
	}

	let columns = number_widget(ParameterWidgetsInfo::new(node_id, ColumnsInput::INDEX, true, context), NumberInput::default().min(1.));
	let rows = number_widget(ParameterWidgetsInfo::new(node_id, RowsInput::INDEX, true, context), NumberInput::default().min(1.));

	widgets.extend([LayoutGroup::row(columns), LayoutGroup::row(rows)]);

	widgets
}

pub(crate) fn spiral_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::generator_nodes::spiral::*;

	let spiral_type = enum_choice::<SpiralType>()
		.for_socket(ParameterWidgetsInfo::new(node_id, SpiralTypeInput::INDEX, true, context))
		.property_row();
	let turns = number_widget(ParameterWidgetsInfo::new(node_id, TurnsInput::INDEX, true, context), NumberInput::default().min(0.1));
	let start_angle = number_widget(ParameterWidgetsInfo::new(node_id, StartAngleInput::INDEX, true, context), NumberInput::default().unit("°"));

	let mut widgets = vec![spiral_type, LayoutGroup::row(turns), LayoutGroup::row(start_angle)];

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in exposure_properties: {err}");
			return Vec::new();
		}
	};

	let Some(spiral_type_input) = document_node.inputs.get(SpiralTypeInput::INDEX) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::SpiralType(spiral_type)) = spiral_type_input.as_non_exposed_value() {
		match spiral_type {
			SpiralType::Archimedean => {
				let inner_radius = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, InnerRadiusInput::INDEX, true, context),
					NumberInput::default().min(0.).unit(" px"),
				));

				let outer_radius = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, OuterRadiusInput::INDEX, true, context),
					NumberInput::default().unit(" px"),
				));

				widgets.extend([inner_radius, outer_radius]);
			}
			SpiralType::Logarithmic => {
				let inner_radius = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, InnerRadiusInput::INDEX, true, context),
					NumberInput::default().min(0.).unit(" px"),
				));

				let outer_radius = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, OuterRadiusInput::INDEX, true, context),
					NumberInput::default().min(0.1).unit(" px"),
				));

				widgets.extend([inner_radius, outer_radius]);
			}
		}
	}

	let angular_resolution = number_widget(
		ParameterWidgetsInfo::new(node_id, AngularResolutionInput::INDEX, true, context),
		NumberInput::default().min(1.).max(180.).unit("°"),
	);

	widgets.push(LayoutGroup::row(angular_resolution));

	widgets
}

pub(crate) const SAMPLE_POLYLINE_DESCRIPTION_SPACING: &str = "Use a point sampling density controlled by a distance between, or specific number of, points.";
pub(crate) const SAMPLE_POLYLINE_DESCRIPTION_SEPARATION: &str = "Distance between each point (exact if 'Adaptive Spacing' is disabled, approximate if enabled).";
pub(crate) const SAMPLE_POLYLINE_DESCRIPTION_QUANTITY: &str = "Number of points to place along the path.";
pub(crate) const SAMPLE_POLYLINE_DESCRIPTION_START_OFFSET: &str = "Exclude some distance from the start of the path before the first point.";
pub(crate) const SAMPLE_POLYLINE_DESCRIPTION_STOP_OFFSET: &str = "Exclude some distance from the end of the path after the last point.";
pub(crate) const SAMPLE_POLYLINE_DESCRIPTION_ADAPTIVE_SPACING: &str = "Round 'Separation' to a nearby value that divides into the path length evenly.";

pub(crate) fn sample_polyline_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::sample_polyline::*;

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in sample_polyline_properties: {err}");
			return Vec::new();
		}
	};

	let current_spacing = document_node.inputs.get(SpacingInput::INDEX).and_then(|input| input.as_value()).cloned();
	let is_quantity = matches!(current_spacing, Some(TaggedValue::PointSpacingType(PointSpacingType::Quantity)));

	let spacing = enum_choice::<PointSpacingType>()
		.for_socket(ParameterWidgetsInfo::new(node_id, SpacingInput::INDEX, true, context))
		.property_row();
	let separation = number_widget(ParameterWidgetsInfo::new(node_id, SeparationInput::INDEX, true, context), NumberInput::default().min(0.).unit(" px"));
	let quantity = number_widget(ParameterWidgetsInfo::new(node_id, QuantityInput::INDEX, true, context), NumberInput::default().min(2.).int());
	let start_offset = number_widget(ParameterWidgetsInfo::new(node_id, StartOffsetInput::INDEX, true, context), NumberInput::default().min(0.).unit(" px"));
	let stop_offset = number_widget(ParameterWidgetsInfo::new(node_id, StopOffsetInput::INDEX, true, context), NumberInput::default().min(0.).unit(" px"));
	let adaptive_spacing = bool_widget(
		ParameterWidgetsInfo::new(node_id, AdaptiveSpacingInput::INDEX, true, context),
		CheckboxInput::default().disabled(is_quantity),
	);

	vec![
		spacing.with_tooltip_description(SAMPLE_POLYLINE_DESCRIPTION_SPACING),
		match current_spacing {
			Some(TaggedValue::PointSpacingType(PointSpacingType::Separation)) => LayoutGroup::row(separation).with_tooltip_description(SAMPLE_POLYLINE_DESCRIPTION_SEPARATION),
			Some(TaggedValue::PointSpacingType(PointSpacingType::Quantity)) => LayoutGroup::row(quantity).with_tooltip_description(SAMPLE_POLYLINE_DESCRIPTION_QUANTITY),
			_ => LayoutGroup::row(vec![]),
		},
		LayoutGroup::row(start_offset).with_tooltip_description(SAMPLE_POLYLINE_DESCRIPTION_START_OFFSET),
		LayoutGroup::row(stop_offset).with_tooltip_description(SAMPLE_POLYLINE_DESCRIPTION_STOP_OFFSET),
		LayoutGroup::row(adaptive_spacing).with_tooltip_description(SAMPLE_POLYLINE_DESCRIPTION_ADAPTIVE_SPACING),
	]
}

pub(crate) fn exposure_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::exposure::*;

	let exposure = number_widget(ParameterWidgetsInfo::new(node_id, ExposureInput::INDEX, true, context), NumberInput::default().min(-20.).max(20.));
	let offset = number_widget(ParameterWidgetsInfo::new(node_id, OffsetInput::INDEX, true, context), NumberInput::default().min(-0.5).max(0.5));
	let gamma_correction = number_widget(
		ParameterWidgetsInfo::new(node_id, GammaCorrectionInput::INDEX, true, context),
		NumberInput::default().min(0.01).max(9.99).increment_step(0.1),
	);

	vec![LayoutGroup::row(exposure), LayoutGroup::row(offset), LayoutGroup::row(gamma_correction)]
}

pub(crate) fn format_number_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::text_nodes::format_number::{DecimalPlacesInput, DecimalSeparatorInput, FixedDecimalsInput, StartAt10000Input, ThousandsSeparatorInput, UseThousandsSeparatorInput};

	// Read current values before borrowing context mutably for widgets
	let (no_decimals, decimal_sep_value, use_thousands, thousands_sep_value) = match get_document_node(node_id, context) {
		Ok(document_node) => {
			let decimal_places = match document_node.inputs.get(DecimalPlacesInput::INDEX).and_then(|input| input.as_value()) {
				Some(&TaggedValue::U32(x)) => x,
				_ => 2,
			};
			let decimal_sep = match document_node.inputs.get(DecimalSeparatorInput::INDEX).and_then(|input| input.as_non_exposed_value()) {
				Some(TaggedValue::String(x)) => Some(x.clone()),
				_ => None,
			};
			let use_thousands = match document_node.inputs.get(UseThousandsSeparatorInput::INDEX).and_then(|input| input.as_value()) {
				Some(&TaggedValue::Bool(x)) => x,
				_ => false,
			};
			let use_thousands = use_thousands || document_node.inputs.get(ThousandsSeparatorInput::INDEX).is_some_and(|input| input.is_exposed());
			let thousands_sep = match document_node.inputs.get(ThousandsSeparatorInput::INDEX).and_then(|input| input.as_non_exposed_value()) {
				Some(TaggedValue::String(x)) => Some(x.clone()),
				_ => None,
			};
			(decimal_places == 0, decimal_sep, use_thousands, thousands_sep)
		}
		Err(err) => {
			log::error!("Could not get document node in format_number_properties: {err}");
			return Vec::new();
		}
	};

	let decimal_places = number_widget(ParameterWidgetsInfo::new(node_id, DecimalPlacesInput::INDEX, true, context), NumberInput::default().min(0.).int());

	// Fixed decimals and decimal separator are disabled when decimal places is 0
	let fixed_decimals = bool_widget(
		ParameterWidgetsInfo::new(node_id, FixedDecimalsInput::INDEX, true, context),
		CheckboxInput::default().disabled(no_decimals),
	);
	let mut decimal_sep_widgets = start_widgets(ParameterWidgetsInfo::new(node_id, DecimalSeparatorInput::INDEX, true, context));
	if let Some(sep) = decimal_sep_value {
		decimal_sep_widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(sep)
				.disabled(no_decimals)
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, DecimalSeparatorInput::INDEX))
				.on_commit(commit_value)
				.widget_instance(),
		]);
	}

	// Thousands separator: checkbox in assist area
	let mut thousands_sep_widgets = start_widgets(ParameterWidgetsInfo::new(node_id, ThousandsSeparatorInput::INDEX, false, context));
	if let Some(sep) = thousands_sep_value {
		thousands_sep_widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(use_thousands)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, UseThousandsSeparatorInput::INDEX))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(sep)
				.disabled(!use_thousands)
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, ThousandsSeparatorInput::INDEX))
				.on_commit(commit_value)
				.widget_instance(),
		]);
	}

	// Start at 10,000: disabled when thousands separator is off
	let start_at_10000 = bool_widget(
		ParameterWidgetsInfo::new(node_id, StartAt10000Input::INDEX, true, context),
		CheckboxInput::default().disabled(!use_thousands),
	);

	vec![
		LayoutGroup::row(decimal_places),
		LayoutGroup::row(decimal_sep_widgets),
		LayoutGroup::row(fixed_decimals),
		LayoutGroup::row(thousands_sep_widgets),
		LayoutGroup::row(start_at_10000),
	]
}

pub(crate) fn string_capitalization_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::text_nodes::string_capitalization::*;

	// Read the current values before borrowing context mutably for widgets
	let (is_simple_case, use_joiner_enabled, joiner_value) = match get_document_node(node_id, context) {
		Ok(document_node) => {
			let capitalization_input = document_node.inputs.get(CapitalizationInput::INDEX);
			let capitalization_exposed = capitalization_input.is_some_and(|input| input.is_exposed());
			// When exposed, the capitalization mode may change dynamically, so we can't assume it's a simple (joiner-inapplicable) mode
			let is_simple = !capitalization_exposed
				&& matches!(
					capitalization_input.and_then(|input| input.as_value()),
					Some(TaggedValue::StringCapitalization(StringCapitalization::LowerCase | StringCapitalization::UpperCase))
				);
			let use_joiner = match document_node.inputs.get(UseJoinerInput::INDEX).and_then(|input| input.as_value()) {
				Some(&TaggedValue::Bool(x)) => x,
				_ => true,
			};
			let joiner = match document_node.inputs.get(JoinerInput::INDEX).and_then(|input| input.as_non_exposed_value()) {
				Some(TaggedValue::String(x)) => Some(x.clone()),
				_ => None,
			};
			(is_simple, use_joiner, joiner)
		}
		Err(err) => {
			log::error!("Could not get document node in string_capitalization_properties: {err}");
			return Vec::new();
		}
	};

	// The joiner controls are disabled when lowercase/UPPERCASE are selected (they don't use word boundaries)
	let joiner_disabled = is_simple_case || !use_joiner_enabled;

	let capitalization = enum_choice::<StringCapitalization>()
		.for_socket(ParameterWidgetsInfo::new(node_id, CapitalizationInput::INDEX, true, context))
		.property_row();

	// Joiner row: the UseJoiner checkbox is drawn in the assist area, followed by the Joiner text input
	let mut joiner_widgets = start_widgets(ParameterWidgetsInfo::new(node_id, JoinerInput::INDEX, false, context));
	if let Some(joiner) = joiner_value {
		let joiner_is_empty = joiner.is_empty();
		joiner_widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(use_joiner_enabled)
				.disabled(is_simple_case)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, UseJoinerInput::INDEX))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(joiner)
				.placeholder(if joiner_is_empty { "Empty" } else { "" })
				.disabled(joiner_disabled)
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, JoinerInput::INDEX))
				.on_commit(commit_value)
				.widget_instance(),
		]);
	}

	// Preset buttons for common joiner values, indented to align with the input field
	let mut joiner_preset_buttons = vec![TextLabel::new("").widget_instance()];
	add_blank_assist(&mut joiner_preset_buttons);
	joiner_preset_buttons.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
	for (label, value, tooltip) in [
		("Empty", "", "Join words without any separator."),
		("Space", " ", "Join words with a space."),
		("Kebab", "-", "Join words with a hyphen."),
		("Snake", "_", "Join words with an underscore."),
	] {
		let value = value.to_string();
		joiner_preset_buttons.push(
			TextButton::new(label)
				.tooltip_description(tooltip)
				.disabled(is_simple_case)
				.on_update(move |_: &TextButton| Message::Batched {
					messages: Box::new([
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: UseJoinerInput::INDEX,
							value: TaggedValue::Bool(true),
						}
						.into(),
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: JoinerInput::INDEX,
							value: TaggedValue::String(value.clone()),
						}
						.into(),
					]),
				})
				.on_commit(commit_value)
				.widget_instance(),
		);
	}

	vec![capitalization, LayoutGroup::row(joiner_widgets), LayoutGroup::row(joiner_preset_buttons)]
}

pub(crate) fn rectangle_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::generator_nodes::rectangle::*;

	// Corner Radius
	let mut corner_radius_row_1 = start_widgets(ParameterWidgetsInfo::new(node_id, CornerRadiusInput::INDEX, true, context));
	corner_radius_row_1.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	let mut corner_radius_row_2 = vec![Separator::new(SeparatorStyle::Unrelated).widget_instance()];
	corner_radius_row_2.push(TextLabel::new("").widget_instance());
	add_blank_assist(&mut corner_radius_row_2);

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in rectangle_properties: {err}");
			return Vec::new();
		}
	};
	let Some(input) = document_node.inputs.get(IndividualCornerRadiiInput::INDEX) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(is_individual)) = input.as_non_exposed_value() {
		// Values
		let Some(input) = document_node.inputs.get(CornerRadiusInput::INDEX) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		let corner_values = match input.as_non_exposed_value() {
			Some(TaggedValue::BoxCorners(corners)) => corners.to_corner_values(),
			_ => [0.; 4],
		};
		let uniform_val = corner_values[0];

		// Uniform/individual radio input widget
		let uniform = RadioEntryData::new("Uniform")
			.label("Uniform")
			.on_update(move |_| Message::Batched {
				messages: Box::new([
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: IndividualCornerRadiiInput::INDEX,
						value: TaggedValue::Bool(false),
					}
					.into(),
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: CornerRadiusInput::INDEX,
						value: TaggedValue::BoxCorners(BoxCorners::from(uniform_val)),
					}
					.into(),
				]),
			})
			.on_commit(commit_value);
		let individual = RadioEntryData::new("Individual")
			.label("Individual")
			.on_update(move |_| Message::Batched {
				messages: Box::new([
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: IndividualCornerRadiiInput::INDEX,
						value: TaggedValue::Bool(true),
					}
					.into(),
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: CornerRadiusInput::INDEX,
						value: TaggedValue::BoxCorners(BoxCorners::from(corner_values.to_vec())),
					}
					.into(),
				]),
			})
			.on_commit(commit_value);
		let radio_input = RadioInput::new(vec![uniform, individual]).selected_index(Some(is_individual as u32)).widget_instance();
		corner_radius_row_1.push(radio_input);

		// Radius value input widget
		let input_widget = if is_individual {
			TextInput::default()
				.value(corner_values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
				.on_update(optionally_update_value(
					move |x: &TextInput| Some(TaggedValue::BoxCorners(BoxCorners::from(x.value.as_str()))),
					node_id,
					CornerRadiusInput::INDEX,
				))
				.widget_instance()
		} else {
			NumberInput::default()
				.value(Some(uniform_val))
				.unit(" px")
				.on_update(update_value(
					move |x: &NumberInput| TaggedValue::BoxCorners(BoxCorners::from(x.value.unwrap())),
					node_id,
					CornerRadiusInput::INDEX,
				))
				.on_commit(commit_value)
				.widget_instance()
		};
		corner_radius_row_2.push(input_widget);
	}

	// Size X
	let size_x = number_widget(ParameterWidgetsInfo::new(node_id, WidthInput::INDEX, true, context), NumberInput::default());

	// Size Y
	let size_y = number_widget(ParameterWidgetsInfo::new(node_id, HeightInput::INDEX, true, context), NumberInput::default());

	// Clamped
	let clamped = bool_widget(ParameterWidgetsInfo::new(node_id, ClampedInput::INDEX, true, context), CheckboxInput::default());

	vec![
		LayoutGroup::row(size_x),
		LayoutGroup::row(size_y),
		LayoutGroup::row(corner_radius_row_1),
		LayoutGroup::row(corner_radius_row_2),
		LayoutGroup::row(clamped),
	]
}

pub(crate) fn node_no_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let text = if context.network_interface.is_layer(&node_id, context.selection_network_path) {
		"Layer has no parameters"
	} else {
		"Node has no parameters"
	};
	string_properties(text)
}

pub(crate) fn generate_node_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> LayoutGroup {
	let mut layout = Vec::new();

	if let Some(properties_override) = context
		.network_interface
		.reference(&node_id, context.selection_network_path)
		.as_ref()
		.and_then(|identifier| resolve_document_node_type(identifier))
		.and_then(|definition| definition.properties)
		.and_then(|properties| NODE_OVERRIDES.get(properties))
	{
		layout = properties_override(node_id, context);
	} else {
		let number_of_inputs = context.network_interface.number_of_inputs(&node_id, context.selection_network_path);
		for input_index in 1..number_of_inputs {
			// Hide inputs that are connected to a scope
			if let Some(NodeInput::Scope(_)) = context
				.network_interface
				.input_from_connector(&InputConnector::node(node_id, input_index), context.selection_network_path)
			{
				continue;
			}

			let row = context.call_widget_override(&node_id, input_index).unwrap_or_else(|| {
				let Some(implementation) = context.network_interface.implementation(&node_id, context.selection_network_path) else {
					log::error!("Could not get implementation for node {node_id}");
					return Vec::new();
				};

				let mut number_options = NumberOptions::default();
				let mut display_decimal_places = None;
				let mut step = None;
				let mut unit_suffix = None;
				let input_type = match implementation {
					DocumentNodeImplementation::ProtoNode(proto_node_identifier) => 'early_return: {
						// Clone to end the `network_interface` borrow held via `implementation`, freeing the mutable borrow `input_type` needs below
						let proto_node_identifier = proto_node_identifier.clone();

						let mut default_type = None;
						if let Some(field) = graphene_std::registry::NODE_METADATA
							.lock()
							.unwrap()
							.get(&proto_node_identifier)
							.and_then(|metadata| metadata.fields.get(input_index))
						{
							number_options = NumberOptions {
								soft_min: field.number_soft_min,
								soft_max: field.number_soft_max,
								hard_min: field.number_hard_min,
								hard_max: field.number_hard_max,
								slider: field.number_mode_range,
							};
							display_decimal_places = field.number_display_decimal_places;
							unit_suffix = field.unit;
							step = field.number_step;
							default_type = field.default_type.clone();
						}

						if let Some(default) = default_type {
							break 'early_return default;
						}

						let Some(implementations) = &interpreted_executor::node_registry::NODE_REGISTRY.get(&proto_node_identifier) else {
							log::error!("Could not get implementation for protonode {proto_node_identifier:?}");
							return Vec::new();
						};

						let mut input_types = implementations.keys().filter_map(|item| item.inputs.get(input_index)).collect::<Vec<_>>();
						input_types.sort_by_key(|ty| ty.type_name());
						let input_type = input_types.first().cloned();

						let Some(input_type) = input_type else { return Vec::new() };
						input_type.clone()
					}
					_ => context
						.network_interface
						.input_type(&InputConnector::node(node_id, input_index), context.selection_network_path)
						.compiled_nested_type()
						.cloned()
						.unwrap_or(concrete!(())),
				};

				property_from_type(node_id, input_index, &input_type, number_options, unit_suffix, display_decimal_places, step, context).unwrap_or_else(|value| value)
			});

			layout.extend(row);
		}
	}

	if layout.is_empty() {
		layout = node_no_properties(node_id, context);
	}

	let display_name = context
		.network_interface
		.node_metadata(&node_id, context.selection_network_path)
		.map(|metadata| metadata.persistent_metadata.display_name.as_str());
	let implementation_name = context.network_interface.implementation_name(&node_id, context.selection_network_path);
	let name = if let Some(display_name) = display_name
		&& implementation_name != display_name
		&& implementation_name != "Custom Node"
		&& !display_name.is_empty()
	{
		format!("{display_name} ({implementation_name})")
	} else {
		implementation_name
	};

	let description = context
		.network_interface
		.reference(&node_id, context.selection_network_path)
		.as_ref()
		.and_then(|identifier| resolve_document_node_type(identifier))
		.map(|definition| definition.description.to_string())
		.filter(|string| string != "TODO")
		.unwrap_or_default();

	let visible = context.network_interface.is_visible(&node_id, context.selection_network_path);
	let pinned = context.network_interface.is_pinned(&node_id, context.selection_network_path);
	let expanded = !context.properties_panel_collapsed_sections.contains(&node_id);

	LayoutGroup::section(name, description, visible, pinned, expanded, node_id.0, Layout(layout))
}

/// The layer that a chain node ultimately feeds, if any. Returns `None` in a nested network since the layer metadata structure
/// is only loaded for the root document network, so a `LayerNodeIdentifier` can't be constructed there.
fn root_layer_for_chain_node(node_id: NodeId, context: &mut NodePropertiesContext) -> Option<LayerNodeIdentifier> {
	if !context.selection_network_path.is_empty() {
		return None;
	}
	let layer_node = context.network_interface.downstream_layer_for_chain_node(&node_id, context.selection_network_path)?;
	Some(LayerNodeIdentifier::new(layer_node, context.network_interface))
}

/// Resolve the viewport-space orientation of a Fill node's gradient by walking downstream to its owning layer
/// and reusing the same helper the Gradient tool uses, so canvas tilt and layer transforms behave identically.
fn gradient_orientation_in_fill_node(node_id: NodeId, gradient_transform: DAffine2, context: &mut NodePropertiesContext) -> Option<bool> {
	let layer = root_layer_for_chain_node(node_id, context)?;
	let transform = graph_modification_utils::gradient_space_transform(layer, context.network_interface);
	Some(graph_modification_utils::gradient_orientation_rightward(transform * gradient_transform))
}

/// Fill Node Widgets LayoutGroup
pub(crate) fn fill_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::fill::*;

	#[derive(Debug, Clone)]
	enum ResolvedFill {
		Solid(Option<Color>),
		Gradient {
			gradient: Gradient,
			gradient_type: GradientType,
			spread_method: GradientSpreadMethod,
			transform: DAffine2,
			/// Whether the transform input holds a plain value (so the "Reverse Direction" button may write to it) rather than a wire.
			transform_is_value: bool,
		},
		Other,
	}

	// Pass blank_assist=false because the assist slot is filled below ("Reverse Stops" button when in gradient mode)
	let mut widgets_first_row = start_widgets(ParameterWidgetsInfo::new(node_id, FillInput::<List<Graphic>>::INDEX, false, context));

	if get_document_node(node_id, context).is_ok_and(|node| node.inputs.get(FillInput::<List<Graphic>>::INDEX).is_some_and(|input| input.is_exposed())) {
		return vec![LayoutGroup::row(widgets_first_row)];
	}

	// A Fill node not attached to a layer (or living in a nested network) still shows its full fill UI; only the gradient's
	// bounding-box default transform needs the layer, and it falls back to a unit box when there isn't one.
	let layer = root_layer_for_chain_node(node_id, context);

	let fill = match get_document_node(node_id, context) {
		Ok(document_node) => match document_node.inputs[FillInput::<List<Graphic>>::INDEX].as_value() {
			Some(TaggedValue::Color(color)) => ResolvedFill::Solid(Some(*color)),
			Some(value) if value.is_no_paint() => ResolvedFill::Solid(None),
			Some(TaggedValue::Gradient(_)) => {
				match graph_modification_utils::read_fill_node_gradient(document_node, || {
					layer.map_or([DVec2::ZERO, DVec2::ONE], |layer| context.network_interface.document_metadata().nonzero_bounding_box(layer))
				}) {
					Some(gradient) => ResolvedFill::Gradient {
						gradient: gradient.stops,
						gradient_type: gradient.gradient_type,
						spread_method: gradient.spread_method,
						transform: gradient.transform,
						transform_is_value: gradient.transform_is_value,
					},
					None => ResolvedFill::Other,
				}
			}
			_ => ResolvedFill::Other,
		},
		Err(_) => ResolvedFill::Other,
	};

	let (backup_color, backup_gradient) = match get_document_node(node_id, context) {
		Ok(document_node) => {
			let backup_color = match document_node.inputs[BackupColorInput::INDEX].as_value() {
				Some(&TaggedValue::Color(color)) => Some(color),
				_ => None,
			};
			let backup_stops = match document_node.inputs[BackupGradientInput::INDEX].as_value() {
				Some(TaggedValue::Gradient(stops)) => stops.clone(),
				_ => Gradient::default(),
			};
			(backup_color, backup_stops)
		}
		Err(_) => (None, Gradient::default()),
	};

	match &fill {
		ResolvedFill::Gradient { gradient: stops, .. } => {
			let stops = stops.clone();

			let reverse_button = IconButton::new("Reverse", 24)
				.tooltip_label("Reverse Stops")
				.tooltip_description("Reverse the gradient color stops.")
				.on_update(update_value(move |_| TaggedValue::Gradient(stops.reversed()), node_id, FillInput::<List<Graphic>>::INDEX))
				.widget_instance();
			widgets_first_row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			widgets_first_row.push(reverse_button);
		}
		_ => add_blank_assist(&mut widgets_first_row),
	}

	let fill_choice_ui = match &fill {
		ResolvedFill::Solid(color) => {
			if let Some(color) = color {
				FillChoiceUI::Solid(SRGBA8::from(*color))
			} else {
				FillChoiceUI::None
			}
		}
		ResolvedFill::Gradient { gradient: stops, .. } => FillChoiceUI::Gradient(GradientUI::from(stops)),
		ResolvedFill::Other => FillChoiceUI::None,
	};

	let solid_set_messages = move |color: Option<Color>| {
		let mut messages = vec![
			NodeGraphMessage::SetInputValue {
				node_id,
				input_index: FillInput::<List<Graphic>>::INDEX,
				value: color.map_or_else(TaggedValue::no_paint, TaggedValue::Color),
			}
			.into(),
		];
		if let Some(color) = color {
			messages.push(
				NodeGraphMessage::SetInputValue {
					node_id,
					input_index: BackupColorInput::INDEX,
					value: TaggedValue::Color(color),
				}
				.into(),
			);
		}
		Message::Batched { messages: messages.into() }
	};

	let gradient_set_messages = move |gradient: Gradient| Message::Batched {
		messages: Box::new([
			NodeGraphMessage::SetInputValue {
				node_id,
				input_index: FillInput::<List<Graphic>>::INDEX,
				value: TaggedValue::Gradient(gradient.clone()),
			}
			.into(),
			NodeGraphMessage::SetInputValue {
				node_id,
				input_index: BackupGradientInput::INDEX,
				value: TaggedValue::Gradient(gradient),
			}
			.into(),
		]),
	};

	widgets_first_row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
	widgets_first_row.push(
		ColorInput::default()
			.value(fill_choice_ui)
			.on_update(move |x: &ColorInput| match &x.value {
				FillChoiceUI::None => solid_set_messages(None),
				FillChoiceUI::Solid(srgba8) => {
					let color = Some(Color::from(*srgba8));
					solid_set_messages(color)
				}
				FillChoiceUI::Gradient(gradient_stops_ui) => {
					let gradient = Gradient::from(gradient_stops_ui);
					gradient_set_messages(gradient)
				}
			})
			.on_commit(commit_value)
			.widget_instance(),
	);

	let mut widgets = vec![LayoutGroup::row(widgets_first_row)];

	let fill_type_switch = {
		let mut row = vec![TextLabel::new("").widget_instance()];
		add_blank_assist(&mut row);

		let entries = vec![
			RadioEntryData::new("solid")
				.label("Solid")
				.on_update(update_value(
					move |_| backup_color.map_or_else(TaggedValue::no_paint, TaggedValue::Color),
					node_id,
					FillInput::<List<Graphic>>::INDEX,
				))
				.on_commit(commit_value),
			RadioEntryData::new("gradient")
				.label("Gradient")
				.on_update(update_value(move |_| TaggedValue::Gradient(backup_gradient.clone()), node_id, FillInput::<List<Graphic>>::INDEX))
				.on_commit(commit_value),
		];

		row.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			RadioInput::new(entries)
				.selected_index(Some(if matches!(fill, ResolvedFill::Gradient { .. }) { 1 } else { 0 }))
				.widget_instance(),
		]);

		LayoutGroup::row(row)
	};
	widgets.push(fill_type_switch);

	if let ResolvedFill::Gradient {
		gradient_type,
		spread_method,
		transform,
		transform_is_value,
		..
	} = fill.clone()
	{
		// Linear/Radial radio: blank assist (the "Reverse Direction" button has been moved down to the spread method row)
		let mut row = vec![TextLabel::new("").widget_instance()];
		add_blank_assist(&mut row);

		let entries = [GradientType::Linear, GradientType::Radial]
			.iter()
			.map(|&grad_type| {
				RadioEntryData::new(format!("{:?}", grad_type))
					.label(format!("{:?}", grad_type))
					.on_update(update_value(move |_| TaggedValue::GradientType(grad_type), node_id, GradientTypeInput::INDEX))
					.on_commit(commit_value)
			})
			.collect();

		row.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			RadioInput::new(entries).selected_index(Some(gradient_type as u32)).widget_instance(),
		]);

		widgets.push(LayoutGroup::row(row));

		// "Reverse Direction" button (assist) plus the Pad/Reflect/Repeat radio. Icon orientation is resolved in viewport
		// space so canvas tilt and layer transforms behave the same as in the Gradient tool's control bar.
		let mut spread_methods_row = vec![TextLabel::new("").widget_instance()];

		// The button writes a value into the transform input, so only offer it when the input isn't wired
		if transform_is_value {
			let start = transform.transform_point2(DVec2::ZERO);
			let end = transform.transform_point2(DVec2::X);
			let new_transform = build_transform_with_y_preservation(transform, end, start);
			let orientation_rightward = gradient_orientation_in_fill_node(node_id, transform, context).unwrap_or(true);

			let reverse_direction_button = IconButton::new(if orientation_rightward { "ReverseRadialGradientToRight" } else { "ReverseRadialGradientToLeft" }, 24)
				.tooltip_label("Reverse Direction")
				.tooltip_description(if gradient_type == GradientType::Radial {
					"Reverse which end the gradient radiates from."
				} else {
					"Swap the start and end points of the gradient line."
				})
				.on_update(move |_| Message::Batched {
					messages: Box::new([
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: HasTransformInput::INDEX,
							value: TaggedValue::Bool(true),
						}
						.into(),
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: TransformInput::INDEX,
							value: TaggedValue::DAffine2(new_transform),
						}
						.into(),
					]),
				})
				.widget_instance();
			spread_methods_row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			spread_methods_row.push(reverse_direction_button);
		} else {
			add_blank_assist(&mut spread_methods_row);
		}

		let spread_method_entries = [GradientSpreadMethod::Pad, GradientSpreadMethod::Reflect, GradientSpreadMethod::Repeat]
			.iter()
			.map(|&spread_method| {
				RadioEntryData::new(format!("{:?}", spread_method))
					.label(format!("{:?}", spread_method))
					.on_update(update_value(move |_| TaggedValue::GradientSpreadMethod(spread_method), node_id, SpreadMethodInput::INDEX))
					.on_commit(commit_value)
			})
			.collect();

		spread_methods_row.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			RadioInput::new(spread_method_entries).selected_index(Some(spread_method as u32)).widget_instance(),
		]);

		widgets.push(LayoutGroup::row(spread_methods_row));
	}

	widgets
}

pub fn stroke_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::stroke::*;

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in stroke_properties: {err}");
			return Vec::new();
		}
	};
	let join_value = match &document_node.inputs[JoinInput::INDEX].as_value() {
		Some(TaggedValue::StrokeJoin(x)) => x,
		_ => &StrokeJoin::Miter,
	};

	let has_dash_lengths = match &document_node.inputs[DashPatternInput::INDEX].as_value() {
		Some(TaggedValue::DashPattern(pattern)) => pattern.0.is_empty(),
		_ => true,
	};
	let miter_limit_disabled = join_value != &StrokeJoin::Miter;

	let color = color_widget(
		ParameterWidgetsInfo::new(node_id, PaintInput::<List<Graphic>>::INDEX, true, context),
		crate::messages::layout::utility_types::widgets::button_widgets::ColorInput::default(),
	);
	let weight = number_widget(ParameterWidgetsInfo::new(node_id, WeightInput::INDEX, true, context), NumberInput::default().unit(" px").min(0.));
	let align = enum_choice::<StrokeAlign>()
		.for_socket(ParameterWidgetsInfo::new(node_id, AlignInput::INDEX, true, context))
		.property_row();
	let cap = enum_choice::<StrokeCap>().for_socket(ParameterWidgetsInfo::new(node_id, CapInput::INDEX, true, context)).property_row();
	let join = enum_choice::<StrokeJoin>()
		.for_socket(ParameterWidgetsInfo::new(node_id, JoinInput::INDEX, true, context))
		.property_row();

	let miter_limit = number_widget(
		ParameterWidgetsInfo::new(node_id, MiterLimitInput::INDEX, true, context),
		NumberInput::default().min(0.).disabled(miter_limit_disabled),
	);
	let paint_order = enum_choice::<PaintOrder>()
		.for_socket(ParameterWidgetsInfo::new(node_id, PaintOrderInput::INDEX, true, context))
		.property_row();
	let disabled_number_input = NumberInput::default().unit(" px").disabled(has_dash_lengths);
	let dash_lengths = dash_pattern_widget(ParameterWidgetsInfo::new(node_id, DashPatternInput::INDEX, true, context), TextInput::default().centered(true));
	let number_input = disabled_number_input;
	let dash_offset = number_widget(ParameterWidgetsInfo::new(node_id, DashOffsetInput::INDEX, true, context), number_input);

	vec![
		color,
		LayoutGroup::row(weight),
		align,
		cap,
		join,
		LayoutGroup::row(miter_limit),
		paint_order,
		LayoutGroup::row(dash_lengths),
		LayoutGroup::row(dash_offset),
	]
}

pub fn offset_path_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::offset_path::*;

	let number_input = NumberInput::default().unit(" px");
	let distance = number_widget(ParameterWidgetsInfo::new(node_id, DistanceInput::INDEX, true, context), number_input);

	let join = enum_choice::<StrokeJoin>()
		.for_socket(ParameterWidgetsInfo::new(node_id, JoinInput::INDEX, true, context))
		.property_row();

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in offset_path_properties: {err}");
			return Vec::new();
		}
	};
	let number_input = NumberInput::default().min(0.).disabled({
		let join_val = match &document_node.inputs[JoinInput::INDEX].as_value() {
			Some(TaggedValue::StrokeJoin(x)) => x,
			_ => &StrokeJoin::Miter,
		};
		join_val != &StrokeJoin::Miter
	});
	let miter_limit = number_widget(ParameterWidgetsInfo::new(node_id, MiterLimitInput::INDEX, true, context), number_input);

	vec![LayoutGroup::row(distance), join, LayoutGroup::row(miter_limit)]
}

pub fn math_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::math_nodes::math::*;

	let expression = (|| {
		let mut widgets = start_widgets(ParameterWidgetsInfo::new(node_id, ExpressionInput::INDEX, true, context));

		let document_node = match get_document_node(node_id, context) {
			Ok(document_node) => document_node,
			Err(err) => {
				log::error!("Could not get document node in offset_path_properties: {err}");
				return Vec::new();
			}
		};
		let Some(input) = document_node.inputs.get(ExpressionInput::INDEX) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				TextInput::new(x.clone())
					.centered(true)
					.on_update(update_value(
						|x: &TextInput| {
							TaggedValue::String({
								let mut expression = x.value.trim().to_string();

								if ["+", "-", "*", "/", "^", "%"].iter().any(|&infix| infix == expression) {
									expression = format!("A {expression} B");
								} else if expression == "^" {
									expression = String::from("A^B");
								}

								expression
							})
						},
						node_id,
						ExpressionInput::INDEX,
					))
					.on_commit(commit_value)
					.widget_instance(),
			])
		}
		widgets
	})();
	let operand_b = number_widget(ParameterWidgetsInfo::new(node_id, OperandBInput::<f64>::INDEX, true, context), NumberInput::default());
	let operand_a_hint = vec![TextLabel::new("(Operand A is the primary input)").widget_instance()];

	vec![
		LayoutGroup::row(expression).with_tooltip_description(r#"A math expression that may incorporate "A" and/or "B", such as "sqrt(A + B) - B^2"."#),
		LayoutGroup::row(operand_b).with_tooltip_description(r#"The value of "B" when calculating the expression."#),
		LayoutGroup::row(operand_a_hint).with_tooltip_description(r#""A" is fed by the value from the previous node in the primary data flow, or it is 0 if disconnected."#),
	]
}

pub struct ParameterWidgetsInfo<'a> {
	network_interface: &'a NodeNetworkInterface,
	resources: &'a ResourceMessageHandler,
	selection_network_path: &'a [NodeId],
	document_node: Option<&'a DocumentNode>,
	node_id: NodeId,
	index: usize,
	name: String,
	description: String,
	input_type: FrontendGraphDataType,
	blank_assist: bool,
	exposable: bool,
	fonts: &'a FontsMessageHandler,
}

impl<'a> ParameterWidgetsInfo<'a> {
	pub fn new(node_id: NodeId, index: usize, blank_assist: bool, context: &'a mut NodePropertiesContext) -> ParameterWidgetsInfo<'a> {
		let (name, description) = context.network_interface.displayed_input_name_and_description(&node_id, index, context.selection_network_path);
		let input_type = context
			.network_interface
			.input_type_not_invalid(&InputConnector::node(node_id, index), context.selection_network_path)
			.displayed_type();
		let document_node = context.network_interface.document_node(&node_id, context.selection_network_path);

		ParameterWidgetsInfo {
			network_interface: context.network_interface,
			resources: context.resources,
			selection_network_path: context.selection_network_path,
			fonts: context.fonts,
			document_node,
			node_id,
			index,
			name,
			description,
			input_type,
			blank_assist,
			exposable: true,
		}
	}

	pub fn is_exposed(&self) -> bool {
		self.document_node.and_then(|node| node.inputs.get(self.index)).map(|input| input.is_exposed()).unwrap_or(false)
	}
}

pub mod choice {
	use super::ParameterWidgetsInfo;
	use crate::messages::tool::tool_messages::tool_prelude::*;
	use graph_craft::document::value::TaggedValue;
	use graphene_std::choice_type::{ChoiceTypeStatic, ChoiceWidgetHint};
	use std::marker::PhantomData;

	pub trait WidgetFactory {
		type Value: Clone + 'static;

		fn disabled(self, disabled: bool) -> Self;

		fn build<U, C>(&self, current: Self::Value, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetInstance
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
			ForSocket { widget_factory: self, parameter_info }
		}

		/// Not yet implemented!
		pub fn for_value(self, _current: E) -> ForValue<Self> {
			todo!()
		}

		pub fn disabled(self, disabled: bool) -> Self {
			Self { disabled, ..self }
		}

		/// Not yet implemented!
		pub fn into_menu_entries(self, _action: impl Fn(E) -> Message + 'static + Send + Sync) -> MenuListEntrySections {
			todo!()
		}

		fn dropdown_menu<U, C>(&self, current: E, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetInstance
		where
			U: Fn(&E) -> Message + 'static + Send + Sync,
			C: Fn(&()) -> Message + 'static + Send + Sync,
		{
			let items = E::list()
				.iter()
				.map(|section| {
					section
						.iter()
						.map(|(item, metadata)| {
							let updater = updater_factory();
							let committer = committer_factory();
							MenuListEntry::new(metadata.name)
								.label(metadata.label)
								.tooltip_label(metadata.label)
								.tooltip_description(metadata.description.unwrap_or_default())
								.on_update(move |_| updater(item))
								.on_commit(committer)
						})
						.collect()
				})
				.collect();
			DropdownInput::new(items).disabled(self.disabled).selected_index(Some(current.as_u32())).widget_instance()
		}

		fn radio_buttons<U, C>(&self, current: E, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetInstance
		where
			U: Fn(&E) -> Message + 'static + Send + Sync,
			C: Fn(&()) -> Message + 'static + Send + Sync,
		{
			let items = E::list()
				.iter()
				.flat_map(|section| section.iter())
				.map(|(item, var_meta)| {
					let updater = updater_factory();
					let committer = committer_factory();
					let entry = RadioEntryData::new(var_meta.name)
						.on_update(move |_| updater(item))
						.on_commit(committer)
						.tooltip_label(var_meta.label)
						.tooltip_description(var_meta.description.unwrap_or_default());
					if let Some(icon) = var_meta.icon { entry.icon(icon) } else { entry.label(var_meta.label) }
				})
				.collect();
			RadioInput::new(items).selected_index(Some(current.as_u32())).disabled(self.disabled).widget_instance()
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

		fn build<U, C>(&self, current: Self::Value, updater_factory: impl Fn() -> U, committer_factory: impl Fn() -> C) -> WidgetInstance
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

		pub fn property_row(self) -> LayoutGroup {
			let ParameterWidgetsInfo { document_node, node_id, index, .. } = self.parameter_info;
			let Some(document_node) = document_node else {
				log::error!("Could not get document node when building property row for node {node_id:?}");
				return LayoutGroup::row(Vec::new());
			};

			let mut widgets = super::start_widgets(self.parameter_info);

			let Some(input) = document_node.inputs.get(index) else {
				log::warn!("A widget failed to be built because its node's input index is invalid.");
				return LayoutGroup::row(vec![]);
			};

			let input: Option<W::Value> = input.as_non_exposed_value().and_then(|v| <&W::Value as TryFrom<&TaggedValue>>::try_from(v).ok()).cloned();

			if let Some(current) = input {
				let committer = || super::commit_value;
				let updater = || super::update_value(move |v: &W::Value| TaggedValue::from(v.clone()), node_id, index);
				let widget = self.widget_factory.build(current, updater, committer);
				widgets.extend_from_slice(&[Separator::new(SeparatorStyle::Unrelated).widget_instance(), widget]);
			}

			let mut row = LayoutGroup::row(widgets);
			if let Some(desc) = self.widget_factory.description() {
				row = row.with_tooltip_description(desc);
			}
			row
		}
	}

	pub struct ForValue<W>(PhantomData<W>);
}
