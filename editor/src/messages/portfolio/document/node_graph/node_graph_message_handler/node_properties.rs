use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::utility_types::ImaginateServerStatus;
use crate::messages::prelude::*;

use document_legacy::layers::layer_info::LayerDataTypeDiscriminant;
use document_legacy::Operation;
use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::{concrete, imaginate_input::*};
use graphene_core::raster::{BlendMode, Color, ImageFrame, LuminanceCalculation, RedGreenBlue, RelativeAbsolute, SelectiveColorChoice};
use graphene_core::text::Font;
use graphene_core::vector::style::{FillType, GradientType, LineCap, LineJoin};
use graphene_core::EditorApi;
use graphene_core::{Cow, Type, TypeDescriptor};

use super::document_node_types::NodePropertiesContext;
use super::{FrontendGraphDataType, IMAGINATE_NODE};

pub fn string_properties(text: impl Into<String>) -> Vec<LayoutGroup> {
	let widget = WidgetHolder::text_widget(text);
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

fn add_blank_assist(widgets: &mut Vec<WidgetHolder>) {
	widgets.extend_from_slice(&[
		WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
		WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
		WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
		WidgetHolder::unrelated_separator(), // TODO: This last one is the separator after the 24px assist.
	]);
}

fn start_widgets(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, data_type: FrontendGraphDataType, blank_assist: bool) -> Vec<WidgetHolder> {
	let input = document_node.inputs.get(index).expect("A widget failed to be built because its node's input index is invalid.");
	let mut widgets = vec![
		expose_widget(node_id, index, data_type, input.is_exposed()),
		WidgetHolder::unrelated_separator(),
		WidgetHolder::text_widget(name),
	];
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
			WidgetHolder::unrelated_separator(),
			TextInput::new(x.clone())
				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, index))
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
			WidgetHolder::unrelated_separator(),
			TextAreaInput::new(x.clone())
				.on_update(update_value(|x: &TextAreaInput| TaggedValue::String(x.value.clone()), node_id, index))
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
			WidgetHolder::unrelated_separator(),
			CheckboxInput::new(*x)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, index))
				.widget_holder(),
		])
	}
	widgets
}

fn vec_f32_input(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, text_props: TextInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Color, blank_assist);

	let from_string = |string: &str| {
		string
			.split(&[',', ' '])
			.filter(|x| !x.is_empty())
			.map(str::parse::<f32>)
			.collect::<Result<Vec<_>, _>>()
			.ok()
			.map(TaggedValue::VecF32)
	};

	if let NodeInput::Value {
		tagged_value: TaggedValue::VecF32(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			text_props
				.value(x.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
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
			WidgetHolder::unrelated_separator(),
			FontInput::new(font.font_family.clone(), font.font_style.clone())
				.on_update(update_value(from_font_input, node_id, index))
				.widget_holder(),
		]);
		second_widgets = Some(vec![
			TextLabel::new("").widget_holder(),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::unrelated_separator(),
			FontInput::new(font.font_family.clone(), font.font_style.clone())
				.is_style_picker(true)
				.on_update(update_value(from_font_input, node_id, index))
				.widget_holder(),
		]);
	}
	(first_widgets, second_widgets)
}

fn number_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, number_props: NumberInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::F64(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			number_props
				.value(Some(x))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.widget_holder(),
		])
	} else if let NodeInput::Value {
		tagged_value: TaggedValue::U32(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::U32((x.value.unwrap()) as u32), node_id, index))
				.widget_holder(),
		])
	} else if let NodeInput::Value {
		tagged_value: TaggedValue::F32(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(move |x: &NumberInput| TaggedValue::F32((x.value.unwrap()) as f32), node_id, index))
				.widget_holder(),
		])
	}
	widgets
}

//TODO Use generalized Version of this as soon as it's available
fn blend_mode(document_node: &DocumentNode, node_id: u64, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::BlendMode(mode),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let calculation_modes = BlendMode::list();
		let mut entries = Vec::with_capacity(calculation_modes.len());
		for method in calculation_modes {
			entries.push(DropdownEntryData::new(method.to_string()).on_update(update_value(move |_| TaggedValue::BlendMode(method), node_id, index)));
		}
		let entries = vec![entries];

		widgets.extend_from_slice(&[WidgetHolder::unrelated_separator(), DropdownInput::new(entries).selected_index(Some(mode as u32)).widget_holder()]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Formula used for blending")
}

// TODO: Generalize this for all dropdowns ( also see blend_mode )
fn luminance_calculation(document_node: &DocumentNode, node_id: u64, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::LuminanceCalculation(calculation),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let calculation_modes = LuminanceCalculation::list();
		let mut entries = Vec::with_capacity(calculation_modes.len());
		for method in calculation_modes {
			entries.push(DropdownEntryData::new(method.to_string()).on_update(update_value(move |_| TaggedValue::LuminanceCalculation(method), node_id, index)));
		}
		let entries = vec![entries];

		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			DropdownInput::new(entries).selected_index(Some(calculation as u32)).widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }.with_tooltip("Formula used to calculate the luminance of a pixel")
}

fn line_cap_widget(document_node: &DocumentNode, node_id: u64, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::LineCap(line_cap),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = [("Butt", LineCap::Butt), ("Round", LineCap::Round), ("Square", LineCap::Square)]
			.into_iter()
			.map(|(name, val)| RadioEntryData::new(name).on_update(update_value(move |_| TaggedValue::LineCap(val), node_id, index)))
			.collect();

		widgets.extend_from_slice(&[WidgetHolder::unrelated_separator(), RadioInput::new(entries).selected_index(line_cap as u32).widget_holder()]);
	}
	LayoutGroup::Row { widgets }
}

fn line_join_widget(document_node: &DocumentNode, node_id: u64, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::LineJoin(line_join),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = [("Miter", LineJoin::Miter), ("Bevel", LineJoin::Bevel), ("Round", LineJoin::Round)]
			.into_iter()
			.map(|(name, val)| RadioEntryData::new(name).on_update(update_value(move |_| TaggedValue::LineJoin(val), node_id, index)))
			.collect();

		widgets.extend_from_slice(&[WidgetHolder::unrelated_separator(), RadioInput::new(entries).selected_index(line_join as u32).widget_holder()]);
	}
	LayoutGroup::Row { widgets }
}

fn fill_type_widget(document_node: &DocumentNode, node_id: u64, index: usize) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, "Fill Type", FrontendGraphDataType::General, true);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::FillType(fill_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = vec![
			RadioEntryData::new("Solid").on_update(update_value(move |_| TaggedValue::FillType(FillType::Solid), node_id, index)),
			RadioEntryData::new("Gradient").on_update(update_value(move |_| TaggedValue::FillType(FillType::Gradient), node_id, index)),
		];

		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			RadioInput::new(entries)
				.selected_index(match fill_type {
					FillType::None | FillType::Solid => 0,
					FillType::Gradient => 1,
				})
				.widget_holder(),
		]);
	}
	LayoutGroup::Row { widgets }
}

fn gradient_type_widget(document_node: &DocumentNode, node_id: u64, index: usize) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, "Gradient Type", FrontendGraphDataType::General, true);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::GradientType(gradient_type),
		exposed: false,
	} = &document_node.inputs[index]
	{
		let entries = vec![
			RadioEntryData::new("Linear").on_update(update_value(move |_| TaggedValue::GradientType(GradientType::Linear), node_id, index)),
			RadioEntryData::new("Radial").on_update(update_value(move |_| TaggedValue::GradientType(GradientType::Radial), node_id, index)),
		];

		widgets.extend_from_slice(&[WidgetHolder::unrelated_separator(), RadioInput::new(entries).selected_index(gradient_type as u32).widget_holder()]);
	}
	LayoutGroup::Row { widgets }
}

fn gradient_row(row: &mut Vec<WidgetHolder>, positions: &Vec<(f64, Option<Color>)>, index: usize, node_id: NodeId, input_index: usize) {
	let label = TextLabel::new(format!("Gradient: {:.0}%", positions[index].0 * 100.)).tooltip("Adjustable by dragging the gradient stops in the viewport with the Gradient tool active");
	row.push(label.widget_holder());
	let on_update = {
		let positions = positions.clone();
		move |color_input: &ColorInput| {
			let mut new_positions = positions.clone();
			new_positions[index].1 = color_input.value;
			TaggedValue::GradientPositions(new_positions)
		}
	};
	let color = ColorInput::new(positions[index].1).on_update(update_value(on_update, node_id, input_index));
	add_blank_assist(row);
	row.push(WidgetHolder::unrelated_separator());
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
		row.push(WidgetHolder::related_separator());
		row.push(
			IconButton::new("Remove", 16)
				.tooltip("Remove this gradient stop")
				.on_update(update_value(on_update, node_id, input_index))
				.widget_holder(),
		);
	}
	// Add button
	if positions.len() != index + 1 {
		let on_update = {
			let positions = positions.clone();
			move |_: &IconButton| {
				let mut new_positions = positions.clone();

				// Blend linearly between the two colours.
				let get_color = |index: usize| match (new_positions[index].1, new_positions.get(index + 1).and_then(|x| x.1)) {
					(Some(a), Some(b)) => Color::from_rgbaf32((a.r() + b.r()) / 2., (a.g() + b.g()) / 2., (a.b() + b.b()) / 2., ((a.a() + b.a()) / 2.).clamp(0., 1.)),
					(Some(v), _) | (_, Some(v)) => Some(v),
					_ => Some(Color::WHITE),
				};
				let get_pos = |index: usize| (new_positions[index].0 + new_positions.get(index + 1).map(|v| v.0).unwrap_or(1.)) / 2.;

				new_positions.push((get_pos(index), get_color(index)));

				new_positions.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

				TaggedValue::GradientPositions(new_positions)
			}
		};

		if !skip_separator {
			row.push(WidgetHolder::related_separator());
		}
		row.push(
			IconButton::new("Add", 16)
				.tooltip("Add a gradient stop after this")
				.on_update(update_value(on_update, node_id, input_index))
				.widget_holder(),
		);
	}
}

fn gradient_positions(rows: &mut Vec<LayoutGroup>, document_node: &DocumentNode, name: &str, node_id: u64, input_index: usize) {
	let mut widgets = vec![expose_widget(node_id, input_index, FrontendGraphDataType::General, document_node.inputs[input_index].is_exposed())];
	widgets.push(WidgetHolder::unrelated_separator());
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
			.widget_holder();

		if widgets.is_empty() {
			widgets.push(TextLabel::new("").widget_holder());
			add_blank_assist(&mut widgets);
		}
		widgets.push(WidgetHolder::unrelated_separator());
		widgets.push(invert);

		rows.push(LayoutGroup::Row { widgets });
	} else {
		widgets.push(TextLabel::new(name).widget_holder());
		rows.push(LayoutGroup::Row { widgets })
	}
}

fn color_widget(document_node: &DocumentNode, node_id: u64, index: usize, name: &str, color_props: ColorInput, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, blank_assist);

	if let NodeInput::Value { tagged_value, exposed: false } = &document_node.inputs[index] {
		if let &TaggedValue::Color(x) = tagged_value {
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				color_props
					.value(Some(x as Color))
					.on_update(update_value(|x: &ColorInput| TaggedValue::Color(x.value.unwrap()), node_id, index))
					.widget_holder(),
			])
		} else if let &TaggedValue::OptionalColor(x) = tagged_value {
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				color_props
					.value(x)
					.on_update(update_value(|x: &ColorInput| TaggedValue::OptionalColor(x.value), node_id, index))
					.widget_holder(),
			])
		}
	}
	LayoutGroup::Row { widgets }
}

fn curves_widget(document_node: &DocumentNode, node_id: u64, index: usize, name: &str, blank_assist: bool) -> LayoutGroup {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::General, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::Curve(curve),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			CurveInput::new(curve.clone())
				.on_update(update_value(|x: &CurveInput| TaggedValue::Curve(x.value.clone()), node_id, index))
				.widget_holder(),
		])
	}
	LayoutGroup::Row { widgets }
}

/// Properties for the input node, with information describing how frames work and a refresh button
pub fn input_properties(_document_node: &DocumentNode, _node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let information = WidgetHolder::text_widget("The graph's input frame is the rasterized artwork under the layer");
	let layer_path = context.layer_path.to_vec();
	let refresh_button = TextButton::new("Refresh Input")
		.tooltip("Refresh the artwork under the layer")
		.on_update(move |_| DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: layer_path.clone() }.into())
		.widget_holder();
	vec![LayoutGroup::Row { widgets: vec![information] }, LayoutGroup::Row { widgets: vec![refresh_button] }]
}

pub fn levels_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let input_shadows = number_widget(document_node, node_id, 1, "Shadows", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let input_midtones = number_widget(document_node, node_id, 2, "Midtones", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let input_highlights = number_widget(document_node, node_id, 3, "Highlights", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let output_minimums = number_widget(document_node, node_id, 4, "Output Minimums", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let output_maximums = number_widget(document_node, node_id, 5, "Output Maximums", NumberInput::default().min(0.).max(100.).unit("%"), true);

	vec![
		LayoutGroup::Row { widgets: input_shadows },
		LayoutGroup::Row { widgets: input_midtones },
		LayoutGroup::Row { widgets: input_highlights },
		LayoutGroup::Row { widgets: output_minimums },
		LayoutGroup::Row { widgets: output_maximums },
	]
}

pub fn grayscale_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	const MIN: f64 = -200.;
	const MAX: f64 = 300.;
	// TODO: Add tint color (blended above using the "Color" blend mode)
	let tint = color_widget(document_node, node_id, 1, "Tint", ColorInput::default(), true);
	let r_weight = number_widget(document_node, node_id, 2, "Reds", NumberInput::default().min(MIN).max(MAX).unit("%"), true);
	let y_weight = number_widget(document_node, node_id, 3, "Yellows", NumberInput::default().min(MIN).max(MAX).unit("%"), true);
	let g_weight = number_widget(document_node, node_id, 4, "Greens", NumberInput::default().min(MIN).max(MAX).unit("%"), true);
	let c_weight = number_widget(document_node, node_id, 5, "Cyans", NumberInput::default().min(MIN).max(MAX).unit("%"), true);
	let b_weight = number_widget(document_node, node_id, 6, "Blues", NumberInput::default().min(MIN).max(MAX).unit("%"), true);
	let m_weight = number_widget(document_node, node_id, 7, "Magentas", NumberInput::default().min(MIN).max(MAX).unit("%"), true);

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
	let backdrop = color_widget(document_node, node_id, 1, "Backdrop", ColorInput::default(), true);
	let blend_mode = blend_mode(document_node, node_id, 2, "Blend Mode", true);
	let opacity = number_widget(document_node, node_id, 3, "Opacity", NumberInput::default().min(0.).max(100.).unit("%"), true);

	vec![backdrop, blend_mode, LayoutGroup::Row { widgets: opacity }]
}

pub fn output_properties(_document_node: &DocumentNode, _node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let output_type = context.executor.previous_output_type(context.layer_path);
	let raster_output_type = concrete!(ImageFrame<Color>);
	let disabled = match output_type {
		Some(output_type) => output_type != raster_output_type,
		None => true,
	};

	let layer_path_1 = context.layer_path.to_vec();
	let layer_path_2 = context.layer_path.to_vec();

	let label = TextLabel::new("The graph's output is drawn in the layer").widget_holder();
	let download_button = TextButton::new("Download Render Output")
		.tooltip("Download the rendered image output as a PNG file")
		.disabled(disabled)
		.on_update(move |_| DocumentMessage::DownloadLayerImageOutput { layer_path: layer_path_1.clone() }.into())
		.widget_holder();
	let copy_button = TextButton::new("Copy Render Output")
		.tooltip("Copy the rendered image output to the clipboard")
		.disabled(disabled)
		.on_update(move |_| DocumentMessage::CopyToClipboardLayerImageOutput { layer_path: layer_path_2.clone() }.into())
		.widget_holder();

	vec![
		LayoutGroup::Row { widgets: vec![label] },
		LayoutGroup::Row {
			widgets: vec![download_button, WidgetHolder::related_separator(), copy_button],
		},
	]
}

pub fn mask_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let mask = color_widget(document_node, node_id, 1, "Stencil", ColorInput::default(), true);

	vec![mask]
}

pub fn luminance_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let luminance_calc = luminance_calculation(document_node, node_id, 1, "Luminance Calc", true);

	vec![luminance_calc]
}

pub fn adjust_hsl_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let hue_shift = number_widget(document_node, node_id, 1, "Hue Shift", NumberInput::default().min(-180.).max(180.).unit("°"), true);
	let saturation_shift = number_widget(document_node, node_id, 2, "Saturation Shift", NumberInput::default().min(-100.).max(100.).unit("%"), true);
	let lightness_shift = number_widget(document_node, node_id, 3, "Lightness Shift", NumberInput::default().min(-100.).max(100.).unit("%"), true);

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

pub fn blur_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let radius = number_widget(document_node, node_id, 1, "Radius", NumberInput::default().min(0.).max(20.).int(), true);
	let sigma = number_widget(document_node, node_id, 2, "Sigma", NumberInput::default().min(0.).max(10000.), true);

	vec![LayoutGroup::Row { widgets: radius }, LayoutGroup::Row { widgets: sigma }]
}

pub fn brush_node_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let color = color_widget(document_node, node_id, 7, "Color", ColorInput::default().allow_none(false), true);

	let size = number_widget(document_node, node_id, 4, "Diameter", NumberInput::default().min(1.).max(100.).unit(" px"), true);
	let hardness = number_widget(document_node, node_id, 5, "Hardness", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let flow = number_widget(document_node, node_id, 6, "Flow", NumberInput::default().min(1.).max(100.).unit("%"), true);

	vec![color, LayoutGroup::Row { widgets: size }, LayoutGroup::Row { widgets: hardness }, LayoutGroup::Row { widgets: flow }]
}

pub fn adjust_threshold_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let thereshold_min = number_widget(document_node, node_id, 1, "Min Luminance", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let thereshold_max = number_widget(document_node, node_id, 2, "Max Luminance", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let luminance_calc = luminance_calculation(document_node, node_id, 3, "Luminance Calc", true);

	vec![LayoutGroup::Row { widgets: thereshold_min }, LayoutGroup::Row { widgets: thereshold_max }, luminance_calc]
}

pub fn adjust_vibrance_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let vibrance = number_widget(document_node, node_id, 1, "Vibrance", NumberInput::default().min(-100.).max(100.).unit("%"), true);

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
	let mut output_channel = vec![WidgetHolder::text_widget("Output Channel"), WidgetHolder::unrelated_separator()];
	add_blank_assist(&mut output_channel);
	if let &NodeInput::Value {
		tagged_value: TaggedValue::RedGreenBlue(choice),
		exposed: false,
	} = &document_node.inputs[output_channel_index]
	{
		let entries = vec![
			RadioEntryData::new(RedGreenBlue::Red.to_string()).on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Red), node_id, output_channel_index)),
			RadioEntryData::new(RedGreenBlue::Green.to_string()).on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Green), node_id, output_channel_index)),
			RadioEntryData::new(RedGreenBlue::Blue.to_string()).on_update(update_value(|_| TaggedValue::RedGreenBlue(RedGreenBlue::Blue), node_id, output_channel_index)),
		];
		output_channel.extend([RadioInput::new(entries).selected_index(choice as u32).widget_holder()]);
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
	let red = number_widget(document_node, node_id, r.0, r.1, NumberInput::default().min(-200.).max(200.).value(Some(r.2)).unit("%"), true);
	let green = number_widget(document_node, node_id, g.0, g.1, NumberInput::default().min(-200.).max(200.).value(Some(g.2)).unit("%"), true);
	let blue = number_widget(document_node, node_id, b.0, b.1, NumberInput::default().min(-200.).max(200.).value(Some(b.2)).unit("%"), true);
	let constant = number_widget(document_node, node_id, c.0, c.1, NumberInput::default().min(-200.).max(200.).value(Some(c.2)).unit("%"), true);

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
	let mut colors = vec![WidgetHolder::text_widget("Colors"), WidgetHolder::unrelated_separator()];
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
					.into_iter()
					.map(|choice| DropdownEntryData::new(choice.to_string()).on_update(update_value(move |_| TaggedValue::SelectiveColorChoice(*choice), node_id, colors_index)))
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
	let cyan = number_widget(document_node, node_id, c.0, c.1, NumberInput::default().min(-100.).max(100.).unit("%"), true);
	let magenta = number_widget(document_node, node_id, m.0, m.1, NumberInput::default().min(-100.).max(100.).unit("%"), true);
	let yellow = number_widget(document_node, node_id, y.0, y.1, NumberInput::default().min(-100.).max(100.).unit("%"), true);
	let black = number_widget(document_node, node_id, k.0, k.1, NumberInput::default().min(-100.).max(100.).unit("%"), true);

	// Mode
	let mode_index = 1;
	let mut mode = start_widgets(document_node, node_id, mode_index, "Mode", FrontendGraphDataType::General, true);
	mode.push(WidgetHolder::unrelated_separator());
	if let &NodeInput::Value {
		tagged_value: TaggedValue::RelativeAbsolute(relative_or_absolute),
		exposed: false,
	} = &document_node.inputs[mode_index]
	{
		let entries = vec![
			RadioEntryData::new("Relative").on_update(update_value(|_| TaggedValue::RelativeAbsolute(RelativeAbsolute::Relative), node_id, mode_index)),
			RadioEntryData::new("Absolute").on_update(update_value(|_| TaggedValue::RelativeAbsolute(RelativeAbsolute::Absolute), node_id, mode_index)),
		];
		mode.push(RadioInput::new(entries).selected_index(relative_or_absolute as u32).widget_holder());
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
pub fn gpu_map_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let map = text_widget(document_node, node_id, 1, "Map", true);

	vec![LayoutGroup::Row { widgets: map }]
}

pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let gamma = number_widget(document_node, node_id, 1, "Factor", NumberInput::default().min(0.).max(100.).unit("%"), true);

	vec![LayoutGroup::Row { widgets: gamma }]
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
	let gamma_input = NumberInput::default().min(0.01).max(9.99).mode_increment().increment_step(0.1);
	let gamma_correction = number_widget(document_node, node_id, 3, "Gamma Correction", gamma_input, true);

	vec![
		LayoutGroup::Row { widgets: exposure },
		LayoutGroup::Row { widgets: offset },
		LayoutGroup::Row { widgets: gamma_correction },
	]
}

pub fn add_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| {
		let widgets = number_widget(document_node, node_id, index, name, NumberInput::default(), true);

		LayoutGroup::Row { widgets }
	};
	vec![operand("Input", 0), operand("Addend", 1)]
}

pub fn transform_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let translation = {
		let index = 1;

		let mut widgets = start_widgets(document_node, node_id, index, "Translation", FrontendGraphDataType::Vector, false);

		let pivot_index = 5;
		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(pivot),
			exposed: false,
		} = document_node.inputs[pivot_index]
		{
			widgets.push(WidgetHolder::unrelated_separator());
			widgets.push(
				PivotAssist::new(pivot.into())
					.on_update(|pivot_assist: &PivotAssist| PropertiesPanelMessage::SetPivot { new_position: pivot_assist.position }.into())
					.widget_holder(),
			);
		} else {
			add_blank_assist(&mut widgets);
		}

		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(vec2),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(vec2.x))
					.label("X")
					.unit(" px")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), vec2.y)), node_id, index))
					.widget_holder(),
				WidgetHolder::related_separator(),
				NumberInput::new(Some(vec2.y))
					.label("Y")
					.unit(" px")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(vec2.x, input.value.unwrap())), node_id, index))
					.widget_holder(),
			]);
		}

		LayoutGroup::Row { widgets }
	};

	let rotation = {
		let index = 2;

		let mut widgets = start_widgets(document_node, node_id, index, "Rotation", FrontendGraphDataType::Number, true);

		if let NodeInput::Value {
			tagged_value: TaggedValue::F64(val),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(val.to_degrees()))
					.unit("°")
					.mode(NumberInputMode::Range)
					.range_min(Some(-180.))
					.range_max(Some(180.))
					.on_update(update_value(|number_input: &NumberInput| TaggedValue::F64(number_input.value.unwrap().to_radians()), node_id, index))
					.widget_holder(),
			]);
		}

		LayoutGroup::Row { widgets }
	};

	let scale = {
		let index = 3;

		let mut widgets = start_widgets(document_node, node_id, index, "Scale", FrontendGraphDataType::Vector, true);

		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(vec2),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(vec2.x))
					.label("X")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), vec2.y)), node_id, index))
					.widget_holder(),
				WidgetHolder::related_separator(),
				NumberInput::new(Some(vec2.y))
					.label("Y")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(vec2.x, input.value.unwrap())), node_id, index))
					.widget_holder(),
			]);
		}

		LayoutGroup::Row { widgets }
	};
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
	let layer_path = context.layer_path.to_vec();

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
	let mask_index = resolve_input("Masking Layer");
	let inpaint_index = resolve_input("Inpaint");
	let mask_blur_index = resolve_input("Mask Blur");
	let mask_fill_index = resolve_input("Mask Starting Fill");
	let faces_index = resolve_input("Improve Faces");
	let tiling_index = resolve_input("Tiling");
	let cached_index = resolve_input("Cached Data");

	let cached_value = &document_node.inputs[cached_index];
	let complete_value = &document_node.inputs[resolve_input("Percent Complete")];
	let status_value = &document_node.inputs[resolve_input("Status")];

	let server_status = {
		let status = match &context.persistent_data.imaginate_server_status {
			ImaginateServerStatus::Unknown => {
				context.responses.add(PortfolioMessage::ImaginateCheckServerStatus);
				"Checking..."
			}
			ImaginateServerStatus::Checking => "Checking...",
			ImaginateServerStatus::Unavailable => "Unavailable",
			ImaginateServerStatus::Connected => "Connected",
		};
		let mut widgets = vec![
			WidgetHolder::text_widget("Server"),
			WidgetHolder::unrelated_separator(),
			IconButton::new("Settings", 24)
				.tooltip("Preferences: Imaginate")
				.on_update(|_| DialogMessage::RequestPreferencesDialog.into())
				.widget_holder(),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::bold_text(status),
			WidgetHolder::related_separator(),
			IconButton::new("Reload", 24)
				.tooltip("Refresh connection status")
				.on_update(|_| PortfolioMessage::ImaginateCheckServerStatus.into())
				.widget_holder(),
		];
		if context.persistent_data.imaginate_server_status == ImaginateServerStatus::Unavailable {
			widgets.extend([
				WidgetHolder::unrelated_separator(),
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

	let &NodeInput::Value {tagged_value: TaggedValue::ImaginateStatus( imaginate_status),..} = status_value else {
		panic!("Invalid status input")
	};
	let NodeInput::Value {tagged_value: TaggedValue::RcImage( cached_data),..} = cached_value else {
		panic!("Invalid cached image input, received {:?}, index: {}", cached_value, cached_index)
	};
	let &NodeInput::Value {tagged_value: TaggedValue::F64( percent_complete),..} = complete_value else {
		panic!("Invalid percent complete input")
	};
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
		// Since we don't serialize the status, we need to derive from other state whether the Idle state is actually supposed to be the Terminated state
		let mut interpreted_status = imaginate_status;
		if imaginate_status == ImaginateStatus::Idle && cached_data.is_some() && percent_complete > 0. && percent_complete < 100. {
			interpreted_status = ImaginateStatus::Terminated;
		}

		let status = match interpreted_status {
			ImaginateStatus::Idle => match cached_data {
				Some(_) => "Done".into(),
				None => "Ready".into(),
			},
			ImaginateStatus::Beginning => "Beginning...".into(),
			ImaginateStatus::Uploading(percent) => format!("Uploading Input Image: {percent:.0}%"),
			ImaginateStatus::Generating => format!("Generating: {percent_complete:.0}%"),
			ImaginateStatus::Terminating => "Terminating...".into(),
			ImaginateStatus::Terminated => format!("{percent_complete:.0}% (Terminated)"),
		};
		let widgets = vec![
			WidgetHolder::text_widget("Progress"),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
			WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
			WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
			WidgetHolder::unrelated_separator(),
			WidgetHolder::bold_text(status),
		];
		LayoutGroup::Row { widgets }.with_tooltip("When generating, the percentage represents how many sampling steps have so far been processed out of the target number")
	};

	let image_controls = {
		let mut widgets = vec![WidgetHolder::text_widget("Image"), WidgetHolder::unrelated_separator()];
		let assist_separators = vec![
			WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
			WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
			WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
			WidgetHolder::unrelated_separator(),
		];

		match imaginate_status {
			ImaginateStatus::Beginning | ImaginateStatus::Uploading(_) => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(TextButton::new("Beginning...").tooltip("Sending image generation request to the server").disabled(true).widget_holder());
			}
			ImaginateStatus::Generating => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(
					TextButton::new("Terminate")
						.tooltip("Cancel the in-progress image generation and keep the latest progress")
						.on_update({
							let imaginate_node = imaginate_node.clone();
							move |_| {
								DocumentMessage::ImaginateTerminate {
									layer_path: layer_path.clone(),
									node_path: imaginate_node.clone(),
								}
								.into()
							}
						})
						.widget_holder(),
				);
			}
			ImaginateStatus::Terminating => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(
					TextButton::new("Terminating...")
						.tooltip("Waiting on the final image generated after termination")
						.disabled(true)
						.widget_holder(),
				);
			}
			ImaginateStatus::Idle | ImaginateStatus::Terminated => widgets.extend_from_slice(&[
				IconButton::new("Random", 24)
					.tooltip("Generate with a new random seed")
					.on_update({
						let imaginate_node = imaginate_node.clone();
						let layer_path = context.layer_path.to_vec();
						move |_| {
							DocumentMessage::ImaginateRandom {
								layer_path: layer_path.clone(),
								imaginate_node: imaginate_node.clone(),
								then_generate: true,
							}
							.into()
						}
					})
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				TextButton::new("Generate")
					.tooltip("Fill layer frame by generating a new image")
					.on_update({
						let imaginate_node = imaginate_node.clone();
						let layer_path = context.layer_path.to_vec();
						move |_| {
							DocumentMessage::ImaginateGenerate {
								layer_path: layer_path.clone(),
								imaginate_node: imaginate_node.clone(),
							}
							.into()
						}
					})
					.widget_holder(),
				WidgetHolder::related_separator(),
				TextButton::new("Clear")
					.tooltip("Remove generated image from the layer frame")
					.disabled(cached_data.is_none())
					.on_update({
						let layer_path = context.layer_path.to_vec();
						move |_| {
							DocumentMessage::ImaginateClear {
								node_id,
								layer_path: layer_path.clone(),
								cached_index,
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

		if let &NodeInput::Value {
			tagged_value: TaggedValue::F64(seed),
			exposed: false,
		} = &document_node.inputs[seed_index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				IconButton::new("Regenerate", 24)
					.tooltip("Set a new random seed")
					.on_update({
						let imaginate_node = imaginate_node.clone();
						let layer_path = context.layer_path.to_vec();
						move |_| {
							DocumentMessage::ImaginateRandom {
								layer_path: layer_path.clone(),
								imaginate_node: imaginate_node.clone(),
								then_generate: false,
							}
							.into()
						}
					})
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(seed))
					.min(0.)
					.int()
					.on_update(update_value(move |input: &NumberInput| TaggedValue::F64(input.value.unwrap()), node_id, seed_index))
					.widget_holder(),
			])
		}
		// Note: Limited by f64. You cannot even have all the possible u64 values :)
		LayoutGroup::Row { widgets }.with_tooltip("Seed determines the random outcome, enabling limitless unique variations")
	};

	// Create the input to the graph using an empty image
	let editor_api = std::borrow::Cow::Owned(EditorApi {
		image_frame: None,
		font_cache: Some(&context.persistent_data.font_cache),
	});
	// Compute the transform input to the image frame
	let image_frame: ImageFrame<Color> = context.executor.compute_input(context.network, &imaginate_node, 0, editor_api).unwrap_or_default();
	let transform = image_frame.transform;

	let resolution = {
		use document_legacy::document::pick_safe_imaginate_resolution;

		let mut widgets = start_widgets(document_node, node_id, resolution_index, "Resolution", FrontendGraphDataType::Vector, false);

		let round = |x: DVec2| {
			let (x, y) = pick_safe_imaginate_resolution(x.into());
			Some(DVec2::new(x as f64, y as f64))
		};

		if let &NodeInput::Value {
			tagged_value: TaggedValue::OptionalDVec2(vec2),
			exposed: false,
		} = &document_node.inputs[resolution_index]
		{
			let dimensions_is_auto = vec2.is_none();
			let vec2 = vec2.unwrap_or_else(|| {
				let w = transform.transform_vector2(DVec2::new(1., 0.)).length();
				let h = transform.transform_vector2(DVec2::new(0., 1.)).length();

				let (x, y) = pick_safe_imaginate_resolution((w, h));

				DVec2::new(x as f64, y as f64)
			});

			let layer_path = context.layer_path.to_vec();
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				IconButton::new("Rescale", 24)
					.tooltip("Set the layer dimensions to this resolution")
					.on_update(move |_| {
						Operation::SetLayerScaleAroundPivot {
							path: layer_path.clone(),
							new_scale: vec2.into(),
						}
						.into()
					})
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
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
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(vec2.x))
					.label("W")
					.min(64.)
					.step(64.)
					.unit(" px")
					.disabled(dimensions_is_auto && !transform_not_connected)
					.on_update(update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(round(DVec2::new(number_input.value.unwrap(), vec2.y))),
						node_id,
						resolution_index,
					))
					.widget_holder(),
				WidgetHolder::related_separator(),
				NumberInput::new(Some(vec2.y))
					.label("H")
					.min(64.)
					.step(64.)
					.unit(" px")
					.disabled(dimensions_is_auto && !transform_not_connected)
					.on_update(update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(round(DVec2::new(vec2.x, number_input.value.unwrap()))),
						node_id,
						resolution_index,
					))
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
				entries.push(DropdownEntryData::new(method.to_string()).on_update(update_value(move |_| TaggedValue::ImaginateSamplingMethod(method), node_id, sampling_method_index)));
			}
			let entries = vec![entries];

			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
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

	let mut layer_reference_input_layer_is_some = false;
	let layer_mask = {
		let mut widgets = start_widgets(document_node, node_id, mask_index, "Masking Layer", FrontendGraphDataType::General, true);

		if let NodeInput::Value {
			tagged_value: TaggedValue::LayerPath(layer_path),
			exposed: false,
		} = &document_node.inputs[mask_index]
		{
			let layer_reference_input_layer = layer_path
				.as_ref()
				.and_then(|path| context.document.layer(path).ok())
				.map(|layer| (layer.name.clone().unwrap_or_default(), LayerDataTypeDiscriminant::from(&layer.data)));

			layer_reference_input_layer_is_some = layer_reference_input_layer.is_some();

			let layer_reference_input_layer_name = layer_reference_input_layer.as_ref().map(|(layer_name, _)| layer_name);
			let layer_reference_input_layer_type = layer_reference_input_layer.as_ref().map(|(_, layer_type)| layer_type);

			widgets.push(WidgetHolder::unrelated_separator());
			if !transform_not_connected {
				widgets.push(
					LayerReferenceInput::new(layer_path.clone(), layer_reference_input_layer_name.cloned(), layer_reference_input_layer_type.cloned())
						.disabled(!use_base_image)
						.on_update(update_value(|input: &LayerReferenceInput| TaggedValue::LayerPath(input.value.clone()), node_id, mask_index))
						.widget_holder(),
				);
			} else {
				widgets.push(TextLabel::new("Requires Transform Input").italic(true).widget_holder());
			}
		}
		LayoutGroup::Row { widgets }.with_tooltip(
			"Reference to a layer or folder which masks parts of the input image. Image generation is constrained to masked areas.\n\
			\n\
			Black shapes represent the masked regions. Lighter shades of gray act as a partial mask, and colors become grayscale. (This is the reverse of traditional masks because it is easier to draw black shapes; this will be changed later when the mask input is a bitmap.)",
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
		layer_mask,
	];

	if use_base_image && layer_reference_input_layer_is_some {
		let in_paint = {
			let mut widgets = start_widgets(document_node, node_id, inpaint_index, "Inpaint", FrontendGraphDataType::Boolean, true);

			if let &NodeInput::Value {
				tagged_value: TaggedValue::Bool(in_paint),
				exposed: false,
			} = &document_node.inputs[inpaint_index]
			{
				widgets.extend_from_slice(&[
					WidgetHolder::unrelated_separator(),
					RadioInput::new(
						[(true, "Inpaint"), (false, "Outpaint")]
							.into_iter()
							.map(|(paint, name)| RadioEntryData::new(name).on_update(update_value(move |_| TaggedValue::Bool(paint), node_id, inpaint_index)))
							.collect(),
					)
					.selected_index(1 - in_paint as u32)
					.widget_holder(),
				]);
			}
			LayoutGroup::Row { widgets }.with_tooltip(
				"Constrain image generation to the interior (inpaint) or exterior (outpaint) of the mask, while referencing the other unchanged parts as context imagery.\n\
				\n\
				An unwanted part of an image can be replaced by drawing around it with a black shape and inpainting with that mask layer.\n\
				\n\
				An image can be uncropped by resizing the Imaginate layer to the target bounds and outpainting with a black rectangle mask matching the original image bounds.",
			)
		};

		let blur_radius = {
			let number_props = NumberInput::default().unit(" px").min(0.).max(25.).int();
			let widgets = number_widget(document_node, node_id, mask_blur_index, "Mask Blur", number_props, true);
			LayoutGroup::Row { widgets }.with_tooltip("Blur radius for the mask. Useful for softening sharp edges to blend the masked area with the rest of the image.")
		};

		let mask_starting_fill = {
			let mut widgets = start_widgets(document_node, node_id, mask_fill_index, "Mask Starting Fill", FrontendGraphDataType::General, true);

			if let &NodeInput::Value {
				tagged_value: TaggedValue::ImaginateMaskStartingFill(starting_fill),
				exposed: false,
			} = &document_node.inputs[mask_fill_index]
			{
				let mask_fill_content_modes = ImaginateMaskStartingFill::list();
				let mut entries = Vec::with_capacity(mask_fill_content_modes.len());
				for mode in mask_fill_content_modes {
					entries.push(DropdownEntryData::new(mode.to_string()).on_update(update_value(move |_| TaggedValue::ImaginateMaskStartingFill(mode), node_id, mask_fill_index)));
				}
				let entries = vec![entries];

				widgets.extend_from_slice(&[
					WidgetHolder::unrelated_separator(),
					DropdownInput::new(entries).selected_index(Some(starting_fill as u32)).widget_holder(),
				]);
			}
			LayoutGroup::Row { widgets }.with_tooltip(
				"Begin in/outpainting the masked areas using this fill content as the starting input image.\n\
				\n\
				Each option can be visualized by generating with 'Sampling Steps' set to 0.",
			)
		};
		layout.extend_from_slice(&[in_paint, blur_radius, mask_starting_fill]);
	}

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

pub fn no_properties(_document_node: &DocumentNode, _node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	string_properties("Node has no properties")
}

pub fn index_node_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
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

	let color = color_widget(document_node, node_id, color_index, "Color", ColorInput::default(), true);
	let weight = number_widget(document_node, node_id, weight_index, "Weight", NumberInput::default().unit("px").min(0.), true);
	let dash_lengths = vec_f32_input(document_node, node_id, dash_lengths_index, "Dash Lengths", TextInput::default().centered(true), true);
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
		let solid_color = color_widget(document_node, node_id, solid_color_index, "Color", ColorInput::default(), true);
		widgets.push(solid_color);
	}

	if fill_type.is_none() || gradient {
		let gradient_type_switch = gradient_type_widget(document_node, node_id, gradient_type_index);
		widgets.push(gradient_type_switch);
		gradient_positions(&mut widgets, document_node, "Gradient Positions", node_id, positions_index);
	}

	widgets
}

pub fn layer_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let name = text_widget(document_node, node_id, 1, "Name", true);
	let blend_mode = blend_mode(document_node, node_id, 2, "Blend Mode", true);
	let opacity = number_widget(document_node, node_id, 3, "Opacity", NumberInput::default().min(0.).max(100.).unit("%"), true);
	let visible = bool_widget(document_node, node_id, 4, "Visible", true);
	let locked = bool_widget(document_node, node_id, 5, "Locked", true);
	let collapsed = bool_widget(document_node, node_id, 6, "Collapsed", true);

	vec![
		LayoutGroup::Row { widgets: name },
		blend_mode,
		LayoutGroup::Row { widgets: opacity },
		LayoutGroup::Row { widgets: visible },
		LayoutGroup::Row { widgets: locked },
		LayoutGroup::Row { widgets: collapsed },
	]
}
pub fn artboard_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let label = text_widget(document_node, node_id, 1, "Label", true);
	vec![LayoutGroup::Row { widgets: label }]
}
