use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::LayerId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graphene_core::raster::ImageFrame;
use graphene_core::vector::brush_stroke::{BrushInputSample, BrushStroke, BrushStyle};
use graphene_core::Color;

use serde::{Deserialize, Serialize};

#[derive(Default)]
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
			diameter: 40.,
			hardness: 50.,
			flow: 100.,
			color: ToolColorOptions::default(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Brush)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BrushToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions(BrushToolMessageOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BrushToolMessageOptionsUpdate {
	ChangeDiameter(f64),
	Color(Option<Color>),
	ColorType(ToolColorType),
	Diameter(f64),
	Flow(f64),
	Hardness(f64),
	WorkingColors(Option<Color>, Option<Color>),
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
	fn tooltip(&self) -> String {
		"Brush Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Brush
	}
}

impl PropertyHolder for BrushTool {
	fn properties(&self) -> Layout {
		let mut widgets = vec![
			NumberInput::new(Some(self.options.diameter))
				.label("Diameter")
				.min(1.)
				.unit(" px")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Diameter(number_input.value.unwrap())).into())
				.widget_holder(),
			WidgetHolder::related_separator(),
			NumberInput::new(Some(self.options.hardness))
				.label("Hardness")
				.min(0.)
				.max(100.)
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Hardness(number_input.value.unwrap())).into())
				.widget_holder(),
			WidgetHolder::related_separator(),
			NumberInput::new(Some(self.options.flow))
				.label("Flow")
				.min(1.)
				.max(100.)
				.unit("%")
				.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Flow(number_input.value.unwrap())).into())
				.widget_holder(),
		];

		widgets.push(WidgetHolder::section_separator());

		widgets.append(&mut self.options.color.create_widgets(
			"Color",
			false,
			WidgetCallback::new(|_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Color(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::ColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::Color(color.value)).into()),
		));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for BrushTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Brush(BrushToolMessage::UpdateOptions(action)) = message {
			match action {
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
					self.register_properties(responses, LayoutTarget::ToolOptions);
				}
				BrushToolMessageOptionsUpdate::Diameter(diameter) => self.options.diameter = diameter,
				BrushToolMessageOptionsUpdate::Hardness(hardness) => self.options.hardness = hardness,
				BrushToolMessageOptionsUpdate::Flow(flow) => self.options.flow = flow,
				BrushToolMessageOptionsUpdate::Color(color) => {
					self.options.color.custom_color = color;
					self.options.color.color_type = ToolColorType::Custom;
				}
				BrushToolMessageOptionsUpdate::ColorType(color_type) => self.options.color.color_type = color_type,
				BrushToolMessageOptionsUpdate::WorkingColors(primary, secondary) => {
					self.options.color.primary_working_color = primary;
					self.options.color.secondary_working_color = secondary;
				}
			}

			responses.add(LayoutMessage::SendLayout {
				layout: self.properties(),
				layout_target: LayoutTarget::ToolOptions,
			});

			return;
		}

		self.fsm_state.process_event(message, &mut self.data, tool_data, &self.options, responses, true);
	}

	fn actions(&self) -> ActionList {
		use BrushToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(BrushToolMessageDiscriminant;
				DragStart,
				DragStop,
				Abort,
				UpdateOptions,
			),
			Drawing => actions!(BrushToolMessageDiscriminant;
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

#[derive(Clone, Debug, Default)]
struct BrushToolData {
	strokes: Vec<BrushStroke>,
	path: Option<Vec<LayerId>>,
}

impl BrushToolData {
	fn update_strokes(&self, responses: &mut VecDeque<Message>) {
		if let Some(layer_path) = self.path.clone() {
			responses.add(NodeGraphMessage::SetQualifiedInputValue {
				layer_path,
				node_path: vec![0],
				input_index: 3,
				value: TaggedValue::BrushStrokes(self.strokes.clone()),
			});
		}
	}

	// fn update_image(&self, node_graph: &NodeGraphExecutor, responses: &mut VecDeque<Message>) {
	// 	let Some(image) = node_graph.introspect_node(&[1]) else { return; };
	// 	let image: &ImageFrame<Color> = image.downcast_ref().unwrap();
	// 	self.set_image(image.clone(), responses)
	// }
	//
	// fn set_image(&self, image_frame: ImageFrame<Color>, responses: &mut VecDeque<Message>) {
	// 	if let Some(layer_path) = self.path.clone() {
	// 		responses.add(NodeGraphMessage::SetQualifiedInputValue {
	// 			layer_path,
	// 			node_path: vec![0],
	// 			input_index: 1,
	// 			value: TaggedValue::ImageFrame(image_frame),
	// 		});
	// 	}
	// }
}

impl Fsm for BrushToolFsmState {
	type ToolData = BrushToolData;
	type ToolOptions = BrushOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData {
			document, global_tool_data, input, ..
		}: &mut ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use BrushToolFsmState::*;
		use BrushToolMessage::*;

		let transform = document.document_legacy.root.transform;

		if let ToolMessage::Brush(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.add(DocumentMessage::StartTransaction);
					let existing_strokes = load_existing_strokes(document);
					let new_layer = existing_strokes.is_none();
					if let Some((layer_path, strokes)) = existing_strokes {
						tool_data.path = Some(layer_path);
						tool_data.strokes = strokes;
					} else {
						responses.add(DocumentMessage::DeselectAllLayers);
						tool_data.path = Some(document.get_path_for_new_layer());
					}

					// Start a new stroke with a single sample.
					let position = transform.inverse().transform_point2(input.mouse.position);
					tool_data.strokes.push(BrushStroke {
						trace: vec![BrushInputSample { position }],
						style: BrushStyle {
							color: tool_options.color.active_color().unwrap_or_default(),
							diameter: tool_options.diameter,
							hardness: tool_options.hardness,
							flow: tool_options.flow,
						},
					});

					if new_layer {
						add_brush_render(tool_options, tool_data, responses);
					} else {
						//tool_data.update_image(node_graph, responses);
						tool_data.update_strokes(responses);
					}

					Drawing
				}

				(Drawing, PointerMove) => {
					let position = transform.inverse().transform_point2(input.mouse.position);
					if let Some(stroke) = tool_data.strokes.last_mut() {
						stroke.trace.push(BrushInputSample { position })
					}
					tool_data.update_strokes(responses);

					Drawing
				}
				(Drawing, DragStop) | (Drawing, Abort) => {
					if !tool_data.strokes.is_empty() {
						responses.add(DocumentMessage::CommitTransaction);
					} else {
						responses.add(DocumentMessage::AbortTransaction);
					}

					tool_data.strokes.clear();
					tool_data.path = None;

					Ready
				}
				(_, WorkingColorChanged) => {
					responses.add(BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::WorkingColors(
						Some(global_tool_data.primary_color),
						Some(global_tool_data.secondary_color),
					)));
					self
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			BrushToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polyline")])]),
			BrushToolFsmState::Drawing => HintData(vec![]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn add_brush_render(tool_options: &BrushOptions, data: &BrushToolData, responses: &mut VecDeque<Message>) {
	let layer_path = data.path.clone().unwrap();

	let brush_node = DocumentNode {
		name: "Brush".to_string(),
		inputs: vec![
			NodeInput::value(TaggedValue::None, false),
			NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
			NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
			NodeInput::value(TaggedValue::BrushStrokes(data.strokes.clone()), false),
			// Diameter
			NodeInput::value(TaggedValue::F64(tool_options.diameter), false),
			// Hardness
			NodeInput::value(TaggedValue::F64(tool_options.hardness), false),
			// Flow
			NodeInput::value(TaggedValue::F64(tool_options.flow), false),
			// Color
			NodeInput::value(TaggedValue::Color(tool_options.color.active_color().unwrap()), false),
		],
		implementation: DocumentNodeImplementation::Unresolved("graphene_std::brush::BrushNode".into()),
		metadata: graph_craft::document::DocumentNodeMetadata { position: (8, 4).into() },
		..Default::default()
	};
	// let monitor_node = DocumentNode {
	// 	name: "Monitor".to_string(),
	// 	implementation: DocumentNodeImplementation::Unresolved("graphene_std::memo::MonitorNode<_>".into()),
	// 	..Default::default()
	// };
	let mut network = NodeNetwork::value_network(brush_node);
	//network.push_node(monitor_node, true);
	network.push_output_node();
	graph_modification_utils::new_custom_layer(network, layer_path, responses);
}

fn load_existing_strokes(document: &DocumentMessageHandler) -> Option<(Vec<LayerId>, Vec<BrushStroke>)> {
	if document.selected_layers().count() != 1 {
		return None;
	}
	let layer_path = document.selected_layers().next()?.to_vec();
	let network = document.document_legacy.layer(&layer_path).ok().and_then(|layer| layer.as_layer_network().ok())?;
	let brush_node = network.nodes.get(&0)?;
	if brush_node.implementation != DocumentNodeImplementation::Unresolved("graphene_std::brush::BrushNode".into()) {
		return None;
	}
	let points_input = brush_node.inputs.get(3)?;
	let NodeInput::Value {
		tagged_value: TaggedValue::BrushStrokes(strokes),
		..
	} = points_input else {
		return None };

	Some((layer_path, strokes.clone()))
}
