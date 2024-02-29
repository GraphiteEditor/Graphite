#![allow(clippy::too_many_arguments)]

use super::document_node_types::NodePropertiesContext;
use super::FrontendGraphDataType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::imaginate_input::{ImaginateSamplingMethod, ImaginateServerStatus, ImaginateStatus};
use graphene_core::memo::IORecord;
use graphene_core::raster::{
	BlendMode, CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, ImageFrame, LuminanceCalculation, NoiseType, RedGreenBlue, RelativeAbsolute, SelectiveColorChoice,
};
use graphene_core::text::Font;
use graphene_core::vector::style::{FillType, GradientType, LineCap, LineJoin};

use glam::{DVec2, IVec2, UVec2};

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
	let input = document_node.inputs.get(index).expect("A widget failed to be built because its node's input index is invalid.");
	let mut widgets = vec![expose_widget(node_id, index, data_type, input.is_exposed()), TextLabel::new(name).widget_holder()];
	if blank_assist {
		add_blank_assist(&mut widgets);
	}

	widgets
}

fn text_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Text, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::String(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
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
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Text, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::String(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
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
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Boolean, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::Bool(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(*x)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	}
	widgets
}

fn vec2_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, x: &str, y: &str, unit: &str, min: Option<f64>, mut assist: impl FnMut(&mut Vec<WidgetHolder>)) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Vector, false);

	assist(&mut widgets);

	if let NodeInput::Value {
		tagged_value: TaggedValue::DVec2(dvec2),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(dvec2.x))
				.label(x)
				.unit(unit)
				.min(min.unwrap_or(-((1_u64 << std::f64::MANTISSA_DIGITS) as f64)))
				.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
				.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), dvec2.y)), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(dvec2.y))
				.label(y)
				.unit(unit)
				.min(min.unwrap_or(-((1_u64 << std::f64::MANTISSA_DIGITS) as f64)))
				.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
				.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(dvec2.x, input.value.unwrap())), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]);
	} else if let NodeInput::Value {
		tagged_value: TaggedValue::IVec2(ivec2),
		exposed: false,
	} = document_node.inputs[index]
	{
		let update_x = move |input: &NumberInput| TaggedValue::IVec2(IVec2::new(input.value.unwrap() as i32, ivec2.y));
		let update_y = move |input: &NumberInput| TaggedValue::IVec2(IVec2::new(ivec2.x, input.value.unwrap() as i32));
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(ivec2.x as f64))
				.int()
				.label(x)
				.unit(unit)
				.min(min.unwrap_or(-((1_u64 << std::f64::MANTISSA_DIGITS) as f64)))
				.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
				.on_update(update_value(update_x, node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(ivec2.y as f64))
				.int()
				.label(y)
				.unit(unit)
				.min(min.unwrap_or(-((1_u64 << std::f64::MANTISSA_DIGITS) as f64)))
				.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
				.on_update(update_value(update_y, node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]);
	} else if let NodeInput::Value {
		tagged_value: TaggedValue::UVec2(uvec2),
		exposed: false,
	} = document_node.inputs[index]
	{
		let update_x = move |input: &NumberInput| TaggedValue::UVec2(UVec2::new(input.value.unwrap() as u32, uvec2.y));
		let update_y = move |input: &NumberInput| TaggedValue::UVec2(UVec2::new(uvec2.x, input.value.unwrap() as u32));
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(uvec2.x as f64))
				.int()
				.label(x)
				.unit(unit)
				.min(min.unwrap_or(0.))
				.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
				.on_update(update_value(update_x, node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(uvec2.y as f64))
				.int()
				.label(y)
				.unit(unit)
				.min(min.unwrap_or(0.))
				.max((1_u64 << std::f64::MANTISSA_DIGITS) as f64)
				.on_update(update_value(update_y, node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		]);
	}

	LayoutGroup::Row { widgets }
}

fn vec_f64_input(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, text_props: TextInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Vector, blank_assist);

	let from_string = |string: &str| {
		string
			.split(&[',', ' '])
			.filter(|x| !x.is_empty())
			.map(str::parse::<f64>)
			.collect::<Result<Vec<_>, _>>()
			.ok()
			.map(TaggedValue::VecF64)
	};

	if let NodeInput::Value {
		tagged_value: TaggedValue::VecF64(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
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
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Vector, blank_assist);

	let from_string = |string: &str| {
		string
			.split(|c: char| !c.is_alphanumeric() && !matches!(c, '.' | '+' | '-'))
			.filter(|x| !x.is_empty())
			.map(|x| x.parse::<f64>().ok())
			.collect::<Option<Vec<_>>>()
			.map(|numbers| numbers.chunks_exact(2).map(|values| DVec2::new(values[0], values[1])).collect())
			.map(TaggedValue::VecDVec2)
	};

	if let NodeInput::Value {
		tagged_value: TaggedValue::VecDVec2(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
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

	if let NodeInput::Value {
		tagged_value: TaggedValue::Font(font),
		exposed: false,
	} = &document_node.inputs[index]
	{
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
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Vector, blank_assist);

	widgets.push(TextLabel::new("Vector data must be supplied through the graph").widget_holder());

	widgets
}

fn number_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, number_props: NumberInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::F64(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	} else if let NodeInput::Value {
		tagged_value: TaggedValue::U32(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::U32((x.value.unwrap()) as u32), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	} else if let NodeInput::Value {
		tagged_value: TaggedValue::F32(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F32((x.value.unwrap()) as f32), node_id, index))
				.on_commit(commit_value)
				.widget_holder(),
		])
	}
	widgets
}

//TODO Generalize this instead of using a separate function per dropdown menu enum
fn color_channel(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::RedGreenBlue(mode),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let calculation_modes = [RedGreenBlue::Red, RedGreenBlue::Green, RedGreenBlue::Blue];
		let mut entries = Vec::with_capacity(calculation_modes.len());
		for method in calculation_modes {
			entries.push(
				MenuListEntry::new(method.to_string())
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

// TODO Generalize this instead of using a separate function per dropdown menu enum
fn noise_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::NoiseType(noise_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = NoiseType::list()
			.iter()
			.map(|noise_type| {
				MenuListEntry::new(noise_type.to_string())
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

// TODO Generalize this instead of using a separate function per dropdown menu enum
fn fractal_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::FractalType(fractal_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = FractalType::list()
			.iter()
			.map(|fractal_type| {
				MenuListEntry::new(fractal_type.to_string())
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

// TODO Generalize this instead of using a separate function per dropdown menu enum
fn cellular_distance_function(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::CellularDistanceFunction(cellular_distance_function),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = CellularDistanceFunction::list()
			.iter()
			.map(|cellular_distance_function| {
				MenuListEntry::new(cellular_distance_function.to_string())
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

// TODO Generalize this instead of using a separate function per dropdown menu enum
fn cellular_return_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::CellularReturnType(cellular_return_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = CellularReturnType::list()
			.iter()
			.map(|cellular_return_type| {
				MenuListEntry::new(cellular_return_type.to_string())
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

// TODO Generalize this instead of using a separate function per dropdown menu enum
fn domain_warp_type(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool, disabled: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::DomainWarpType(domain_warp_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = DomainWarpType::list()
			.iter()
			.map(|domain_warp_type| {
				MenuListEntry::new(domain_warp_type.to_string())
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
	if let &NodeInput::Value {
		tagged_value: TaggedValue::BlendMode(blend_mode),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = BlendMode::list_svg_subset()
			.iter()
			.map(|category| {
				category
					.iter()
					.map(|blend_mode| {
						MenuListEntry::new(blend_mode.to_string())
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
	if let &NodeInput::Value {
		tagged_value: TaggedValue::LuminanceCalculation(calculation),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let calculation_modes = LuminanceCalculation::list();
		let mut entries = Vec::with_capacity(calculation_modes.len());
		for method in calculation_modes {
			entries.push(
				MenuListEntry::new(method.to_string())
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

fn line_cap_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::LineCap(line_cap),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = [("Butt", LineCap::Butt), ("Round", LineCap::Round), ("Square", LineCap::Square)]
			.into_iter()
			.map(|(name, val)| {
				RadioEntryData::new(name)
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
	if let &NodeInput::Value {
		tagged_value: TaggedValue::LineJoin(line_join),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = [("Miter", LineJoin::Miter), ("Bevel", LineJoin::Bevel), ("Round", LineJoin::Round)]
			.into_iter()
			.map(|(name, val)| {
				RadioEntryData::new(name)
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

fn fill_type_widget(document_node: &DocumentNode, node_id: NodeId, index: usize) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, "Fill Type", FrontendGraphDataType::General, true);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::FillType(fill_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = vec![
			RadioEntryData::new("Solid")
				.on_update(update_value(move |_| TaggedValue::FillType(FillType::Solid), node_id, index))
				.on_commit(commit_value),
			RadioEntryData::new("Gradient")
				.on_update(update_value(move |_| TaggedValue::FillType(FillType::Gradient), node_id, index))
				.on_commit(commit_value),
		];

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries)
				.selected_index(match fill_type {
					FillType::Solid => Some(0),
					FillType::Gradient => Some(1),
				})
				.widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }
}

fn gradient_type_widget(document_node: &DocumentNode, node_id: NodeId, index: usize) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, "Gradient Type", FrontendGraphDataType::General, true);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::GradientType(gradient_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = vec![
			RadioEntryData::new("Linear")
				.on_update(update_value(move |_| TaggedValue::GradientType(GradientType::Linear), node_id, index))
				.on_commit(commit_value),
			RadioEntryData::new("Radial")
				.on_update(update_value(move |_| TaggedValue::GradientType(GradientType::Radial), node_id, index))
				.on_commit(commit_value),
		];

		widgets.extend_from_slice(&[
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			RadioInput::new(entries).selected_index(Some(gradient_type as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }
}

fn gradient_row(row: &mut Vec<WidgetHolder>, positions: &Vec<(f64, Color)>, index: usize, node_id: NodeId, input_index: usize) {
	let label = TextLabel::new(format!("Gradient: {:.0}%", positions[index].0 * 100.)).tooltip("Adjustable by dragging the gradient stops in the viewport with the Gradient tool active");
	row.push(label.widget_holder());
	let on_update = {
		let positions = positions.clone();
		move |color_button: &ColorButton| {
			let mut new_positions = positions.clone();
			new_positions[index].1 = color_button.value.unwrap();
			TaggedValue::GradientPositions(new_positions)
		}
	};
	let color = ColorButton::new(Some(positions[index].1))
		.on_update(update_value(on_update, node_id, input_index))
		.on_commit(commit_value)
		.allow_none(false);
	add_blank_assist(row);
	row.push(Separator::new(SeparatorType::Unrelated).widget_holder());
	row.push(color.widget_holder());

	let mut skip_separator = false;
	// Remove button
	if positions.len() != index + 1 && index != 0 {
		let on_update = {
			let in_positions = positions.clone();
			move |_: &IconButton| {
				let mut new_positions = in_positions.clone();
				new_positions.remove(index);
				TaggedValue::GradientPositions(new_positions)
			}
		};

		skip_separator = true;
		row.push(Separator::new(SeparatorType::Related).widget_holder());
		row.push(
			IconButton::new("Remove", 16)
				.tooltip("Remove this gradient stop")
				.on_update(update_value(on_update, node_id, input_index))
				.on_commit(commit_value)
				.widget_holder(),
		);
	}
	// Add button
	if positions.len() != index + 1 {
		let on_update = {
			let positions = positions.clone();
			move |_: &IconButton| {
				let mut new_positions = positions.clone();

				// Blend linearly between the two colors.
				let get_color = |index: usize| match (new_positions[index].1, new_positions.get(index + 1).map(|x| x.1)) {
					(a, Some(b)) => Color::from_rgbaf32_unchecked((a.r() + b.r()) / 2., (a.g() + b.g()) / 2., (a.b() + b.b()) / 2., ((a.a() + b.a()) / 2.).clamp(0., 1.)),
					(_, None) => Color::WHITE,
				};
				let get_pos = |index: usize| (new_positions[index].0 + new_positions.get(index + 1).map(|v| v.0).unwrap_or(1.)) / 2.;

				new_positions.push((get_pos(index), get_color(index)));

				new_positions.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

				TaggedValue::GradientPositions(new_positions)
			}
		};

		if !skip_separator {
			row.push(Separator::new(SeparatorType::Related).widget_holder());
		}
		row.push(
			IconButton::new("Add", 16)
				.tooltip("Add a gradient stop after this")
				.on_update(update_value(on_update, node_id, input_index))
				.on_commit(commit_value)
				.widget_holder(),
		);
	}
}

fn gradient_positions(rows: &mut Vec<LayoutGroup>, document_node: &DocumentNode, name: &str, node_id: NodeId, input_index: usize) {
	let mut widgets = vec![expose_widget(node_id, input_index, FrontendGraphDataType::General, document_node.inputs[input_index].is_exposed())];
	if let NodeInput::Value {
		tagged_value: TaggedValue::GradientPositions(gradient_positions),
		exposed: false,
	} = &document_node.inputs[input_index]
	{
		for index in 0..gradient_positions.len() {
			gradient_row(&mut widgets, gradient_positions, index, node_id, input_index);

			let widgets = std::mem::take(&mut widgets);
			rows.push(LayoutGroup::Row { widgets });
		}
		let on_update = {
			let gradient_positions = gradient_positions.clone();
			move |_: &TextButton| {
				let mut new_positions = gradient_positions.clone();
				new_positions = new_positions.iter().map(|(distance, color)| (1. - distance, *color)).collect();
				new_positions.reverse();
				TaggedValue::GradientPositions(new_positions)
			}
		};
		let invert = TextButton::new("Invert")
			.icon(Some("Swap".into()))
			.tooltip("Reverse the order of each color stop")
			.on_update(update_value(on_update, node_id, input_index))
			.on_commit(commit_value)
			.widget_holder();

		if widgets.is_empty() {
			widgets.push(TextLabel::new("").widget_holder());
			add_blank_assist(&mut widgets);
		}
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(invert);

		rows.push(LayoutGroup::Row { widgets });
	} else {
		widgets.push(TextLabel::new(name).widget_holder());
		rows.push(LayoutGroup::Row { widgets })
	}
}

fn color_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, color_props: ColorButton, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Color, blank_assist);

	if let NodeInput::Value { tagged_value, exposed: false } = &document_node.inputs[index] {
		if let &TaggedValue::Color(x) = tagged_value {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				color_props
					.value(Some(x as Color))
					.on_update(update_value(|x: &ColorButton| TaggedValue::Color(x.value.unwrap()), node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
			])
		} else if let &TaggedValue::OptionalColor(x) = tagged_value {
			widgets.extend_from_slice(&[
				Separator::new(SeparatorType::Unrelated).widget_holder(),
				color_props
					.value(x)
					.on_update(update_value(|x: &ColorButton| TaggedValue::OptionalColor(x.value), node_id, index))
					.on_commit(commit_value)
					.widget_holder(),
			])
		}
	}
	LayoutGroup::Row { widgets }
}

fn curves_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::Curve(curve),
		exposed: false,
	} = &document_node.inputs[index]
	{
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
	let color_channel = color_channel(document_node, node_id, 1, "From", true);

	vec![color_channel]
}

// Noise Type is commented out for now as there is only one type of noise (White Noise).
// As soon as there are more types of noise, this should be uncommented.
pub fn noise_pattern_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	// Get the current values of the inputs of interest so they can set whether certain inputs are disabled based on various conditions.
	let current_noise_type = match &document_node.inputs[4] {
		NodeInput::Value {
			tagged_value: TaggedValue::NoiseType(noise_type),
			..
		} => Some(*noise_type),
		_ => None,
	};
	let current_domain_warp_type = match &document_node.inputs[5] {
		NodeInput::Value {
			tagged_value: TaggedValue::DomainWarpType(domain_warp_type),
			..
		} => Some(*domain_warp_type),
		_ => None,
	};
	let current_fractal_type = match &document_node.inputs[7] {
		NodeInput::Value {
			tagged_value: TaggedValue::FractalType(fractal_type),
			..
		} => Some(*fractal_type),
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
	let is_monochrome = if let &NodeInput::Value {
		tagged_value: TaggedValue::Bool(monochrome_choice),
		..
	} = &document_node.inputs[monochrome_index]
	{
		monochrome_choice
	} else {
		false
	};

	// Output channel choice
	let output_channel_index = 18;
	let mut output_channel = vec![TextLabel::new("Output Channel").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];
	add_blank_assist(&mut output_channel);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::RedGreenBlue(choice),
		exposed: false,
	} = &document_node.inputs[output_channel_index]
	{
		let entries = vec![
			RadioEntryData::new(RedGreenBlue::Red.to_string())
				.on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Red), node_id, output_channel_index))
				.on_commit(commit_value),
			RadioEntryData::new(RedGreenBlue::Green.to_string())
				.on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Green), node_id, output_channel_index))
				.on_commit(commit_value),
			RadioEntryData::new(RedGreenBlue::Blue.to_string())
				.on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Blue), node_id, output_channel_index))
				.on_commit(commit_value),
		];
		output_channel.extend([RadioInput::new(entries).selected_index(Some(choice as u32)).widget_holder()]);
	};
	let is_output_channel = if let &NodeInput::Value {
		tagged_value: TaggedValue::RedGreenBlue(choice),
		..
	} = &document_node.inputs[output_channel_index]
	{
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
	if let &NodeInput::Value {
		tagged_value: TaggedValue::SelectiveColorChoice(choice),
		exposed: false,
	} = &document_node.inputs[colors_index]
	{
		use SelectiveColorChoice::*;
		let entries = [[Reds, Yellows, Greens, Cyans, Blues, Magentas].as_slice(), [Whites, Neutrals, Blacks].as_slice()]
			.into_iter()
			.map(|section| {
				section
					.iter()
					.map(|choice| {
						MenuListEntry::new(choice.to_string())
							.on_update(update_value(move |_| TaggedValue::SelectiveColorChoice(*choice), node_id, colors_index))
							.on_commit(commit_value)
					})
					.collect()
			})
			.collect();
		colors.extend([DropdownInput::new(entries).selected_index(Some(choice as u32)).widget_holder()]);
	};
	let colors_choice_index = if let &NodeInput::Value {
		tagged_value: TaggedValue::SelectiveColorChoice(choice),
		..
	} = &document_node.inputs[colors_index]
	{
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
	if let &NodeInput::Value {
		tagged_value: TaggedValue::RelativeAbsolute(relative_or_absolute),
		exposed: false,
	} = &document_node.inputs[mode_index]
	{
		let entries = vec![
			RadioEntryData::new("Relative")
				.on_update(update_value(|_| TaggedValue::RelativeAbsolute(RelativeAbsolute::Relative), node_id, mode_index))
				.on_commit(commit_value),
			RadioEntryData::new("Absolute")
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
	let operand = |name: &str, index| {
		let widgets = number_widget(document_node, node_id, index, name, NumberInput::default(), true);

		LayoutGroup::Row { widgets }
	};
	vec![operand("Size X", 1), operand("Size Y", 2)]
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
	let translation_assist = |widgets: &mut Vec<WidgetHolder>| {
		let pivot_index = 5;
		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(pivot),
			exposed: false,
		} = document_node.inputs[pivot_index]
		{
			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
			widgets.push(
				PivotInput::new(pivot.into())
					.on_update(update_value(|pivot: &PivotInput| TaggedValue::DVec2(Into::<Option<DVec2>>::into(pivot.position).unwrap()), node_id, 5))
					.on_commit(commit_value)
					.widget_holder(),
			);
		} else {
			add_blank_assist(widgets);
		}
	};
	let translation = vec2_widget(document_node, node_id, 1, "Translation", "X", "Y", " px", None, translation_assist);

	let rotation = {
		let index = 2;

		let mut widgets = start_widgets(document_node, node_id, index, "Rotation", FrontendGraphDataType::Number, true);

		if let NodeInput::Value {
			tagged_value: TaggedValue::F64(val),
			exposed: false,
		} = document_node.inputs[index]
		{
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

pub fn node_section_font(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
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

	let resolve_input = |name: &str| {
		super::IMAGINATE_NODE
			.inputs
			.iter()
			.position(|input| input.name == name)
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

	let &NodeInput::Value {
		tagged_value: TaggedValue::ImaginateController(ref controller),
		..
	} = controller
	else {
		panic!("Invalid output status input")
	};
	let imaginate_status = controller.get_status();

	let use_base_image = if let &NodeInput::Value {
		tagged_value: TaggedValue::Bool(use_base_image),
		..
	} = &document_node.inputs[base_img_index]
	{
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

		if let &NodeInput::Value {
			tagged_value: TaggedValue::F64(seed),
			exposed: false,
		} = &document_node.inputs[seed_index]
		{
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
			context.network,
			&imaginate_node,
			|network| network.inputs.first().copied(),
			|frame: &IORecord<(), ImageFrame<Color>>| (frame.output.image.width, frame.output.image.height),
		)
		.unwrap_or_default();

	let resolution = {
		use graphene_std::imaginate::pick_safe_imaginate_resolution;

		let mut widgets = start_widgets(document_node, node_id, resolution_index, "Resolution", FrontendGraphDataType::Vector, false);

		let round = |x: DVec2| {
			let (x, y) = pick_safe_imaginate_resolution(x.into());
			DVec2::new(x as f64, y as f64)
		};

		if let &NodeInput::Value {
			tagged_value: TaggedValue::OptionalDVec2(vec2),
			exposed: false,
		} = &document_node.inputs[resolution_index]
		{
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

		if let &NodeInput::Value {
			tagged_value: TaggedValue::ImaginateSamplingMethod(sampling_method),
			exposed: false,
		} = &document_node.inputs[sampling_method_index]
		{
			let sampling_methods = ImaginateSamplingMethod::list();
			let mut entries = Vec::with_capacity(sampling_methods.len());
			for method in sampling_methods {
				entries.push(
					MenuListEntry::new(method.to_string())
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

	// 		if let &NodeInput::Value {
	// 			tagged_value: TaggedValue::Bool(in_paint),
	// 			exposed: false,
	// 		} = &document_node.inputs[inpaint_index]
	// 		{
	// 			widgets.extend_from_slice(&[
	// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
	// 				RadioInput::new(
	// 					[(true, "Inpaint"), (false, "Outpaint")]
	// 						.into_iter()
	// 						.map(|(paint, name)| RadioEntryData::new(name).on_update(update_value(move |_| TaggedValue::Bool(paint), node_id, inpaint_index)))
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

	// 		if let &NodeInput::Value {
	// 			tagged_value: TaggedValue::ImaginateMaskStartingFill(starting_fill),
	// 			exposed: false,
	// 		} = &document_node.inputs[mask_fill_index]
	// 		{
	// 			let mask_fill_content_modes = ImaginateMaskStartingFill::list();
	// 			let mut entries = Vec::with_capacity(mask_fill_content_modes.len());
	// 			for mode in mask_fill_content_modes {
	// 				entries.push(MenuListEntry::new(mode.to_string()).on_update(update_value(move |_| TaggedValue::ImaginateMaskStartingFill(mode), node_id, mask_fill_index)));
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

pub fn node_no_properties(_document_node: &DocumentNode, _node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	string_properties("Node has no properties")
}

pub fn layer_no_properties(_document_node: &DocumentNode, _node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	string_properties("Layer has no properties")
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
	LayoutGroup::Section { name, layout }
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
	let dash_lengths = vec_f64_input(document_node, node_id, dash_lengths_index, "Dash Lengths", TextInput::default().centered(true), true);
	let dash_offset = number_widget(document_node, node_id, dash_offset_index, "Dash Offset", NumberInput::default().unit("px").min(0.), true);
	let line_cap = line_cap_widget(document_node, node_id, line_cap_index, "Line Cap", true);
	let line_join = line_join_widget(document_node, node_id, line_join_index, "Line Join", true);
	let miter_limit = number_widget(document_node, node_id, miter_limit_index, "Miter Limit", NumberInput::default().min(0.), true);

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
	let count = number_widget(document_node, node_id, 2, "Count", NumberInput::default().min(1.), true);

	vec![direction, LayoutGroup::Row { widgets: count }]
}

pub fn circular_repeat_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let angle_offset = number_widget(document_node, node_id, 1, "Angle Offset", NumberInput::default().unit("°"), true);
	let radius = number_widget(document_node, node_id, 2, "Radius", NumberInput::default(), true); // TODO: What units?
	let count = number_widget(document_node, node_id, 3, "Count", NumberInput::default().min(1.), true);

	vec![LayoutGroup::Row { widgets: angle_offset }, LayoutGroup::Row { widgets: radius }, LayoutGroup::Row { widgets: count }]
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
	let fill_type_index = 1;
	let solid_color_index = 2;
	let gradient_type_index = 3;
	let positions_index = 7;

	let fill_type = if let &NodeInput::Value {
		tagged_value: TaggedValue::FillType(fill_type),
		..
	} = &document_node.inputs[fill_type_index]
	{
		Some(fill_type)
	} else {
		None
	};

	let mut widgets = Vec::new();
	let gradient = fill_type == Some(graphene_core::vector::style::FillType::Gradient);
	let solid = fill_type == Some(graphene_core::vector::style::FillType::Solid);

	let fill_type_switch = fill_type_widget(document_node, node_id, fill_type_index);
	widgets.push(fill_type_switch);

	if fill_type.is_none() || solid {
		let solid_color = color_widget(document_node, node_id, solid_color_index, "Color", ColorButton::default(), true);
		widgets.push(solid_color);
	}

	if fill_type.is_none() || gradient {
		let gradient_type_switch = gradient_type_widget(document_node, node_id, gradient_type_index);
		widgets.push(gradient_type_switch);
		gradient_positions(&mut widgets, document_node, "Gradient Positions", node_id, positions_index);
	}

	widgets
}

pub fn artboard_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let location = vec2_widget(document_node, node_id, 1, "Location", "X", "Y", " px", None, add_blank_assist);
	let dimensions = vec2_widget(document_node, node_id, 2, "Dimensions", "W", "H", " px", None, add_blank_assist);
	let background = color_widget(document_node, node_id, 3, "Background", ColorButton::default().allow_none(false), true);
	let clip = LayoutGroup::Row {
		widgets: bool_widget(document_node, node_id, 4, "Clip", true),
	};
	vec![location, dimensions, background, clip]
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
