use crate::consts::CREATE_CURVE_THRESHOLD;
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{LayoutRow, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{Fsm, ToolActionHandlerData};
use crate::viewport_tools::vector_editor::shape_editor::ShapeEditor;

use graphene::layers::style;
use graphene::layers::vector::vector_shape::VectorShape;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use kurbo::{PathEl, Point};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct PenTool {
	fsm_state: PenToolFsmState,
	tool_data: PenToolData,
	options: PenOptions,
}

pub struct PenOptions {
	line_weight: f64,
}

impl Default for PenOptions {
	fn default() -> Self {
		Self { line_weight: 5. }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PenToolMessage {
	// Standard messages
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	Undo,
	UpdateOptions(PenOptionsUpdate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	Drawing,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PenOptionsUpdate {
	LineWeight(f64),
}

impl PropertyHolder for PenTool {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: Some(self.options.line_weight),
				is_integer: false,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value.unwrap())).into()),
				..NumberInput::default()
			}))],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for PenTool {
	fn process_action(&mut self, action: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Pen(PenToolMessage::UpdateOptions(action)) = action {
			match action {
				PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.tool_data, tool_data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use PenToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(PenToolMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			Drawing => actions!(PenToolMessageDiscriminant; DragStart, DragStop, PointerMove, Confirm, Abort),
		}
	}
}

impl Default for PenToolFsmState {
	fn default() -> Self {
		PenToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct PenToolData {
	weight: f64,
	path: Option<Vec<LayerId>>,
	curve_shape: VectorShape,
	bez_path: Vec<PathEl>,
	snap_handler: SnapHandler,
	shape_editor: ShapeEditor,
	drag_start_position: DVec2,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;
	type ToolOptions = PenOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, global_tool_data, input, font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use PenToolFsmState::*;
		use PenToolMessage::*;

		let transform = document.graphene_document.root.transform;

		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(_, DocumentIsDirty) => {
					// TODO make sure that the shape outlines will update when the canvas moves
					// tool_data.shape_editor.update_shapes(document, responses);
					self
				}
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					// Create a new layer and prep snap system
					tool_data.path = Some(document.get_path_for_new_layer());
					tool_data.snap_handler.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
					tool_data.snap_handler.add_all_document_handles(document, &[], &[]);
					let snapped_position = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);

					// Get the position and set properties
					let start_position = transform.inverse().transform_point2(snapped_position);
					tool_data.weight = tool_options.line_weight;

					// Create the initial shape with a `bez_path` (only contains a moveto initially)
					if let Some(layer_path) = &tool_data.path {
						tool_data.bez_path = start_bez_path(start_position);
						responses.push_back(
							Operation::AddShape {
								path: layer_path.clone(),
								transform: transform.to_cols_array(),
								insert_index: -1,
								bez_path: tool_data.bez_path.clone().into_iter().collect(),
								style: style::PathStyle::new(Some(style::Stroke::new(global_tool_data.primary_color, tool_data.weight)), style::Fill::None),
							}
							.into(),
						);
					}

					add_to_curve(tool_data, input, transform, document, responses);
					Drawing
				}
				(Drawing, DragStart) => {
					tool_data.drag_start_position = input.mouse.position;
					add_to_curve(tool_data, input, transform, document, responses);
					Drawing
				}
				(Drawing, DragStop) => {
					// Deselect everything (this means we are no longer dragging the handle)
					tool_data.shape_editor.deselect_all_points(&document.graphene_document, responses);

					// If the drag does not exceed the threshold, then replace the curve with a line
					if tool_data.drag_start_position.distance(input.mouse.position) < CREATE_CURVE_THRESHOLD {
						// Modify the second to last element (as we have an unplaced element tracing to the cursor as the last element)
						let replace_index = tool_data.bez_path.len() - 2;
						let line_from_curve = convert_curve_to_line(tool_data.bez_path[replace_index]);
						replace_path_element(tool_data, transform, replace_index, line_from_curve, responses);
					}

					// Reselect the last point
					// if let Some(last_anchor) = tool_data.shape_editor.select_last_anchor() {
					// 	last_anchor.select_point(ControlPointType::Anchor as usize, true, responses);
					// }

					// Move the newly selected points to the cursor
					let snapped_position = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);
					tool_data.shape_editor.move_selected_points(snapped_position, responses);

					Drawing
				}
				(Drawing, PointerMove) => {
					// Move selected points
					let snapped_position = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);
					tool_data.shape_editor.move_selected_points(snapped_position, responses);

					Drawing
				}
				(Drawing, Confirm) | (Drawing, Abort) => {
					// Cleanup, we are either canceling or finished drawing
					if tool_data.bez_path.len() >= 2 {
						// Remove the last segment
						remove_from_curve(tool_data);
						if let Some(layer_path) = &tool_data.path {
							responses.push_back(apply_bez_path(layer_path.clone(), tool_data.bez_path.clone(), transform));
						}

						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					// TODO Tell overlay manager to remove the overlays
					//tool_data.shape_editor.remove_overlays();
					tool_data.shape_editor.clear_target_layers();

					tool_data.path = None;
					tool_data.snap_handler.cleanup(responses);

					Ready
				}
				(_, Abort) => {
					// TODO Tell overlay manager to remove the overlays
					//data.shape_editor.remove_overlays();
					tool_data.shape_editor.clear_target_layers();
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
			PenToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![],
				mouse: Some(MouseMotion::Lmb),
				label: String::from("Draw Path"),
				plus: false,
			}])]),
			PenToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Extend Path"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyEnter])],
					mouse: None,
					label: String::from("End Path"),
					plus: false,
				}]),
			]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

/// Add to the curve and select the second anchor of the last point and the newly added anchor point
fn add_to_curve(tool_data: &mut PenToolData, input: &InputPreprocessorMessageHandler, transform: DAffine2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	// Refresh tool_data's representation of the path
	update_path_representation(tool_data);

	// Setup our position params
	let snapped_position = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);
	let position = transform.inverse().transform_point2(snapped_position);

	// Add a curve to the path
	if let Some(layer_path) = &tool_data.path {
		// Push curve onto path
		let point = Point { x: position.x, y: position.y };
		tool_data.bez_path.push(PathEl::CurveTo(point, point, point));

		responses.push_back(apply_bez_path(layer_path.clone(), tool_data.bez_path.clone(), transform));

		// Clear previous overlays
		// TODO Tell the overlay manager to remove all overlays
		// tool_data.shape_editor.remove_overlays(responses);

		// TODO Rebuild consider no kurbo and async messages to graphene
		// Create a new `shape` from the updated `bez_path`
		// let bez_path = data.bez_path.clone().into_iter().collect();
		// tool_data.curve_shape = VectorShape::new(layer_path.to_vec(), transform, &bez_path, false, responses);
		// tool_data.shape_editor.set_shapes_to_modify(vec![data.curve_shape.clone()]);

		// // Select the second to last `PathEl`'s handle
		// tool_data.shape_editor.set_shape_selected(0);
		// let handle_element = tool_data.shape_editor.select_nth_anchor(0, -2);
		// handle_element.select_point(ControlPointType::Handle2 as usize, true, responses);

		// // Select the last `PathEl`'s anchor point
		// if let Some(last_anchor) = tool_data.shape_editor.select_last_anchor() {
		// 	last_anchor.select_point(ControlPointType::Anchor as usize, true, responses);
		// }
		// tool_data.shape_editor.set_selected_mirror_options(true, true);
	}
}

/// Replace a `PathEl` with another inside of `bez_path` by index
fn replace_path_element(tool_data: &mut PenToolData, transform: DAffine2, replace_index: usize, replacement: PathEl, responses: &mut VecDeque<Message>) {
	tool_data.bez_path[replace_index] = replacement;
	if let Some(layer_path) = &tool_data.path {
		responses.push_back(apply_bez_path(layer_path.clone(), tool_data.bez_path.clone(), transform));
	}
}

/// Remove a curve from the end of the `bez_path`
fn remove_from_curve(tool_data: &mut PenToolData) {
	// Refresh tool_data's representation of the path
	update_path_representation(tool_data);
	tool_data.bez_path.pop();
}

/// Create the initial moveto for the `bez_path`
fn start_bez_path(start_position: DVec2) -> Vec<PathEl> {
	vec![PathEl::MoveTo(Point {
		x: start_position.x,
		y: start_position.y,
	})]
}

/// Convert curve `PathEl` into a line `PathEl`
fn convert_curve_to_line(curve: PathEl) -> PathEl {
	match curve {
		PathEl::CurveTo(_, _, p) => PathEl::LineTo(p),
		_ => PathEl::MoveTo(Point::ZERO),
	}
}

/// Update data's version of `bez_path` to match `ShapeEditor`'s version
fn update_path_representation(tool_data: &mut PenToolData) {
	// TODO Rebuild consider no kurbo and async messages to graphene
	// if !tool_data.shape_editor.shapes_to_modify.is_empty() {
	// 	// Hacky way of saving the curve changes
	// 	tool_data.bez_path = data.shape_editor.shapes_to_modify[0].bez_path.elements().to_vec();
	// }
}

/// Apply the `bez_path` to the `shape` in the viewport
fn apply_bez_path(layer_path: Vec<LayerId>, bez_path: Vec<PathEl>, transform: DAffine2) -> Message {
	Operation::SetShapePathInViewport {
		path: layer_path,
		bez_path: bez_path.into_iter().collect(),
		transform: transform.to_cols_array(),
	}
	.into()
}
