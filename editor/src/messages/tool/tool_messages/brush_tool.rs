use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::{DocumentToolData, EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::LayerId;
use document_legacy::Operation;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::{concrete, Type, TypeDescriptor};
use graphene_core::vector::style::Stroke;
use graphene_core::Cow;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct BrushTool {
	fsm_state: BrushToolFsmState,
	data: BrushToolData,
	options: BrushOptions,
}

pub struct BrushOptions {
	line_weight: f64,
}

impl Default for BrushOptions {
	fn default() -> Self {
		Self { line_weight: 5. }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Brush)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BrushToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions(BrushToolMessageOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum BrushToolMessageOptionsUpdate {
	LineWeight(f64),
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
		let weight = NumberInput::new(Some(self.options.line_weight))
			.unit(" px")
			.label("Weight")
			.min(1.)
			.on_update(|number_input: &NumberInput| BrushToolMessage::UpdateOptions(BrushToolMessageOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
			.widget_holder();
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: vec![weight] }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for BrushTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Brush(BrushToolMessage::UpdateOptions(action)) = message {
			match action {
				BrushToolMessageOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
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
			),
			Drawing => actions!(BrushToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
			),
		}
	}
}

impl ToolTransition for BrushTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(BrushToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Debug, Default)]
struct BrushToolData {
	points: Vec<DVec2>,
	weight: f64,
	path: Option<Vec<LayerId>>,
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
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					tool_data.path = Some(document.get_path_for_new_layer());

					let pos = transform.inverse().transform_point2(input.mouse.position);

					tool_data.points.push(pos);

					tool_data.weight = tool_options.line_weight;

					add_polyline(tool_data, global_tool_data, responses);

					Drawing
				}
				(Drawing, PointerMove) => {
					let pos = transform.inverse().transform_point2(input.mouse.position);

					if tool_data.points.last() != Some(&pos) {
						tool_data.points.push(pos);
					}

					add_polyline(tool_data, global_tool_data, responses);

					Drawing
				}
				(Drawing, DragStop) | (Drawing, Abort) => {
					if !tool_data.points.is_empty() {
						responses.push_back(remove_preview(tool_data));
						add_brush_render(tool_data, global_tool_data, responses);
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					tool_data.path = None;
					tool_data.points.clear();

					Ready
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

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

fn remove_preview(data: &BrushToolData) -> Message {
	Operation::DeleteLayer { path: data.path.clone().unwrap() }.into()
}

fn add_polyline(data: &BrushToolData, tool_data: &DocumentToolData, responses: &mut VecDeque<Message>) {
	let layer_path = data.path.clone().unwrap();
	let subpath = bezier_rs::Subpath::from_anchors(data.points.iter().copied(), false);
	graph_modification_utils::new_vector_layer(vec![subpath], layer_path.clone(), responses);

	responses.add(GraphOperationMessage::StrokeSet {
		layer: layer_path,
		stroke: Stroke::new(tool_data.primary_color, data.weight),
	});
}

fn add_brush_render(data: &BrushToolData, tool_data: &DocumentToolData, responses: &mut VecDeque<Message>) {
	let layer_path = data.path.clone().unwrap();
	log::debug!("Points: {:?}", data.points);
	let brush_node = DocumentNode {
		name: "Brush".to_string(),
		inputs: vec![
			NodeInput::ShortCircut(concrete!(())),
			NodeInput::value(TaggedValue::VecDVec2(data.points.clone()), false),
			// Size
			NodeInput::value(TaggedValue::F64(data.weight), false),
			// Hardness
			NodeInput::value(TaggedValue::F64(0.0), false),
			// Opacity
			NodeInput::value(TaggedValue::F64(1.0), false),
			NodeInput::value(TaggedValue::Color(tool_data.primary_color), false),
		],
		implementation: DocumentNodeImplementation::Unresolved("graphene_std::brush::BrushNode".into()),
		metadata: graph_craft::document::DocumentNodeMetadata { position: (8, 4).into() },
	};
	let mut network = NodeNetwork::value_network(brush_node);
	network.push_output_node();
	graph_modification_utils::new_custom_layer(network, layer_path.clone(), responses);
}
