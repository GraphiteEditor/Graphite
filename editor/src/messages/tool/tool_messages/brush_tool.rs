use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::consts::{BRUSH_FLOW_DEFAULT, BRUSH_HARDNESS_DEFAULT, BRUSH_SIZE_DEFAULT};
use crate::messages::portfolio::document::graph_operation::transform_utils::get_current_transform;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_proto_node_type};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, InputConnector, OutputConnector};
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, selection_changed_since_last_sync, solid};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::brush::basic_brush::basic_brush as active_brush;
use graphene_std::brush::{Channel, Stroke};
use graphene_std::vector::style::{FillChoice, FillChoiceUI};

const SAMPLE_MERGE_DISTANCE: f64 = 1.;
const SAMPLE_MERGE_PRESSURE: f64 = 0.01;

#[derive(Default, ExtractField)]
pub struct BrushTool {
	fsm_state: BrushToolFsmState,
	data: BrushToolData,
	options: BrushOptions,
}

pub struct BrushOptions {
	diameter: f64,
	hardness: f64,
	flow: f64,
	color: ToolColorOptions,
	last_synced_selection: Vec<LayerNodeIdentifier>,
}

impl Default for BrushOptions {
	fn default() -> Self {
		Self {
			diameter: BRUSH_SIZE_DEFAULT,
			hardness: BRUSH_HARDNESS_DEFAULT,
			flow: BRUSH_FLOW_DEFAULT,
			color: ToolColorOptions::default(),
			last_synced_selection: Vec::new(),
		}
	}
}

impl BrushOptions {
	fn active_color(&self) -> Color {
		self.color.active_color().unwrap_or_default()
	}
}

#[impl_message(Message, ToolMessage, Brush)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum BrushToolMessage {
	// Standard messages
	Abort,
	SelectionChanged,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions { options: BrushToolMessageOptionsUpdate },
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum BrushToolMessageOptionsUpdate {
	ChangeDiameter(f64),
	Color(Option<Color>),
	Diameter(f64),
	Hardness(f64),
	Flow(f64),
	WorkingColorsChanged,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BrushToolFsmState {
	#[default]
	Ready,
	Drawing,
}

impl ToolMetadata for BrushTool {
	fn icon_name(&self) -> String {
		"RasterBrushTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Brush Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Brush
	}
}

impl LayoutHolder for BrushTool {
	fn layout(&self) -> Layout {
		let widgets = vec![
			ColorInput::new(FillChoiceUI::from(self.options.color.fill_choice.as_ref().unwrap_or(&FillChoice::None)))
				.mixed(self.options.color.fill_choice.is_none())
				.narrow(true)
				.on_update(|color: &ColorInput| {
					BrushToolMessage::UpdateOptions {
						options: BrushToolMessageOptionsUpdate::Color(color.value.as_solid().map(Color::from)),
					}
					.into()
				})
				.widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			NumberInput::new(Some(self.options.diameter))
				.label("Diameter")
				.min(1.)
				.unit(" px")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions { options: BrushToolMessageOptionsUpdate::Diameter(number_input.value.unwrap()) }.into())
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(Some(self.options.hardness))
				.label("Hardness")
				.min(0.)
				.max(100.)
				.mode_range()
				.unit("%")
				.on_update(|number_input: &NumberInput| {
					BrushToolMessage::UpdateOptions {
						options: BrushToolMessageOptionsUpdate::Hardness(number_input.value.unwrap()),
					}
					.into()
				})
				.widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			NumberInput::new(Some(self.options.flow))
				.label("Flow")
				.min(1.)
				.max(100.)
				.mode_range()
				.unit("%")
				.on_update(|number_input: &NumberInput| {
					BrushToolMessage::UpdateOptions {
						options: BrushToolMessageOptionsUpdate::Flow(number_input.value.unwrap()),
					}
					.into()
				})
				.widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for BrushTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		if matches!(&message, ToolMessage::Brush(BrushToolMessage::SelectionChanged)) {
			if self.fsm_state == BrushToolFsmState::Ready && selection_changed_since_last_sync(&mut self.options.last_synced_selection, context.document) {
				self.sync_options_from_selection(context.document, responses);
			}
			return;
		}

		let ToolMessage::Brush(BrushToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, true);
			return;
		};
		match options {
			BrushToolMessageOptionsUpdate::ChangeDiameter(change) => {
				let needs_rounding = ((self.options.diameter + change.abs() / 2.) % change.abs() - change.abs() / 2.).abs() > 0.5;
				if needs_rounding && change > 0. {
					self.options.diameter = (self.options.diameter / change.abs()).ceil() * change.abs();
				} else if needs_rounding && change < 0. {
					self.options.diameter = (self.options.diameter / change.abs()).floor() * change.abs();
				} else {
					self.options.diameter = (self.options.diameter / change.abs()).round() * change.abs() + change;
				}
				self.options.diameter = self.options.diameter.max(1.);
				self.send_layout(responses, LayoutTarget::ToolOptions);
			}
			BrushToolMessageOptionsUpdate::Diameter(diameter) => self.options.diameter = diameter,
			BrushToolMessageOptionsUpdate::Hardness(hardness) => self.options.hardness = hardness,
			BrushToolMessageOptionsUpdate::Flow(flow) => self.options.flow = flow,
			BrushToolMessageOptionsUpdate::Color(color) => {
				if let Some(color) = color {
					responses.add(ToolMessage::SelectWorkingColor { color, primary: true });
				}
			}
			BrushToolMessageOptionsUpdate::WorkingColorsChanged => {
				self.options.color.fill_choice = Some(solid(context.global_tool_data.primary_color));
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			BrushToolFsmState::Ready => actions!(BrushToolMessageDiscriminant;
				DragStart,
				DragStop,
				UpdateOptions,
			),
			BrushToolFsmState::Drawing => actions!(BrushToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
				UpdateOptions,
			),
		}
	}
}

impl ToolTransition for BrushTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(BrushToolMessage::Abort.into()),
			selection_changed: Some(BrushToolMessage::SelectionChanged.into()),
			working_color_changed: Some(BrushToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

impl BrushTool {
	fn sync_options_from_selection(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(strokes_node) = selected_strokes_node(document) else { return };
		let Some(node) = document.network_interface.document_network().nodes.get(&strokes_node) else {
			return;
		};
		let value = |index: usize| node.inputs.get(index).and_then(|input| input.as_value());
		if let Some(TaggedValue::F64(diameter)) = value(STROKES_DIAMETER_INPUT) {
			self.options.diameter = *diameter;
		}
		if let Some(TaggedValue::F64(hardness)) = value(STROKES_HARDNESS_INPUT) {
			self.options.hardness = *hardness;
		}
		if let Some(TaggedValue::F64(flow)) = value(STROKES_FLOW_INPUT) {
			self.options.flow = *flow;
		}
		if let Some(TaggedValue::Color(Some(color))) = value(STROKES_COLOR_INPUT)
			&& *color != self.options.active_color()
		{
			responses.add(ToolMessage::SelectWorkingColor { color: *color, primary: true });
		} else {
			self.send_layout(responses, LayoutTarget::ToolOptions);
		}
	}
}

const STROKES_COLOR_INPUT: usize = 1;
const STROKES_DIAMETER_INPUT: usize = 2;
const STROKES_HARDNESS_INPUT: usize = 3;
const STROKES_FLOW_INPUT: usize = 4;

#[derive(Clone, Debug, Default)]
struct BrushToolData {
	stroke: Stroke,
	stroke_node_id: Option<NodeId>,
	strokes_before: Vec<Stroke>,
	layer: Option<LayerNodeIdentifier>,
	transform: DAffine2,
	last_sample: (DVec2, Option<f64>),
}

enum BrushTarget {
	Existing { strokes_node_id: NodeId, strokes: Vec<Stroke> },
	NewGroup { parent: LayerNodeIdentifier, insert_index: usize },
	FillEmpty { layer: LayerNodeIdentifier },
}

impl BrushToolData {
	fn resolve_target(&mut self, document: &DocumentMessageHandler, options: &BrushOptions) -> Option<(LayerNodeIdentifier, BrushTarget)> {
		self.layer = None;

		let selected_nodes = document.network_interface.selected_nodes();
		let mut selected_layers = selected_nodes.selected_layers(document.metadata());
		let selected_layer = selected_layers.next().filter(|_| selected_layers.next().is_none())?;

		if self.load_brush_layer(document, selected_layer) {
			return Some((
				selected_layer,
				BrushTarget::NewGroup {
					parent: selected_layer,
					insert_index: 0,
				},
			));
		}

		let parent = selected_layer.parent(document.metadata()).filter(|&parent| parent != LayerNodeIdentifier::ROOT_PARENT)?;
		if !self.load_brush_layer(document, parent) {
			return None;
		}

		let Some(output) = document.network_interface.upstream_output_connector(&InputConnector::node(selected_layer.to_node(), 1), &[]) else {
			return Some((parent, BrushTarget::FillEmpty { layer: selected_layer }));
		};

		let new_group = || {
			let insert_index = parent.children(document.metadata()).position(|child| child == selected_layer).unwrap_or_default();
			BrushTarget::NewGroup { parent, insert_index }
		};
		let OutputConnector::Node { node_id: strokes_node_id, .. } = output else {
			return Some((parent, new_group()));
		};
		if document.network_interface.reference(&strokes_node_id, &[]) != Some(DefinitionIdentifier::ProtoNode(graphene_std::brush::brush_strokes::IDENTIFIER)) {
			return Some((parent, new_group()));
		}
		let strokes = document
			.network_interface
			.document_network()
			.nodes
			.get(&strokes_node_id)
			.and_then(|node| node.inputs.first())
			.and_then(|input| input.as_value())
			.and_then(|value| if let TaggedValue::Strokes(strokes) = value { Some(strokes.clone()) } else { None });
		match strokes {
			Some(strokes) if Self::style_matches(document, strokes_node_id, options) => Some((parent, BrushTarget::Existing { strokes_node_id, strokes })),
			_ => Some((parent, new_group())),
		}
	}

	fn load_brush_layer(&mut self, document: &DocumentMessageHandler, candidate: LayerNodeIdentifier) -> bool {
		self.transform = DAffine2::IDENTITY;

		for node_id in document.network_interface.upstream_flow_back_from_nodes(vec![candidate.to_node()], &[], FlowType::HorizontalFlow) {
			let Some(node) = document.network_interface.document_network().nodes.get(&node_id) else {
				continue;
			};
			let Some(reference) = document.network_interface.reference(&node_id, &[]) else {
				continue;
			};

			if reference == DefinitionIdentifier::ProtoNode(active_brush::IDENTIFIER) && node_id != candidate.to_node() {
				self.layer = Some(candidate);
				return true;
			}

			if reference == DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER) {
				self.transform = get_current_transform(&node.inputs) * self.transform;
			}
		}

		self.transform = DAffine2::IDENTITY;
		false
	}

	fn style_matches(document: &DocumentMessageHandler, strokes_node: NodeId, options: &BrushOptions) -> bool {
		let Some(node) = document.network_interface.document_network().nodes.get(&strokes_node) else {
			return false;
		};
		let value = |index: usize| node.inputs.get(index).and_then(|input| input.as_value());
		matches!(value(STROKES_COLOR_INPUT), Some(TaggedValue::Color(color)) if *color == Some(options.active_color()))
			&& matches!(value(STROKES_DIAMETER_INPUT), Some(TaggedValue::F64(diameter)) if *diameter == options.diameter)
			&& matches!(value(STROKES_HARDNESS_INPUT), Some(TaggedValue::F64(hardness)) if *hardness == options.hardness)
			&& matches!(value(STROKES_FLOW_INPUT), Some(TaggedValue::F64(flow)) if *flow == options.flow)
	}

	fn push_sample(&mut self, position: DVec2, pressure: Option<f64>, elapsed_milliseconds: f64) {
		self.stroke.position.push(position);
		if let Channel::Samples(times) = &mut self.stroke.time {
			times.push(elapsed_milliseconds / 1000.);
		}
		if let Channel::Samples(pressures) = &mut self.stroke.pressure {
			pressures.push(pressure.unwrap_or(1.) as f32);
		}
	}

	fn update_stroke(&self, responses: &mut VecDeque<Message>) {
		let Some(stroke_node_id) = self.stroke_node_id else { return };
		let mut strokes = self.strokes_before.clone();
		strokes.push(self.stroke.clone());
		responses.add(NodeGraphMessage::SetInputValue {
			node_id: stroke_node_id,
			input_index: 0,
			value: TaggedValue::Strokes(strokes),
		});
	}
}

impl Fsm for BrushToolFsmState {
	type ToolData = BrushToolData;
	type ToolOptions = BrushOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext { document, input, .. } = tool_action_data;

		let ToolMessage::Brush(event) = event else { return self };
		match (self, event) {
			(BrushToolFsmState::Ready, BrushToolMessage::DragStart) => {
				responses.add(DocumentMessage::StartTransaction);

				// A new brush layer needs a graph run before the stroke can start.
				let Some((brush_layer, target)) = tool_data.resolve_target(document, tool_options) else {
					new_brush_layer(document, responses);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![BrushToolMessage::DragStart.into()],
					});
					return BrushToolFsmState::Ready;
				};

				let pos = document
					.network_interface
					.document_metadata()
					.downstream_transform_to_viewport(brush_layer)
					.inverse()
					.transform_point2(input.mouse.position);
				let layer_position = tool_data.transform.inverse().transform_point2(pos);

				let pressure = input.mouse.pressure;
				tool_data.stroke = Stroke {
					time: Channel::Samples(Vec::new()),
					seed: generate_uuid(),
					..Default::default()
				};
				if pressure.is_some() {
					tool_data.stroke.pressure = Channel::Samples(Vec::new());
				}
				tool_data.push_sample(layer_position, pressure, input.mouse.time.unwrap_or(0.));
				tool_data.last_sample = (input.mouse.position, pressure);

				match target {
					BrushTarget::Existing { strokes_node_id, strokes } => {
						tool_data.stroke_node_id = Some(strokes_node_id);
						tool_data.strokes_before = strokes;
					}
					BrushTarget::NewGroup { parent, insert_index } => {
						let group_id = NodeId::new();
						let strokes_node_id = NodeId::new();
						tool_data.stroke_node_id = Some(strokes_node_id);
						tool_data.strokes_before = Vec::new();
						responses.add(GraphOperationMessage::NewBrushGroupLayer {
							id: group_id,
							strokes_node_id,
							parent,
							insert_index,
							color: tool_options.active_color(),
							diameter: tool_options.diameter,
							hardness: tool_options.hardness,
							flow: tool_options.flow,
						});
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![group_id] });
					}
					BrushTarget::FillEmpty { layer } => {
						let strokes_node_id = NodeId::new();
						tool_data.stroke_node_id = Some(strokes_node_id);
						tool_data.strokes_before = Vec::new();
						responses.add(GraphOperationMessage::NewBrushStrokesNode {
							layer,
							strokes_node_id,
							color: tool_options.active_color(),
							diameter: tool_options.diameter,
							hardness: tool_options.hardness,
							flow: tool_options.flow,
						});
					}
				}
				tool_data.update_stroke(responses);

				BrushToolFsmState::Drawing
			}

			(BrushToolFsmState::Drawing, BrushToolMessage::PointerMove) => {
				let pressure = input.mouse.pressure;

				if pressure == Some(0.) {
					return BrushToolFsmState::Drawing;
				}

				let (last_position, last_pressure) = tool_data.last_sample;
				let moved = input.mouse.position.distance(last_position) >= SAMPLE_MERGE_DISTANCE;
				let pressure_changed = match (pressure, last_pressure) {
					(Some(pressure), Some(last_pressure)) => (pressure - last_pressure).abs() >= SAMPLE_MERGE_PRESSURE,
					(pressure, last_pressure) => pressure.is_some() != last_pressure.is_some(),
				};
				if !moved && !pressure_changed {
					return BrushToolFsmState::Drawing;
				}

				if let Some(layer) = tool_data.layer {
					let layer_position = document
						.network_interface
						.document_metadata()
						.downstream_transform_to_viewport(layer)
						.inverse()
						.transform_point2(input.mouse.position);
					let layer_position = tool_data.transform.inverse().transform_point2(layer_position);

					tool_data.push_sample(layer_position, pressure, input.mouse.time.unwrap_or(0.));
					tool_data.last_sample = (input.mouse.position, pressure);
				}
				tool_data.update_stroke(responses);

				BrushToolFsmState::Drawing
			}

			(BrushToolFsmState::Drawing, BrushToolMessage::DragStop) => {
				if tool_data.stroke_node_id.is_some() {
					responses.add(DocumentMessage::EndTransaction);
				} else {
					responses.add(DocumentMessage::AbortTransaction);
				}
				tool_data.stroke_node_id = None;
				tool_data.stroke = Stroke::default();
				tool_data.strokes_before = Vec::new();

				BrushToolFsmState::Ready
			}
			(BrushToolFsmState::Drawing, BrushToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.stroke_node_id = None;
				tool_data.stroke = Stroke::default();
				tool_data.strokes_before = Vec::new();

				BrushToolFsmState::Ready
			}
			(_, BrushToolMessage::WorkingColorChanged) => {
				responses.add(BrushToolMessage::UpdateOptions {
					options: BrushToolMessageOptionsUpdate::WorkingColorsChanged,
				});
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			BrushToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw")]),
				HintGroup(vec![HintInfo::multi_keys([[Key::BracketLeft], [Key::BracketRight]], "Shrink/Grow Brush")]),
			]),
			BrushToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn selected_strokes_node(document: &DocumentMessageHandler) -> Option<NodeId> {
	let selected_nodes = document.network_interface.selected_nodes();
	let mut selected_layers = selected_nodes.selected_layers(document.metadata());
	let selected_layer = selected_layers.next().filter(|_| selected_layers.next().is_none())?;

	let group = if is_brush_layer(document, selected_layer) {
		selected_layer.children(document.metadata()).next()?
	} else {
		let parent = selected_layer.parent(document.metadata()).filter(|&parent| parent != LayerNodeIdentifier::ROOT_PARENT)?;
		if !is_brush_layer(document, parent) {
			return None;
		}
		selected_layer
	};

	let OutputConnector::Node { node_id, .. } = document.network_interface.upstream_output_connector(&InputConnector::node(group.to_node(), 1), &[])? else {
		return None;
	};
	(document.network_interface.reference(&node_id, &[]) == Some(DefinitionIdentifier::ProtoNode(graphene_std::brush::brush_strokes::IDENTIFIER))).then_some(node_id)
}

fn is_brush_layer(document: &DocumentMessageHandler, candidate: LayerNodeIdentifier) -> bool {
	document
		.network_interface
		.upstream_flow_back_from_nodes(vec![candidate.to_node()], &[], FlowType::HorizontalFlow)
		.any(|node_id| node_id != candidate.to_node() && document.network_interface.reference(&node_id, &[]) == Some(DefinitionIdentifier::ProtoNode(active_brush::IDENTIFIER)))
}

fn new_brush_layer(document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	responses.add(DocumentMessage::DeselectAllLayers);

	let brush_node = resolve_proto_node_type(active_brush::IDENTIFIER)
		.expect("Brush node does not exist")
		.node_template_input_override([None, Some(NodeInput::value(TaggedValue::BrushCache(Default::default()), false))]);

	let id = NodeId::new();
	responses.add(GraphOperationMessage::NewCustomLayer {
		id,
		nodes: vec![(NodeId(0), brush_node)],
		parent: document.new_layer_parent(true),
		insert_index: 0,
	});
	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });

	LayerNodeIdentifier::new_unchecked(id)
}
