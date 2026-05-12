use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::color_selector::{DrawingToolState, apply_line_weight};
use crate::messages::tool::common_functionality::graph_modification_utils;
use graph_craft::document::value::TaggedValue;
use graphene_std::NodeInputDecleration;
use graphene_std::choice_type::ChoiceTypeStatic;
use graphene_std::list::List;
use graphene_std::vector::style::{PaintOrder, StrokeAlign, StrokeCap, StrokeJoin};

/// All non-color stroke-related options surfaced in the control bar popover.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum StrokeOptionsUpdate {
	LineWeight(f64),
	Align(StrokeAlign),
	Cap(StrokeCap),
	Join(StrokeJoin),
	MiterLimit(f64),
	PaintOrder(PaintOrder),
	DashLengths(Vec<f64>),
	DashOffset(f64),
}

/// Builds the control-bar popover button that opens the stroke options panel (weight, align, caps, joins, miter limit, paint order, dash).
/// `to_message` adapts a [`StrokeOptionsUpdate`] into the calling tool's `UpdateOptions` message.
pub fn create_stroke_options_popover_widget<F>(drawing: &DrawingToolState, disabled: bool, to_message: F) -> WidgetInstance
where
	F: Fn(StrokeOptionsUpdate) -> Message + 'static + Send + Sync + Clone,
{
	PopoverButton::new()
		.popover_layout(Layout(build_popover_rows(drawing, to_message)))
		.disabled(disabled)
		.tooltip_label("Stroke Options")
		.widget_instance()
}

/// Dispatches a [`StrokeOptionsUpdate`] to the matching apply helper and updates `drawing` in lockstep.
pub fn apply_stroke_option(drawing: &mut DrawingToolState, update: StrokeOptionsUpdate, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	match update {
		StrokeOptionsUpdate::LineWeight(weight) => apply_line_weight(drawing, weight, document, responses),
		StrokeOptionsUpdate::Align(align) => apply_stroke_align(drawing, align, document, responses),
		StrokeOptionsUpdate::Cap(cap) => apply_stroke_cap(drawing, cap, document, responses),
		StrokeOptionsUpdate::Join(join) => apply_stroke_join(drawing, join, document, responses),
		StrokeOptionsUpdate::MiterLimit(limit) => apply_miter_limit(drawing, limit, document, responses),
		StrokeOptionsUpdate::PaintOrder(order) => apply_paint_order(drawing, order, document, responses),
		StrokeOptionsUpdate::DashLengths(lengths) => apply_dash_lengths(drawing, lengths, document, responses),
		StrokeOptionsUpdate::DashOffset(offset) => apply_dash_offset(drawing, offset, document, responses),
	}
}

fn build_popover_rows<F>(drawing: &DrawingToolState, to_message: F) -> Vec<LayoutGroup>
where
	F: Fn(StrokeOptionsUpdate) -> Message + 'static + Send + Sync + Clone,
{
	// Miter limit only matters when the join is `Miter`; mixed (`None`) keeps the row visible so the user can still edit the value.
	let show_miter_limit = drawing.stroke_join != Some(StrokeJoin::Bevel) && drawing.stroke_join != Some(StrokeJoin::Round);
	let has_dash = !drawing.effective_dash_lengths().is_empty();

	let mut rows = vec![
		LayoutGroup::row(vec![TextLabel::new("Stroke").bold(true).widget_instance()]),
		LayoutGroup::row(weight_row(drawing.line_weight, to_message.clone())),
		LayoutGroup::row(dash_lengths_row(drawing.dash_lengths.as_deref(), to_message.clone())),
	];
	if has_dash {
		rows.push(LayoutGroup::row(dash_offset_row(drawing.dash_offset, to_message.clone())));
	}
	rows.push(LayoutGroup::row(enum_radio_row::<PaintOrder, _>("Order", drawing.paint_order, false, {
		let to_message = to_message.clone();
		move |value| to_message(StrokeOptionsUpdate::PaintOrder(value))
	})));
	rows.push(LayoutGroup::row(enum_radio_row::<StrokeAlign, _>("Align", drawing.stroke_align, false, {
		let to_message = to_message.clone();
		move |value| to_message(StrokeOptionsUpdate::Align(value))
	})));
	rows.push(LayoutGroup::row(enum_radio_row::<StrokeCap, _>("Cap", drawing.stroke_cap, false, {
		let to_message = to_message.clone();
		move |value| to_message(StrokeOptionsUpdate::Cap(value))
	})));
	rows.push(LayoutGroup::row(enum_radio_row::<StrokeJoin, _>("Join", drawing.stroke_join, false, {
		let to_message = to_message.clone();
		move |value| to_message(StrokeOptionsUpdate::Join(value))
	})));
	if show_miter_limit {
		rows.push(LayoutGroup::row(miter_limit_row(drawing.miter_limit, to_message)));
	}
	rows
}

fn weight_row<F>(weight: Option<f64>, to_message: F) -> Vec<WidgetInstance>
where
	F: Fn(StrokeOptionsUpdate) -> Message + 'static + Send + Sync,
{
	vec![
		TextLabel::new("Weight").table_align(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		NumberInput::new(weight)
			.unit(" px")
			.min(0.)
			.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number: &NumberInput| number.value.map_or(Message::NoOp, |value| to_message(StrokeOptionsUpdate::LineWeight(value))))
			.on_commit(|_| DocumentMessage::StartTransaction.into())
			.widget_instance(),
	]
}

fn miter_limit_row<F>(limit: Option<f64>, to_message: F) -> Vec<WidgetInstance>
where
	F: Fn(StrokeOptionsUpdate) -> Message + 'static + Send + Sync,
{
	vec![
		TextLabel::new("Limit").table_align(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		NumberInput::new(limit)
			.min(0.)
			.on_update(move |number: &NumberInput| number.value.map_or(Message::NoOp, |value| to_message(StrokeOptionsUpdate::MiterLimit(value))))
			.on_commit(|_| DocumentMessage::StartTransaction.into())
			.widget_instance(),
	]
}

fn enum_radio_row<E, F>(label_text: &str, current: Option<E>, disabled: bool, to_message: F) -> Vec<WidgetInstance>
where
	E: ChoiceTypeStatic + 'static,
	F: Fn(E) -> Message + 'static + Send + Sync + Clone,
{
	let entries = E::list()
		.iter()
		.flat_map(|section| section.iter())
		.map(|(value, meta)| {
			let to_message = to_message.clone();
			let value = *value;
			let entry = RadioEntryData::new(meta.name)
				.tooltip_label(meta.label)
				.tooltip_description(meta.description.unwrap_or_default())
				.on_update(move |_| to_message(value))
				.on_commit(|_| DocumentMessage::StartTransaction.into());
			if let Some(icon) = meta.icon { entry.icon(icon) } else { entry.label(meta.label) }
		})
		.collect();
	vec![
		TextLabel::new(label_text).table_align(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		RadioInput::new(entries).selected_index(current.map(|c| c.as_u32())).disabled(disabled).widget_instance(),
	]
}

fn dash_lengths_row<F>(current: Option<&[f64]>, to_message: F) -> Vec<WidgetInstance>
where
	F: Fn(StrokeOptionsUpdate) -> Message + 'static + Send + Sync,
{
	let text = current
		.map(|values| values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
		.unwrap_or_else(|| "-".to_string());
	vec![
		TextLabel::new("Dash").table_align(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		TextInput::new(text)
			.centered(true)
			.tooltip_label("Dash Pattern")
			.tooltip_description("Comma-separated dash and gap lengths.")
			.on_update(move |input: &TextInput| {
				let parsed = input.value.split(&[',', ' ']).filter(|piece| !piece.is_empty()).map(str::parse::<f64>).collect::<Result<Vec<_>, _>>();
				parsed.map_or(Message::NoOp, |lengths| to_message(StrokeOptionsUpdate::DashLengths(lengths)))
			})
			.on_commit(|_| DocumentMessage::StartTransaction.into())
			.widget_instance(),
	]
}

fn dash_offset_row<F>(offset: Option<f64>, to_message: F) -> Vec<WidgetInstance>
where
	F: Fn(StrokeOptionsUpdate) -> Message + 'static + Send + Sync,
{
	vec![
		TextLabel::new("Offset").table_align(true).widget_instance(),
		Separator::new(SeparatorStyle::Unrelated).widget_instance(),
		NumberInput::new(offset)
			.unit(" px")
			.on_update(move |number: &NumberInput| number.value.map_or(Message::NoOp, |value| to_message(StrokeOptionsUpdate::DashOffset(value))))
			.on_commit(|_| DocumentMessage::StartTransaction.into())
			.widget_instance(),
	]
}

// =============
// APPLY HELPERS
// =============

pub fn apply_stroke_align(drawing: &mut DrawingToolState, align: StrokeAlign, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.stroke_align = Some(align);
	set_stroke_input_for_selected(document, graphene_std::vector::stroke::AlignInput::INDEX, TaggedValue::StrokeAlign(align), responses);
}

pub fn apply_stroke_cap(drawing: &mut DrawingToolState, cap: StrokeCap, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.stroke_cap = Some(cap);
	set_stroke_input_for_selected(document, graphene_std::vector::stroke::CapInput::INDEX, TaggedValue::StrokeCap(cap), responses);
}

pub fn apply_stroke_join(drawing: &mut DrawingToolState, join: StrokeJoin, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.stroke_join = Some(join);
	set_stroke_input_for_selected(document, graphene_std::vector::stroke::JoinInput::INDEX, TaggedValue::StrokeJoin(join), responses);
}

pub fn apply_miter_limit(drawing: &mut DrawingToolState, limit: f64, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.miter_limit = Some(limit);
	set_stroke_input_for_selected(document, graphene_std::vector::stroke::MiterLimitInput::INDEX, TaggedValue::F64(limit), responses);
}

pub fn apply_paint_order(drawing: &mut DrawingToolState, order: PaintOrder, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.paint_order = Some(order);
	set_stroke_input_for_selected(document, graphene_std::vector::stroke::PaintOrderInput::INDEX, TaggedValue::PaintOrder(order), responses);
}

pub fn apply_dash_lengths(drawing: &mut DrawingToolState, lengths: Vec<f64>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.dash_lengths = Some(lengths.clone());
	set_stroke_input_for_selected(document, graphene_std::vector::stroke::DashLengthsInput::<List<f64>>::INDEX, TaggedValue::F64Array(lengths), responses);
}

pub fn apply_dash_offset(drawing: &mut DrawingToolState, offset: f64, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.dash_offset = Some(offset);
	set_stroke_input_for_selected(document, graphene_std::vector::stroke::DashOffsetInput::INDEX, TaggedValue::F64(offset), responses);
}

fn set_stroke_input_for_selected(document: &DocumentMessageHandler, input_index: usize, value: TaggedValue, responses: &mut VecDeque<Message>) {
	graph_modification_utils::set_proto_node_input_for_selected_layers(document, graphene_std::vector::stroke::IDENTIFIER, input_index, value, responses);
}
