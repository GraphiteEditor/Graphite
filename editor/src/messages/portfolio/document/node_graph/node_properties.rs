#![allow(clippy::too_many_arguments)]

use super::document_node_definitions::{NodePropertiesContext, IMAGINATE_NODE};
use super::utility_types::FrontendGraphDataType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::imaginate_input::{ImaginateMaskStartingFill, ImaginateSamplingMethod, ImaginateServerStatus, ImaginateStatus};
use graph_craft::Type;
use graphene_core::memo::IORecord;
use graphene_core::raster::curve::Curve;
use graphene_core::raster::{
	BlendMode, CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, ImageFrame, LuminanceCalculation, NoiseType, RedGreenBlue, RedGreenBlueAlpha, RelativeAbsolute,
	SelectiveColorChoice,
};
use graphene_core::text::Font;
use graphene_core::vector::misc::CentroidType;
use graphene_core::vector::style::{GradientType, LineCap, LineJoin};
use graphene_std::vector::style::{Fill, FillChoice, FillType, GradientStops};

use glam::{DAffine2, DVec2, IVec2, UVec2};
use graphene_std::transform::Footprint;
use graphene_std::vector::misc::BooleanOperation;
use graphene_std::vector::VectorData;

pub(crate) fn string_properties(text: impl Into<String>) -> Vec<LayoutGroup> {
	let widget = TextLabel::new(text).widget_holder();
	vec![LayoutGroup::Row { widgets: vec![widget] }]
}

fn optionally_update_value<T>(value: impl Fn(&T) -> Option<TaggedValue> + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	move |input_value: &T| {
		if let Some(value) = value(input_value) {
			NodeGraphMessage::SetInputValue { node_id, input_index, value }.into()
		} else {
			Message::NoOp
		}
	}
}

fn update_value<T>(value: impl Fn(&T) -> TaggedValue + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	optionally_update_value(move |v| Some(value(v)), node_id, input_index)
}

fn commit_value<T>(_: &T) -> Message {
	DocumentMessage::AddTransaction.into()
}

fn expose_widget(node_id: NodeId, index: usize, data_type: FrontendGraphDataType, exposed: bool) -> WidgetHolder {
	ParameterExposeButton::new()
		.exposed(exposed)
		.data_type(data_type)
		.tooltip("Expose this parameter input in node graph")
		.on_update(move |_parameter| {
			NodeGraphMessage::ExposeInput {
				node_id,
				input_index: index,
				new_exposed: !exposed,
			}
			.into()
		})
		.widget_holder()
}

// TODO: Remove this when we have proper entry row formatting that includes room for Assists.
fn add_blank_assist(widgets: &mut Vec<WidgetHolder>) {
	widgets.extend_from_slice(&[
		// Custom CSS specific to the Properties panel converts this Section separator into the width of an assist (24px).
		Separator::new(SeparatorType::Section).widget_holder(),
		// This last one is the separator after the 24px assist.
		Separator::new(SeparatorType::Unrelated).widget_holder(),
	]);
}

fn start_widgets(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, data_type: FrontendGraphDataType, blank_assist: bool) -> Vec<WidgetHolder> {
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	let mut widgets = vec![expose_widget(node_id, index, data_type, input.is_exposed()), TextLabel::new(name).widget_holder()];
	if blank_assist {
		add_blank_assist(&mut widgets);
	}

	widgets
}

pub(crate) fn property_from_type(
	document_node: &DocumentNode,
	node_id: NodeId,
	index: usize,
	name: &str,
	ty: &Type,
	_context: &mut NodePropertiesContext,
	number_options: (Option<f64>, Option<f64>, Option<(f64, f64)>),
) -> Vec<LayoutGroup> {
	let (mut number_min, mut number_max, range) = number_options;
	let mut number_input = NumberInput::default();
	if let Some((range_start, range_end)) = range {
		number_min = Some(range_start);
		number_max = Some(range_end);
		number_input = number_input.mode_range().min(range_start).max(range_end);
	}

	let min = |x: f64| number_min.unwrap_or(x);
	let max = |x: f64| number_max.unwrap_or(x);

	let mut extra_widgets = vec![];
	let widgets = match ty {
		Type::Concrete(concrete_type) => {
			match concrete_type.alias.as_ref().map(|x| x.as_ref()) {
				// Aliased types (ambiguous values)
				Some("Percentage") => number_widget(document_node, node_id, index, name, number_input.percentage().min(min(0.)).max(max(100.)), true).into(),
				Some("SignedPercentage") => number_widget(document_node, node_id, index, name, number_input.percentage().min(min(-100.)).max(max(100.)), true).into(),
				Some("Angle") => number_widget(document_node, node_id, index, name, number_input.mode_range().min(min(-180.)).max(max(180.)).unit("°"), true).into(),
				Some("PixelLength") => number_widget(document_node, node_id, index, name, number_input.min(min(0.)).unit(" px"), true).into(),
				Some("Length") => number_widget(document_node, node_id, index, name, number_input.min(min(0.)), true).into(),
				Some("Fraction") => number_widget(document_node, node_id, index, name, number_input.min(min(0.)).max(max(1.)), true).into(),
				Some("IntegerCount") => number_widget(document_node, node_id, index, name, number_input.int().min(min(1.)), true).into(),
				Some("SeedValue") => number_widget(document_node, node_id, index, name, number_input.int().min(min(0.)), true).into(),
				Some("Resolution") => vec2_widget(document_node, node_id, index, name, "W", "H", " px", Some(64.), add_blank_assist),

				// For all other types, use TypeId-based matching
				_ => {
					if let Some(internal_id) = concrete_type.id {
						use std::any::TypeId;
						match internal_id {
							x if x == TypeId::of::<bool>() => bool_widget(document_node, node_id, index, name, CheckboxInput::default(), true).into(),
							x if x == TypeId::of::<f64>() => number_widget(document_node, node_id, index, name, number_input.min(min(f64::NEG_INFINITY)).max(max(f64::INFINITY)), true).into(),
							x if x == TypeId::of::<u32>() => number_widget(document_node, node_id, index, name, number_input.int().min(min(0.)).max(max(f64::from(u32::MAX))), true).into(),
							x if x == TypeId::of::<u64>() => number_widget(document_node, node_id, index, name, number_input.int().min(min(0.)), true).into(),
							x if x == TypeId::of::<String>() => text_widget(document_node, node_id, index, name, true).into(),
							x if x == TypeId::of::<Color>() => color_widget(document_node, node_id, index, name, ColorButton::default().allow_none(false), true),
							x if x == TypeId::of::<Option<Color>>() => color_widget(document_node, node_id, index, name, ColorButton::default().allow_none(true), true),
							x if x == TypeId::of::<DVec2>() => vec2_widget(document_node, node_id, index, name, "X", "Y", "", None, add_blank_assist),
							x if x == TypeId::of::<UVec2>() => vec2_widget(document_node, node_id, index, name, "X", "Y", "", Some(0.), add_blank_assist),
							x if x == TypeId::of::<IVec2>() => vec2_widget(document_node, node_id, index, name, "X", "Y", "", None, add_blank_assist),
							x if x == TypeId::of::<Vec<f64>>() => vec_f64_input(document_node, node_id, index, name, TextInput::default(), true).into(),
							x if x == TypeId::of::<Vec<DVec2>>() => vec_dvec2_input(document_node, node_id, index, name, TextInput::default(), true).into(),
							x if x == TypeId::of::<Font>() => {
								let (font_widgets, style_widgets) = font_inputs(document_node, node_id, index, name, false);
								font_widgets.into_iter().chain(style_widgets.unwrap_or_default()).collect::<Vec<_>>().into()
							}
							x if x == TypeId::of::<Curve>() => curves_widget(document_node, node_id, index, name, true),
							x if x == TypeId::of::<GradientStops>() => color_widget(document_node, node_id, index, name, ColorButton::default().allow_none(false), true),
							x if x == TypeId::of::<VectorData>() => vector_widget(document_node, node_id, index, name, true).into(),
							x if x == TypeId::of::<Footprint>() => {
								let widgets = footprint_widget(document_node, node_id, index);
								let (last, rest) = widgets.split_last().expect("Footprint widget should return multiple rows");
								extra_widgets = rest.to_vec();
								last.clone()
							}
							x if x == TypeId::of::<BlendMode>() => blend_mode(document_node, node_id, index, name, true),
							x if x == TypeId::of::<RedGreenBlue>() => color_channel(document_node, node_id, index, name, true),
							x if x == TypeId::of::<RedGreenBlueAlpha>() => rgba_channel(document_node, node_id, index, name, true),
							x if x == TypeId::of::<NoiseType>() => noise_type(document_node, node_id, index, name, true),
							x if x == TypeId::of::<FractalType>() => fractal_type(document_node, node_id, index, name, true, false),
							x if x == TypeId::of::<CellularDistanceFunction>() => cellular_distance_function(document_node, node_id, index, name, true, false),
							x if x == TypeId::of::<CellularReturnType>() => cellular_return_type(document_node, node_id, index, name, true, false),
							x if x == TypeId::of::<DomainWarpType>() => domain_warp_type(document_node, node_id, index, name, true, false),
							x if x == TypeId::of::<RelativeAbsolute>() => vec![DropdownInput::new(vec![vec![
								MenuListEntry::new("Relative")
									.label("Relative")
									.on_update(update_value(|_| TaggedValue::RelativeAbsolute(RelativeAbsolute::Relative), node_id, index)),
								MenuListEntry::new("Absolute")
									.label("Absolute")
									.on_update(update_value(|_| TaggedValue::RelativeAbsolute(RelativeAbsolute::Absolute), node_id, index)),
							]])
							.widget_holder()]
							.into(),
							x if x == TypeId::of::<LineCap>() => line_cap_widget(document_node, node_id, index, name, true),
							x if x == TypeId::of::<LineJoin>() => line_join_widget(document_node, node_id, index, name, true),
							x if x == TypeId::of::<FillType>() => vec![DropdownInput::new(vec![vec![
								MenuListEntry::new("Solid")
									.label("Solid")
									.on_update(update_value(|_| TaggedValue::FillType(FillType::Solid), node_id, index)),
								MenuListEntry::new("Gradient")
									.label("Gradient")
									.on_update(update_value(|_| TaggedValue::FillType(FillType::Gradient), node_id, index)),
							]])
							.widget_holder()]
							.into(),
							x if x == TypeId::of::<GradientType>() => vec![DropdownInput::new(vec![vec![
								MenuListEntry::new("Linear")
									.label("Linear")
									.on_update(update_value(|_| TaggedValue::GradientType(GradientType::Linear), node_id, index)),
								MenuListEntry::new("Radial")
									.label("Radial")
									.on_update(update_value(|_| TaggedValue::GradientType(GradientType::Radial), node_id, index)),
							]])
							.widget_holder()]
							.into(),
							x if x == TypeId::of::<BooleanOperation>() => boolean_operation_radio_buttons(document_node, node_id, index, name, true),
							x if x == TypeId::of::<CentroidType>() => centroid_widget(document_node, node_id, index),
							x if x == TypeId::of::<LuminanceCalculation>() => luminance_calculation(document_node, node_id, index, name, true),
							x if x == TypeId::of::<ImaginateSamplingMethod>() => vec![DropdownInput::new(
								ImaginateSamplingMethod::list()
									.into_iter()
									.map(|method| {
										vec![MenuListEntry::new(format!("{:?}", method)).label(method.to_string()).on_update(update_value(
											move |_| TaggedValue::ImaginateSamplingMethod(method),
											node_id,
											index,
										))]
									})
									.collect(),
							)
							.widget_holder()]
							.into(),
							x if x == TypeId::of::<ImaginateMaskStartingFill>() => vec![DropdownInput::new(
								ImaginateMaskStartingFill::list()
									.into_iter()
									.map(|fill| {
										vec![MenuListEntry::new(format!("{:?}", fill)).label(fill.to_string()).on_update(update_value(
											move |_| TaggedValue::ImaginateMaskStartingFill(fill),
											node_id,
											index,
										))]
									})
									.collect(),
							)
							.widget_holder()]
							.into(),
							_ => vec![TextLabel::new(format!("Unsupported type: {}", concrete_type.name)).widget_holder()].into(),
						}
					} else {
						vec![TextLabel::new(format!("Unsupported type: {}", concrete_type.name)).widget_holder()].into()
					}
				}
			}
		}
		Type::Generic(_) => vec![TextLabel::new("Generic type (not supported)").widget_holder()].into(),
		Type::Fn(_, out) => return property_from_type(document_node, node_id, index, name, out, _context, number_options),
		Type::Future(_) => vec![TextLabel::new("Future type (not supported)").widget_holder()].into(),
	};
	extra_widgets.push(widgets);
	extra_widgets
}

fn text_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

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

fn text_area_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

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

fn bool_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, checkbox_input: CheckboxInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(x)) = &input.as_non_exposed_value() {
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

fn footprint_widget(document_node: &DocumentNode, node_id: NodeId, index: usize) -> Vec<LayoutGroup> {
	let mut location_widgets = start_widgets(document_node, node_id, index, "Footprint", FrontendGraphDataType::General, true);
	location_widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let mut scale_widgets = vec![TextLabel::new("").widget_holder()];
	add_blank_assist(&mut scale_widgets);
	scale_widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let mut resolution_widgets = vec![TextLabel::new("").widget_holder()];
	add_blank_assist(&mut resolution_widgets);
	resolution_widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Footprint(footprint)) = &input.as_non_exposed_value() {
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

	vec![
		LayoutGroup::Row { widgets: location_widgets },
		LayoutGroup::Row { widgets: scale_widgets },
		LayoutGroup::Row { widgets: resolution_widgets },
	]
}

fn vec2_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, x: &str, y: &str, unit: &str, min: Option<f64>, mut assist: impl FnMut(&mut Vec<WidgetHolder>)) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, false);

	assist(&mut widgets);

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
		_ => {}
	}

	LayoutGroup::Row { widgets }
}

fn vec_f64_input(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, text_input: TextInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, blank_assist);

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

fn vec_dvec2_input(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, text_props: TextInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, blank_assist);

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

fn font_inputs(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> (Vec<WidgetHolder>, Option<Vec<WidgetHolder>>) {
	let mut first_widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
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

fn vector_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::VectorData, blank_assist);

	widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
	widgets.push(TextLabel::new("Vector data must be supplied through the graph").widget_holder());

	widgets
}

fn number_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, number_props: NumberInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, blank_assist);

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
		_ => {}
	}

	widgets
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn color_channel(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::RedGreenBlue(mode)) = &input.as_non_exposed_value() {
		let calculation_modes = [RedGreenBlue::Red, RedGreenBlue::Green, RedGreenBlue::Blue];
		let mut entries = Vec::with_capacity(calculation_modes.len());
		for method in calculation_modes {
			entries.push(
				MenuListEntry::new(format!("{method:?}"))
					.label(method.to_string())
					.on_update(update_value(move |_| TaggedValue::RedGreenBlue(method), node_id, index))
					.on_commit(commit_value),
			);
		}
		let entries = vec![entries];

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(entries).selected_index(Some(mode as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Color Channel")
}

fn rgba_channel(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::RedGreenBlueAlpha(mode)) = &input.as_non_exposed_value() {
		let calculation_modes = [RedGreenBlueAlpha::Red, RedGreenBlueAlpha::Green, RedGreenBlueAlpha::Blue, RedGreenBlueAlpha::Alpha];
		let mut entries = Vec::with_capacity(calculation_modes.len());
		for method in calculation_modes {
			entries.push(
				MenuListEntry::new(format!("{method:?}"))
					.label(method.to_string())
					.on_update(update_value(move |_| TaggedValue::RedGreenBlueAlpha(method), node_id, index))
					.on_commit(commit_value),
			);
		}
		let entries = vec![entries];

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(entries).selected_index(Some(mode as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Color Channel")
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn noise_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::NoiseType(noise_type)) = &input.as_non_exposed_value() {
		let entries = NoiseType::list()
			.iter()
			.map(|noise_type| {
				MenuListEntry::new(format!("{noise_type:?}"))
					.label(noise_type.to_string())
					.on_update(update_value(move |_| TaggedValue::NoiseType(*noise_type), node_id, index))
					.on_commit(commit_value)
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(vec![entries]).selected_index(Some(noise_type as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Style of noise pattern")
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn fractal_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::FractalType(fractal_type)) = &input.as_non_exposed_value() {
		let entries = FractalType::list()
			.iter()
			.map(|fractal_type| {
				MenuListEntry::new(format!("{fractal_type:?}"))
					.label(fractal_type.to_string())
					.on_update(update_value(move |_| TaggedValue::FractalType(*fractal_type), node_id, index))
					.on_commit(commit_value)
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(vec![entries]).selected_index(Some(fractal_type as u32)).disabled(disabled).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Style of layered levels of the noise pattern")
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn cellular_distance_function(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::CellularDistanceFunction(cellular_distance_function)) = &input.as_non_exposed_value() {
		let entries = CellularDistanceFunction::list()
			.iter()
			.map(|cellular_distance_function| {
				MenuListEntry::new(format!("{cellular_distance_function:?}"))
					.label(cellular_distance_function.to_string())
					.on_update(update_value(move |_| TaggedValue::CellularDistanceFunction(*cellular_distance_function), node_id, index))
					.on_commit(commit_value)
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(vec![entries])
				.selected_index(Some(cellular_distance_function as u32))
				.disabled(disabled)
				.widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Distance function used by the cellular noise")
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn cellular_return_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::CellularReturnType(cellular_return_type)) = &input.as_non_exposed_value() {
		let entries = CellularReturnType::list()
			.iter()
			.map(|cellular_return_type| {
				MenuListEntry::new(format!("{cellular_return_type:?}"))
					.label(cellular_return_type.to_string())
					.on_update(update_value(move |_| TaggedValue::CellularReturnType(*cellular_return_type), node_id, index))
					.on_commit(commit_value)
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(vec![entries]).selected_index(Some(cellular_return_type as u32)).disabled(disabled).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Return type of the cellular noise")
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn domain_warp_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::DomainWarpType(domain_warp_type)) = &input.as_non_exposed_value() {
		let entries = DomainWarpType::list()
			.iter()
			.map(|domain_warp_type| {
				MenuListEntry::new(format!("{domain_warp_type:?}"))
					.label(domain_warp_type.to_string())
					.on_update(update_value(move |_| TaggedValue::DomainWarpType(*domain_warp_type), node_id, index))
					.on_commit(commit_value)
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(vec![entries]).selected_index(Some(domain_warp_type as u32)).disabled(disabled).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Type of domain warp")
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn blend_mode(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::BlendMode(blend_mode)) = &input.as_non_exposed_value() {
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

// TODO: Generalize this for all dropdowns (also see blend_mode and channel_extration)
fn luminance_calculation(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::LuminanceCalculation(calculation)) = &input.as_non_exposed_value() {
		let calculation_modes = LuminanceCalculation::list();
		let mut entries = Vec::with_capacity(calculation_modes.len());
		for method in calculation_modes {
			entries.push(
				MenuListEntry::new(format!("{method:?}"))
					.label(method.to_string())
					.on_update(update_value(move |_| TaggedValue::LuminanceCalculation(method), node_id, index))
					.on_commit(commit_value),
			);
		}
		let entries = vec![entries];

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(entries).selected_index(Some(calculation as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Formula used to calculate the luminance of a pixel")
}

fn boolean_operation_radio_buttons(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::BooleanOperation(calculation)) = &input.as_non_exposed_value() {
		let operations = BooleanOperation::list();
		let icons = BooleanOperation::icons();
		let mut entries = Vec::with_capacity(operations.len());

		for (operation, icon) in operations.into_iter().zip(icons.into_iter()) {
			entries.push(
				RadioEntryData::new(format!("{operation:?}"))
					.icon(icon)
					.tooltip(operation.to_string())
					.on_update(update_value(move |_| TaggedValue::BooleanOperation(operation), node_id, index))
					.on_commit(commit_value),
			);
		}

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries).selected_index(Some(calculation as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }
}

fn line_cap_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::LineCap(line_cap)) = &input.as_non_exposed_value() {
		let entries = [("Butt", LineCap::Butt), ("Round", LineCap::Round), ("Square", LineCap::Square)]
			.into_iter()
			.map(|(name, val)| {
				RadioEntryData::new(format!("{val:?}"))
					.label(name)
					.on_update(update_value(move |_| TaggedValue::LineCap(val), node_id, index))
					.on_commit(commit_value)
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries).selected_index(Some(line_cap as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }
}

fn line_join_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::LineJoin(line_join)) = &input.as_non_exposed_value() {
		let entries = [("Miter", LineJoin::Miter), ("Bevel", LineJoin::Bevel), ("Round", LineJoin::Round)]
			.into_iter()
			.map(|(name, val)| {
				RadioEntryData::new(format!("{val:?}"))
					.label(name)
					.on_update(update_value(move |_| TaggedValue::LineJoin(val), node_id, index))
					.on_commit(commit_value)
			})
			.collect();

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries).selected_index(Some(line_join as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }
}

fn color_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, color_button: ColorButton, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	// Return early with just the label if the input is exposed to the graph, meaning we don't want to show the color picker widget in the Properties panel
	let NodeInput::Value { tagged_value, exposed: false } = &document_node.inputs[index] else {
		return LayoutGroup::Row { widgets };
	};

	widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	match &**tagged_value {
		TaggedValue::Color(color) => widgets.push(
			color_button
				.value(FillChoice::Solid(*color))
				.on_update(update_value(|x: &ColorButton| TaggedValue::Color(x.value.as_solid().unwrap_or_default()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		),
		TaggedValue::OptionalColor(color) => widgets.push(
			color_button
				.value(match color {
					Some(color) => FillChoice::Solid(*color),
					None => FillChoice::None,
				})
				.on_update(update_value(|x: &ColorButton| TaggedValue::OptionalColor(x.value.as_solid()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		),
		TaggedValue::GradientStops(ref x) => widgets.push(
			color_button
				.value(FillChoice::Gradient(x.clone()))
				.on_update(update_value(
					|x: &ColorButton| TaggedValue::GradientStops(x.value.as_gradient().cloned().unwrap_or_default()),
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

fn curves_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

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

fn centroid_widget(document_node: &DocumentNode, node_id: NodeId, index: usize) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, "Centroid Type", FrontendGraphDataType::General, true);
	let Some(input) = document_node.inputs.get(index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return LayoutGroup::Row { widgets: vec![] };
	};
	if let Some(&TaggedValue::CentroidType(centroid_type)) = &input.as_non_exposed_value() {
		let entries = vec![
			RadioEntryData::new("area")
				.label("Area")
				.tooltip("Center of mass for the interior area of the shape")
				.on_update(update_value(move |_| TaggedValue::CentroidType(CentroidType::Area), node_id, index))
				.on_commit(commit_value),
			RadioEntryData::new("length")
				.label("Length")
				.tooltip("Center of mass for the perimeter arc length of the shape")
				.on_update(update_value(move |_| TaggedValue::CentroidType(CentroidType::Length), node_id, index))
				.on_commit(commit_value),
		];

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries)
				.selected_index(match centroid_type {
					CentroidType::Area => Some(0),
					CentroidType::Length => Some(1),
				})
				.widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }
}

pub(crate) fn load_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let url = text_widget(document_node, node_id, 1, "URL", true);

	vec![LayoutGroup::Row { widgets: url }]
}

pub(crate) fn mask_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let mask = color_widget(document_node, node_id, 1, "Stencil", ColorButton::default(), true);

	vec![mask]
}

pub(crate) fn insert_channel_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let color_channel = color_channel(document_node, node_id, 2, "Into", true);

	vec![color_channel]
}

// Noise Type is commented out for now as there is only one type of noise (White Noise).
// As soon as there are more types of noise, this should be uncommented.
pub(crate) fn noise_pattern_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	// Get the current values of the inputs of interest so they can set whether certain inputs are disabled based on various conditions.
	let current_noise_type = match &document_node.inputs[3].as_value() {
		Some(&TaggedValue::NoiseType(noise_type)) => Some(noise_type),
		_ => None,
	};
	let current_domain_warp_type = match &document_node.inputs[4].as_value() {
		Some(&TaggedValue::DomainWarpType(domain_warp_type)) => Some(domain_warp_type),
		_ => None,
	};
	let current_fractal_type = match &document_node.inputs[6].as_value() {
		Some(&TaggedValue::FractalType(fractal_type)) => Some(fractal_type),
		_ => None,
	};
	let fractal_active = current_fractal_type != Some(FractalType::None);
	let coherent_noise_active = current_noise_type != Some(NoiseType::WhiteNoise);
	let cellular_noise_active = current_noise_type == Some(NoiseType::Cellular);
	let ping_pong_active = current_fractal_type == Some(FractalType::PingPong);
	let domain_warp_active = current_domain_warp_type != Some(DomainWarpType::None);
	let domain_warp_only_fractal_type_wrongly_active =
		!domain_warp_active && (current_fractal_type == Some(FractalType::DomainWarpIndependent) || current_fractal_type == Some(FractalType::DomainWarpProgressive));

	// All
	let clip = LayoutGroup::Row {
		widgets: bool_widget(document_node, node_id, 0, "Clip", CheckboxInput::default(), true),
	};
	let seed = number_widget(document_node, node_id, 1, "Seed", NumberInput::default().min(0.).is_integer(true), true);
	let scale = number_widget(document_node, node_id, 2, "Scale", NumberInput::default().min(0.).disabled(!coherent_noise_active), true);
	let noise_type_row = noise_type(document_node, node_id, 3, "Noise Type", true);

	// Domain Warp
	let domain_warp_type_row = domain_warp_type(document_node, node_id, 4, "Domain Warp Type", true, !coherent_noise_active);
	let domain_warp_amplitude = number_widget(
		document_node,
		node_id,
		5,
		"Domain Warp Amplitude",
		NumberInput::default().min(0.).disabled(!coherent_noise_active || !domain_warp_active),
		true,
	);

	// Fractal
	let fractal_type_row = fractal_type(document_node, node_id, 6, "Fractal Type", true, !coherent_noise_active);
	let fractal_octaves = number_widget(
		document_node,
		node_id,
		7,
		"Fractal Octaves",
		NumberInput::default()
			.mode_range()
			.min(1.)
			.max(10.)
			.range_max(Some(4.))
			.is_integer(true)
			.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
		true,
	);
	let fractal_lacunarity = number_widget(
		document_node,
		node_id,
		8,
		"Fractal Lacunarity",
		NumberInput::default()
			.mode_range()
			.min(0.)
			.range_max(Some(10.))
			.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
		true,
	);
	let fractal_gain = number_widget(
		document_node,
		node_id,
		9,
		"Fractal Gain",
		NumberInput::default()
			.mode_range()
			.min(0.)
			.range_max(Some(10.))
			.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
		true,
	);
	let fractal_weighted_strength = number_widget(
		document_node,
		node_id,
		10,
		"Fractal Weighted Strength",
		NumberInput::default()
			.mode_range()
			.min(0.)
			.max(1.) // Defined for the 0-1 range
			.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
		true,
	);
	let fractal_ping_pong_strength = number_widget(
		document_node,
		node_id,
		11,
		"Fractal Ping Pong Strength",
		NumberInput::default()
			.mode_range()
			.min(0.)
			.range_max(Some(10.))
			.disabled(!ping_pong_active || !coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
		true,
	);

	// Cellular
	let cellular_distance_function_row = cellular_distance_function(document_node, node_id, 12, "Cellular Distance Function", true, !coherent_noise_active || !cellular_noise_active);
	let cellular_return_type = cellular_return_type(document_node, node_id, 13, "Cellular Return Type", true, !coherent_noise_active || !cellular_noise_active);
	let cellular_jitter = number_widget(
		document_node,
		node_id,
		14,
		"Cellular Jitter",
		NumberInput::default()
			.mode_range()
			.range_min(Some(0.))
			.range_max(Some(1.))
			.disabled(!coherent_noise_active || !cellular_noise_active),
		true,
	);

	vec![
		// All
		clip,
		LayoutGroup::Row { widgets: seed },
		LayoutGroup::Row { widgets: scale },
		noise_type_row,
		LayoutGroup::Row { widgets: Vec::new() },
		// Domain Warp
		domain_warp_type_row,
		LayoutGroup::Row { widgets: domain_warp_amplitude },
		LayoutGroup::Row { widgets: Vec::new() },
		// Fractal
		fractal_type_row,
		LayoutGroup::Row { widgets: fractal_octaves },
		LayoutGroup::Row { widgets: fractal_lacunarity },
		LayoutGroup::Row { widgets: fractal_gain },
		LayoutGroup::Row { widgets: fractal_weighted_strength },
		LayoutGroup::Row { widgets: fractal_ping_pong_strength },
		LayoutGroup::Row { widgets: Vec::new() },
		// Cellular
		cellular_distance_function_row,
		cellular_return_type,
		LayoutGroup::Row { widgets: cellular_jitter },
	]
}

pub(crate) fn brightness_contrast_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let is_use_classic = match &document_node.inputs[3].as_value() {
		Some(&TaggedValue::Bool(value)) => value,
		_ => false,
	};
	let ((b_min, b_max), (c_min, c_max)) = if is_use_classic { ((-100., 100.), (-100., 100.)) } else { ((-100., 150.), (-50., 100.)) };

	let brightness = number_widget(
		document_node,
		node_id,
		1,
		"Brightness",
		NumberInput::default().mode_range().range_min(Some(b_min)).range_max(Some(b_max)).unit("%").display_decimal_places(2),
		true,
	);
	let contrast = number_widget(
		document_node,
		node_id,
		2,
		"Contrast",
		NumberInput::default().mode_range().range_min(Some(c_min)).range_max(Some(c_max)).unit("%").display_decimal_places(2),
		true,
	);
	let use_classic = bool_widget(document_node, node_id, 3, "Use Classic", CheckboxInput::default(), true);

	vec![
		LayoutGroup::Row { widgets: brightness },
		LayoutGroup::Row { widgets: contrast },
		LayoutGroup::Row { widgets: use_classic },
	]
}

pub(crate) fn curves_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let curves = curves_widget(document_node, node_id, 1, "Curve", true);

	vec![curves]
}

pub(crate) fn _blur_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let radius = number_widget(document_node, node_id, 1, "Radius", NumberInput::default().min(0.).max(20.).int(), true);
	let sigma = number_widget(document_node, node_id, 2, "Sigma", NumberInput::default().min(0.).max(10000.), true);

	vec![LayoutGroup::Row { widgets: radius }, LayoutGroup::Row { widgets: sigma }]
}

pub(crate) fn assign_colors_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let fill_index = 1;
	let stroke_index = 2;
	let gradient_index = 3;
	let reverse_index = 4;
	let randomize_index = 5;
	let seed_index = 6;
	let repeat_every_index = 7;

	let fill_row = bool_widget(document_node, node_id, fill_index, "Fill", CheckboxInput::default(), true);
	let stroke_row = bool_widget(document_node, node_id, stroke_index, "Stroke", CheckboxInput::default(), true);
	let gradient_row = color_widget(document_node, node_id, gradient_index, "Gradient", ColorButton::default().allow_none(false), true);
	let reverse_row = bool_widget(document_node, node_id, reverse_index, "Reverse", CheckboxInput::default(), true);
	let randomize_enabled = if let Some(&TaggedValue::Bool(randomize_enabled)) = &document_node.inputs[randomize_index].as_value() {
		randomize_enabled
	} else {
		false
	};
	let randomize_row = bool_widget(document_node, node_id, randomize_index, "Randomize", CheckboxInput::default(), true);
	let seed_row = number_widget(document_node, node_id, seed_index, "Seed", NumberInput::default().min(0.).int().disabled(!randomize_enabled), true);
	let repeat_every_row = number_widget(
		document_node,
		node_id,
		repeat_every_index,
		"Repeat Every",
		NumberInput::default().min(0.).int().disabled(randomize_enabled),
		true,
	);

	vec![
		LayoutGroup::Row { widgets: fill_row },
		LayoutGroup::Row { widgets: stroke_row },
		gradient_row,
		LayoutGroup::Row { widgets: reverse_row },
		LayoutGroup::Row { widgets: randomize_row },
		LayoutGroup::Row { widgets: seed_row },
		LayoutGroup::Row { widgets: repeat_every_row },
	]
}

pub(crate) fn channel_mixer_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	// Monochrome
	let monochrome_index = 1;
	let monochrome = bool_widget(document_node, node_id, monochrome_index, "Monochrome", CheckboxInput::default(), true);
	let is_monochrome = if let Some(&TaggedValue::Bool(monochrome_choice)) = &document_node.inputs[monochrome_index].as_value() {
		monochrome_choice
	} else {
		false
	};

	// Output channel choice
	let output_channel_index = 18;
	let mut output_channel = vec![TextLabel::new("Output Channel").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];
	add_blank_assist(&mut output_channel);

	let Some(input) = document_node.inputs.get(output_channel_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::RedGreenBlue(choice)) = input.as_non_exposed_value() {
		let entries = vec![
			RadioEntryData::new(format!("{:?}", RedGreenBlue::Red))
				.label(RedGreenBlue::Red.to_string())
				.on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Red), node_id, output_channel_index))
				.on_commit(commit_value),
			RadioEntryData::new(format!("{:?}", RedGreenBlue::Green))
				.label(RedGreenBlue::Green.to_string())
				.on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Green), node_id, output_channel_index))
				.on_commit(commit_value),
			RadioEntryData::new(format!("{:?}", RedGreenBlue::Blue))
				.label(RedGreenBlue::Blue.to_string())
				.on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Blue), node_id, output_channel_index))
				.on_commit(commit_value),
		];
		output_channel.extend([RadioInput::new(entries).selected_index(Some(choice as u32)).widget_holder()]);
	};

	let is_output_channel = if let Some(&TaggedValue::RedGreenBlue(choice)) = &document_node.inputs[output_channel_index].as_value() {
		choice
	} else {
		warn!("Channel Mixer node properties panel could not be displayed.");
		return vec![];
	};

	// Channel values
	let (r, g, b, c) = match (is_monochrome, is_output_channel) {
		(true, _) => ((2, "Red", 40.), (3, "Green", 40.), (4, "Blue", 20.), (5, "Constant", 0.)),
		(false, RedGreenBlue::Red) => ((6, "(Red) Red", 100.), (7, "(Red) Green", 0.), (8, "(Red) Blue", 0.), (9, "(Red) Constant", 0.)),
		(false, RedGreenBlue::Green) => ((10, "(Green) Red", 0.), (11, "(Green) Green", 100.), (12, "(Green) Blue", 0.), (13, "(Green) Constant", 0.)),
		(false, RedGreenBlue::Blue) => ((14, "(Blue) Red", 0.), (15, "(Blue) Green", 0.), (16, "(Blue) Blue", 100.), (17, "(Blue) Constant", 0.)),
	};
	let red = number_widget(
		document_node,
		node_id,
		r.0,
		r.1,
		NumberInput::default().mode_range().min(-200.).max(200.).value(Some(r.2)).unit("%"),
		true,
	);
	let green = number_widget(
		document_node,
		node_id,
		g.0,
		g.1,
		NumberInput::default().mode_range().min(-200.).max(200.).value(Some(g.2)).unit("%"),
		true,
	);
	let blue = number_widget(
		document_node,
		node_id,
		b.0,
		b.1,
		NumberInput::default().mode_range().min(-200.).max(200.).value(Some(b.2)).unit("%"),
		true,
	);
	let constant = number_widget(
		document_node,
		node_id,
		c.0,
		c.1,
		NumberInput::default().mode_range().min(-200.).max(200.).value(Some(c.2)).unit("%"),
		true,
	);

	// Monochrome
	let mut layout = vec![LayoutGroup::Row { widgets: monochrome }];
	// Output channel choice
	if !is_monochrome {
		layout.push(LayoutGroup::Row { widgets: output_channel });
	};
	// Channel values
	layout.extend([
		LayoutGroup::Row { widgets: red },
		LayoutGroup::Row { widgets: green },
		LayoutGroup::Row { widgets: blue },
		LayoutGroup::Row { widgets: constant },
	]);
	layout
}

pub(crate) fn selective_color_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	// Colors choice
	let colors_index = 38;
	let mut colors = vec![TextLabel::new("Colors").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];
	add_blank_assist(&mut colors);

	let Some(input) = document_node.inputs.get(colors_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::SelectiveColorChoice(choice)) = input.as_non_exposed_value() {
		use SelectiveColorChoice::*;
		let entries = [[Reds, Yellows, Greens, Cyans, Blues, Magentas].as_slice(), [Whites, Neutrals, Blacks].as_slice()]
			.into_iter()
			.map(|section| {
				section
					.iter()
					.map(|choice| {
						MenuListEntry::new(format!("{choice:?}"))
							.label(choice.to_string())
							.on_update(update_value(move |_| TaggedValue::SelectiveColorChoice(*choice), node_id, colors_index))
							.on_commit(commit_value)
					})
					.collect()
			})
			.collect();
		colors.extend([DropdownInput::new(entries).selected_index(Some(choice as u32)).widget_holder()]);
	}

	let colors_choice_index = if let Some(&TaggedValue::SelectiveColorChoice(choice)) = &document_node.inputs[colors_index].as_value() {
		choice
	} else {
		warn!("Selective Color node properties panel could not be displayed.");
		return vec![];
	};

	// CMYK
	let (c, m, y, k) = match colors_choice_index {
		SelectiveColorChoice::Reds => ((2, "(Reds) Cyan"), (3, "(Reds) Magenta"), (4, "(Reds) Yellow"), (5, "(Reds) Black")),
		SelectiveColorChoice::Yellows => ((6, "(Yellows) Cyan"), (7, "(Yellows) Magenta"), (8, "(Yellows) Yellow"), (9, "(Yellows) Black")),
		SelectiveColorChoice::Greens => ((10, "(Greens) Cyan"), (11, "(Greens) Magenta"), (12, "(Greens) Yellow"), (13, "(Greens) Black")),
		SelectiveColorChoice::Cyans => ((14, "(Cyans) Cyan"), (15, "(Cyans) Magenta"), (16, "(Cyans) Yellow"), (17, "(Cyans) Black")),
		SelectiveColorChoice::Blues => ((18, "(Blues) Cyan"), (19, "(Blues) Magenta"), (20, "(Blues) Yellow"), (21, "(Blues) Black")),
		SelectiveColorChoice::Magentas => ((22, "(Magentas) Cyan"), (23, "(Magentas) Magenta"), (24, "(Magentas) Yellow"), (25, "(Magentas) Black")),
		SelectiveColorChoice::Whites => ((26, "(Whites) Cyan"), (27, "(Whites) Magenta"), (28, "(Whites) Yellow"), (29, "(Whites) Black")),
		SelectiveColorChoice::Neutrals => ((30, "(Neutrals) Cyan"), (31, "(Neutrals) Magenta"), (32, "(Neutrals) Yellow"), (33, "(Neutrals) Black")),
		SelectiveColorChoice::Blacks => ((34, "(Blacks) Cyan"), (35, "(Blacks) Magenta"), (36, "(Blacks) Yellow"), (37, "(Blacks) Black")),
	};
	let cyan = number_widget(document_node, node_id, c.0, c.1, NumberInput::default().mode_range().min(-100.).max(100.).unit("%"), true);
	let magenta = number_widget(document_node, node_id, m.0, m.1, NumberInput::default().mode_range().min(-100.).max(100.).unit("%"), true);
	let yellow = number_widget(document_node, node_id, y.0, y.1, NumberInput::default().mode_range().min(-100.).max(100.).unit("%"), true);
	let black = number_widget(document_node, node_id, k.0, k.1, NumberInput::default().mode_range().min(-100.).max(100.).unit("%"), true);

	// Mode
	let mode_index = 1;
	let mut mode = start_widgets(document_node, node_id, mode_index, "Mode", FrontendGraphDataType::General, true);
	mode.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let Some(input) = document_node.inputs.get(mode_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::RelativeAbsolute(relative_or_absolute)) = &input.as_non_exposed_value() {
		let entries = vec![
			RadioEntryData::new("relative")
				.label("Relative")
				.on_update(update_value(|_| TaggedValue::RelativeAbsolute(RelativeAbsolute::Relative), node_id, mode_index))
				.on_commit(commit_value),
			RadioEntryData::new("absolute")
				.label("Absolute")
				.on_update(update_value(|_| TaggedValue::RelativeAbsolute(RelativeAbsolute::Absolute), node_id, mode_index))
				.on_commit(commit_value),
		];
		mode.push(RadioInput::new(entries).selected_index(Some(relative_or_absolute as u32)).widget_holder());
	};

	vec![
		// Colors choice
		LayoutGroup::Row { widgets: colors },
		// CMYK
		LayoutGroup::Row { widgets: cyan },
		LayoutGroup::Row { widgets: magenta },
		LayoutGroup::Row { widgets: yellow },
		LayoutGroup::Row { widgets: black },
		// Mode
		LayoutGroup::Row { widgets: mode },
	]
}

#[cfg(feature = "gpu")]
pub(crate) fn _gpu_map_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let map = text_widget(document_node, node_id, 1, "Map", true);

	vec![LayoutGroup::Row { widgets: map }]
}

pub(crate) fn exposure_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let exposure = number_widget(document_node, node_id, 1, "Exposure", NumberInput::default().min(-20.).max(20.), true);
	let offset = number_widget(document_node, node_id, 2, "Offset", NumberInput::default().min(-0.5).max(0.5), true);
	let gamma_input = NumberInput::default().min(0.01).max(9.99).increment_step(0.1);
	let gamma_correction = number_widget(document_node, node_id, 3, "Gamma Correction", gamma_input, true);

	vec![
		LayoutGroup::Row { widgets: exposure },
		LayoutGroup::Row { widgets: offset },
		LayoutGroup::Row { widgets: gamma_correction },
	]
}

pub(crate) fn rectangle_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let size_x_index = 1;
	let size_y_index = 2;
	let corner_rounding_type_index = 3;
	let corner_radius_index = 4;
	let clamped_index = 5;

	// Size X
	let size_x = number_widget(document_node, node_id, size_x_index, "Size X", NumberInput::default(), true);

	// Size Y
	let size_y = number_widget(document_node, node_id, size_y_index, "Size Y", NumberInput::default(), true);

	// Corner Radius
	let mut corner_radius_row_1 = start_widgets(document_node, node_id, corner_radius_index, "Corner Radius", FrontendGraphDataType::Number, true);
	corner_radius_row_1.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	let mut corner_radius_row_2 = vec![Separator::new(SeparatorType::Unrelated).widget_holder()];
	corner_radius_row_2.push(TextLabel::new("").widget_holder());
	add_blank_assist(&mut corner_radius_row_2);

	let Some(input) = document_node.inputs.get(corner_rounding_type_index) else {
		log::warn!("A widget failed to be built because its node's input index is invalid.");
		return vec![];
	};
	if let Some(&TaggedValue::Bool(is_individual)) = &input.as_non_exposed_value() {
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
	let clamped = bool_widget(document_node, node_id, clamped_index, "Clamped", CheckboxInput::default(), true);

	vec![
		LayoutGroup::Row { widgets: size_x },
		LayoutGroup::Row { widgets: size_y },
		LayoutGroup::Row { widgets: corner_radius_row_1 },
		LayoutGroup::Row { widgets: corner_radius_row_2 },
		LayoutGroup::Row { widgets: clamped },
	]
}

pub(crate) fn line_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| vec2_widget(document_node, node_id, index, name, "X", "Y", " px", None, add_blank_assist);
	vec![operand("Start", 1), operand("End", 2)]
}

pub(crate) fn spline_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	vec![LayoutGroup::Row {
		widgets: vec_dvec2_input(document_node, node_id, 1, "Points", TextInput::default().centered(true), true),
	}]
}

pub(crate) fn transform_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let translation = vec2_widget(document_node, node_id, 1, "Translation", "X", "Y", " px", None, add_blank_assist);

	let rotation = {
		let index = 2;

		let mut widgets = start_widgets(document_node, node_id, index, "Rotation", FrontendGraphDataType::Number, true);

		let Some(input) = document_node.inputs.get(index) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		if let Some(&TaggedValue::F64(val)) = input.as_non_exposed_value() {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(val.to_degrees()))
					.unit("°")
					.mode(NumberInputMode::Range)
					.range_min(Some(-180.))
					.range_max(Some(180.))
					.on_update(update_value(|number_input: &NumberInput| TaggedValue::F64(number_input.value.unwrap().to_radians()), node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
			]);
		}

		LayoutGroup::Row { widgets }
	};

	let scale = vec2_widget(document_node, node_id, 3, "Scale", "W", "H", "x", None, add_blank_assist);

	vec![translation, rotation, scale]
}
pub(crate) fn rasterize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	footprint_widget(document_node, node_id, 1)
}

pub(crate) fn text_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let text = text_area_widget(document_node, node_id, 1, "Text", true);
	let (font, style) = font_inputs(document_node, node_id, 2, "Font", true);
	let size = number_widget(document_node, node_id, 3, "Size", NumberInput::default().unit(" px").min(1.), true);
	let line_height_ratio = number_widget(document_node, node_id, 4, "Line Height", NumberInput::default().min(0.).step(0.1), true);
	let character_spacing = number_widget(document_node, node_id, 5, "Character Spacing", NumberInput::default().min(0.).step(0.1), true);

	let mut result = vec![LayoutGroup::Row { widgets: text }, LayoutGroup::Row { widgets: font }];
	if let Some(style) = style {
		result.push(LayoutGroup::Row { widgets: style });
	}
	result.push(LayoutGroup::Row { widgets: size });
	result.push(LayoutGroup::Row { widgets: line_height_ratio });
	result.push(LayoutGroup::Row { widgets: character_spacing });
	result
}

pub(crate) fn imaginate_properties(document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let imaginate_node = [context.selection_network_path, &[node_id]].concat();

	let resolve_input = |name: &str| {
		IMAGINATE_NODE
			.default_node_template()
			.persistent_node_metadata
			.input_names
			.iter()
			.position(|input| input == name)
			.unwrap_or_else(|| panic!("Input {name} not found"))
	};
	let seed_index = resolve_input("Seed");
	let resolution_index = resolve_input("Resolution");
	let samples_index = resolve_input("Samples");
	let sampling_method_index = resolve_input("Sampling Method");
	let text_guidance_index = resolve_input("Prompt Guidance");
	let text_index = resolve_input("Prompt");
	let neg_index = resolve_input("Negative Prompt");
	let base_img_index = resolve_input("Adapt Input Image");
	let img_creativity_index = resolve_input("Image Creativity");
	// let mask_index = resolve_input("Masking Layer");
	// let inpaint_index = resolve_input("Inpaint");
	// let mask_blur_index = resolve_input("Mask Blur");
	// let mask_fill_index = resolve_input("Mask Starting Fill");
	let faces_index = resolve_input("Improve Faces");
	let tiling_index = resolve_input("Tiling");

	let controller = &document_node.inputs[resolve_input("Controller")];

	let server_status = {
		let server_status = context.persistent_data.imaginate.server_status();
		let status_text = server_status.to_text();
		let mut widgets = vec![
			TextLabel::new("Server").widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			IconButton::new("Settings", 24)
				.tooltip("Preferences: Imaginate")
				.on_update(|_| DialogMessage::RequestPreferencesDialog.into())
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new(status_text).bold(true).widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			IconButton::new("Reload", 24)
				.tooltip("Refresh connection status")
				.on_update(|_| PortfolioMessage::ImaginateCheckServerStatus.into())
				.widget_holder(),
		];
		if let ImaginateServerStatus::Unavailable | ImaginateServerStatus::Failed(_) = server_status {
			widgets.extend([
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				TextButton::new("Server Help")
					.tooltip("Learn how to connect Imaginate to an image generation server")
					.on_update(|_| {
						FrontendMessage::TriggerVisitLink {
							url: "https://github.com/GraphiteEditor/Graphite/discussions/1089".to_string(),
						}
						.into()
					})
					.widget_holder(),
			]);
		}
		LayoutGroup::Row { widgets }.with_tooltip("Connection status to the server that computes generated images")
	};

	let Some(TaggedValue::ImaginateController(controller)) = controller.as_value() else {
		panic!("Invalid output status input")
	};
	let imaginate_status = controller.get_status();

	let use_base_image = if let Some(&TaggedValue::Bool(use_base_image)) = &document_node.inputs[base_img_index].as_value() {
		use_base_image
	} else {
		true
	};

	let transform_not_connected = false;

	let progress = {
		let mut widgets = vec![TextLabel::new("Progress").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];
		add_blank_assist(&mut widgets);
		let status = imaginate_status.to_text();
		widgets.push(TextLabel::new(status.as_ref()).bold(true).widget_holder());
		LayoutGroup::Row { widgets }.with_tooltip(match imaginate_status {
			ImaginateStatus::Failed(_) => status.as_ref(),
			_ => "When generating, the percentage represents how many sampling steps have so far been processed out of the target number",
		})
	};

	let image_controls = {
		let mut widgets = vec![TextLabel::new("Image").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];

		match &imaginate_status {
			ImaginateStatus::Beginning | ImaginateStatus::Uploading => {
				add_blank_assist(&mut widgets);
				widgets.push(TextButton::new("Beginning...").tooltip("Sending image generation request to the server").disabled(true).widget_holder());
			}
			ImaginateStatus::Generating(_) => {
				add_blank_assist(&mut widgets);
				widgets.push(
					TextButton::new("Terminate")
						.tooltip("Cancel the in-progress image generation and keep the latest progress")
						.on_update({
							let controller = controller.clone();
							move |_| {
								controller.request_termination();
								Message::NoOp
							}
						})
						.widget_holder(),
				);
			}
			ImaginateStatus::Terminating => {
				add_blank_assist(&mut widgets);
				widgets.push(
					TextButton::new("Terminating...")
						.tooltip("Waiting on the final image generated after termination")
						.disabled(true)
						.widget_holder(),
				);
			}
			ImaginateStatus::Ready | ImaginateStatus::ReadyDone | ImaginateStatus::Terminated | ImaginateStatus::Failed(_) => widgets.extend_from_slice(&[
				IconButton::new("Random", 24)
					.tooltip("Generate with a new random seed")
					.on_update({
						let imaginate_node = imaginate_node.clone();
						let controller = controller.clone();
						move |_| {
							controller.trigger_regenerate();
							DocumentMessage::ImaginateRandom {
								imaginate_node: imaginate_node.clone(),
								then_generate: true,
							}
							.into()
						}
					})
					.widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				TextButton::new("Generate")
					.tooltip("Fill layer frame by generating a new image")
					.on_update({
						let controller = controller.clone();
						let imaginate_node = imaginate_node.clone();
						move |_| {
							controller.trigger_regenerate();
							DocumentMessage::ImaginateGenerate {
								imaginate_node: imaginate_node.clone(),
							}
							.into()
						}
					})
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				TextButton::new("Clear")
					.tooltip("Remove generated image from the layer frame")
					.disabled(!matches!(imaginate_status, ImaginateStatus::ReadyDone))
					.on_update({
						let controller = controller.clone();
						let imaginate_node = imaginate_node.clone();
						move |_| {
							controller.set_status(ImaginateStatus::Ready);
							DocumentMessage::ImaginateGenerate {
								imaginate_node: imaginate_node.clone(),
							}
							.into()
						}
					})
					.widget_holder(),
			]),
		}
		LayoutGroup::Row { widgets }.with_tooltip("Buttons that control the image generation process")
	};

	// Requires custom layout for the regenerate button
	let seed = {
		let mut widgets = start_widgets(document_node, node_id, seed_index, "Seed", FrontendGraphDataType::Number, false);

		let Some(input) = document_node.inputs.get(seed_index) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		if let Some(&TaggedValue::F64(seed)) = &input.as_non_exposed_value() {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				IconButton::new("Regenerate", 24)
					.tooltip("Set a new random seed")
					.on_update({
						let imaginate_node = imaginate_node.clone();
						move |_| {
							DocumentMessage::ImaginateRandom {
								imaginate_node: imaginate_node.clone(),
								then_generate: false,
							}
							.into()
						}
					})
					.widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				NumberInput::new(Some(seed))
					.int()
					.min(-((1_u64 << f64::MANTISSA_DIGITS) as f64))
					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
					.on_update(update_value(move |input: &NumberInput| TaggedValue::F64(input.value.unwrap()), node_id, seed_index))
					.on_commit(commit_value)
					.mode(NumberInputMode::Increment)
					.widget_holder(),
			])
		}
		// Note: Limited by f64. You cannot even have all the possible u64 values :)
		LayoutGroup::Row { widgets }.with_tooltip("Seed determines the random outcome, enabling limitless unique variations")
	};

	// let transform = context
	// 	.executor
	// 	.introspect_node_in_network(context.network, &imaginate_node, |network| network.inputs.first().copied(), |frame: &ImageFrame<Color>| frame.transform)
	// 	.unwrap_or_default();
	let image_size = context
		.executor
		.introspect_node_in_network(
			context.network_interface.network(&[]).unwrap(),
			&imaginate_node,
			|network| {
				network
					.nodes
					.iter()
					.find(|node| {
						node.1
							.inputs
							.iter()
							.any(|node_input| if let NodeInput::Network { import_index, .. } = node_input { *import_index == 0 } else { false })
					})
					.map(|(node_id, _)| node_id)
					.copied()
			},
			|frame: &IORecord<(), ImageFrame<Color>>| (frame.output.image.width, frame.output.image.height),
		)
		.unwrap_or_default();

	let resolution = {
		use graphene_std::imaginate::pick_safe_imaginate_resolution;

		let mut widgets = start_widgets(document_node, node_id, resolution_index, "Resolution", FrontendGraphDataType::Number, false);

		let round = |size: DVec2| {
			let (x, y) = pick_safe_imaginate_resolution(size.into());
			DVec2::new(x as f64, y as f64)
		};

		let Some(input) = document_node.inputs.get(resolution_index) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		if let Some(&TaggedValue::OptionalDVec2(vec2)) = &input.as_non_exposed_value() {
			let dimensions_is_auto = vec2.is_none();
			let vec2 = vec2.unwrap_or_else(|| round((image_size.0 as f64, image_size.1 as f64).into()));

			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				IconButton::new("Rescale", 24)
					.tooltip("Set the layer dimensions to this resolution")
					.on_update(move |_| DialogMessage::RequestComingSoonDialog { issue: None }.into())
					.widget_holder(),
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				CheckboxInput::new(!dimensions_is_auto || transform_not_connected)
					.icon("Edit12px")
					.tooltip({
						let message = "Set a custom resolution instead of using the input's dimensions (rounded to the nearest 64)";
						let manual_message = "Set a custom resolution instead of using the input's dimensions (rounded to the nearest 64).\n\
							\n\
							(Resolution must be set manually while the 'Transform' input is disconnected.)";

						if transform_not_connected {
							manual_message
						} else {
							message
						}
					})
					.disabled(transform_not_connected)
					.on_update(update_value(
						move |checkbox_input: &CheckboxInput| TaggedValue::OptionalDVec2(if checkbox_input.checked { Some(vec2) } else { None }),
						node_id,
						resolution_index,
					))
					.on_commit(commit_value)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(vec2.x))
					.label("W")
					.min(64.)
					.step(64.)
					.unit(" px")
					.disabled(dimensions_is_auto && !transform_not_connected)
					.on_update(update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(Some(round(DVec2::new(number_input.value.unwrap(), vec2.y)))),
						node_id,
						resolution_index,
					))
					.on_commit(commit_value)
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				NumberInput::new(Some(vec2.y))
					.label("H")
					.min(64.)
					.step(64.)
					.unit(" px")
					.disabled(dimensions_is_auto && !transform_not_connected)
					.on_update(update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(Some(round(DVec2::new(vec2.x, number_input.value.unwrap())))),
						node_id,
						resolution_index,
					))
					.on_commit(commit_value)
					.widget_holder(),
			])
		}
		LayoutGroup::Row { widgets }.with_tooltip(
			"Width and height of the image that will be generated. Larger resolutions take longer to compute.\n\
			\n\
			512x512 yields optimal results because the AI is trained to understand that scale best. Larger sizes may tend to integrate the prompt's subject more than once. Small sizes are often incoherent.\n\
			\n\
			Dimensions must be a multiple of 64, so these are set by rounding the layer dimensions. A resolution exceeding 1 megapixel is reduced below that limit because larger sizes may exceed available GPU memory on the server.")
	};

	let sampling_steps = {
		let widgets = number_widget(document_node, node_id, samples_index, "Sampling Steps", NumberInput::default().min(0.).max(150.).int(), true);
		LayoutGroup::Row { widgets }.with_tooltip("Number of iterations to improve the image generation quality, with diminishing returns around 40 when using the Euler A sampling method")
	};

	let sampling_method = {
		let mut widgets = start_widgets(document_node, node_id, sampling_method_index, "Sampling Method", FrontendGraphDataType::General, true);

		let Some(input) = document_node.inputs.get(sampling_method_index) else {
			log::warn!("A widget failed to be built because its node's input index is invalid.");
			return vec![];
		};
		if let Some(&TaggedValue::ImaginateSamplingMethod(sampling_method)) = &input.as_non_exposed_value() {
			let sampling_methods = ImaginateSamplingMethod::list();
			let mut entries = Vec::with_capacity(sampling_methods.len());
			for method in sampling_methods {
				entries.push(
					MenuListEntry::new(format!("{method:?}"))
						.label(method.to_string())
						.on_update(update_value(move |_| TaggedValue::ImaginateSamplingMethod(method), node_id, sampling_method_index))
						.on_commit(commit_value),
				);
			}
			let entries = vec![entries];

			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				DropdownInput::new(entries).selected_index(Some(sampling_method as u32)).widget_holder(),
			]);
		}
		LayoutGroup::Row { widgets }.with_tooltip("Algorithm used to generate the image during each sampling step")
	};

	let text_guidance = {
		let widgets = number_widget(document_node, node_id, text_guidance_index, "Prompt Guidance", NumberInput::default().min(0.).max(30.), true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Amplification of the text prompt's influence over the outcome. At 0, the prompt is entirely ignored.\n\
			\n\
			Lower values are more creative and exploratory. Higher values are more literal and uninspired.\n\
			\n\
			This parameter is otherwise known as CFG (classifier-free guidance).",
		)
	};

	let text_prompt = {
		let widgets = text_area_widget(document_node, node_id, text_index, "Prompt", true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Description of the desired image subject and style.\n\
			\n\
			Include an artist name like \"Rembrandt\" or art medium like \"watercolor\" or \"photography\" to influence the look. List multiple to meld styles.\n\
			\n\
			To boost (or lessen) the importance of a word or phrase, wrap it in parentheses ending with a colon and a multiplier, for example:\n\
			\"Colorless green ideas (sleep:1.3) furiously\"",
		)
	};
	let negative_prompt = {
		let widgets = text_area_widget(document_node, node_id, neg_index, "Negative Prompt", true);
		LayoutGroup::Row { widgets }.with_tooltip("A negative text prompt can be used to list things like objects or colors to avoid")
	};
	let base_image = {
		let widgets = bool_widget(document_node, node_id, base_img_index, "Adapt Input Image", CheckboxInput::default(), true);
		LayoutGroup::Row { widgets }.with_tooltip("Generate an image based upon the bitmap data plugged into this node")
	};
	let image_creativity = {
		let props = NumberInput::default().percentage().disabled(!use_base_image);
		let widgets = number_widget(document_node, node_id, img_creativity_index, "Image Creativity", props, true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Strength of the artistic liberties allowing changes from the input image. The image is unchanged at 0% and completely different at 100%.\n\
			\n\
			This parameter is otherwise known as denoising strength.",
		)
	};

	let mut layout = vec![
		server_status,
		progress,
		image_controls,
		seed,
		resolution,
		sampling_steps,
		sampling_method,
		text_guidance,
		text_prompt,
		negative_prompt,
		base_image,
		image_creativity,
		// layer_mask,
	];

	// if use_base_image && layer_reference_input_layer_is_some {
	// 	let in_paint = {
	// 		let mut widgets = start_widgets(document_node, node_id, inpaint_index, "Inpaint", FrontendGraphDataType::Boolean, true);

	// 		if let Some(& TaggedValue::Bool(in_paint)
	//)/ 		} = &document_node.inputs[inpaint_index].as_non_exposed_value()
	// 		{
	// 			widgets.extend_from_slice(&[
	// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
	// 				RadioInput::new(
	// 					[(true, "Inpaint"), (false, "Outpaint")]
	// 						.into_iter()
	// 						.map(|(paint, name)| RadioEntryData::new(name).label(name).on_update(update_value(move |_| TaggedValue::Bool(paint), node_id, inpaint_index)))
	// 						.collect(),
	// 				)
	// 				.selected_index(Some(1 - in_paint as u32))
	// 				.widget_holder(),
	// 			]);
	// 		}
	// 		LayoutGroup::Row { widgets }.with_tooltip(
	// 			"Constrain image generation to the interior (inpaint) or exterior (outpaint) of the mask, while referencing the other unchanged parts as context imagery.\n\
	// 			\n\
	// 			An unwanted part of an image can be replaced by drawing around it with a black shape and inpainting with that mask layer.\n\
	// 			\n\
	// 			An image can be uncropped by resizing the Imaginate layer to the target bounds and outpainting with a black rectangle mask matching the original image bounds.",
	// 		)
	// 	};

	// 	let blur_radius = {
	// 		let number_props = NumberInput::default().unit(" px").min(0.).max(25.).int();
	// 		let widgets = number_widget(document_node, node_id, mask_blur_index, "Mask Blur", number_props, true);
	// 		LayoutGroup::Row { widgets }.with_tooltip("Blur radius for the mask. Useful for softening sharp edges to blend the masked area with the rest of the image.")
	// 	};

	// 	let mask_starting_fill = {
	// 		let mut widgets = start_widgets(document_node, node_id, mask_fill_index, "Mask Starting Fill", FrontendGraphDataType::General, true);

	// 		if let Some(& TaggedValue::ImaginateMaskStartingFill(starting_fill)
	//)/ 		} = &document_node.inputs[mask_fill_index].as_non_exposed_value()
	// 		{
	// 			let mask_fill_content_modes = ImaginateMaskStartingFill::list();
	// 			let mut entries = Vec::with_capacity(mask_fill_content_modes.len());
	// 			for mode in mask_fill_content_modes {
	// 				entries.push(MenuListEntry::new(format!("{mode:?}")).label(mode.to_string()).on_update(update_value(move |_| TaggedValue::ImaginateMaskStartingFill(mode), node_id, mask_fill_index)));
	// 			}
	// 			let entries = vec![entries];

	// 			widgets.extend_from_slice(&[
	// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
	// 				DropdownInput::new(entries).selected_index(Some(starting_fill as u32)).widget_holder(),
	// 			]);
	// 		}
	// 		LayoutGroup::Row { widgets }.with_tooltip(
	// 			"Begin in/outpainting the masked areas using this fill content as the starting input image.\n\
	// 			\n\
	// 			Each option can be visualized by generating with 'Sampling Steps' set to 0.",
	// 		)
	// 	};
	// 	layout.extend_from_slice(&[in_paint, blur_radius, mask_starting_fill]);
	// }

	let improve_faces = {
		let widgets = bool_widget(document_node, node_id, faces_index, "Improve Faces", CheckboxInput::default(), true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Postprocess human (or human-like) faces to look subtly less distorted.\n\
			\n\
			This filter can be used on its own by enabling 'Adapt Input Image' and setting 'Sampling Steps' to 0.",
		)
	};
	let tiling = {
		let widgets = bool_widget(document_node, node_id, tiling_index, "Tiling", CheckboxInput::default(), true);
		LayoutGroup::Row { widgets }.with_tooltip("Generate the image so its edges loop seamlessly to make repeatable patterns or textures")
	};
	layout.extend_from_slice(&[improve_faces, tiling]);

	layout
}

fn unknown_node_properties(reference: &String) -> Vec<LayoutGroup> {
	string_properties(format!("Node '{}' cannot be found in library", reference))
}

pub(crate) fn node_no_properties(_document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	string_properties(if context.network_interface.is_layer(&node_id, context.selection_network_path) {
		"Layer has no properties"
	} else {
		"Node has no properties"
	})
}

pub(crate) fn index_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let index = number_widget(document_node, node_id, 1, "Index", NumberInput::default().min(0.), true);

	vec![LayoutGroup::Row { widgets: index }]
}

pub(crate) fn generate_node_properties(document_node: &DocumentNode, node_id: NodeId, pinned: bool, context: &mut NodePropertiesContext) -> LayoutGroup {
	let reference = context.network_interface.reference(&node_id, context.selection_network_path).clone();
	let layout = if let Some(ref reference) = reference {
		match super::document_node_definitions::resolve_document_node_type(reference) {
			Some(document_node_type) => (document_node_type.properties)(document_node, node_id, context),
			None => unknown_node_properties(reference),
		}
	} else {
		node_no_properties(document_node, node_id, context)
	};

	LayoutGroup::Section {
		name: reference.unwrap_or_default(),
		visible: document_node.visible,
		pinned,
		id: node_id.0,
		layout,
	}
}

pub(crate) fn boolean_operation_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operation_index = 1;
	let operation = boolean_operation_radio_buttons(document_node, node_id, operation_index, "Operation", true);

	vec![operation]
}

pub(crate) fn copy_to_points_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let instance_index = 1;
	let random_scale_min_index = 2;
	let random_scale_max_index = 3;
	let random_scale_bias_index = 4;
	let random_scale_seed_index = 5;
	let random_rotation_index = 6;
	let random_rotation_seed_index = 7;

	let instance = vector_widget(document_node, node_id, instance_index, "Instance", true);

	let random_scale_min = number_widget(
		document_node,
		node_id,
		random_scale_min_index,
		"Random Scale Min",
		NumberInput::default().min(0.).mode_range().range_min(Some(0.)).range_max(Some(2.)).unit("x"),
		true,
	);
	let random_scale_max = number_widget(
		document_node,
		node_id,
		random_scale_max_index,
		"Random Scale Max",
		NumberInput::default().min(0.).mode_range().range_min(Some(0.)).range_max(Some(2.)).unit("x"),
		true,
	);
	let random_scale_bias = number_widget(
		document_node,
		node_id,
		random_scale_bias_index,
		"Random Scale Bias",
		NumberInput::default().mode_range().range_min(Some(-50.)).range_max(Some(50.)),
		true,
	);
	let random_scale_seed = number_widget(document_node, node_id, random_scale_seed_index, "Random Scale Seed", NumberInput::default().int().min(0.), true);

	let random_rotation = number_widget(
		document_node,
		node_id,
		random_rotation_index,
		"Random Rotation",
		NumberInput::default().min(0.).max(360.).mode_range().unit("°"),
		true,
	);
	let random_rotation_seed = number_widget(document_node, node_id, random_rotation_seed_index, "Random Rotation Seed", NumberInput::default().int().min(0.), true);

	vec![
		LayoutGroup::Row { widgets: instance }.with_tooltip("Artwork to be copied and placed at each point"),
		LayoutGroup::Row { widgets: random_scale_min }.with_tooltip("Minimum range of randomized sizes given to each instance"),
		LayoutGroup::Row { widgets: random_scale_max }.with_tooltip("Maximum range of randomized sizes given to each instance"),
		LayoutGroup::Row { widgets: random_scale_bias }
			.with_tooltip("Bias for the probability distribution of randomized sizes (0 is uniform, negatives favor more of small sizes, positives favor more of large sizes)"),
		LayoutGroup::Row { widgets: random_scale_seed }.with_tooltip("Seed to determine unique variations on all the randomized instance sizes"),
		LayoutGroup::Row { widgets: random_rotation }.with_tooltip("Range of randomized angles given to each instance, in degrees ranging from furthest clockwise to counterclockwise"),
		LayoutGroup::Row { widgets: random_rotation_seed }.with_tooltip("Seed to determine unique variations on all the randomized instance angles"),
	]
}

pub(crate) fn sample_points_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let spacing = number_widget(document_node, node_id, 1, "Spacing", NumberInput::default().min(1.).unit(" px"), true);
	let start_offset = number_widget(document_node, node_id, 2, "Start Offset", NumberInput::default().min(0.).unit(" px"), true);
	let stop_offset = number_widget(document_node, node_id, 3, "Stop Offset", NumberInput::default().min(0.).unit(" px"), true);
	let adaptive_spacing = bool_widget(document_node, node_id, 4, "Adaptive Spacing", CheckboxInput::default(), true);

	vec![
		LayoutGroup::Row { widgets: spacing }.with_tooltip("Distance between each instance (exact if 'Adaptive Spacing' is disabled, approximate if enabled)"),
		LayoutGroup::Row { widgets: start_offset }.with_tooltip("Exclude some distance from the start of the path before the first instance"),
		LayoutGroup::Row { widgets: stop_offset }.with_tooltip("Exclude some distance from the end of the path after the last instance"),
		LayoutGroup::Row { widgets: adaptive_spacing }.with_tooltip("Round 'Spacing' to a nearby value that divides into the path length evenly"),
	]
}

pub(crate) fn poisson_disk_points_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let separation_disk_diameter_index = 1;
	let seed_index = 2;

	let spacing = number_widget(
		document_node,
		node_id,
		separation_disk_diameter_index,
		"Separation Disk Diameter",
		NumberInput::default().min(0.01).mode_range().range_min(Some(1.)).range_max(Some(100.)),
		true,
	);

	let seed = number_widget(document_node, node_id, seed_index, "Seed", NumberInput::default().int().min(0.), true);

	vec![LayoutGroup::Row { widgets: spacing }, LayoutGroup::Row { widgets: seed }]
}

/// Fill Node Widgets LayoutGroup
pub(crate) fn fill_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let fill_index = 1;
	let backup_color_index = 2;
	let backup_gradient_index = 3;

	let mut widgets_first_row = start_widgets(document_node, node_id, fill_index, "Fill", FrontendGraphDataType::General, true);

	let (fill, backup_color, backup_gradient) = if let (Some(TaggedValue::Fill(fill)), Some(&TaggedValue::OptionalColor(backup_color)), Some(TaggedValue::Gradient(backup_gradient))) = (
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
		ColorButton::default()
			.value(fill.clone().into())
			.on_update(move |x: &ColorButton| {
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
			RadioEntryData::new("linear")
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
			RadioEntryData::new("radial")
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

pub fn stroke_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let color_index = 1;
	let weight_index = 2;
	let dash_lengths_index = 3;
	let dash_offset_index = 4;
	let line_cap_index = 5;
	let line_join_index = 6;
	let miter_limit_index = 7;

	let color = color_widget(document_node, node_id, color_index, "Color", ColorButton::default(), true);
	let weight = number_widget(document_node, node_id, weight_index, "Weight", NumberInput::default().unit(" px").min(0.), true);

	let dash_lengths_val = match &document_node.inputs[dash_lengths_index].as_value() {
		Some(TaggedValue::VecF64(x)) => x,
		_ => &vec![],
	};
	let dash_lengths = vec_f64_input(document_node, node_id, dash_lengths_index, "Dash Lengths", TextInput::default().centered(true), true);
	let number_input = NumberInput::default().unit(" px").disabled(dash_lengths_val.is_empty());
	let dash_offset = number_widget(document_node, node_id, dash_offset_index, "Dash Offset", number_input, true);
	let line_cap = line_cap_widget(document_node, node_id, line_cap_index, "Line Cap", true);
	let line_join = line_join_widget(document_node, node_id, line_join_index, "Line Join", true);
	let line_join_val = match &document_node.inputs[line_join_index].as_value() {
		Some(TaggedValue::LineJoin(x)) => x,
		_ => &LineJoin::Miter,
	};
	let number_input = NumberInput::default().min(0.).disabled(line_join_val != &LineJoin::Miter);
	let miter_limit = number_widget(document_node, node_id, miter_limit_index, "Miter Limit", number_input, true);

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

pub fn offset_path_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let distance_index = 1;
	let line_join_index = 2;
	let miter_limit_index = 3;

	let number_input = NumberInput::default().unit(" px");
	let distance = number_widget(document_node, node_id, distance_index, "Offset", number_input, true);

	let line_join = line_join_widget(document_node, node_id, line_join_index, "Line Join", true);
	let line_join_val = match &document_node.inputs[line_join_index].as_value() {
		Some(TaggedValue::LineJoin(x)) => x,
		_ => &LineJoin::Miter,
	};

	let number_input = NumberInput::default().min(0.).disabled(line_join_val != &LineJoin::Miter);
	let miter_limit = number_widget(document_node, node_id, miter_limit_index, "Miter Limit", number_input, true);

	vec![LayoutGroup::Row { widgets: distance }, line_join, LayoutGroup::Row { widgets: miter_limit }]
}

pub(crate) fn artboard_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let location = vec2_widget(document_node, node_id, 2, "Location", "X", "Y", " px", None, add_blank_assist);
	let dimensions = vec2_widget(document_node, node_id, 3, "Dimensions", "W", "H", " px", None, add_blank_assist);
	let background = color_widget(document_node, node_id, 4, "Background", ColorButton::default().allow_none(false), true);
	let clip = bool_widget(document_node, node_id, 5, "Clip", CheckboxInput::default(), true);

	let clip_row = LayoutGroup::Row { widgets: clip };

	vec![location, dimensions, background, clip_row]
}
