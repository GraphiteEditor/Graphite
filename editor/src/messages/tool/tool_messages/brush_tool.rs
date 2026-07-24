use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::consts::DEFAULT_BRUSH_SIZE;
use crate::messages::portfolio::document::graph_operation::transform_utils::get_current_transform;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_proto_node_type};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, InputConnector, OutputConnector};
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, solid};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Color;
use graphene_std::brush::airbrush::airbrush as active_brush;
use graphene_std::brush::{Channel, Stroke};
use graphene_std::vector::style::{FillChoice, FillChoiceUI};

const BRUSH_MAX_SIZE: f64 = 5000.;

/// Viewport-space distance below which a new sample is merged into the previous one.
const SAMPLE_MERGE_DISTANCE: f64 = 1.;
/// Pressure change that records a sample even without movement, so pressure ramps from a
/// stationary pen survive the merge.
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
}

impl Default for BrushOptions {
	fn default() -> Self {
		Self {
			diameter: DEFAULT_BRUSH_SIZE,
			hardness: 80.,
			flow: 100.,
			color: ToolColorOptions::default(),
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
				.max(BRUSH_MAX_SIZE) /* Anything bigger would cause the application to be unresponsive and eventually die */
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
				// User picked a color: push to the global primary working color (no tool-local customization).
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
			working_color_changed: Some(BrushToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

/// Document input indices of the brush strokes node's style parameters (input 0 is the strokes
/// value).
const STROKES_COLOR_INPUT: usize = 1;
const STROKES_DIAMETER_INPUT: usize = 2;
const STROKES_HARDNESS_INPUT: usize = 3;
const STROKES_FLOW_INPUT: usize = 4;

#[derive(Clone, Debug, Default)]
struct BrushToolData {
	/// The in-progress stroke, appended after `strokes_before` into the strokes node's value
	/// input while drawing.
	stroke: Stroke,
	stroke_node_id: Option<NodeId>,
	/// The finished strokes already in the strokes node, read from the document at pen-down (the
	/// document is the source of truth — undo may have removed strokes since the last drag).
	strokes_before: Vec<Stroke>,
	/// The brush layer being drawn into.
	layer: Option<LayerNodeIdentifier>,
	/// The brush node in `layer`'s chain, whose strokes input feeds from the strokes node.
	brush_node_id: Option<NodeId>,
	transform: DAffine2,
	/// Event timestamp (ms) at pen-down; the stroke's time channel is relative to it.
	start_time: f64,
	/// Viewport position and pressure of the last recorded sample, for merging near-duplicates.
	last_sample: (DVec2, Option<f64>),
}

impl BrushToolData {
	/// Finds the brush node upstream of the single selected layer and returns that layer; style
	/// differences are handled per strokes group at pen-down, not by starting a new layer.
	fn load_existing_brush(&mut self, document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
		self.transform = DAffine2::IDENTITY;
		self.brush_node_id = None;

		if document.network_interface.selected_nodes().selected_layers(document.metadata()).count() != 1 {
			return None;
		}
		let layer = document.network_interface.selected_nodes().selected_layers(document.metadata()).next()?;

		for node_id in document.network_interface.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::HorizontalFlow) {
			let Some(node) = document.network_interface.document_network().nodes.get(&node_id) else {
				continue;
			};
			let Some(reference) = document.network_interface.reference(&node_id, &[]) else {
				continue;
			};

			if reference == DefinitionIdentifier::ProtoNode(active_brush::IDENTIFIER) && node_id != layer.to_node() {
				self.brush_node_id = Some(node_id);
				// A foreign strokes input means this layer is not tool-managed; start a new one.
				if self.find_strokes_node(document).is_err() {
					self.brush_node_id = None;
					return None;
				}

				self.layer = Some(layer);
				return Some(layer);
			}

			if reference == DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER) {
				self.transform = get_current_transform(&node.inputs) * self.transform;
			}
		}

		self.transform = DAffine2::IDENTITY;
		None
	}

	/// Whether the strokes node's style inputs equal the current tool options.
	fn style_matches(document: &DocumentMessageHandler, strokes_node: NodeId, options: &BrushOptions) -> bool {
		let Some(node) = document.network_interface.document_network().nodes.get(&strokes_node) else {
			return false;
		};
		let value = |index: usize| node.inputs.get(index).and_then(|input| input.as_value());
		matches!(value(STROKES_COLOR_INPUT), Some(TaggedValue::Color(color)) if *color == Some(options.active_color()))
			&& matches!(value(STROKES_DIAMETER_INPUT), Some(TaggedValue::F64(diameter)) if *diameter == options.diameter)
			&& matches!(value(STROKES_HARDNESS_INPUT), Some(TaggedValue::F64(hardness)) if *hardness == options.hardness / 100.)
			&& matches!(value(STROKES_FLOW_INPUT), Some(TaggedValue::F64(flow)) if *flow == options.flow / 100.)
	}

	/// Appends one sample. A stroke that started without pressure (mouse) keeps the uniform
	/// pressure default instead of storing fabricated per-sample data.
	fn push_sample(&mut self, position: DVec2, pressure: Option<f64>, elapsed_seconds: f64) {
		self.stroke.position.push(position);
		if let Channel::Samples(times) = &mut self.stroke.time {
			times.push(elapsed_seconds);
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

	/// The brush strokes node in the chain of the top group layer feeding the brush node (or wired
	/// in directly), with its current strokes. Only the top group may be appended to: matching a
	/// lower group would slide new strokes underneath everything drawn since.
	/// `Ok(None)` means the input is an unconnected value — the caller creates the first group.
	/// `Err` means it is fed by something this tool does not manage, which must not be clobbered.
	fn find_strokes_node(&self, document: &DocumentMessageHandler) -> Result<Option<(NodeId, Vec<Stroke>)>, ()> {
		let brush_node = self.brush_node_id.ok_or(())?;
		let Some(output) = document.network_interface.upstream_output_connector(&InputConnector::node(brush_node, 0), &[]) else {
			return Ok(None);
		};
		let OutputConnector::Node { node_id, .. } = output else { return Err(()) };
		let mut node_id = node_id;
		if document.network_interface.reference(&node_id, &[]) == Some(DefinitionIdentifier::Network("Merge".into())) {
			let Some(OutputConnector::Node { node_id: chain_node, .. }) = document.network_interface.upstream_output_connector(&InputConnector::node(node_id, 1), &[]) else {
				return Err(());
			};
			node_id = chain_node;
		}
		let reference = document.network_interface.reference(&node_id, &[]).ok_or(())?;
		if reference != DefinitionIdentifier::ProtoNode(graphene_std::brush::brush_strokes::brush_strokes::IDENTIFIER) {
			return Err(());
		}
		let node = document.network_interface.document_network().nodes.get(&node_id).ok_or(())?;
		let Some(TaggedValue::Strokes(strokes)) = node.inputs.first().and_then(|input| input.as_value()) else {
			return Err(());
		};
		Ok(Some((node_id, strokes.clone())))
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
				let loaded_layer = tool_data.load_existing_brush(document);

				if let Some(layer) = loaded_layer {
					let pos = document
						.network_interface
						.document_metadata()
						.downstream_transform_to_viewport(layer)
						.inverse()
						.transform_point2(input.mouse.position);
					let layer_position = tool_data.transform.inverse().transform_point2(pos);

					// Start a new stroke. A pressure-reporting device gets a per-sample pressure
					// channel; a mouse keeps pressure at the uniform default.
					let pressure = input.mouse.pressure;
					tool_data.start_time = input.mouse.time.unwrap_or(input.time as f64);
					tool_data.stroke = Stroke {
						time: Channel::Samples(Vec::new()),
						seed: generate_uuid(),
						..Default::default()
					};
					if pressure.is_some() {
						tool_data.stroke.pressure = Channel::Samples(Vec::new());
					}
					tool_data.push_sample(layer_position, pressure, 0.);
					tool_data.last_sample = (input.mouse.position, pressure);

					// Strokes append to the top styling group when its style matches the tool
					// options; otherwise a fresh group layer is stacked on top.
					match tool_data.find_strokes_node(document) {
						Ok(Some((strokes_node_id, strokes))) if BrushToolData::style_matches(document, strokes_node_id, tool_options) => {
							tool_data.stroke_node_id = Some(strokes_node_id);
							tool_data.strokes_before = strokes;
						}
						Ok(_) => {
							let strokes_node_id = NodeId::new();
							tool_data.stroke_node_id = Some(strokes_node_id);
							tool_data.strokes_before = Vec::new();
							responses.add(GraphOperationMessage::NewBrushGroupLayer {
								id: NodeId::new(),
								strokes_node_id,
								parent: layer,
								color: tool_options.active_color(),
								diameter: tool_options.diameter,
								hardness: tool_options.hardness / 100.,
								flow: tool_options.flow / 100.,
							});
						}
						Err(()) => {
							responses.add(DocumentMessage::AbortTransaction);
							return BrushToolFsmState::Ready;
						}
					}
					tool_data.update_stroke(responses);

					BrushToolFsmState::Drawing
				}
				// Create the new brush layer, wait for the graph run, and then start the stroke on it
				else {
					new_brush_layer(document, responses);
					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![BrushToolMessage::DragStart.into()],
					});
					BrushToolFsmState::Ready
				}
			}

			(BrushToolFsmState::Drawing, BrushToolMessage::PointerMove) => {
				let pressure = input.mouse.pressure;

				if pressure == Some(0.) {
					return BrushToolFsmState::Drawing;
				}

				// A resting pen streams samples that render identically; merge them instead of
				// growing the stroke (and re-sending it) on every event.
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

					let elapsed = (input.mouse.time.unwrap_or(input.time as f64) - tool_data.start_time).max(0.) / 1000.;
					tool_data.push_sample(layer_position, pressure, elapsed);
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

fn new_brush_layer(document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	responses.add(DocumentMessage::DeselectAllLayers);

	// Input 0 (the strokes input) and the trailing pipeline scope input keep their defaults.
	// The texture cache must be a fresh handle: the definition template holds one shared default,
	// and cloning it would share the cache across every brush layer.
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
