#![allow(clippy::too_many_arguments)]

use super::document_node_definitions::{NODE_OVERRIDES, NodePropertiesContext};
use super::utility_types::FrontendGraphDataType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface};
use crate::messages::portfolio::fonts::utility_types::FontCatalogStyle;
use crate::messages::portfolio::ingest::utility_types::{IngestAction, TypeFilter};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use choice::enum_choice;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graph_craft::application_io::resource::{DataSource, Resource, ResourceId};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput};
use graph_craft::{Type, concrete, item};
use graphene_std::animation::RealTimeMode;
use graphene_std::color::SRGBA8;
use graphene_std::extract_xy::XY;
use graphene_std::raster::{
	AdjustmentChannel, BlendMode, CellularDistanceFunction, CellularReturnType, Color, DesaturateMethod, DomainWarpType, FractalType, HueSaturationRange, NoiseType, RedGreenBlue, RedGreenBlueAlpha,
	RelativeAbsolute, SelectiveColorChoice, TonalRange,
};
use graphene_std::raster_types::Image;
use graphene_std::text::{Font, TextAlign};
use graphene_std::text_nodes::{StringCapitalization, TextDenomination};
use graphene_std::transfer_curve::TransferCurve;
use graphene_std::transform::{Footprint, ReferencePoint, ScaleType, Transform};
use graphene_std::vector::misc::BooleanOperation;
use graphene_std::vector::misc::{
	ArcType, BoxCorners, CentroidType, ExtrudeJoiningAlgorithm, GridType, InterpolationDistribution, MergeByDistanceAlgorithm, PointSpacingType, RowsOrColumns, SpiralType,
};
use graphene_std::vector::style::{
	FillChoice, Gradient, GradientForm, GradientHueDirection, GradientInterpolation, GradientRamp, GradientSettings, GradientSpace, GradientSpread, GradientStops, StrokeAlign, StrokeCap, StrokeJoin,
	build_transform_with_y_preservation,
};
use graphene_std::vector::{QRCodeErrorCorrectionLevel, VectorModification};
use graphene_std::{NodeParameter, ParameterRef};
use std::path::PathBuf;

pub(crate) fn string_properties(text: &str) -> Vec<LayoutGroup> {
	let widget = TextLabel::new(text).widget_instance();
	vec![LayoutGroup::row(vec![widget])]
}

fn optionally_update_value<T>(
	value: impl Fn(&T) -> Option<TaggedValue> + 'static + Send + Sync,
	node_id: NodeId,
	parameter: impl Into<ParameterRef>,
) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	optionally_update_value_at_index(value, node_id, parameter.into().input_index)
}

fn optionally_update_value_at_index<T>(value: impl Fn(&T) -> Option<TaggedValue> + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	move |input_value: &T| match value(input_value) {
		Some(value) => NodeGraphMessage::SetInputValue {
			node_id,
			input_index,
			value: value.into(),
		}
		.into(),
		None => Message::NoOp,
	}
}

pub fn update_value<T>(value: impl Fn(&T) -> TaggedValue + 'static + Send + Sync, node_id: NodeId, parameter: impl Into<ParameterRef>) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	optionally_update_value_at_index(move |v| Some(value(v)), node_id, parameter.into().input_index)
}

/// Like [`update_value`], for callers that receive the input index dynamically (e.g. widget overrides).
pub fn update_value_at_index<T>(value: impl Fn(&T) -> TaggedValue + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	optionally_update_value_at_index(move |v| Some(value(v)), node_id, input_index)
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
				input_connector: InputConnector::node_at_index(node_id, index),
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

pub fn start_widgets(parameter_widgets_info: &ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	if parameter_widgets_info.document_node.is_none() {
		log::warn!("A widget failed to be built because its document node is invalid.");
		return vec![];
	}

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};

	let mut widgets = Vec::with_capacity(6);
	if parameter_widgets_info.exposable {
		widgets.push(expose_widget(
			parameter_widgets_info.node_id,
			parameter_widgets_info.index,
			parameter_widgets_info.input_type,
			input.is_exposed(),
		));
	}
	widgets.push(
		TextLabel::new(parameter_widgets_info.name.clone())
			.tooltip_description(parameter_widgets_info.description.clone())
			.widget_instance(),
	);

	if parameter_widgets_info.blank_assist || input.is_exposed() {
		add_blank_assist(&mut widgets);
	}

	if input.is_exposed() {
		widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		widgets.push(jump_to_source_widget(input, parameter_widgets_info.network_interface, parameter_widgets_info.selection_network_path));
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

/// The values a range slider's two ends map to linearly and the one its double-click restores, if known.
#[derive(Clone, Copy)]
pub struct SliderRange {
	pub min: f64,
	pub max: f64,
	pub default: Option<f64>,
}

impl SliderRange {
	fn position(self, value: f64) -> f64 {
		((value - self.min) / (self.max - self.min)).clamp(0., 1.)
	}

	fn value(self, position: f64) -> f64 {
		(self.min + position * (self.max - self.min)).clamp(self.min, self.max)
	}
}

/// The number a parameter's definition gives it by default, which a slider's double-click restores.
fn definition_default_number(parameter_widgets_info: &ParameterWidgetsInfo) -> Option<f64> {
	let identifier = parameter_widgets_info
		.network_interface
		.reference(&parameter_widgets_info.node_id, parameter_widgets_info.selection_network_path)?;
	let input = resolve_document_node_type(&identifier)?.node_template.inputs.get(parameter_widgets_info.index)?;

	match input.as_value()? {
		TaggedValue::F64(value) => Some(*value),
		_ => None,
	}
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

	// A range-mode number clamped at both ends by its own hard bounds, or by a type whose extent is a true limit, becomes a range
	// slider beside its number input, unless a soft bound lets typing pass the slider. An Angle's type default is no such limit.
	let no_soft_bounds = soft_min.is_none() && soft_max.is_none();
	let hard_both_ends = hard_min.is_some() && hard_max.is_some();
	let number_or_slider = |default_info: ParameterWidgetsInfo, number_input: NumberInput, type_limits: bool| -> LayoutGroup {
		let fixed_extent = number_input.mode == NumberInputMode::Range && no_soft_bounds && (hard_both_ends || type_limits);
		match (number_input.min, number_input.max) {
			(Some(min), Some(max)) if fixed_extent && min.is_finite() && max.is_finite() && min < max => {
				let default = definition_default_number(&default_info);
				range_slider_widget(default_info, number_input.mode_increment(), SliderRange { min, max, default }).into()
			}
			_ => number_widget(default_info, number_input).into(),
		}
	};

	let default_info = ParameterWidgetsInfo::at_index(node_id, index, true, context);

	// A type with no widget can only be supplied through the graph, labeled with a placeholder row
	let unsupported_widgets = |default_info: ParameterWidgetsInfo, type_label: String| {
		let is_exposed = default_info.is_exposed();

		let mut widgets = start_widgets(&default_info);
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
				Some("Percentage") => number_or_slider(default_info, bounded(number_input.percentage(), 0., 100.), true),
				Some("SignedPercentage") => number_or_slider(default_info, bounded(number_input.percentage(), -100., 100.), true),
				Some("Angle") => number_or_slider(default_info, bounded(number_input.mode_range(), -180., 180.).unit(unit.unwrap_or("°")), false),
				Some("Multiplier") => number_widget(default_info, bounded(number_input, f64::NEG_INFINITY, f64::INFINITY).unit(unit.unwrap_or("x"))).into(),
				Some("PixelLength") => number_widget(default_info, bounded(number_input, 0., f64::INFINITY).unit(unit.unwrap_or(" px"))).into(),
				Some("Length") => number_widget(default_info, bounded(number_input, 0., f64::INFINITY)).into(),
				Some("Fraction") => number_or_slider(default_info, bounded(number_input.mode_range(), 0., 1.), true),
				Some("Progression") => progression_widget(default_info, bounded(number_input, 0., f64::INFINITY)).into(),
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
						Some(x) if id_is::<f64>(x) => number_or_slider(default_info, bounded(number_input, f64::NEG_INFINITY, f64::INFINITY), false),
						Some(x) if id_is::<i64>(x) => number_widget(default_info, bounded(number_input.int(), f64::NEG_INFINITY, f64::INFINITY)).into(),
						Some(x) if id_is::<bool>(x) => bool_widget(default_info, CheckboxInput::default()).into(),
						Some(x) if id_is::<String>(x) => text_widget(default_info).into(),
						Some(x) if id_is::<DVec2>(x) => vec2_widget(default_info, "X", "Y", "", None, false),
						Some(x) if id_is::<DAffine2>(x) => transform_widget(default_info, &mut extra_widgets),
						Some(x) if id_is::<Color>(x) => color_widget(default_info, ColorInput::default().allow_none(false)),
						Some(x) if id_is::<Gradient>(x) => color_widget(default_info, ColorInput::default().allow_none(false)),
						// ============
						// STRUCT TYPES
						// ============
						Some(x) if id_is::<Font>(x) => font_widget(default_info),
						Some(x) if id_is::<TransferCurve>(x) => transfer_curve_widget(default_info),
						Some(x) if id_is::<Footprint>(x) => footprint_widget(default_info, &mut extra_widgets),
						Some(x) if id_is::<Box<VectorModification>>(x) => vector_modification_widget(default_info).into(),
						Some(x) if id_is::<Image<Color>>(x) => image_data_widget(default_info).into(),
						Some(x) if id_is::<Resource>(x) => resource_widget(default_info, Vec::new()).into(),
						// ===============================
						// MANUALLY IMPLEMENTED ENUM TYPES
						// ===============================
						Some(x) if id_is::<ReferencePoint>(x) => reference_point_widget(default_info, false).into(),
						Some(x) if id_is::<BlendMode>(x) => blend_mode_widget(default_info),
						// =========================
						// AUTO-GENERATED ENUM TYPES
						// =========================
						Some(x) if id_is::<GradientForm>(x) => enum_choice::<GradientForm>().for_socket(default_info).property_row(),
						Some(x) if id_is::<GradientSpread>(x) => enum_choice::<GradientSpread>().for_socket(default_info).property_row(),
						Some(x) if id_is::<GradientSpace>(x) => enum_choice::<GradientSpace>().for_socket(default_info).property_row(),
						Some(x) if id_is::<GradientHueDirection>(x) => enum_choice::<GradientHueDirection>().for_socket(default_info).property_row(),
						Some(x) if id_is::<GradientInterpolation>(x) => enum_choice::<GradientInterpolation>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RealTimeMode>(x) => enum_choice::<RealTimeMode>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RedGreenBlue>(x) => enum_choice::<RedGreenBlue>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RedGreenBlueAlpha>(x) => enum_choice::<RedGreenBlueAlpha>().for_socket(default_info).property_row(),
						Some(x) if id_is::<XY>(x) => enum_choice::<XY>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StringCapitalization>(x) => enum_choice::<StringCapitalization>().for_socket(default_info).property_row(),
						Some(x) if id_is::<TextDenomination>(x) => enum_choice::<TextDenomination>().for_socket(default_info).property_row(),
						Some(x) if id_is::<NoiseType>(x) => enum_choice::<NoiseType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<FractalType>(x) => enum_choice::<FractalType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<CellularDistanceFunction>(x) => enum_choice::<CellularDistanceFunction>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<CellularReturnType>(x) => enum_choice::<CellularReturnType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<DomainWarpType>(x) => enum_choice::<DomainWarpType>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<RelativeAbsolute>(x) => enum_choice::<RelativeAbsolute>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<TonalRange>(x) => enum_choice::<TonalRange>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<AdjustmentChannel>(x) => enum_choice::<AdjustmentChannel>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<HueSaturationRange>(x) => enum_choice::<HueSaturationRange>().for_socket(default_info).disabled(false).property_row(),
						Some(x) if id_is::<GridType>(x) => enum_choice::<GridType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StrokeCap>(x) => enum_choice::<StrokeCap>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StrokeJoin>(x) => enum_choice::<StrokeJoin>().for_socket(default_info).property_row(),
						Some(x) if id_is::<StrokeAlign>(x) => enum_choice::<StrokeAlign>().for_socket(default_info).property_row(),
						Some(x) if id_is::<ArcType>(x) => enum_choice::<ArcType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<RowsOrColumns>(x) => enum_choice::<RowsOrColumns>().for_socket(default_info).property_row(),
						Some(x) if id_is::<TextAlign>(x) => enum_choice::<TextAlign>().for_socket(default_info).property_row(),
						Some(x) if id_is::<MergeByDistanceAlgorithm>(x) => enum_choice::<MergeByDistanceAlgorithm>().for_socket(default_info).property_row(),
						Some(x) if id_is::<ExtrudeJoiningAlgorithm>(x) => enum_choice::<ExtrudeJoiningAlgorithm>().for_socket(default_info).property_row(),
						Some(x) if id_is::<PointSpacingType>(x) => enum_choice::<PointSpacingType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<BooleanOperation>(x) => enum_choice::<BooleanOperation>().for_socket(default_info).property_row(),
						Some(x) if id_is::<CentroidType>(x) => enum_choice::<CentroidType>().for_socket(default_info).property_row(),
						Some(x) if id_is::<DesaturateMethod>(x) => enum_choice::<DesaturateMethod>().for_socket(default_info).property_row(),
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
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(x.clone())
				.on_update(parameter_widgets_info.update_value(|x: &TextInput| TaggedValue::String(x.value.clone())))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

pub fn text_area_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::String(x)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextAreaInput::new(x.clone())
				.on_update(parameter_widgets_info.update_value(|x: &TextAreaInput| TaggedValue::String(x.value.clone())))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

pub fn bool_widget(parameter_widgets_info: ParameterWidgetsInfo, checkbox_input: CheckboxInput) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(x)) = input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			checkbox_input
				.checked(x)
				.on_update(parameter_widgets_info.update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked)))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

pub fn reference_point_widget(parameter_widgets_info: ParameterWidgetsInfo, disabled: bool) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::ReferencePoint(reference_point)) = input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			CheckboxInput::new(reference_point != ReferencePoint::None)
				.on_update(parameter_widgets_info.update_value(move |x: &CheckboxInput| TaggedValue::ReferencePoint(if x.checked { ReferencePoint::Center } else { ReferencePoint::None })))
				.disabled(disabled)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			ReferencePointInput::new(reference_point)
				.on_update(parameter_widgets_info.update_value(move |x: &ReferencePointInput| TaggedValue::ReferencePoint(x.value)))
				.disabled(disabled)
				.widget_instance(),
		])
	}
	widgets
}

pub fn vector_modification_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id: _, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(&parameter_widgets_info);

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

pub fn image_data_widget(parameter_widgets_info: ParameterWidgetsInfo) -> Vec<WidgetInstance> {
	let ParameterWidgetsInfo { document_node, node_id: _, index, .. } = parameter_widgets_info;

	let mut widgets = start_widgets(&parameter_widgets_info);

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

	let mut location_widgets = start_widgets(&parameter_widgets_info);
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
				.on_update(parameter_widgets_info.update_value(move |x: &NumberInput| {
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
				}))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(Some(top_left.y))
				.label("Y")
				.unit(" px")
				.on_update(parameter_widgets_info.update_value(move |x: &NumberInput| {
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
				}))
				.on_commit(commit_value)
				.widget_instance(),
		]);

		scale_widgets.extend_from_slice(&[
			NumberInput::new(Some(bounds.x))
				.label("W")
				.unit(" px")
				.on_update(update_value_at_index(
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
				.on_update(update_value_at_index(
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
				.on_update(parameter_widgets_info.update_value(move |x: &NumberInput| {
					let resolution = (bounds * x.value.unwrap_or(100.) / 100.).as_uvec2().max((1, 1).into()).min((4000, 4000).into());

					let footprint = Footprint { resolution, ..footprint };
					TaggedValue::Footprint(footprint)
				}))
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

	let mut location_widgets = start_widgets(&parameter_widgets_info);
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
				.on_update(parameter_widgets_info.update_value(move |x: &NumberInput| {
					let mut transform = transform;
					transform.translation.x = x.value.unwrap_or(transform.translation.x);
					TaggedValue::DAffine2(transform)
				}))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(Some(translation.y))
				.label("Y")
				.unit(" px")
				.on_update(parameter_widgets_info.update_value(move |y: &NumberInput| {
					let mut transform = transform;
					transform.translation.y = y.value.unwrap_or(transform.translation.y);
					TaggedValue::DAffine2(transform)
				}))
				.on_commit(commit_value)
				.widget_instance(),
		]);

		rotation_widgets.extend_from_slice(&[NumberInput::new(Some(rotation.to_degrees()))
			.unit("°")
			.mode(NumberInputMode::Range)
			.range_min(Some(-180.))
			.range_max(Some(180.))
			.on_update(update_value_at_index(
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
				.on_update(update_value_at_index(
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
				.on_update(update_value_at_index(
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
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
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
					.on_update(parameter_widgets_info.update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), dvec2.y))))
					.on_commit(commit_value)
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				NumberInput::new(Some(dvec2.y))
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.is_integer(is_integer)
					.on_update(parameter_widgets_info.update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(dvec2.x, input.value.unwrap()))))
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
					.on_update(parameter_widgets_info.update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), value))))
					.on_commit(commit_value)
					.widget_instance(),
				Separator::new(SeparatorStyle::Related).widget_instance(),
				NumberInput::new(Some(value))
					.label(y)
					.unit(unit)
					.min(min.unwrap_or(-((1_u64 << f64::MANTISSA_DIGITS) as f64)))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.is_integer(is_integer)
					.on_update(parameter_widgets_info.update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(value, input.value.unwrap()))))
					.on_commit(commit_value)
					.widget_instance(),
			]);
		}
		_ => {}
	}

	LayoutGroup::row(widgets)
}

pub fn array_of_number_widget(parameter_widgets_info: ParameterWidgetsInfo, text_input: TextInput) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let from_string = |string: &str| {
		string
			.split(&[',', ' '])
			.filter(|x| !x.is_empty())
			.map(graphene_std::core_types::misc::parse_f64)
			.collect::<Option<Vec<_>>>()
			.map(TaggedValue::F64Array)
	};

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::F64Array(values)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			text_input
				.value(values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
				.on_update(parameter_widgets_info.optionally_update_value(move |x: &TextInput| from_string(&x.value)))
				.widget_instance(),
		])
	}
	widgets
}

pub fn dash_pattern_widget(parameter_widgets_info: ParameterWidgetsInfo, text_input: TextInput) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(TaggedValue::DashPattern(lengths)) = &input.as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			text_input
				.value(lengths.iter().map(|length| length.to_string()).collect::<Vec<_>>().join(", "))
				.on_update(parameter_widgets_info.optionally_update_value(move |input: &TextInput| Some(TaggedValue::DashPattern(graphene_std::core_types::misc::parse_f64_list(&input.value)))))
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
					value: TaggedValue::Resource(resource_id).into(),
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

	let mut first_widgets = start_widgets(&parameter_widgets_info);
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
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
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
				.on_update(parameter_widgets_info.update_value(move |input: &NumberInput| TaggedValue::F64(whole_part + input.value.unwrap())))
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
				.on_update(parameter_widgets_info.update_value(move |input: &NumberInput| TaggedValue::F64(input.value.unwrap() + fractional_part)))
				.on_commit(commit_value)
				.widget_instance(),
		])
	}
	widgets
}

/// `parameter_widgets_info` is for the f64 parameter. `bool_input_index` is the input index of the bool parameter for the checkbox.
/// A number row gated by the bool input at `bool_input_index`, drawn as a checkbox in the assist slot after the label like the
/// Opacity node's toggles, so the caller passes `blank_assist = false`. Given a `slider`, a range slider spanning it sits between them.
pub fn optional_f64_widget(parameter_widgets_info: ParameterWidgetsInfo, bool_input_index: usize, number_props: NumberInput, slider: Option<SliderRange>) -> Vec<WidgetInstance> {
	let node_id = parameter_widgets_info.node_id;
	let enabled = parameter_widgets_info
		.document_node
		.and_then(|document_node| document_node.inputs.get(bool_input_index))
		.and_then(|input| input.as_non_exposed_value())
		.and_then(|value| if let TaggedValue::Bool(enabled) = value { Some(*enabled) } else { None });
	let label_count = start_widgets(&parameter_widgets_info).len();
	let exposed = parameter_widgets_info.is_exposed();

	let number_props = number_props.disabled(enabled == Some(false));
	let mut widgets = match slider {
		Some(slider) => range_slider_widget(parameter_widgets_info, number_props, slider),
		None => number_widget(parameter_widgets_info, number_props),
	};

	if let Some(enabled) = enabled
		&& !exposed
	{
		let checkbox = [
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(enabled)
				.on_update(update_value_at_index(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, bool_input_index))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
		];
		widgets.splice(label_count..label_count, checkbox);
	}

	widgets
}

/// `parameter_widgets_info` is for the color parameter. `bool_input_index` is the input index of the bool parameter, drawn as a checkbox in front of the color.
/// A color row gated by the bool input at `bool_input_index`, whose checkbox takes the assist slot after the label like the
/// Opacity node's toggles, so the caller passes `blank_assist = false`. An exposed color shows neither, as in that node.
pub fn optional_color_widget(parameter_widgets_info: ParameterWidgetsInfo, bool_input_index: usize, color_button: ColorInput) -> LayoutGroup {
	let node_id = parameter_widgets_info.node_id;
	let enabled = parameter_widgets_info
		.document_node
		.and_then(|document_node| document_node.inputs.get(bool_input_index))
		.and_then(|input| input.as_non_exposed_value())
		.and_then(|value| if let TaggedValue::Bool(enabled) = value { Some(*enabled) } else { None });
	let label_count = start_widgets(&parameter_widgets_info).len();
	let exposed = parameter_widgets_info.is_exposed();

	let LayoutGroup::Row(mut row) = color_widget(parameter_widgets_info, color_button.disabled(enabled == Some(false))) else {
		return LayoutGroup::row(Vec::new());
	};
	if let Some(enabled) = enabled
		&& !exposed
	{
		let checkbox = [
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(enabled)
				.on_update(update_value_at_index(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, bool_input_index))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
		];
		row.widgets.splice(label_count..label_count, checkbox);
	}

	LayoutGroup::Row(row)
}

pub fn number_widget(parameter_widgets_info: ParameterWidgetsInfo, number_props: NumberInput) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	match input.as_non_exposed_value() {
		Some(&TaggedValue::F64(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.value(Some(x))
				.on_update(parameter_widgets_info.update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap())))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		Some(&TaggedValue::I64(x)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
				.value(Some(x as f64))
				.on_update(parameter_widgets_info.update_value(move |x: &NumberInput| TaggedValue::I64(x.value.unwrap().round() as i64)))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		Some(&TaggedValue::DVec2(dvec2)) => widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			number_props
			// We use an arbitrary `y` instead of an arbitrary `x` here because the "Grid" node's "Spacing" value's height should be used from rectangular mode when transferred to "Y Spacing" in isometric mode
				.value(Some(dvec2.y))
				.on_update(parameter_widgets_info.update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap())))
				.on_commit(commit_value)
				.widget_instance(),
		]),
		_ => {}
	}

	widgets
}

// TODO: Auto-generate this enum dropdown menu widget
pub fn blend_mode_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
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
							.on_update(parameter_widgets_info.update_value(move |_| TaggedValue::BlendMode(*blend_mode)))
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
	let mut widgets = start_widgets(&parameter_widgets_info);

	// Return early with just the label if the input is exposed to the graph, meaning we don't want to show the color picker widget in the Properties panel
	let Some(NodeInput::Value { tagged_value, exposed: false }) = parameter_widgets_info.input() else {
		return LayoutGroup::row(widgets);
	};

	// Add a separator
	widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

	// Add the color input
	let widget_value = match &**tagged_value {
		TaggedValue::Color(color) => FillChoice::<SRGBA8>::Solid(SRGBA8::from(*color)),
		TaggedValue::GradientRamp(ramp) => FillChoice::<SRGBA8>::Gradient(GradientRamp::from(ramp)),
		value if value.is_no_paint() => FillChoice::<SRGBA8>::None,
		x => {
			warn!("Color {x:?}");
			return LayoutGroup::row(widgets);
		}
	};

	// A paint input (`allow_none`) stores the pick as a plain color, gradient, or no-paint type default,
	// while a plain color or gradient input always keeps its own value type
	let on_update: fn(&ColorInput) -> TaggedValue = if color_button.allow_none {
		|input| match &input.value {
			FillChoice::<SRGBA8>::None => TaggedValue::no_paint(),
			FillChoice::<SRGBA8>::Solid(srgba) => TaggedValue::Color(Color::from(*srgba)),
			FillChoice::<SRGBA8>::Gradient(ramp) => TaggedValue::GradientRamp(GradientRamp::from(ramp)),
		}
	} else if matches!(&**tagged_value, TaggedValue::GradientRamp(_)) {
		|input| TaggedValue::GradientRamp(input.value.as_gradient().map(GradientRamp::from).unwrap_or_else(GradientRamp::black_to_white))
	} else {
		|input| TaggedValue::Color(input.value.as_solid().map(Color::from).unwrap_or(Color::TRANSPARENT))
	};

	widgets.push(
		color_button
			.value(widget_value)
			.on_update(parameter_widgets_info.update_value(on_update))
			.on_commit(commit_value)
			.widget_instance(),
	);

	LayoutGroup::row(widgets)
}

/// A [`TransferCurve`] input's row: the label, then the curve editor spanning the unit square when the input is not exposed.
pub fn transfer_curve_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(NodeInput::Value { tagged_value, exposed: false }) = parameter_widgets_info.input() else {
		return LayoutGroup::row(widgets);
	};
	let TaggedValue::TransferCurve(points) = &**tagged_value else { return LayoutGroup::row(widgets) };
	let curve = TransferCurve::from(points.clone());

	widgets.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
	widgets.push(
		TransferCurveInput::new(curve.points().iter().map(|point| (point.x, point.y)).collect())
			.domain([0., 1.])
			.range([0., 1.])
			.clamp_to_range(true)
			.allow_insert(true)
			.allow_delete(true)
			.on_update(parameter_widgets_info.update_value(move |update: &TransferCurveInputUpdate| {
				let mut curve = curve.clone();
				match *update {
					TransferCurveInputUpdate::MovePoint { index, x, y } => curve.move_point(index as usize, DVec2::new(x, y)),
					TransferCurveInputUpdate::InsertPoint { x, y } => {
						curve.insert_point(DVec2::new(x, y));
					}
					// A transfer curve keeps at least its two end points
					TransferCurveInputUpdate::DeletePoint { index } if curve.points().len() > 2 => curve.remove_point(index as usize),
					TransferCurveInputUpdate::DeletePoint { .. } => {}
				}
				TaggedValue::TransferCurve(curve.points().to_vec())
			}))
			.on_commit(commit_value)
			.widget_instance(),
	);

	LayoutGroup::row(widgets)
}

pub fn font_widget(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
	let (font_widgets, style_widgets) = font_inputs(parameter_widgets_info);
	font_widgets.into_iter().chain(style_widgets.unwrap_or_default()).collect::<Vec<_>>().into()
}

/// A dropdown of the document's uploaded files, led by "None" and a "Browse…" entry that uploads another file matching the given filters, as dropping a file onto it also does.
pub fn resource_widget(parameter_widgets_info: ParameterWidgetsInfo, filters: Vec<TypeFilter>) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	let selected = match input.as_non_exposed_value() {
		Some(TaggedValue::Resource(resource_id)) => Some(*resource_id),
		Some(TaggedValue::TypeDefault(_)) => None,
		_ => return widgets,
	};

	let ParameterWidgetsInfo {
		document_id,
		node_id,
		index,
		resources,
		network_interface,
		..
	} = parameter_widgets_info;
	let use_counts = network_interface.collect_resources_use_counts();

	// Fonts are the only resources the registry tells apart, so a picker also lists the files of other types.
	// TODO: Record each resource's data type so a picker lists only the files its input accepts.
	let mut files: Vec<(ResourceId, String, String)> = resources
		.registry
		.resolved()
		.filter(|info| !info.sources.iter().any(|source| matches!(source, DataSource::Font { .. })))
		.map(|info| {
			let hash = info.hash.map(|hash| hash.to_string()[..8].to_string()).unwrap_or_default();
			let users = use_counts.get(&info.id).copied().unwrap_or(0);
			let tooltip_description = match users {
				0 => "Not used by any node input. This resource will be dropped upon document reload.".to_string(),
				users => format!("Used by {users} node input{}.", if users == 1 { "" } else { "s" }),
			};
			let uses = match users {
				0 => "unused".to_string(),
				1 => "1 use".to_string(),
				users => format!("{users} uses"),
			};
			(info.id, format!("{hash} · {uses}"), tooltip_description)
		})
		.collect();
	files.sort();

	// Entries assign only on click, since a hover preview leaves the replaced file unreferenced and garbage collected
	let assign_on_click = |value: TaggedValue| {
		move |_: &()| Message::Batched {
			messages: Box::new([
				DocumentMessage::AddTransaction.into(),
				NodeGraphMessage::SetInputValue {
					node_id,
					input_index: index,
					value: value.clone().into(),
				}
				.into(),
			]),
		}
	};
	let file_drop_action = IngestAction::resource_input(document_id, node_id, index, &filters);

	let none = MenuListEntry::new("none")
		.label("None")
		.tooltip_description("No resource assigned to this input.")
		.on_update(|_| Message::NoOp)
		.on_commit(assign_on_click(TaggedValue::TypeDefault(item!(Resource))));
	let browse = MenuListEntry::new("browse")
		.label("Browse…")
		.tooltip_description("Pick a file from disk to use for this input.")
		.on_update(|_| Message::NoOp)
		.on_commit(move |_| {
			IngestMessage::SetResourceInput {
				document_id,
				node_id,
				input_index: index,
				filters: filters.clone(),
			}
			.into()
		});
	let file_entries = files
		.iter()
		.map(|(resource_id, label, tooltip_description)| {
			MenuListEntry::new(format!("{resource_id:?}"))
				.label(label.clone())
				.tooltip_description(tooltip_description.clone())
				.on_update(|_| Message::NoOp)
				.on_commit(assign_on_click(TaggedValue::Resource(*resource_id)))
		})
		.collect();
	let selected_index = match selected {
		None => Some(0),
		Some(selected) => files.iter().position(|(resource_id, ..)| *resource_id == selected).map(|position| position as u32 + 2),
	};

	widgets.extend_from_slice(&[
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		DropdownInput::new(vec![vec![none, browse], file_entries])
			.selected_index(selected_index)
			.on_file_drop(move |file| {
				IngestMessage::Ingest {
					data: file.data.clone(),
					action: file_drop_action.clone(),
					mime_type: file.mime_type.clone(),
					path: Some(PathBuf::from(&file.name)),
				}
				.into()
			})
			.widget_instance(),
	]);
	widgets
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
	Ok(match document_node.input(RandomizeInput).and_then(|input| input.as_value()) {
		Some(TaggedValue::Bool(randomize_enabled)) => *randomize_enabled,
		_ => false,
	})
}

/// 2-stop black-to-white gradient track for sliders that map a value to a grayscale axis.
fn bw_track() -> Gradient {
	Gradient::from(vec![Color::BLACK, Color::WHITE])
}

/// 3-stop black-to-color-to-white gradient track for sliders that map a value to a hue's full luminance range.
fn color_track(color: Color) -> Gradient {
	Gradient::from(vec![Color::BLACK, color, Color::WHITE])
}

pub(crate) fn brightness_contrast_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::brightness_contrast::*;

	// Use Classic toggle changes the brightness range
	let use_classic_value = get_document_node(node_id, context)
		.ok()
		.and_then(|document_node| document_node.input(UseClassicInput).and_then(|input| input.as_value()))
		.and_then(|tagged| if let TaggedValue::Bool(value) = tagged { Some(*value) } else { None })
		.unwrap_or(false);

	let brightness_min = if use_classic_value { -100. } else { -150. };
	let brightness_max = if use_classic_value { 100. } else { 150. };

	let brightness = gradient_slider_row(
		node_id,
		context,
		BrightnessInput,
		bw_track(),
		Color::WHITE,
		brightness_min,
		brightness_max,
		0.,
		NumberInput::default().mode_increment().unit("%").min(brightness_min).max(brightness_max),
	);

	let contrast_min = if use_classic_value { -100. } else { -50. };
	let zero_position = -contrast_min / (100. - contrast_min);
	let mut contrast_track = Gradient::from(vec![Color::MIDDLE_GRAY, Color::BLACK, Color::MIDDLE_GRAY]);
	contrast_track.set_positions(&[0., zero_position, 1.]);
	let contrast = gradient_slider_row(
		node_id,
		context,
		ContrastInput,
		contrast_track,
		Color::WHITE,
		contrast_min,
		100.,
		0.,
		NumberInput::default().mode_increment().unit("%").min(contrast_min).max(100.),
	);

	let use_classic = bool_widget(ParameterWidgetsInfo::new(node_id, UseClassicInput, true, context), CheckboxInput::default());

	let mut layout = vec![brightness, contrast, LayoutGroup::row(use_classic)];
	if use_classic_value {
		let number_input = NumberInput::default().mode_increment().min(0.).max(255.);
		layout.push(gradient_slider_row(node_id, context, ClassicPivotInput, bw_track(), Color::WHITE, 0., 255., 127., number_input));
	}

	layout
}

pub(crate) fn transfer_curves_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::curves::*;

	let mut channel_info = ParameterWidgetsInfo::new(node_id, ChannelInput, true, context);
	channel_info.exposable = false;
	let channel = enum_choice::<AdjustmentChannel>().for_socket(channel_info).property_row();

	let channel_value = match get_document_node(node_id, context).ok().and_then(|document_node| document_node.input_value(ChannelInput).cloned()) {
		Some(TaggedValue::AdjustmentChannel(channel)) => channel,
		_ => AdjustmentChannel::Rgb,
	};
	let curve_parameter: ParameterRef = match channel_value {
		AdjustmentChannel::Rgb => CurveInput.into(),
		AdjustmentChannel::Red => RedCurveInput.into(),
		AdjustmentChannel::Green => GreenCurveInput.into(),
		AdjustmentChannel::Blue => BlueCurveInput.into(),
		AdjustmentChannel::Alpha => AlphaCurveInput.into(),
	};
	let transfer_curve = transfer_curve_widget(ParameterWidgetsInfo::new(node_id, curve_parameter, true, context));

	vec![channel, transfer_curve]
}

pub(crate) fn levels_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::levels::*;

	let mut channel_info = ParameterWidgetsInfo::new(node_id, ChannelInput, true, context);
	channel_info.exposable = false;
	let channel = enum_choice::<AdjustmentChannel>().for_socket(channel_info).property_row();

	let channel_value = match get_document_node(node_id, context).ok().and_then(|document_node| document_node.input_value(ChannelInput).cloned()) {
		Some(TaggedValue::AdjustmentChannel(channel)) => channel,
		_ => AdjustmentChannel::Rgb,
	};
	let [shadows, midtones, highlights, output_minimums, output_maximums]: [ParameterRef; 5] = match channel_value {
		AdjustmentChannel::Rgb => [
			ShadowsInput.into(),
			MidtonesInput.into(),
			HighlightsInput.into(),
			OutputMinimumsInput.into(),
			OutputMaximumsInput.into(),
		],
		AdjustmentChannel::Red => [
			RedShadowsInput.into(),
			RedMidtonesInput.into(),
			RedHighlightsInput.into(),
			RedOutputMinimumsInput.into(),
			RedOutputMaximumsInput.into(),
		],
		AdjustmentChannel::Green => [
			GreenShadowsInput.into(),
			GreenMidtonesInput.into(),
			GreenHighlightsInput.into(),
			GreenOutputMinimumsInput.into(),
			GreenOutputMaximumsInput.into(),
		],
		AdjustmentChannel::Blue => [
			BlueShadowsInput.into(),
			BlueMidtonesInput.into(),
			BlueHighlightsInput.into(),
			BlueOutputMinimumsInput.into(),
			BlueOutputMaximumsInput.into(),
		],
		AdjustmentChannel::Alpha => [
			AlphaShadowsInput.into(),
			AlphaMidtonesInput.into(),
			AlphaHighlightsInput.into(),
			AlphaOutputMinimumsInput.into(),
			AlphaOutputMaximumsInput.into(),
		],
	};

	let input_range_params = [
		SliderSectionParam::new(shadows, Color::BLACK, 0., MarkerScale::Percent),
		SliderSectionParam::new(midtones, Color::MIDDLE_GRAY, 1., MarkerScale::Gamma).between_neighbors(),
		SliderSectionParam::new(highlights, Color::WHITE, 100., MarkerScale::Percent),
	];
	let output_range_params = [
		SliderSectionParam::new(output_minimums, Color::BLACK, 0., MarkerScale::Percent),
		SliderSectionParam::new(output_maximums, Color::WHITE, 100., MarkerScale::Percent),
	];

	let mut layout = vec![channel];
	build_shared_slider_section(node_id, context, &bw_track(), &input_range_params, &mut layout);
	build_shared_slider_section(node_id, context, &bw_track(), &output_range_params, &mut layout);
	layout
}

/// How a shared slider marker's value maps onto its track.
#[derive(Clone, Copy)]
enum MarkerScale {
	/// A 0..100 percentage, placed linearly.
	Percent,
	/// A gamma of 0.01..9.99 running from 9.99 at the left to 0.01 at the right, logarithmic on each side of the 1 at its center.
	Gamma,
	/// A hue of 0..360 degrees, placed linearly on a track that wraps around.
	Degrees,
}

impl MarkerScale {
	fn position(self, value: f64) -> f64 {
		match self {
			Self::Percent => value / 100.,
			Self::Gamma if value >= 1. => 0.5 - 0.5 * value.log10() / 9.99_f64.log10(),
			Self::Gamma => 0.5 + 0.5 * value.log10() / 0.01_f64.log10(),
			Self::Degrees => {
				// A full turn stays at the far end, so only a value beyond one turn wraps
				let turns = value / 360.;
				if (0.0..=1.).contains(&turns) { turns } else { turns.rem_euclid(1.) }
			}
		}
		.clamp(0., 1.)
	}

	fn value(self, position: f64) -> f64 {
		match self {
			Self::Percent => (position * 100.).clamp(0., 100.),
			Self::Gamma if position <= 0.5 => 9.99_f64.powf(1. - 2. * position).clamp(1., 9.99),
			Self::Gamma => 0.01_f64.powf(2. * position - 1.).clamp(0.01, 1.),
			Self::Degrees => (position * 360.).clamp(0., 360.),
		}
	}

	fn number_input(self) -> NumberInput {
		match self {
			Self::Percent => NumberInput::default().mode_increment().unit("%").min(0.).max(100.).display_decimal_places(0),
			Self::Gamma => NumberInput::default().mode_increment().min(0.01).max(9.99).display_decimal_places(2),
			Self::Degrees => NumberInput::default().mode_increment().unit("°").min(0.).max(360.).display_decimal_places(0),
		}
	}

	/// Whether the track wraps around, so its markers may sit in any order.
	fn cyclic(self) -> bool {
		matches!(self, Self::Degrees)
	}
}

/// One parameter of a shared slider section and how its marker sits on the track.
struct SliderSectionParam {
	parameter: ParameterRef,
	handle_color: Color,
	/// The value a double-click resets to.
	default_value: f64,
	scale: MarkerScale,
	/// Whether the marker and the next parameter's marker form one split handle.
	pair_with_next: bool,
	/// Whether a dashed line joins the marker to the next parameter's marker.
	dash_to_next: bool,
	/// Whether the marker takes its scale position within the span between its neighbors rather than the whole track, following them as they move.
	between_neighbors: bool,
}

impl SliderSectionParam {
	fn new(parameter: impl Into<ParameterRef>, handle_color: Color, default_value: f64, scale: MarkerScale) -> Self {
		Self {
			parameter: parameter.into(),
			handle_color,
			default_value,
			scale,
			pair_with_next: false,
			dash_to_next: false,
			between_neighbors: false,
		}
	}

	fn between_neighbors(mut self) -> Self {
		self.between_neighbors = true;
		self
	}

	fn pair_with_next(mut self) -> Self {
		self.pair_with_next = true;
		self
	}

	fn dash_to_next(mut self) -> Self {
		self.dash_to_next = true;
		self
	}
}

/// Append a section of related parameters as rows: a shared slider over `track` (with one marker per non-exposed parameter) sits on the first non-exposed row
/// alongside its 60px number input, and the remaining non-exposed rows show only their 60px number input. Exposed parameters render as the standard exposed-row display.
/// Marker positions are clamped to non-decreasing display order so they never visually cross even if the underlying values do.
fn build_shared_slider_section(node_id: NodeId, context: &mut NodePropertiesContext, track: &Gradient, params: &[SliderSectionParam], layout: &mut Vec<LayoutGroup>) {
	// Snapshot exposure and values before the mutable-borrow loop
	let exposure_and_value: Vec<(bool, f64)> = match get_document_node(node_id, context) {
		Ok(document_node) => params
			.iter()
			.map(|param| {
				let input = document_node.inputs.get(param.parameter.input_index);
				let exposed = input.is_some_and(|input| input.is_exposed());
				let value = input
					.and_then(|input| input.as_value())
					.and_then(|tagged| if let TaggedValue::F64(value) = tagged { Some(*value) } else { None })
					.unwrap_or(0.);
				(exposed, value)
			})
			.collect(),
		Err(err) => {
			log::error!("Could not get document node in build_shared_slider_section: {err}");
			return;
		}
	};

	// Build markers for all non-exposed params, linking one to the next only when both have markers
	let mut marker_input_indices = Vec::new();
	let mut marker_default_positions = Vec::new();
	let mut marker_scales = Vec::new();
	let mut marker_positions = Vec::new();
	let mut marker_between = Vec::new();
	let mut marker_colors_and_links = Vec::new();
	for (i, param) in params.iter().enumerate() {
		let (exposed, value) = exposure_and_value[i];
		if exposed {
			continue;
		}
		let next_has_marker = exposure_and_value.get(i + 1).is_some_and(|&(next_exposed, _)| !next_exposed);
		marker_positions.push(param.scale.position(value));
		marker_input_indices.push(param.parameter.input_index);
		marker_default_positions.push(param.scale.position(param.default_value));
		marker_scales.push(param.scale);
		marker_between.push(param.between_neighbors);
		marker_colors_and_links.push((param.handle_color, param.pair_with_next && next_has_marker, param.dash_to_next && next_has_marker));
	}

	// Enforce non-decreasing order so markers never visually cross, matching the node's algorithm where shadows takes precedence.
	// A marker placed between its neighbors bounds nothing here and instead takes its scale position within their settled span.
	let cyclic = params.iter().any(|param| param.scale.cyclic());
	if !cyclic {
		let mut floor = 0.;
		for (position, &between) in marker_positions.iter_mut().zip(&marker_between) {
			if between {
				continue;
			}
			*position = position.max(floor);
			floor = *position;
		}
	}
	for i in 0..marker_positions.len() {
		if marker_between[i] {
			let left = if i == 0 { 0. } else { marker_positions[i - 1] };
			let right = marker_positions.get(i + 1).copied().unwrap_or(1.);
			marker_positions[i] = left + marker_positions[i] * (right - left);
		}
	}

	let slider_markers: Vec<SliderMarker> = marker_positions
		.iter()
		.zip(&marker_colors_and_links)
		.zip(&marker_between)
		.map(|((&position, &(handle_color, paired, dashed)), &between)| {
			let mut marker = SliderMarker::new(position, 0.5, handle_color);
			if paired {
				marker = marker.pair_with_next();
			}
			if dashed {
				marker = marker.dash_to_next();
			}
			if between {
				marker = marker.between_neighbors();
			}
			marker
		})
		.collect();

	// Build the shared slider widget (placed on the first non-exposed row)
	let slider_widget = (!slider_markers.is_empty()).then(|| {
		SliderInput::new(GradientStops::from(track))
			.track_space(GradientSpace::RgbGamma)
			.markers(slider_markers)
			.show_midpoints(false)
			.allow_insert(false)
			.allow_delete(false)
			.allow_reorder(false)
			.allow_wrap(cyclic)
			.narrow(true)
			.on_update({
				let marker_input_indices = marker_input_indices.clone();
				let marker_default_positions = marker_default_positions.clone();
				let marker_scales = marker_scales.clone();
				let marker_positions = marker_positions.clone();
				let marker_between = marker_between.clone();
				move |update: &SliderInputUpdate| {
					let i = match update {
						SliderInputUpdate::MoveMarker { index, .. } | SliderInputUpdate::ResetMarker { index } => *index as usize,
						_ => return Message::NoOp,
					};
					let (Some(&input_index), Some(&scale), Some(&between), Some(&default_position)) =
						(marker_input_indices.get(i), marker_scales.get(i), marker_between.get(i), marker_default_positions.get(i))
					else {
						return Message::NoOp;
					};

					// The span the marker's scale maps onto: its neighbors' positions when placed between them, otherwise the track between the
					// nearest markers that bound it, which a marker placed between its neighbors never does
					let bounding = |j: usize| between || !marker_between[j];
					let left = (0..i).rev().find(|&j| bounding(j)).map_or(0., |j| marker_positions[j]);
					let right = (i + 1..marker_positions.len()).find(|&j| bounding(j)).map_or(1., |j| marker_positions[j]);

					let scale_position = match update {
						SliderInputUpdate::MoveMarker { position, .. } if between => {
							let span = right - left;
							if span <= f64::EPSILON {
								return Message::NoOp;
							}
							((position - left) / span).clamp(0., 1.)
						}
						SliderInputUpdate::MoveMarker { position, .. } => *position,
						// A default that would cross a neighbor falls back to the midpoint between them
						SliderInputUpdate::ResetMarker { .. } if between || cyclic || (left..=right).contains(&default_position) => default_position,
						SliderInputUpdate::ResetMarker { .. } => (left + right) / 2.,
						_ => return Message::NoOp,
					};
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index,
						value: TaggedValue::F64(scale.value(scale_position)).into(),
					}
					.into()
				}
			})
			.on_commit(commit_value)
			.widget_instance()
	});
	let slider_owner = marker_input_indices.first().copied();

	// One row per parameter: first non-exposed carries the shared slider, others get just a number input
	for (i, param) in params.iter().enumerate() {
		let (exposed, current) = exposure_and_value[i];
		let input_index = param.parameter.input_index;
		let number_input = param.scale.number_input();

		if exposed {
			let row = number_widget(ParameterWidgetsInfo::at_index(node_id, input_index, true, context), number_input.clone());
			layout.push(LayoutGroup::row(row));
		} else {
			let mut row = start_widgets(&ParameterWidgetsInfo::at_index(node_id, input_index, true, context));
			row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());

			if Some(input_index) == slider_owner
				&& let Some(slider) = &slider_widget
			{
				row.push(slider.clone());
				row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			}

			row.push(
				number_input
					.clone()
					.value(Some(current))
					.min_width(60)
					.max_width(60)
					.on_update(update_value_at_index(move |widget: &NumberInput| TaggedValue::F64(widget.value.unwrap_or(0.)), node_id, input_index))
					.on_commit(commit_value)
					.widget_instance(),
			);
			layout.push(LayoutGroup::row(row));
		}
	}
}

pub(crate) fn hue_saturation_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::hue_saturation::*;

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in hue_saturation_properties: {err}");
			return Vec::new();
		}
	};
	let colorize_value = matches!(document_node.input_value(ColorizeInput), Some(TaggedValue::Bool(true)));
	let range_value = match document_node.input_value(RangeInput) {
		Some(TaggedValue::HueSaturationRange(range)) => *range,
		_ => HueSaturationRange::Master,
	};
	let slider_value = |parameter: &ParameterRef| match document_node.inputs.get(parameter.input_index).and_then(|input| input.as_value()) {
		Some(TaggedValue::F64(value)) => *value as f32,
		_ => 0.,
	};

	// The three sliders of the master, of the colorize mode, or of the selected range
	let (hue, saturation, lightness): (ParameterRef, ParameterRef, ParameterRef) = if colorize_value {
		(ColorizeHueInput.into(), ColorizeSaturationInput.into(), ColorizeLightnessInput.into())
	} else {
		match range_value {
			HueSaturationRange::Master => (HueInput.into(), SaturationInput.into(), LightnessInput.into()),
			HueSaturationRange::Reds => (RedsHueInput.into(), RedsSaturationInput.into(), RedsLightnessInput.into()),
			HueSaturationRange::Yellows => (YellowsHueInput.into(), YellowsSaturationInput.into(), YellowsLightnessInput.into()),
			HueSaturationRange::Greens => (GreensHueInput.into(), GreensSaturationInput.into(), GreensLightnessInput.into()),
			HueSaturationRange::Cyans => (CyansHueInput.into(), CyansSaturationInput.into(), CyansLightnessInput.into()),
			HueSaturationRange::Blues => (BluesHueInput.into(), BluesSaturationInput.into(), BluesLightnessInput.into()),
			HueSaturationRange::Magentas => (MagentasHueInput.into(), MagentasSaturationInput.into(), MagentasLightnessInput.into()),
		}
	};
	let range_values: Option<[ParameterRef; 4]> = match range_value {
		HueSaturationRange::Reds => Some([RedsFalloffStartInput.into(), RedsRangeStartInput.into(), RedsRangeEndInput.into(), RedsFalloffEndInput.into()]),
		HueSaturationRange::Yellows => Some([
			YellowsFalloffStartInput.into(),
			YellowsRangeStartInput.into(),
			YellowsRangeEndInput.into(),
			YellowsFalloffEndInput.into(),
		]),
		HueSaturationRange::Greens => Some([GreensFalloffStartInput.into(), GreensRangeStartInput.into(), GreensRangeEndInput.into(), GreensFalloffEndInput.into()]),
		HueSaturationRange::Cyans => Some([CyansFalloffStartInput.into(), CyansRangeStartInput.into(), CyansRangeEndInput.into(), CyansFalloffEndInput.into()]),
		HueSaturationRange::Blues => Some([BluesFalloffStartInput.into(), BluesRangeStartInput.into(), BluesRangeEndInput.into(), BluesFalloffEndInput.into()]),
		HueSaturationRange::Magentas => Some([
			MagentasFalloffStartInput.into(),
			MagentasRangeStartInput.into(),
			MagentasRangeEndInput.into(),
			MagentasFalloffEndInput.into(),
		]),
		HueSaturationRange::Master => None,
	};

	let range_defaults: Option<[f64; 4]> = match range_value {
		HueSaturationRange::Reds => Some([315., 345., 15., 45.]),
		HueSaturationRange::Yellows => Some([15., 45., 75., 105.]),
		HueSaturationRange::Greens => Some([75., 105., 135., 165.]),
		HueSaturationRange::Cyans => Some([135., 165., 195., 225.]),
		HueSaturationRange::Blues => Some([195., 225., 255., 285.]),
		HueSaturationRange::Magentas => Some([255., 285., 315., 345.]),
		HueSaturationRange::Master => None,
	};

	// Every saturation track fades from one middle gray. Colorize and a range head for the hue they act on. The master track favors none,
	// sweeping in OkLCh at the gray's lightness the long way from azure (220°) to magenta (330°), skipping the dull blue and purple, as chroma climbs to the gamut.
	use color::ColorSpace as _;
	let gray_lightness = 0.7;
	let oklch = |lightness: f32, chroma: f32, hue: f32| {
		let [r, g, b] = color::Oklch::to_linear_srgb([lightness, chroma, hue]);
		Color::from_rgbf32_unchecked(r.clamp(0., 1.), g.clamp(0., 1.), b.clamp(0., 1.))
	};
	// Fades from the gray to the pure hue at `turns` with the chroma rising evenly while the lightness eases to the hue's own
	let toward_hue = |turns: f32| {
		let pure = Color::from_hsva(turns.rem_euclid(1.), 1., 1., 1.);
		let [pure_lightness, pure_chroma, pure_hue] = color::Oklch::from_linear_srgb([pure.r(), pure.g(), pure.b()]);
		let stops = 24;
		let stop = |i: i32| {
			let t = i as f32 / stops as f32;
			oklch(gray_lightness + (pure_lightness - gray_lightness) * t, pure_chroma * t, pure_hue)
		};
		Gradient::from((0..=stops).map(stop).collect::<Vec<_>>())
	};
	let saturation_track = if colorize_value {
		toward_hue(slider_value(&hue) / 360.)
	} else if let Some([_, range_start, range_end, _]) = &range_values {
		let (start, end) = (slider_value(range_start), slider_value(range_end));
		let center = start + (end - start).rem_euclid(360.) / 2.;
		toward_hue(center / 360.)
	} else {
		let in_gamut = |lightness: f32, chroma: f32, hue: f32| color::Oklch::to_linear_srgb([lightness, chroma, hue]).iter().all(|channel| (0.0..=1.).contains(channel));
		let gamut_chroma = |lightness: f32, hue: f32| {
			let (mut inside, mut outside) = (0., 0.4);
			for _ in 0..16 {
				let chroma = (inside + outside) / 2.;
				if in_gamut(lightness, chroma, hue) {
					inside = chroma;
				} else {
					outside = chroma;
				}
			}
			inside
		};
		let stops = 80;
		let stop = |i: i32| {
			let t = i as f32 / stops as f32;
			// A triangle wave gives every hue the same width, where a cosine would linger at its turnarounds
			let bounce = (4. * t + 1.).rem_euclid(2.);
			let along = if bounce <= 1. { bounce } else { 2. - bounce };
			let hue = 330. + 250. * along;
			oklch(gray_lightness, gamut_chroma(gray_lightness, hue) * t, hue)
		};
		Gradient::from((0..=stops).map(stop).collect::<Vec<_>>())
	};
	let hue_track = Gradient::from(vec![Color::RED, Color::YELLOW, Color::GREEN, Color::CYAN, Color::BLUE, Color::MAGENTA, Color::RED]);
	let (hue_min, hue_max, hue_default) = if colorize_value { (0., 360., 24.) } else { (-180., 180., 0.) };
	let (saturation_min, saturation_default) = if colorize_value { (0., 25.) } else { (-100., 0.) };

	// Colorize replaces the ranges, so while it is on the selector stays but grayed out and the selected range's edges hide
	let mut range_info = ParameterWidgetsInfo::new(node_id, RangeInput, true, context);
	range_info.exposable = false;
	let mut layout = vec![enum_choice::<HueSaturationRange>().for_socket(range_info).disabled(colorize_value).property_row()];

	layout.extend([
		gradient_slider_row(
			node_id,
			context,
			hue,
			hue_track.clone(),
			Color::WHITE,
			hue_min,
			hue_max,
			hue_default,
			NumberInput::default().mode_increment().unit("°").min(hue_min).max(hue_max),
		),
		gradient_slider_row(
			node_id,
			context,
			saturation,
			saturation_track,
			Color::WHITE,
			saturation_min,
			100.,
			saturation_default,
			NumberInput::default().mode_increment().unit("%").min(saturation_min).max(100.),
		),
		gradient_slider_row(
			node_id,
			context,
			lightness,
			bw_track(),
			Color::WHITE,
			-100.,
			100.,
			0.,
			NumberInput::default().mode_increment().unit("%").min(-100.).max(100.),
		),
	]);

	// The selected range's edges share one rainbow as two split handles, a falloff half joined to a range half, with the range dashed between them
	if !colorize_value && let (Some(values), Some(defaults)) = (range_values, range_defaults) {
		let [falloff_start, range_start, range_end, falloff_end] = values;
		let params = [
			SliderSectionParam::new(falloff_start, Color::WHITE, defaults[0], MarkerScale::Degrees).pair_with_next(),
			SliderSectionParam::new(range_start, Color::WHITE, defaults[1], MarkerScale::Degrees).dash_to_next(),
			SliderSectionParam::new(range_end, Color::WHITE, defaults[2], MarkerScale::Degrees).pair_with_next(),
			SliderSectionParam::new(falloff_end, Color::WHITE, defaults[3], MarkerScale::Degrees),
		];
		build_shared_slider_section(node_id, context, &hue_track, &params, &mut layout);
	}

	let colorize = bool_widget(ParameterWidgetsInfo::new(node_id, ColorizeInput, true, context), CheckboxInput::default());
	layout.push(LayoutGroup::row(colorize));

	layout
}

/// A single-marker `SliderInput` over `track` driving the number at `input_index`: the marker sits at `position`, double-click
/// returns it to `default_position`, and each move sets the input to `value_at` the new position.
fn value_slider(
	node_id: NodeId,
	input_index: usize,
	track: GradientStops<SRGBA8>,
	handle_color: Color,
	position: f64,
	default_position: Option<f64>,
	value_at: impl Fn(f64) -> TaggedValue + 'static + Send + Sync,
) -> SliderInput {
	SliderInput::new(track)
		.track_space(GradientSpace::RgbGamma)
		.markers(vec![SliderMarker::new(position, 0.5, handle_color)])
		.show_midpoints(false)
		.allow_insert(false)
		.allow_delete(false)
		.allow_reorder(false)
		.on_update(move |update: &SliderInputUpdate| {
			let new_position = match update {
				SliderInputUpdate::MoveMarker { index: 0, position } => Some(*position),
				SliderInputUpdate::ResetMarker { index: 0 } => default_position,
				_ => None,
			};
			let Some(new_position) = new_position else { return Message::NoOp };

			NodeGraphMessage::SetInputValue {
				node_id,
				input_index,
				value: value_at(new_position).into(),
			}
			.into()
		})
		.on_commit(commit_value)
}

/// A row with a range slider and a 60px number input for the number at `parameter_widgets_info`. The slider's 0..1 position maps
/// to the number through `position_of` and `value_at`, and double-click restores `default`.
fn slider_row(
	parameter_widgets_info: ParameterWidgetsInfo,
	number_props: NumberInput,
	default: Option<f64>,
	position_of: impl Fn(f64) -> f64,
	value_at: impl Fn(f64) -> f64 + 'static + Send + Sync,
) -> Vec<WidgetInstance> {
	let mut widgets = start_widgets(&parameter_widgets_info);

	let Some(input) = parameter_widgets_info.input() else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	// An exposed input shows only its label and source
	let (current, tagged_value): (f64, fn(f64) -> TaggedValue) = match input.as_non_exposed_value() {
		Some(&TaggedValue::F64(value)) => (value, TaggedValue::F64),
		_ => return widgets,
	};
	let ParameterWidgetsInfo { node_id, index, .. } = parameter_widgets_info;

	widgets.extend_from_slice(&[
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		value_slider(
			node_id,
			index,
			GradientStops::default(),
			Color::WHITE,
			position_of(current),
			default.map(position_of),
			move |position| tagged_value(value_at(position)),
		)
		.range_slider(true)
		.disabled(number_props.disabled)
		.widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		number_props
			.value(Some(current))
			.min_width(60)
			.max_width(60)
			.on_update(update_value_at_index(move |x: &NumberInput| tagged_value(x.value.unwrap_or_default()), node_id, index))
			.on_commit(commit_value)
			.widget_instance(),
	]);

	widgets
}

/// A slider row running linearly across `slider`'s bounds.
pub(crate) fn range_slider_widget(parameter_widgets_info: ParameterWidgetsInfo, number_props: NumberInput, slider: SliderRange) -> Vec<WidgetInstance> {
	slider_row(
		parameter_widgets_info,
		number_props,
		slider.default,
		move |value| slider.position(value),
		move |position| slider.value(position),
	)
}

/// Build a row with a single-marker `SliderInput` over `track` and a 60px `NumberInput`. The marker maps `value_min..value_max` to position 0..1, and double-click resets to `default_value`.
fn gradient_slider_row(
	node_id: NodeId,
	context: &mut NodePropertiesContext,
	parameter: impl Into<ParameterRef>,
	track: Gradient,
	handle_color: Color,
	value_min: f64,
	value_max: f64,
	default_value: f64,
	number_input: NumberInput,
) -> LayoutGroup {
	let input_index = parameter.into().input_index;
	let mut row = start_widgets(&ParameterWidgetsInfo::at_index(node_id, input_index, true, context));

	let current = get_document_node(node_id, context)
		.ok()
		.and_then(|document_node| document_node.inputs.get(input_index))
		.and_then(|input| input.as_non_exposed_value())
		.and_then(|tagged| if let TaggedValue::F64(value) = tagged { Some(*value) } else { None });

	// Only add the slider and number widgets when the input is not exposed
	if let Some(current) = current {
		let slider = SliderRange {
			min: value_min,
			max: value_max,
			default: Some(default_value),
		};
		let value_at = move |position| TaggedValue::F64(slider.value(position));

		row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		row.push(
			value_slider(
				node_id,
				input_index,
				GradientStops::from(&track),
				handle_color,
				slider.position(current),
				Some(slider.position(default_value)),
				value_at,
			)
			.narrow(true)
			.widget_instance(),
		);
		row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
		row.push(
			number_input
				.value(Some(current))
				.min_width(60)
				.max_width(60)
				.display_decimal_places(0)
				.on_update(update_value_at_index(move |widget: &NumberInput| TaggedValue::F64(widget.value.unwrap_or(0.)), node_id, input_index))
				.on_commit(commit_value)
				.widget_instance(),
		);
	}

	LayoutGroup::row(row)
}

pub(crate) fn threshold_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::threshold::*;

	let params = [
		SliderSectionParam::new(MinLuminanceInput, Color::WHITE, 50., MarkerScale::Percent).dash_to_next(),
		SliderSectionParam::new(MaxLuminanceInput, Color::WHITE, 100., MarkerScale::Percent),
	];

	let mut layout = Vec::with_capacity(2);
	build_shared_slider_section(node_id, context, &bw_track(), &params, &mut layout);

	layout
}

pub(crate) fn vibrance_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::vibrance::*;

	let number_input = NumberInput::default().mode_increment().unit("%").min(-100.).max(100.);
	let slider = SliderRange {
		min: -100.,
		max: 100.,
		default: Some(0.),
	};
	let vibrance = range_slider_widget(ParameterWidgetsInfo::new(node_id, VibranceInput, true, context), number_input.clone(), slider);
	let saturation = range_slider_widget(ParameterWidgetsInfo::new(node_id, SaturationInput, true, context), number_input, slider);

	vec![LayoutGroup::row(vibrance), LayoutGroup::row(saturation)]
}

pub(crate) fn color_balance_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::color_balance::*;

	let mut tone_info = ParameterWidgetsInfo::new(node_id, ToneInput, true, context);
	tone_info.exposable = false;
	let tone = enum_choice::<TonalRange>().for_socket(tone_info).property_row();
	let preserve_luminosity = bool_widget(ParameterWidgetsInfo::new(node_id, PreserveLuminosityInput, true, context), CheckboxInput::default());

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in color_balance_properties: {err}");
			return Vec::new();
		}
	};
	let tone_choice = match document_node.input_value(ToneInput) {
		Some(TaggedValue::TonalRange(choice)) => *choice,
		_ => {
			warn!("Color Balance node properties panel could not be displayed.");
			return vec![];
		}
	};

	// Only the selected tone's three sliders are shown
	let parameters: [ParameterRef; 3] = match tone_choice {
		TonalRange::Shadows => [ShadowsCyanRedInput.into(), ShadowsMagentaGreenInput.into(), ShadowsYellowBlueInput.into()],
		TonalRange::Midtones => [MidtonesCyanRedInput.into(), MidtonesMagentaGreenInput.into(), MidtonesYellowBlueInput.into()],
		TonalRange::Highlights => [HighlightsCyanRedInput.into(), HighlightsMagentaGreenInput.into(), HighlightsYellowBlueInput.into()],
	};
	let tracks = [
		Gradient::from(vec![Color::CYAN, Color::RED]),
		Gradient::from(vec![Color::MAGENTA, Color::GREEN]),
		Gradient::from(vec![Color::YELLOW, Color::BLUE]),
	];
	let number_input = NumberInput::default().mode_increment().unit("%").min(-100.).max(100.);

	let mut layout = vec![tone];
	for (parameter, track) in parameters.into_iter().zip(tracks) {
		layout.push(gradient_slider_row(node_id, context, parameter, track, Color::WHITE, -100., 100., 0., number_input.clone()));
	}
	layout.push(LayoutGroup::row(preserve_luminosity));

	layout
}

pub(crate) fn black_and_white_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::black_and_white::*;

	let number_input = NumberInput::default().mode_increment().unit("%").min(-200.).max(300.);

	let use_tint: ParameterRef = UseTintInput.into();
	let tint = optional_color_widget(ParameterWidgetsInfo::new(node_id, TintInput, false, context), use_tint.input_index, ColorInput::default());

	let mut layout = vec![tint];
	let params: &[(ParameterRef, Color, f64)] = &[
		(RedsInput.into(), Color::RED, 40.),
		(YellowsInput.into(), Color::YELLOW, 60.),
		(GreensInput.into(), Color::GREEN, 40.),
		(CyansInput.into(), Color::CYAN, 60.),
		(BluesInput.into(), Color::BLUE, 20.),
		(MagentasInput.into(), Color::MAGENTA, 80.),
	];
	for (parameter, color, default) in params {
		layout.push(gradient_slider_row(
			node_id,
			context,
			parameter.clone(),
			color_track(*color),
			Color::WHITE,
			-200.,
			300.,
			*default,
			number_input.clone(),
		));
	}

	layout
}

pub(crate) fn channel_mixer_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::raster::channel_mixer::*;

	let is_monochrome = bool_widget(ParameterWidgetsInfo::new(node_id, MonochromeInput, true, context), CheckboxInput::default());
	let mut parameter_info = ParameterWidgetsInfo::new(node_id, OutputChannelInput, true, context);
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
	let is_monochrome_value = match document_node.input_value(MonochromeInput) {
		Some(TaggedValue::Bool(monochrome_choice)) => *monochrome_choice,
		_ => false,
	};
	// Output channel choice
	let output_channel_value = match &document_node.input_value(OutputChannelInput) {
		Some(TaggedValue::RedGreenBlue(choice)) => choice,
		_ => {
			warn!("Channel Mixer node properties panel could not be displayed.");
			return vec![];
		}
	};

	// The edited parameters and their defaults depend on the monochrome toggle and output channel selection
	let (parameters, defaults): ([ParameterRef; 4], [f64; 4]) = match (is_monochrome_value, output_channel_value) {
		(true, _) => (
			[MonochromeRInput.into(), MonochromeGInput.into(), MonochromeBInput.into(), MonochromeCInput.into()],
			[40., 40., 20., 0.],
		),
		(false, RedGreenBlue::Red) => ([RedRInput.into(), RedGInput.into(), RedBInput.into(), RedCInput.into()], [100., 0., 0., 0.]),
		(false, RedGreenBlue::Green) => ([GreenRInput.into(), GreenGInput.into(), GreenBInput.into(), GreenCInput.into()], [0., 100., 0., 0.]),
		(false, RedGreenBlue::Blue) => ([BlueRInput.into(), BlueGInput.into(), BlueBInput.into(), BlueCInput.into()], [0., 0., 100., 0.]),
	};

	let number_input = NumberInput::default().mode_increment().unit("%").min(-200.).max(200.);
	let tracks = [color_track(Color::RED), color_track(Color::GREEN), color_track(Color::BLUE), bw_track()];

	let mut layout = vec![LayoutGroup::row(is_monochrome)];
	if !is_monochrome_value {
		layout.push(output_channel);
	}
	for (i, (parameter, &default)) in parameters.into_iter().zip(defaults.iter()).enumerate() {
		layout.push(gradient_slider_row(
			node_id,
			context,
			parameter,
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

	let mut default_info = ParameterWidgetsInfo::new(node_id, ColorsInput, true, context);
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
	let colors_choice = match &document_node.input_value(ColorsInput) {
		Some(TaggedValue::SelectiveColorChoice(choice)) => choice,
		_ => {
			warn!("Selective Color node properties panel could not be displayed.");
			return vec![];
		}
	};
	// CMYK
	let parameters: [ParameterRef; 4] = match colors_choice {
		SelectiveColorChoice::Reds => [RCInput.into(), RMInput.into(), RYInput.into(), RKInput.into()],
		SelectiveColorChoice::Yellows => [YCInput.into(), YMInput.into(), YYInput.into(), YKInput.into()],
		SelectiveColorChoice::Greens => [GCInput.into(), GMInput.into(), GYInput.into(), GKInput.into()],
		SelectiveColorChoice::Cyans => [CCInput.into(), CMInput.into(), CYInput.into(), CKInput.into()],
		SelectiveColorChoice::Blues => [BCInput.into(), BMInput.into(), BYInput.into(), BKInput.into()],
		SelectiveColorChoice::Magentas => [MCInput.into(), MMInput.into(), MYInput.into(), MKInput.into()],
		SelectiveColorChoice::Whites => [WCInput.into(), WMInput.into(), WYInput.into(), WKInput.into()],
		SelectiveColorChoice::Neutrals => [NCInput.into(), NMInput.into(), NYInput.into(), NKInput.into()],
		SelectiveColorChoice::Blacks => [KCInput.into(), KMInput.into(), KYInput.into(), KKInput.into()],
	};

	let tracks = [color_track(Color::CYAN), color_track(Color::MAGENTA), color_track(Color::YELLOW), bw_track()];
	let number_input = NumberInput::default().mode_increment().unit("%").min(-100.).max(100.);

	// Mode
	let mode = enum_choice::<RelativeAbsolute>()
		.for_socket(ParameterWidgetsInfo::new(node_id, ModeInput, true, context))
		.property_row();

	let mut layout = vec![colors];
	for (i, parameter) in parameters.into_iter().enumerate() {
		layout.push(gradient_slider_row(node_id, context, parameter, tracks[i].clone(), Color::WHITE, -100., 100., 0., number_input.clone()));
	}
	layout.push(mode);

	layout
}

pub(crate) fn grid_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::generator_nodes::grid::*;

	let grid_type = enum_choice::<GridType>().for_socket(ParameterWidgetsInfo::new(node_id, GridTypeInput, true, context)).property_row();

	let mut widgets = vec![grid_type];

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in grid_properties: {err}");
			return Vec::new();
		}
	};
	let Some(grid_type_input) = document_node.input(GridTypeInput) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::GridType(grid_type)) = grid_type_input.as_non_exposed_value() {
		match grid_type {
			GridType::Rectangular => {
				let spacing = vec2_widget(ParameterWidgetsInfo::new(node_id, SpacingInput, true, context), "W", "H", " px", Some(0.), false);
				widgets.push(spacing);
			}
			GridType::Isometric => {
				let spacing = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, SpacingInput, true, context),
					NumberInput::default().label("H").min(0.).unit(" px"),
				));
				let angles = vec2_widget(ParameterWidgetsInfo::new(node_id, AnglesInput, true, context), "", "", "°", None, false);
				widgets.extend([spacing, angles]);
			}
		}
	}

	let columns = number_widget(ParameterWidgetsInfo::new(node_id, ColumnsInput, true, context), NumberInput::default().min(1.));
	let rows = number_widget(ParameterWidgetsInfo::new(node_id, RowsInput, true, context), NumberInput::default().min(1.));

	let connect_cells = bool_widget(ParameterWidgetsInfo::new(node_id, ConnectCellsInput, true, context), CheckboxInput::default());

	widgets.extend([LayoutGroup::row(columns), LayoutGroup::row(rows), LayoutGroup::row(connect_cells)]);

	widgets
}

pub(crate) fn spiral_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::generator_nodes::spiral::*;

	let spiral_type = enum_choice::<SpiralType>()
		.for_socket(ParameterWidgetsInfo::new(node_id, SpiralTypeInput, true, context))
		.property_row();
	let turns = number_widget(ParameterWidgetsInfo::new(node_id, TurnsInput, true, context), NumberInput::default().min(0.1));
	let start_angle = number_widget(ParameterWidgetsInfo::new(node_id, StartAngleInput, true, context), NumberInput::default().unit("°"));

	let mut widgets = vec![spiral_type, LayoutGroup::row(turns), LayoutGroup::row(start_angle)];

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in exposure_properties: {err}");
			return Vec::new();
		}
	};

	let Some(spiral_type_input) = document_node.input(SpiralTypeInput) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::SpiralType(spiral_type)) = spiral_type_input.as_non_exposed_value() {
		match spiral_type {
			SpiralType::Archimedean => {
				let inner_radius = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, InnerRadiusInput, true, context),
					NumberInput::default().min(0.).unit(" px"),
				));

				let outer_radius = LayoutGroup::row(number_widget(ParameterWidgetsInfo::new(node_id, OuterRadiusInput, true, context), NumberInput::default().unit(" px")));

				widgets.extend([inner_radius, outer_radius]);
			}
			SpiralType::Logarithmic => {
				let inner_radius = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, InnerRadiusInput, true, context),
					NumberInput::default().min(0.).unit(" px"),
				));

				let outer_radius = LayoutGroup::row(number_widget(
					ParameterWidgetsInfo::new(node_id, OuterRadiusInput, true, context),
					NumberInput::default().min(0.1).unit(" px"),
				));

				widgets.extend([inner_radius, outer_radius]);
			}
		}
	}

	let angular_resolution = number_widget(
		ParameterWidgetsInfo::new(node_id, AngularResolutionInput, true, context),
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

	let current_spacing = document_node.input(SpacingInput).and_then(|input| input.as_value()).cloned();
	let is_quantity = matches!(current_spacing, Some(TaggedValue::PointSpacingType(PointSpacingType::Quantity)));

	let spacing = enum_choice::<PointSpacingType>()
		.for_socket(ParameterWidgetsInfo::new(node_id, SpacingInput, true, context))
		.property_row();
	let separation = number_widget(ParameterWidgetsInfo::new(node_id, SeparationInput, true, context), NumberInput::default().min(0.).unit(" px"));
	let quantity = number_widget(ParameterWidgetsInfo::new(node_id, QuantityInput, true, context), NumberInput::default().min(2.).int());
	let start_offset = number_widget(ParameterWidgetsInfo::new(node_id, StartOffsetInput, true, context), NumberInput::default().min(0.).unit(" px"));
	let stop_offset = number_widget(ParameterWidgetsInfo::new(node_id, StopOffsetInput, true, context), NumberInput::default().min(0.).unit(" px"));
	let adaptive_spacing = bool_widget(ParameterWidgetsInfo::new(node_id, AdaptiveSpacingInput, true, context), CheckboxInput::default().disabled(is_quantity));

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

	let exposure = range_slider_widget(
		ParameterWidgetsInfo::new(node_id, ExposureInput, true, context),
		NumberInput::default().min(-20.).max(20.),
		SliderRange {
			min: -20.,
			max: 20.,
			default: Some(0.),
		},
	);
	let offset = range_slider_widget(
		ParameterWidgetsInfo::new(node_id, OffsetInput, true, context),
		NumberInput::default().min(-0.5).max(0.5),
		SliderRange {
			min: -0.5,
			max: 0.5,
			default: Some(0.),
		},
	);

	let gamma_correction = slider_row(
		ParameterWidgetsInfo::new(node_id, GammaCorrectionInput, true, context),
		MarkerScale::Gamma.number_input().increment_step(0.1),
		Some(1.),
		|gamma| MarkerScale::Gamma.position(gamma),
		|position| MarkerScale::Gamma.value(position),
	);

	vec![LayoutGroup::row(exposure), LayoutGroup::row(offset), LayoutGroup::row(gamma_correction)]
}

pub(crate) fn format_number_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::text_nodes::format_number::{DecimalPlacesInput, DecimalSeparatorInput, FixedDecimalsInput, StartAt10000Input, ThousandsSeparatorInput, UseThousandsSeparatorInput};

	// Read current values before borrowing context mutably for widgets
	let (no_decimals, decimal_sep_value, use_thousands, thousands_sep_value) = match get_document_node(node_id, context) {
		Ok(document_node) => {
			let decimal_places = match document_node.input(DecimalPlacesInput).and_then(|input| input.as_value()) {
				Some(&TaggedValue::I64(x)) => x,
				_ => 2,
			};
			let decimal_sep = match document_node.input(DecimalSeparatorInput).and_then(|input| input.as_non_exposed_value()) {
				Some(TaggedValue::String(x)) => Some(x.clone()),
				_ => None,
			};
			let use_thousands = match document_node.input(UseThousandsSeparatorInput).and_then(|input| input.as_value()) {
				Some(&TaggedValue::Bool(x)) => x,
				_ => false,
			};
			let use_thousands = use_thousands || document_node.input(ThousandsSeparatorInput).is_some_and(|input| input.is_exposed());
			let thousands_sep = match document_node.input(ThousandsSeparatorInput).and_then(|input| input.as_non_exposed_value()) {
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

	let decimal_places = number_widget(ParameterWidgetsInfo::new(node_id, DecimalPlacesInput, true, context), NumberInput::default().min(0.).int());

	// Fixed decimals and decimal separator are disabled when decimal places is 0
	let fixed_decimals = bool_widget(ParameterWidgetsInfo::new(node_id, FixedDecimalsInput, true, context), CheckboxInput::default().disabled(no_decimals));
	let mut decimal_sep_widgets = start_widgets(&ParameterWidgetsInfo::new(node_id, DecimalSeparatorInput, true, context));
	if let Some(sep) = decimal_sep_value {
		decimal_sep_widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(sep)
				.disabled(no_decimals)
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, DecimalSeparatorInput))
				.on_commit(commit_value)
				.widget_instance(),
		]);
	}

	// Thousands separator: checkbox in assist area
	let mut thousands_sep_widgets = start_widgets(&ParameterWidgetsInfo::new(node_id, ThousandsSeparatorInput, false, context));
	if let Some(sep) = thousands_sep_value {
		thousands_sep_widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(use_thousands)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, UseThousandsSeparatorInput))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(sep)
				.disabled(!use_thousands)
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, ThousandsSeparatorInput))
				.on_commit(commit_value)
				.widget_instance(),
		]);
	}

	// Start at 10,000: disabled when thousands separator is off
	let start_at_10000 = bool_widget(ParameterWidgetsInfo::new(node_id, StartAt10000Input, true, context), CheckboxInput::default().disabled(!use_thousands));

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
			let capitalization_input = document_node.input(CapitalizationInput);
			let capitalization_exposed = capitalization_input.is_some_and(|input| input.is_exposed());
			// When exposed, the capitalization mode may change dynamically, so we can't assume it's a simple (joiner-inapplicable) mode
			let is_simple = !capitalization_exposed
				&& matches!(
					capitalization_input.and_then(|input| input.as_value()),
					Some(TaggedValue::StringCapitalization(StringCapitalization::LowerCase | StringCapitalization::UpperCase))
				);
			let use_joiner = match document_node.input(UseJoinerInput).and_then(|input| input.as_value()) {
				Some(&TaggedValue::Bool(x)) => x,
				_ => true,
			};
			let joiner = match document_node.input(JoinerInput).and_then(|input| input.as_non_exposed_value()) {
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
		.for_socket(ParameterWidgetsInfo::new(node_id, CapitalizationInput, true, context))
		.property_row();

	// Joiner row: the UseJoiner checkbox is drawn in the assist area, followed by the Joiner text input
	let mut joiner_widgets = start_widgets(&ParameterWidgetsInfo::new(node_id, JoinerInput, false, context));
	if let Some(joiner) = joiner_value {
		let joiner_is_empty = joiner.is_empty();
		joiner_widgets.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			CheckboxInput::new(use_joiner_enabled)
				.disabled(is_simple_case)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, UseJoinerInput))
				.on_commit(commit_value)
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(joiner)
				.placeholder(if joiner_is_empty { "Empty" } else { "" })
				.disabled(joiner_disabled)
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, JoinerInput))
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
							value: TaggedValue::Bool(true).into(),
						}
						.into(),
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: JoinerInput::INDEX,
							value: TaggedValue::String(value.clone()).into(),
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
	let mut corner_radius_row_1 = start_widgets(&ParameterWidgetsInfo::new(node_id, CornerRadiusInput, true, context));
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
	let Some(input) = document_node.input(IndividualCornerRadiiInput) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(is_individual)) = input.as_non_exposed_value() {
		// Values
		let Some(input) = document_node.input(CornerRadiusInput) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		let corner_values = match input.as_non_exposed_value() {
			Some(TaggedValue::BoxCorners(values)) => BoxCorners::from(values.clone()).to_corner_values(),
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
						value: TaggedValue::Bool(false).into(),
					}
					.into(),
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: CornerRadiusInput::INDEX,
						value: TaggedValue::BoxCorners(vec![uniform_val]).into(),
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
						value: TaggedValue::Bool(true).into(),
					}
					.into(),
					NodeGraphMessage::SetInputValue {
						node_id,
						input_index: CornerRadiusInput::INDEX,
						value: TaggedValue::BoxCorners(corner_values.to_vec()).into(),
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
					move |x: &TextInput| Some(TaggedValue::BoxCorners(graphene_std::core_types::misc::parse_f64_list(&x.value))),
					node_id,
					CornerRadiusInput,
				))
				.widget_instance()
		} else {
			NumberInput::default()
				.value(Some(uniform_val))
				.unit(" px")
				.on_update(update_value(move |x: &NumberInput| TaggedValue::BoxCorners(vec![x.value.unwrap()]), node_id, CornerRadiusInput))
				.on_commit(commit_value)
				.widget_instance()
		};
		corner_radius_row_2.push(input_widget);
	}

	// Size X
	let size_x = number_widget(ParameterWidgetsInfo::new(node_id, WidthInput, true, context), NumberInput::default());

	// Size Y
	let size_y = number_widget(ParameterWidgetsInfo::new(node_id, HeightInput, true, context), NumberInput::default());

	// Clamped
	let clamped = bool_widget(ParameterWidgetsInfo::new(node_id, ClampedInput, true, context), CheckboxInput::default());

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
				.input_from_connector(&InputConnector::node_at_index(node_id, input_index), context.selection_network_path)
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
						.input_type(&InputConnector::node_at_index(node_id, input_index), context.selection_network_path)
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
	let transform = graph_modification_utils::gradient_to_viewport_transform(layer, context.network_interface);
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
			gradient_form: GradientForm,
			settings: GradientSettings,
			transform: DAffine2,
			/// Whether the transform input holds a plain value (so the "Reverse Direction" button may write to it) rather than a wire.
			transform_is_value: bool,
		},
		Other,
	}

	// Pass blank_assist=false because the assist slot is filled below ("Reverse Stops" button when in gradient mode)
	let mut widgets_first_row = start_widgets(&ParameterWidgetsInfo::new(node_id, PaintInput, false, context));

	if get_document_node(node_id, context).is_ok_and(|node| node.input(PaintInput).is_some_and(|input| input.is_exposed())) {
		return vec![LayoutGroup::row(widgets_first_row)];
	}

	// A Fill node not attached to a layer (or living in a nested network) still shows its full fill UI; only the gradient's
	// bounding-box default transform needs the layer, and it falls back to a unit box when there isn't one.
	let layer = root_layer_for_chain_node(node_id, context);

	let fill = match get_document_node(node_id, context) {
		Ok(document_node) => match document_node.input_value(PaintInput) {
			Some(TaggedValue::Color(color)) => ResolvedFill::Solid(Some(*color)),
			Some(value) if value.is_no_paint() => ResolvedFill::Solid(None),
			Some(TaggedValue::GradientRamp(_)) => {
				match graph_modification_utils::read_fill_node_gradient(document_node, || {
					layer.map_or([DVec2::ZERO, DVec2::ONE], |layer| context.network_interface.document_metadata().nonzero_bounding_box(layer))
				}) {
					Some(gradient) => ResolvedFill::Gradient {
						gradient: gradient.stops,
						gradient_form: gradient.gradient_form,
						settings: gradient.settings,
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
			let backup_color = match document_node.input_value(BackupColorInput) {
				Some(&TaggedValue::Color(color)) => Some(color),
				_ => None,
			};
			let backup_stops = match document_node.input_value(BackupGradientInput) {
				Some(TaggedValue::GradientRamp(ramp)) => ramp.clone(),
				_ => GradientRamp::black_to_white(),
			};
			(backup_color, backup_stops)
		}
		Err(_) => (None, GradientRamp::black_to_white()),
	};

	match &fill {
		ResolvedFill::Gradient { gradient: stops, settings, .. } => {
			let stops = stops.clone();
			let settings = *settings;

			let reverse_button = IconButton::new("Reverse", 24)
				.tooltip_label("Reverse Stops")
				.tooltip_description("Reverse the gradient color stops.")
				.on_update(update_value(
					move |_| TaggedValue::GradientRamp(GradientRamp::from(stops.reversed(settings.cyclic)).with_settings(settings)),
					node_id,
					PaintInput,
				))
				.widget_instance();
			widgets_first_row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			widgets_first_row.push(reverse_button);
		}
		_ => add_blank_assist(&mut widgets_first_row),
	}

	let widget_value = match &fill {
		ResolvedFill::Solid(color) => {
			if let Some(color) = color {
				FillChoice::<SRGBA8>::Solid(SRGBA8::from(*color))
			} else {
				FillChoice::<SRGBA8>::None
			}
		}
		ResolvedFill::Gradient { gradient: stops, settings, .. } => FillChoice::<SRGBA8>::Gradient(GradientRamp::from(stops).with_settings(*settings)),
		ResolvedFill::Other => FillChoice::<SRGBA8>::None,
	};

	let solid_set_messages = move |color: Option<Color>| {
		let mut messages = vec![
			NodeGraphMessage::SetInputValue {
				node_id,
				input_index: PaintInput::INDEX,
				value: color.map_or_else(TaggedValue::no_paint, TaggedValue::Color).into(),
			}
			.into(),
		];
		if let Some(color) = color {
			messages.push(
				NodeGraphMessage::SetInputValue {
					node_id,
					input_index: BackupColorInput::INDEX,
					value: TaggedValue::Color(color).into(),
				}
				.into(),
			);
		}
		Message::Batched { messages: messages.into() }
	};

	let gradient_set_messages = move |ramp: GradientRamp| Message::Batched {
		messages: Box::new([
			NodeGraphMessage::SetInputValue {
				node_id,
				input_index: PaintInput::INDEX,
				value: TaggedValue::GradientRamp(ramp.clone()).into(),
			}
			.into(),
			NodeGraphMessage::SetInputValue {
				node_id,
				input_index: BackupGradientInput::INDEX,
				value: TaggedValue::GradientRamp(ramp).into(),
			}
			.into(),
		]),
	};

	widgets_first_row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
	widgets_first_row.push(
		ColorInput::default()
			.value(widget_value)
			.on_update(move |x: &ColorInput| match &x.value {
				FillChoice::<SRGBA8>::None => solid_set_messages(None),
				FillChoice::<SRGBA8>::Solid(srgba8) => {
					let color = Some(Color::from(*srgba8));
					solid_set_messages(color)
				}
				FillChoice::<SRGBA8>::Gradient(ramp) => gradient_set_messages(GradientRamp::from(ramp)),
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
				.on_update(update_value(move |_| backup_color.map_or_else(TaggedValue::no_paint, TaggedValue::Color), node_id, PaintInput))
				.on_commit(commit_value),
			RadioEntryData::new("gradient")
				.label("Gradient")
				.on_update(update_value(move |_| TaggedValue::GradientRamp(backup_gradient.clone()), node_id, PaintInput))
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
		gradient_form,
		transform,
		transform_is_value,
		..
	} = fill.clone()
	{
		// "Reverse Direction" button (assist) beside the Linear/Radial radio. Icon orientation is resolved in viewport
		// space so canvas tilt and layer transforms behave the same as in the Gradient tool's control bar.
		let mut row = vec![TextLabel::new("").widget_instance()];

		// The button writes a value into the transform input, so only offer it when the input isn't wired
		if transform_is_value {
			let start = transform.transform_point2(DVec2::ZERO);
			let end = transform.transform_point2(DVec2::X);
			let new_transform = build_transform_with_y_preservation(transform, end, start);
			let orientation_rightward = gradient_orientation_in_fill_node(node_id, transform, context).unwrap_or(true);

			let reverse_direction_button = IconButton::new(if orientation_rightward { "ReverseRadialGradientToRight" } else { "ReverseRadialGradientToLeft" }, 24)
				.tooltip_label("Reverse Direction")
				.tooltip_description(graph_modification_utils::reverse_direction_tooltip_description(gradient_form))
				.on_update(move |_| Message::Batched {
					messages: Box::new([
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: HasTransformInput::INDEX,
							value: TaggedValue::Bool(true).into(),
						}
						.into(),
						NodeGraphMessage::SetInputValue {
							node_id,
							input_index: TransformInput::INDEX,
							value: TaggedValue::DAffine2(new_transform).into(),
						}
						.into(),
					]),
				})
				.widget_instance();
			row.push(Separator::new(SeparatorStyle::Unrelated).widget_instance());
			row.push(reverse_direction_button);
		} else {
			add_blank_assist(&mut row);
		}

		let entries = [GradientForm::Linear, GradientForm::Radial]
			.iter()
			.map(|&gradient_form| {
				RadioEntryData::new(format!("{:?}", gradient_form))
					.label(gradient_form.to_string())
					.on_update(update_value(move |_| TaggedValue::GradientForm(gradient_form), node_id, GradientFormInput))
					.on_commit(commit_value)
			})
			.collect();

		row.extend_from_slice(&[
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			RadioInput::new(entries).selected_index(Some(gradient_form as u32)).widget_instance(),
		]);

		widgets.push(LayoutGroup::row(row));
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
	let join_value = match &document_node.input_value(JoinInput) {
		Some(TaggedValue::StrokeJoin(x)) => x,
		_ => &StrokeJoin::Miter,
	};

	let has_dash_lengths = match &document_node.input_value(DashPatternInput) {
		Some(TaggedValue::DashPattern(lengths)) => lengths.is_empty(),
		_ => true,
	};
	let miter_limit_disabled = join_value != &StrokeJoin::Miter;

	let color = color_widget(
		ParameterWidgetsInfo::new(node_id, PaintInput, true, context),
		crate::messages::layout::utility_types::widgets::button_widgets::ColorInput::default(),
	);
	let weight = number_widget(ParameterWidgetsInfo::new(node_id, WeightInput, true, context), NumberInput::default().unit(" px").min(0.));
	let align = enum_choice::<StrokeAlign>().for_socket(ParameterWidgetsInfo::new(node_id, AlignInput, true, context)).property_row();
	let cap = enum_choice::<StrokeCap>().for_socket(ParameterWidgetsInfo::new(node_id, CapInput, true, context)).property_row();
	let join = enum_choice::<StrokeJoin>().for_socket(ParameterWidgetsInfo::new(node_id, JoinInput, true, context)).property_row();

	let miter_limit = number_widget(
		ParameterWidgetsInfo::new(node_id, MiterLimitInput, true, context),
		NumberInput::default().min(0.).disabled(miter_limit_disabled),
	);
	let disabled_number_input = NumberInput::default().unit(" px").disabled(has_dash_lengths);
	let dash_lengths = dash_pattern_widget(ParameterWidgetsInfo::new(node_id, DashPatternInput, true, context), TextInput::default().centered(true));
	let number_input = disabled_number_input;
	let dash_offset = number_widget(ParameterWidgetsInfo::new(node_id, DashOffsetInput, true, context), number_input);

	vec![
		color,
		LayoutGroup::row(weight),
		align,
		cap,
		join,
		LayoutGroup::row(miter_limit),
		LayoutGroup::row(dash_lengths),
		LayoutGroup::row(dash_offset),
	]
}

pub fn offset_path_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	use graphene_std::vector::offset_path::*;

	let number_input = NumberInput::default().unit(" px");
	let distance = number_widget(ParameterWidgetsInfo::new(node_id, DistanceInput, true, context), number_input);

	let join = enum_choice::<StrokeJoin>().for_socket(ParameterWidgetsInfo::new(node_id, JoinInput, true, context)).property_row();

	let document_node = match get_document_node(node_id, context) {
		Ok(document_node) => document_node,
		Err(err) => {
			log::error!("Could not get document node in offset_path_properties: {err}");
			return Vec::new();
		}
	};
	let number_input = NumberInput::default().min(0.).disabled({
		let join_value = match &document_node.input_value(JoinInput) {
			Some(TaggedValue::StrokeJoin(x)) => x,
			_ => &StrokeJoin::Miter,
		};
		join_value != &StrokeJoin::Miter
	});
	let miter_limit = number_widget(ParameterWidgetsInfo::new(node_id, MiterLimitInput, true, context), number_input);

	vec![LayoutGroup::row(distance), join, LayoutGroup::row(miter_limit)]
}

pub struct ParameterWidgetsInfo<'a> {
	document_id: DocumentId,
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
	/// Reference the parameter by its symbol, e.g. `ParameterWidgetsInfo::new(node_id, brightness_contrast::BrightnessInput, true, context)`, or by an erased [`ParameterRef`] chosen at runtime.
	pub fn new(node_id: NodeId, parameter: impl Into<ParameterRef>, blank_assist: bool, context: &'a mut NodePropertiesContext) -> ParameterWidgetsInfo<'a> {
		Self::at_index(node_id, parameter.into().input_index, blank_assist, context)
	}

	/// The input slot this parameter row edits.
	pub fn input(&self) -> Option<&'a NodeInput> {
		self.document_node?.inputs.get(self.index)
	}

	/// A widget callback that writes the callback-produced value to this parameter.
	pub fn update_value<T>(&self, value: impl Fn(&T) -> TaggedValue + 'static + Send + Sync) -> impl Fn(&T) -> Message + 'static + Send + Sync {
		update_value_at_index(value, self.node_id, self.index)
	}

	/// Like [`Self::update_value`], for callbacks that sometimes produce no value.
	pub fn optionally_update_value<T>(&self, value: impl Fn(&T) -> Option<TaggedValue> + 'static + Send + Sync) -> impl Fn(&T) -> Message + 'static + Send + Sync {
		optionally_update_value_at_index(value, self.node_id, self.index)
	}

	/// Reference the parameter by a runtime input index, for callers that receive the index dynamically (e.g. widget overrides).
	pub fn at_index(node_id: NodeId, index: usize, blank_assist: bool, context: &'a mut NodePropertiesContext) -> ParameterWidgetsInfo<'a> {
		let (name, description) = context.network_interface.displayed_input_name_and_description(&node_id, index, context.selection_network_path);
		let input_type = context
			.network_interface
			.input_type_not_invalid(&InputConnector::node_at_index(node_id, index), context.selection_network_path)
			.displayed_type();
		let document_node = context.network_interface.document_node(&node_id, context.selection_network_path);

		ParameterWidgetsInfo {
			document_id: context.document_id,
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
			let updater = std::sync::Arc::new(updater_factory());
			let committer = std::sync::Arc::new(committer_factory());

			let items = MenuListEntry::sections_from_choice_type(move |variant: E| updater(&variant))
				.into_iter()
				.map(|section| {
					section
						.into_iter()
						.map(|entry| {
							let committer = committer.clone();
							entry.on_commit(move |value| committer(value))
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
			// The entry builder clones one callback across all variants, so each factory yields a single shared handle
			let updater = std::sync::Arc::new(updater_factory());
			let committer = std::sync::Arc::new(committer_factory());

			let items = RadioEntryData::list_from_choice_type(move |variant: E| updater(&variant))
				.into_iter()
				.map(|entry| {
					let committer = committer.clone();
					entry.on_commit(move |value| committer(value))
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
			if self.parameter_info.document_node.is_none() {
				log::error!("Could not get document node when building property row for node {:?}", self.parameter_info.node_id);
				return LayoutGroup::row(Vec::new());
			}

			let mut widgets = super::start_widgets(&self.parameter_info);

			let Some(input) = self.parameter_info.input() else {
				log::warn!("A widget failed to be built because its node's input index is invalid.");
				return LayoutGroup::row(vec![]);
			};

			let input: Option<W::Value> = input.as_non_exposed_value().and_then(|v| <&W::Value as TryFrom<&TaggedValue>>::try_from(v).ok()).cloned();

			if let Some(current) = input {
				let committer = || super::commit_value;
				let updater = || self.parameter_info.update_value(move |v: &W::Value| TaggedValue::from(v.clone()));
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
