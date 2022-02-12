use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{LayoutRow, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::viewport_tools::vector_editor::shape_editor::ShapeEditor;
use crate::viewport_tools::vector_editor::vector_shape::VectorShape;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use graphene::layers::style;
use kurbo::{PathEl, Point};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Pen {
	fsm_state: PenToolFsmState,
	data: PenToolData,
	options: PenOptions,
}

pub struct PenOptions {
	line_weight: u32,
}

impl Default for PenOptions {
	fn default() -> Self {
		Self { line_weight: 5 }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PenMessage {
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
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PenOptionsUpdate {
	LineWeight(u32),
}

impl PropertyHolder for Pen {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			name: "".into(),
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: self.options.line_weight as f64,
				is_integer: true,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input| PenMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value as u32)).into()),
				..NumberInput::default()
			}))],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Pen {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Pen(PenMessage::UpdateOptions(action)) = action {
			match action {
				PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &self.options, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use PenToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(PenMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			Drawing => actions!(PenMessageDiscriminant; DragStart, DragStop, PointerMove, Confirm, Abort),
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
	weight: u32,
	path: Option<Vec<LayerId>>,
	curve_shape: VectorShape,
	bez_path: Vec<PathEl>,
	snap_handler: SnapHandler,
	shape_editor: ShapeEditor,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;
	type ToolOptions = PenOptions;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use PenMessage::*;
		use PenToolFsmState::*;

		let transform = document.graphene_document.root.transform;

		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(_, DocumentIsDirty) => {
					data.shape_editor.update_shapes(document, responses);
					self
				}
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					// Create a new layer and prep snap system
					data.path = Some(document.get_path_for_new_layer());
					data.snap_handler.start_snap(document, document.bounding_boxes(None, None), true, true);
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);

					// Get the position and set properties
					let start_position = transform.inverse().transform_point2(snapped_position);
					data.weight = tool_options.line_weight;

					// Create the initial shape with a bez_path (only contains a moveto initially)
					if let Some(layer_path) = &data.path {
						data.bez_path = start_bez_path(start_position);
						responses.push_back(
							Operation::AddShape {
								path: layer_path.clone(),
								transform: transform.to_cols_array(),
								insert_index: -1,
								bez_path: data.bez_path.clone().into_iter().collect(),
								style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight as f32)), None),
								closed: false,
							}
							.into(),
						);
					}

					add_to_curve(data, input, transform, document, responses);
					Drawing
				}
				(Drawing, DragStart) => {
					add_to_curve(data, input, transform, document, responses);
					Drawing
				}
				(Drawing, DragStop) => {
					// Deselect everything (this means we are no longer dragging the handle)
					data.shape_editor.deselect_all(responses);

					// Reselect the last point
					if let Some(last_anchor) = data.shape_editor.select_last_anchor() {
						last_anchor.select_point(0, true, responses);
					}

					Drawing
				}
				(Drawing, PointerMove) => {
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
					//data.shape_editor.update_shapes(document, responses);
					data.shape_editor.move_selected_points(snapped_position, false, responses);

					Drawing
				}
				(Drawing, Confirm) | (Drawing, Abort) => {
					// Add a curve to the path
					if let Some(layer_path) = &data.path {
						remove_curve_from_end(&mut data.bez_path);
						responses.push_back(apply_bez_path(layer_path.clone(), data.bez_path.clone(), transform));
					}

					// Cleanup, we are either canceling or finished drawing
					if data.bez_path.len() >= 2 {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					data.shape_editor.remove_overlays(responses);
					data.shape_editor.clear_shapes_to_modify();

					data.path = None;
					data.snap_handler.cleanup(responses);

					Ready
				}
				(_, Abort) => {
					data.shape_editor.remove_overlays(responses);
					data.shape_editor.clear_shapes_to_modify();
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

// Add to the curve and select the second anchor of the last point and the newly added anchor point
fn add_to_curve(data: &mut PenToolData, input: &InputPreprocessorMessageHandler, transform: DAffine2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	// We need to make sure we have the most up-to-date bez_path
	// Would like to remove this hack eventually
	if !data.shape_editor.shapes_to_modify.is_empty() {
		// Hacky way of saving the curve changes
		data.bez_path = data.shape_editor.shapes_to_modify[0].bez_path.elements().to_vec();
	}

	// Setup our position params
	let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
	let position = transform.inverse().transform_point2(snapped_position);

	// Add a curve to the path
	if let Some(layer_path) = &data.path {
		add_curve_to_end(position, &mut data.bez_path);
		responses.push_back(apply_bez_path(layer_path.clone(), data.bez_path.clone(), transform));

		// Clear previous overlays
		data.shape_editor.remove_overlays(responses);

		// Create a new shape from the updated bez_path
		let bez_path = data.bez_path.clone().into_iter().collect();
		data.curve_shape = VectorShape::new(layer_path.to_vec(), transform, &bez_path, false, responses);
		data.shape_editor.set_shapes_to_modify(vec![data.curve_shape.clone()]);

		// Select the second to last segment's handle
		data.shape_editor.set_shape_selected(0);
		let handle_element = data.shape_editor.select_nth_anchor(0, -2);
		handle_element.select_point(2, true, responses);

		// Select the last segment's anchor point
		if let Some(last_anchor) = data.shape_editor.select_last_anchor() {
			last_anchor.select_point(0, true, responses);
		}
		data.shape_editor.set_selected_mirror_options(true, true);
	}
}

// Create the initial moveto for the bez_path
fn start_bez_path(start_position: DVec2) -> Vec<PathEl> {
	vec![PathEl::MoveTo(Point {
		x: start_position.x,
		y: start_position.y,
	})]
}

// Add a curve to the bez_path
fn add_curve_to_end(point: DVec2, bez_path: &mut Vec<PathEl>) {
	let point = Point { x: point.x, y: point.y };
	bez_path.push(PathEl::CurveTo(point, point, point));
}

// Add a curve to the bez_path
fn remove_curve_from_end(bez_path: &mut Vec<PathEl>) {
	bez_path.pop();
}

// Apply the bez_path to the shape in the viewport
fn apply_bez_path(layer_path: Vec<LayerId>, bez_path: Vec<PathEl>, transform: DAffine2) -> Message {
	Operation::SetShapePathInViewport {
		path: layer_path,
		bez_path: bez_path.into_iter().collect(),
		transform: transform.to_cols_array(),
	}
	.into()
}
