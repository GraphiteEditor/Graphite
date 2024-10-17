use super::tool_prelude::*;
use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_functions::path_endpoint_overlays;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::utility_functions::should_extend;

use graph_craft::document::NodeId;
use graphene_core::vector::VectorModificationType;
use graphene_core::Color;
use graphene_std::vector::{PointId, SegmentId};

use glam::DVec2;

#[derive(Default)]
pub struct FreehandTool {
	fsm_state: FreehandToolFsmState,
	data: FreehandToolData,
	options: FreehandOptions,
}

pub struct FreehandOptions {
	line_weight: f64,
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for FreehandOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_none(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Freehand)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FreehandToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions(FreehandOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FreehandOptionsUpdate {
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FreehandToolFsmState {
	#[default]
	Ready,
	Drawing,
}

impl ToolMetadata for FreehandTool {
	fn icon_name(&self) -> String {
		"VectorFreehandTool".into()
	}
	fn tooltip(&self) -> String {
		"Freehand Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Freehand
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(1.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for FreehandTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::FillColor(color.value.as_solid())).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for FreehandTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Freehand(FreehandToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			FreehandOptionsUpdate::FillColor(color) => {
				self.options.fill.custom_color = color;
				self.options.fill.color_type = ToolColorType::Custom;
			}
			FreehandOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
			FreehandOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			FreehandOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			FreehandOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			FreehandOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
				self.options.fill.primary_working_color = primary;
				self.options.fill.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			FreehandToolFsmState::Ready => actions!(FreehandToolMessageDiscriminant;
				DragStart,
				DragStop,
			),
			FreehandToolFsmState::Drawing => actions!(FreehandToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
			),
		}
	}
}

impl ToolTransition for FreehandTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|overlay_context: OverlayContext| FreehandToolMessage::Overlays(overlay_context).into()),
			tool_abort: Some(FreehandToolMessage::Abort.into()),
			working_color_changed: Some(FreehandToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct FreehandToolData {
	end_point: Option<(DVec2, PointId)>,
	dragged: bool,
	weight: f64,
	layer: Option<LayerNodeIdentifier>,
}

impl Fsm for FreehandToolFsmState {
	type ToolData = FreehandToolData;
	type ToolOptions = FreehandOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			shape_editor,
			..
		} = tool_action_data;

		let ToolMessage::Freehand(event) = event else {
			return self;
		};
		match (self, event) {
			(_, FreehandToolMessage::Overlays(mut overlay_context)) => {
				path_endpoint_overlays(document, shape_editor, &mut overlay_context);

				self
			}
			(FreehandToolFsmState::Ready, FreehandToolMessage::DragStart) => {
				responses.add(DocumentMessage::StartTransaction);

				tool_data.dragged = false;
				tool_data.end_point = None;
				tool_data.weight = tool_options.line_weight;

				// Extend an endpoint of the selected path
				let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
				if let Some((layer, point, position)) = should_extend(document, input.mouse.position, crate::consts::SNAP_POINT_TOLERANCE, selected_nodes.selected_layers(document.metadata())) {
					tool_data.layer = Some(layer);
					tool_data.end_point = Some((position, point));

					extend_path_with_next_segment(tool_data, position, responses);

					return FreehandToolFsmState::Drawing;
				}

				responses.add(DocumentMessage::DeselectAllLayers);

				let parent = document.new_layer_parent(true);

				let node_type = resolve_document_node_type("Path").expect("Path node does not exist");
				let node = node_type.default_node_template();
				let nodes = vec![(NodeId(0), node)];

				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
				responses.add(Message::StartBuffer);
				tool_options.fill.apply_fill(layer, responses);
				tool_options.stroke.apply_stroke(tool_data.weight, layer, responses);
				tool_data.layer = Some(layer);

				FreehandToolFsmState::Drawing
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::PointerMove) => {
				if let Some(layer) = tool_data.layer {
					let transform = document.metadata().transform_to_viewport(layer);
					let position = transform.inverse().transform_point2(input.mouse.position);

					extend_path_with_next_segment(tool_data, position, responses);
				}

				FreehandToolFsmState::Drawing
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::DragStop) => {
				if tool_data.dragged {
					responses.add(DocumentMessage::CommitTransaction);
				} else {
					responses.add(DocumentMessage::EndTransaction);
				}

				tool_data.end_point = None;
				tool_data.layer = None;

				FreehandToolFsmState::Ready
			}
			(FreehandToolFsmState::Drawing, FreehandToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.layer = None;
				tool_data.end_point = None;

				FreehandToolFsmState::Ready
			}
			(_, FreehandToolMessage::WorkingColorChanged) => {
				responses.add(FreehandToolMessage::UpdateOptions(FreehandOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			FreehandToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Polyline")])]),
			FreehandToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn extend_path_with_next_segment(tool_data: &mut FreehandToolData, position: DVec2, responses: &mut VecDeque<Message>) {
	if !tool_data.end_point.map_or(true, |(last_pos, _)| position != last_pos) || !position.is_finite() {
		return;
	}

	let Some(layer) = tool_data.layer else { return };

	let id = PointId::generate();
	responses.add(GraphOperationMessage::Vector {
		layer,
		modification_type: VectorModificationType::InsertPoint { id, position },
	});

	if let Some((_, previous_position)) = tool_data.end_point {
		let next_id = SegmentId::generate();
		let points = [previous_position, id];

		responses.add(GraphOperationMessage::Vector {
			layer,
			modification_type: VectorModificationType::InsertSegment {
				id: next_id,
				points,
				handles: [None, None],
			},
		});
	}

	tool_data.dragged = true;
	tool_data.end_point = Some((position, id));
}
