#![allow(clippy::too_many_arguments)]

use super::document_node_types::{NodePropertiesContext, IMAGINATE_NODE};
use super::utility_types::FrontendGraphDataType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::imaginate_input::{ImaginateSamplingMethod, ImaginateServerStatus, ImaginateStatus};
use graphene_core::memo::IORecord;
use graphene_core::raster::{
	BlendMode, CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, ImageFrame, LuminanceCalculation, NoiseType, RedGreenBlue, RedGreenBlueAlpha, RelativeAbsolute,
	SelectiveColorChoice,
};
use graphene_core::text::Font;
use graphene_core::vector::misc::CentroidType;
use graphene_core::vector::style::{GradientType, LineCap, LineJoin};
use graphene_std::vector::style::{Fill, FillChoice};

use glam::{DAffine2, DVec2, IVec2, UVec2};
use graphene_std::transform::Footprint;
use graphene_std::vector::misc::BooleanOperation;

pub fn string_properties(text: impl Into<String>) -> Vec<LayoutGroup> {
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
	DocumentMessage::StartTransaction.into()
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
		log::warn!(
			"A widget named '{name}' for node {} (alias '{}') failed to be built because its node's input index {index} is invalid.",
			document_node.name,
			document_node.alias
		);
		return vec![];
	};
	let mut widgets = vec![expose_widget(node_id, index, data_type, input.is_exposed()), TextLabel::new(name).widget_holder()];
	if blank_assist {
		add_blank_assist(&mut widgets);
	}

	widgets
}

fn text_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	if let Some(TaggedValue::String(x)) = &document_node.inputs[index].as_non_exposed_value() {
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

	if let Some(TaggedValue::String(x)) = &document_node.inputs[index].as_non_exposed_value() {
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

fn bool_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	if let Some(&TaggedValue::Bool(x)) = &document_node.inputs[index].as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(x)
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

	if let Some(&TaggedValue::Footprint(footprint)) = &document_node.inputs[index].as_non_exposed_value() {
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

	if let Some(&TaggedValue::DVec2(dvec2)) = document_node.inputs[index].as_non_exposed_value() {
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
	} else if let Some(&TaggedValue::IVec2(ivec2)) = document_node.inputs[index].as_non_exposed_value() {
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
	} else if let Some(&TaggedValue::UVec2(uvec2)) = document_node.inputs[index].as_non_exposed_value() {
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

	LayoutGroup::Row { widgets }
}

fn vec_f64_input(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, text_props: TextInput, blank_assist: bool) -> Vec<WidgetHolder> {
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

	if let Some(TaggedValue::VecF64(x)) = &document_node.inputs[index].as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			text_props
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

	if let Some(TaggedValue::VecDVec2(x)) = &document_node.inputs[index].as_non_exposed_value() {
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

	if let Some(TaggedValue::Font(font)) = &document_node.inputs[index].as_non_exposed_value() {
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

	if let Some(&TaggedValue::F64(x)) = document_node.inputs[index].as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	} else if let Some(&TaggedValue::U32(x)) = document_node.inputs[index].as_non_exposed_value() {
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::U32((x.value.unwrap()) as u32), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	}
	widgets
}

// TODO: Generalize this instead of using a separate function per dropdown menu enum
fn color_channel(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let Some(&TaggedValue::RedGreenBlue(mode)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::RedGreenBlueAlpha(mode)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::NoiseType(noise_type)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::FractalType(fractal_type)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::CellularDistanceFunction(cellular_distance_function)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::CellularReturnType(cellular_return_type)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::DomainWarpType(domain_warp_type)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::BlendMode(blend_mode)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::LuminanceCalculation(calculation)) = &document_node.inputs[index].as_non_exposed_value() {
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

	if let Some(&TaggedValue::BooleanOperation(calculation)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::LineCap(line_cap)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::LineJoin(line_join)) = &document_node.inputs[index].as_non_exposed_value() {
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

fn color_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, color_props: ColorButton, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	// Return early with just the label if the input is exposed to the graph, meaning we don't want to show the color picker widget in the Properties panel
	let NodeInput::Value { tagged_value, exposed: false } = &document_node.inputs[index] else {
		return LayoutGroup::Row { widgets };
	};

	widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

	match &**tagged_value {
		TaggedValue::Color(color) => widgets.push(
			color_props
				.value(FillChoice::Solid(*color))
				.on_update(update_value(|x: &ColorButton| TaggedValue::Color(x.value.as_solid().unwrap_or_default()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		),
		TaggedValue::OptionalColor(color) => widgets.push(
			color_props
				.value(match color {
					Some(color) => FillChoice::Solid(*color),
					None => FillChoice::None,
				})
				.on_update(update_value(|x: &ColorButton| TaggedValue::OptionalColor(x.value.as_solid()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		),
		TaggedValue::GradientStops(ref x) => widgets.push(
			color_props
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

	if let Some(TaggedValue::Curve(curve)) = &document_node.inputs[index].as_non_exposed_value() {
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
	if let Some(&TaggedValue::CentroidType(centroid_type)) = &document_node.inputs[index].as_non_exposed_value() {
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

pub fn levels_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let input_shadows = number_widget(document_node, node_id, 1, "Shadows", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);
	let input_midtones = number_widget(document_node, node_id, 2, "Midtones", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);
	let input_highlights = number_widget(document_node, node_id, 3, "Highlights", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);
	let output_minimums = number_widget(document_node, node_id, 4, "Output Minimums", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);
	let output_maximums = number_widget(document_node, node_id, 5, "Output Maximums", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);

	vec![
		LayoutGroup::Row { widgets: input_shadows },
		LayoutGroup::Row { widgets: input_midtones },
		LayoutGroup::Row { widgets: input_highlights },
		LayoutGroup::Row { widgets: output_minimums },
		LayoutGroup::Row { widgets: output_maximums },
	]
}

pub fn black_and_white_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	const MIN: f64 = -200.;
	const MAX: f64 = 300.;
	// TODO: Add tint color (blended above using the "Color" blend mode)
	let tint = color_widget(document_node, node_id, 1, "Tint", ColorButton::default(), true);
	let r_weight = number_widget(document_node, node_id, 2, "Reds", NumberInput::default().mode_range().min(MIN).max(MAX).unit("%"), true);
	let y_weight = number_widget(document_node, node_id, 3, "Yellows", NumberInput::default().mode_range().min(MIN).max(MAX).unit("%"), true);
	let g_weight = number_widget(document_node, node_id, 4, "Greens", NumberInput::default().mode_range().min(MIN).max(MAX).unit("%"), true);
	let c_weight = number_widget(document_node, node_id, 5, "Cyans", NumberInput::default().mode_range().min(MIN).max(MAX).unit("%"), true);
	let b_weight = number_widget(document_node, node_id, 6, "Blues", NumberInput::default().mode_range().min(MIN).max(MAX).unit("%"), true);
	let m_weight = number_widget(document_node, node_id, 7, "Magentas", NumberInput::default().mode_range().min(MIN).max(MAX).unit("%"), true);

	vec![
		tint,
		LayoutGroup::Row { widgets: r_weight },
		LayoutGroup::Row { widgets: y_weight },
		LayoutGroup::Row { widgets: g_weight },
		LayoutGroup::Row { widgets: c_weight },
		LayoutGroup::Row { widgets: b_weight },
		LayoutGroup::Row { widgets: m_weight },
	]
}

pub fn blend_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let backdrop = color_widget(document_node, node_id, 1, "Backdrop", ColorButton::default(), true);
	let blend_mode = blend_mode(document_node, node_id, 2, "Blend Mode", true);
	let opacity = number_widget(document_node, node_id, 3, "Opacity", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);

	vec![backdrop, blend_mode, LayoutGroup::Row { widgets: opacity }]
}

pub fn number_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 0, "Number", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn vector2_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let x = number_widget(document_node, node_id, 1, "X", NumberInput::default(), true);
	let y = number_widget(document_node, node_id, 2, "Y", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets: x }, LayoutGroup::Row { widgets: y }]
}

pub fn boolean_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = bool_widget(document_node, node_id, 0, "Bool", true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn color_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	vec![color_widget(document_node, node_id, 0, "Color", ColorButton::default(), true)]
}

pub fn load_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let url = text_widget(document_node, node_id, 1, "Url", true);

	vec![LayoutGroup::Row { widgets: url }]
}

pub fn output_properties(_document_node: &DocumentNode, _node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let label = TextLabel::new("Graphics fed into the Output are drawn in the viewport").widget_holder();

	vec![LayoutGroup::Row { widgets: vec![label] }]
}

pub fn mask_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let mask = color_widget(document_node, node_id, 1, "Stencil", ColorButton::default(), true);

	vec![mask]
}

pub fn color_channel_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	vec![color_channel(document_node, node_id, 0, "Channel", true)]
}

pub fn luminance_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let luminance_calc = luminance_calculation(document_node, node_id, 1, "Luminance Calc", true);

	vec![luminance_calc]
}

pub fn insert_channel_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let color_channel = color_channel(document_node, node_id, 2, "Into", true);

	vec![color_channel]
}

pub fn extract_channel_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let color_channel = rgba_channel(document_node, node_id, 1, "From", true);

	vec![color_channel]
}

// Noise Type is commented out for now as there is only one type of noise (White Noise).
// As soon as there are more types of noise, this should be uncommented.
pub fn noise_pattern_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	// Get the current values of the inputs of interest so they can set whether certain inputs are disabled based on various conditions.
	let current_noise_type = match &document_node.inputs[4].as_value() {
		Some(&TaggedValue::NoiseType(noise_type)) => Some(noise_type),
		_ => None,
	};
	let current_domain_warp_type = match &document_node.inputs[5].as_value() {
		Some(&TaggedValue::DomainWarpType(domain_warp_type)) => Some(domain_warp_type),
		_ => None,
	};
	let current_fractal_type = match &document_node.inputs[7].as_value() {
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
	let dimensions = vec2_widget(document_node, node_id, 1, "Dimensions", "W", "H", "px", Some(1.), add_blank_assist);
	let seed = number_widget(document_node, node_id, 2, "Seed", NumberInput::default().min(0.).is_integer(true), true);
	let scale = number_widget(document_node, node_id, 3, "Scale", NumberInput::default().min(0.).disabled(!coherent_noise_active), true);
	let noise_type_row = noise_type(document_node, node_id, 4, "Noise Type", true);

	// Domain Warp
	let domain_warp_type_row = domain_warp_type(document_node, node_id, 5, "Domain Warp Type", true, !coherent_noise_active);
	let domain_warp_amplitude = number_widget(
		document_node,
		node_id,
		6,
		"Domain Warp Amplitude",
		NumberInput::default().min(0.).disabled(!coherent_noise_active || !domain_warp_active),
		true,
	);

	// Fractal
	let fractal_type_row = fractal_type(document_node, node_id, 7, "Fractal Type", true, !coherent_noise_active);
	let fractal_octaves = number_widget(
		document_node,
		node_id,
		8,
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
		9,
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
		10,
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
		11,
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
		12,
		"Fractal Ping Pong Strength",
		NumberInput::default()
			.mode_range()
			.min(0.)
			.range_max(Some(10.))
			.disabled(!ping_pong_active || !coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
		true,
	);

	// Cellular
	let cellular_distance_function_row = cellular_distance_function(document_node, node_id, 13, "Cellular Distance Function", true, !coherent_noise_active || !cellular_noise_active);
	let cellular_return_type = cellular_return_type(document_node, node_id, 14, "Cellular Return Type", true, !coherent_noise_active || !cellular_noise_active);
	let cellular_jitter = number_widget(
		document_node,
		node_id,
		15,
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
		dimensions,
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

pub fn adjust_hsl_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let hue_shift = number_widget(document_node, node_id, 1, "Hue Shift", NumberInput::default().min(-180.).max(180.).unit("°"), true);
	let saturation_shift = number_widget(document_node, node_id, 2, "Saturation Shift", NumberInput::default().mode_range().min(-100.).max(100.).unit("%"), true);
	let lightness_shift = number_widget(document_node, node_id, 3, "Lightness Shift", NumberInput::default().mode_range().min(-100.).max(100.).unit("%"), true);

	vec![
		LayoutGroup::Row { widgets: hue_shift },
		LayoutGroup::Row { widgets: saturation_shift },
		LayoutGroup::Row { widgets: lightness_shift },
	]
}

pub fn brightness_contrast_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let brightness = number_widget(document_node, node_id, 1, "Brightness", NumberInput::default().min(-150.).max(150.), true);
	let contrast = number_widget(document_node, node_id, 2, "Contrast", NumberInput::default().min(-100.).max(100.), true);
	let use_legacy = bool_widget(document_node, node_id, 3, "Use Legacy", true);

	vec![
		LayoutGroup::Row { widgets: brightness },
		LayoutGroup::Row { widgets: contrast },
		LayoutGroup::Row { widgets: use_legacy },
	]
}

pub fn curves_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let curves = curves_widget(document_node, node_id, 1, "Curve", true);

	vec![curves]
}

pub fn _blur_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let radius = number_widget(document_node, node_id, 1, "Radius", NumberInput::default().min(0.).max(20.).int(), true);
	let sigma = number_widget(document_node, node_id, 2, "Sigma", NumberInput::default().min(0.).max(10000.), true);

	vec![LayoutGroup::Row { widgets: radius }, LayoutGroup::Row { widgets: sigma }]
}

pub fn adjust_threshold_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let thereshold_min = number_widget(document_node, node_id, 1, "Min Luminance", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);
	let thereshold_max = number_widget(document_node, node_id, 2, "Max Luminance", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);
	let luminance_calc = luminance_calculation(document_node, node_id, 3, "Luminance Calc", true);

	vec![LayoutGroup::Row { widgets: thereshold_min }, LayoutGroup::Row { widgets: thereshold_max }, luminance_calc]
}

pub fn adjust_vibrance_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let vibrance = number_widget(document_node, node_id, 1, "Vibrance", NumberInput::default().mode_range().min(-100.).max(100.).unit("%"), true);

	vec![LayoutGroup::Row { widgets: vibrance }]
}

pub fn adjust_channel_mixer_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	// Monochrome
	let monochrome_index = 1;
	let monochrome = bool_widget(document_node, node_id, monochrome_index, "Monochrome", true);
	let is_monochrome = if let Some(&TaggedValue::Bool(monochrome_choice)) = &document_node.inputs[monochrome_index].as_value() {
		monochrome_choice
	} else {
		false
	};

	// Output channel choice
	let output_channel_index = 18;
	let mut output_channel = vec![TextLabel::new("Output Channel").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];
	add_blank_assist(&mut output_channel);
	if let Some(&TaggedValue::RedGreenBlue(choice)) = &document_node.inputs[output_channel_index].as_non_exposed_value() {
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

pub fn adjust_selective_color_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	// Colors choice
	let colors_index = 38;
	let mut colors = vec![TextLabel::new("Colors").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];
	add_blank_assist(&mut colors);
	if let Some(&TaggedValue::SelectiveColorChoice(choice)) = &document_node.inputs[colors_index].as_non_exposed_value() {
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
	};
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
	if let Some(&TaggedValue::RelativeAbsolute(relative_or_absolute)) = &document_node.inputs[mode_index].as_non_exposed_value() {
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
pub fn _gpu_map_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let map = text_widget(document_node, node_id, 1, "Map", true);

	vec![LayoutGroup::Row { widgets: map }]
}

pub fn opacity_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let gamma = number_widget(document_node, node_id, 1, "Factor", NumberInput::default().mode_range().min(0.).max(100.).unit("%"), true);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn blend_mode_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	vec![blend_mode(document_node, node_id, 1, "Blend Mode", true)]
}

pub fn blend_mode_value_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	vec![blend_mode(document_node, node_id, 0, "Blend Mode", true)]
}

pub fn posterize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_widget(document_node, node_id, 1, "Levels", NumberInput::default().min(2.).max(255.).int(), true);

	vec![LayoutGroup::Row { widgets: value }]
}

#[cfg(feature = "quantization")]
pub fn quantize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_widget(document_node, node_id, 1, "Levels", NumberInput::default().min(1.).max(1000.).int(), true);
	let index = number_widget(document_node, node_id, 1, "Fit Fn Index", NumberInput::default().min(0.).max(2.).int(), true);

	vec![LayoutGroup::Row { widgets: value }, LayoutGroup::Row { widgets: index }]
}
pub fn exposure_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
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

pub fn add_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Addend", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn subtract_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Subtrahend", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn divide_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Divisor", NumberInput::new(Some(1.)), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn multiply_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Multiplicand", NumberInput::new(Some(1.)), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn exponent_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Power", NumberInput::new(Some(2.)), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn log_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Base", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn max_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Maximum", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn min_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Minimum", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn eq_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Equals", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn modulo_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = number_widget(document_node, node_id, 1, "Modulo", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets }]
}

pub fn circle_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	vec![LayoutGroup::Row {
		widgets: number_widget(document_node, node_id, 1, "Radius", NumberInput::default(), true),
	}]
}

pub fn ellipse_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| {
		let widgets = number_widget(document_node, node_id, index, name, NumberInput::default(), true);

		LayoutGroup::Row { widgets }
	};
	vec![operand("Radius X", 1), operand("Radius Y", 2)]
}

pub fn rectangle_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
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

	if let Some(&TaggedValue::Bool(is_individual)) = &document_node.inputs[corner_rounding_type_index].as_non_exposed_value() {
		// Values
		let uniform_val = match document_node.inputs[corner_radius_index].as_non_exposed_value() {
			Some(TaggedValue::F64(x)) => *x,
			Some(TaggedValue::F64Array4(x)) => x[0],
			_ => 0.,
		};
		let individual_val = match document_node.inputs[corner_radius_index].as_non_exposed_value() {
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
	let clamped = bool_widget(document_node, node_id, clamped_index, "Clamped", true);

	vec![
		LayoutGroup::Row { widgets: size_x },
		LayoutGroup::Row { widgets: size_y },
		LayoutGroup::Row { widgets: corner_radius_row_1 },
		LayoutGroup::Row { widgets: corner_radius_row_2 },
		LayoutGroup::Row { widgets: clamped },
	]
}

pub fn regular_polygon_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let points = number_widget(document_node, node_id, 1, "Points", NumberInput::default().min(3.), true);
	let radius = number_widget(document_node, node_id, 2, "Radius", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets: points }, LayoutGroup::Row { widgets: radius }]
}

pub fn star_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let points = number_widget(document_node, node_id, 1, "Points", NumberInput::default().min(2.), true);
	let radius = number_widget(document_node, node_id, 2, "Radius", NumberInput::default(), true);
	let inner_radius = number_widget(document_node, node_id, 3, "Inner Radius", NumberInput::default(), true);

	vec![LayoutGroup::Row { widgets: points }, LayoutGroup::Row { widgets: radius }, LayoutGroup::Row { widgets: inner_radius }]
}

pub fn line_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| vec2_widget(document_node, node_id, index, name, "X", "Y", "px", None, add_blank_assist);
	vec![operand("Start", 1), operand("End", 2)]
}
pub fn spline_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	vec![LayoutGroup::Row {
		widgets: vec_dvec2_input(document_node, node_id, 1, "Points", TextInput::default().centered(true), true),
	}]
}

pub fn logic_operator_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let widgets = bool_widget(document_node, node_id, 0, "Operand B", true);
	vec![LayoutGroup::Row { widgets }]
}

pub fn transform_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let translation = vec2_widget(document_node, node_id, 1, "Translation", "X", "Y", " px", None, add_blank_assist);

	let rotation = {
		let index = 2;

		let mut widgets = start_widgets(document_node, node_id, index, "Rotation", FrontendGraphDataType::Number, true);

		if let Some(&TaggedValue::F64(val)) = document_node.inputs[index].as_non_exposed_value() {
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
pub fn rasterize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	footprint_widget(document_node, node_id, 1)
}

pub fn text_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let text = text_area_widget(document_node, node_id, 1, "Text", true);
	let (font, style) = font_inputs(document_node, node_id, 2, "Font", true);
	let size = number_widget(document_node, node_id, 3, "Size", NumberInput::default().unit(" px").min(1.), true);

	let mut result = vec![LayoutGroup::Row { widgets: text }, LayoutGroup::Row { widgets: font }];
	if let Some(style) = style {
		result.push(LayoutGroup::Row { widgets: style });
	}
	result.push(LayoutGroup::Row { widgets: size });
	result
}

pub fn imaginate_properties(document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let imaginate_node = [context.nested_path, &[node_id]].concat();

	let resolve_input = |name: &str| IMAGINATE_NODE.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"));
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
						move |_| {
							controller.trigger_regenerate();
							DocumentMessage::ImaginateGenerate.into()
						}
					})
					.widget_holder(),
				Separator::new(SeparatorType::Related).widget_holder(),
				TextButton::new("Clear")
					.tooltip("Remove generated image from the layer frame")
					.disabled(!matches!(imaginate_status, ImaginateStatus::ReadyDone))
					.on_update({
						let controller = controller.clone();
						move |_| {
							controller.set_status(ImaginateStatus::Ready);
							DocumentMessage::ImaginateGenerate.into()
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

		if let Some(&TaggedValue::F64(seed)) = &document_node.inputs[seed_index].as_non_exposed_value() {
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
			context.document_network,
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

		let round = |x: DVec2| {
			let (x, y) = pick_safe_imaginate_resolution(x.into());
			DVec2::new(x as f64, y as f64)
		};

		if let Some(&TaggedValue::OptionalDVec2(vec2)) = &document_node.inputs[resolution_index].as_non_exposed_value() {
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

		if let Some(&TaggedValue::ImaginateSamplingMethod(sampling_method)) = &document_node.inputs[sampling_method_index].as_non_exposed_value() {
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
		let widgets = bool_widget(document_node, node_id, base_img_index, "Adapt Input Image", true);
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
		let widgets = bool_widget(document_node, node_id, faces_index, "Improve Faces", true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Postprocess human (or human-like) faces to look subtly less distorted.\n\
			\n\
			This filter can be used on its own by enabling 'Adapt Input Image' and setting 'Sampling Steps' to 0.",
		)
	};
	let tiling = {
		let widgets = bool_widget(document_node, node_id, tiling_index, "Tiling", true);
		LayoutGroup::Row { widgets }.with_tooltip("Generate the image so its edges loop seamlessly to make repeatable patterns or textures")
	};
	layout.extend_from_slice(&[improve_faces, tiling]);

	layout
}

fn unknown_node_properties(document_node: &DocumentNode) -> Vec<LayoutGroup> {
	string_properties(format!("Node '{}' cannot be found in library", document_node.name))
}

pub fn node_no_properties(document_node: &DocumentNode, _node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	string_properties(if document_node.is_layer { "Layer has no properties" } else { "Node has no properties" })
}

pub fn index_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let index = number_widget(document_node, node_id, 1, "Index", NumberInput::default().min(0.), true);

	vec![LayoutGroup::Row { widgets: index }]
}

pub fn generate_node_properties(document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext) -> LayoutGroup {
	let name = document_node.name.clone();
	let layout = match super::document_node_types::resolve_document_node_type(&name) {
		Some(document_node_type) => (document_node_type.properties)(document_node, node_id, context),
		None => unknown_node_properties(document_node),
	};
	LayoutGroup::Section {
		name,
		visible: document_node.visible,
		id: node_id.0,
		layout,
	}
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
	let weight = number_widget(document_node, node_id, weight_index, "Weight", NumberInput::default().unit("px").min(0.), true);

	let dash_lengths_val = match &document_node.inputs[dash_lengths_index].as_value() {
		Some(TaggedValue::VecF64(x)) => x,
		_ => &vec![],
	};
	let dash_lengths = vec_f64_input(document_node, node_id, dash_lengths_index, "Dash Lengths", TextInput::default().centered(true), true);
	let number_input = NumberInput::default().unit("px").disabled(dash_lengths_val.is_empty());
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

pub fn repeat_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let direction = vec2_widget(document_node, node_id, 1, "Direction", "X", "Y", " px", None, add_blank_assist);
	let angle = number_widget(document_node, node_id, 2, "Angle", NumberInput::default().unit("°"), true);
	let instances = number_widget(document_node, node_id, 3, "Instances", NumberInput::default().min(1.).is_integer(true), true);

	vec![direction, LayoutGroup::Row { widgets: angle }, LayoutGroup::Row { widgets: instances }]
}

pub fn circular_repeat_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let angle_offset = number_widget(document_node, node_id, 1, "Angle Offset", NumberInput::default().unit("°"), true);
	let radius = number_widget(document_node, node_id, 2, "Radius", NumberInput::default(), true); // TODO: What units?
	let instances = number_widget(document_node, node_id, 3, "Instances", NumberInput::default().min(1.).is_integer(true), true);

	vec![
		LayoutGroup::Row { widgets: angle_offset },
		LayoutGroup::Row { widgets: radius },
		LayoutGroup::Row { widgets: instances },
	]
}

pub fn binary_boolean_operation_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let lower_vector_data = vector_widget(document_node, node_id, 1, "Lower Vector Data", true);
	let operation = boolean_operation_radio_buttons(document_node, node_id, 2, "Operation", true);

	vec![LayoutGroup::Row { widgets: lower_vector_data }, operation]
}

pub fn boolean_operation_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let vector_data = vector_widget(document_node, node_id, 1, "Vector Data", true);
	let operation = boolean_operation_radio_buttons(document_node, node_id, 2, "Operation", true);

	vec![LayoutGroup::Row { widgets: vector_data }, operation]
}

pub fn copy_to_points_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let instance = vector_widget(document_node, node_id, 1, "Instance", true);

	let random_scale_min = number_widget(
		document_node,
		node_id,
		2,
		"Random Scale Min",
		NumberInput::default().min(0.).mode_range().range_min(Some(0.)).range_max(Some(2.)).unit("x"),
		true,
	);
	let random_scale_max = number_widget(
		document_node,
		node_id,
		3,
		"Random Scale Max",
		NumberInput::default().min(0.).mode_range().range_min(Some(0.)).range_max(Some(2.)).unit("x"),
		true,
	);
	let random_scale_bias = number_widget(
		document_node,
		node_id,
		4,
		"Random Scale Bias",
		NumberInput::default().mode_range().range_min(Some(-50.)).range_max(Some(50.)),
		true,
	);

	let random_rotation = number_widget(document_node, node_id, 5, "Random Rotation", NumberInput::default().min(0.).max(360.).mode_range().unit("°"), true);

	vec![
		LayoutGroup::Row { widgets: instance }.with_tooltip("Artwork to be copied and placed at each point"),
		LayoutGroup::Row { widgets: random_scale_min }.with_tooltip("Minimum range of randomized sizes given to each instance"),
		LayoutGroup::Row { widgets: random_scale_max }.with_tooltip("Maximum range of randomized sizes given to each instance"),
		LayoutGroup::Row { widgets: random_scale_bias }
			.with_tooltip("Bias for the probability distribution of randomized sizes (0 is uniform, negatives favor more of small sizes, positives favor more of large sizes)"),
		LayoutGroup::Row { widgets: random_rotation }.with_tooltip("Range of randomized angles given to each instance, in degrees ranging from furthest clockwise to counterclockwise"),
	]
}

pub fn sample_points_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let spacing = number_widget(document_node, node_id, 1, "Spacing", NumberInput::default().min(1.).unit(" px"), true);
	let start_offset = number_widget(document_node, node_id, 2, "Start Offset", NumberInput::default().min(0.).unit(" px"), true);
	let stop_offset = number_widget(document_node, node_id, 3, "Stop Offset", NumberInput::default().min(0.).unit(" px"), true);
	let adaptive_spacing = bool_widget(document_node, node_id, 4, "Adaptive Spacing", true);

	vec![
		LayoutGroup::Row { widgets: spacing }.with_tooltip("Distance between each instance (exact if 'Adaptive Spacing' is disabled, approximate if enabled)"),
		LayoutGroup::Row { widgets: start_offset }.with_tooltip("Exclude some distance from the start of the path before the first instance"),
		LayoutGroup::Row { widgets: stop_offset }.with_tooltip("Exclude some distance from the end of the path after the last instance"),
		LayoutGroup::Row { widgets: adaptive_spacing }.with_tooltip("Round 'Spacing' to a nearby value that divides into the path length evenly"),
	]
}

pub fn poisson_disk_points_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let spacing = number_widget(
		document_node,
		node_id,
		1,
		"Separation Disk Diameter",
		NumberInput::default().min(0.01).mode_range().range_min(Some(1.)).range_max(Some(100.)),
		true,
	);

	vec![LayoutGroup::Row { widgets: spacing }]
}

pub fn morph_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let start_index = number_widget(document_node, node_id, 2, "Start Index", NumberInput::default().min(0.), true);
	let time = number_widget(document_node, node_id, 3, "Time", NumberInput::default().min(0.).max(1.).mode_range(), true);

	vec![
		LayoutGroup::Row { widgets: start_index }.with_tooltip("The index of point on the target that morphs to the first point of the source"),
		LayoutGroup::Row { widgets: time }.with_tooltip("Linear time of transition - 0. is source, 1. is target"),
	]
}

/// Fill Node Widgets LayoutGroup
pub fn fill_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
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
		add_blank_assist(&mut row);

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

	if let Fill::Gradient(gradient) = fill {
		let mut row = vec![TextLabel::new("").widget_holder()];
		add_blank_assist(&mut row);

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

pub fn artboard_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let location = vec2_widget(document_node, node_id, 2, "Location", "X", "Y", " px", None, add_blank_assist);
	let dimensions = vec2_widget(document_node, node_id, 3, "Dimensions", "W", "H", " px", None, add_blank_assist);
	let background = color_widget(document_node, node_id, 4, "Background", ColorButton::default().allow_none(false), true);
	let clip = bool_widget(document_node, node_id, 5, "Clip", true);

	let clip_row = LayoutGroup::Row { widgets: clip };

	vec![location, dimensions, background, clip_row]
}

pub fn color_fill_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let color = color_widget(document_node, node_id, 1, "Color", ColorButton::default(), true);
	vec![color]
}

pub fn color_overlay_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let color = color_widget(document_node, node_id, 1, "Color", ColorButton::default(), true);
	let blend_mode = blend_mode(document_node, node_id, 2, "Blend Mode", true);
	let opacity = number_widget(document_node, node_id, 3, "Opacity", NumberInput::default().percentage(), true);

	vec![color, blend_mode, LayoutGroup::Row { widgets: opacity }]
}

pub fn image_color_palette(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let size = number_widget(document_node, node_id, 1, "Max Size", NumberInput::default().int().min(1.).max(28.), true);

	vec![LayoutGroup::Row { widgets: size }]
}

pub fn centroid_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let centroid_type = centroid_widget(document_node, node_id, 1);

	vec![centroid_type]
}
