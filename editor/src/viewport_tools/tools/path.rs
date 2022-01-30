use crate::consts::SELECTION_THRESHOLD;
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::shape_manipulation::ManipulationHandler;
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::intersection::Quad;

use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Path {
	fsm_state: PathToolFsmState,
	data: PathToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PathMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	SelectionChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Path {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	// Different actions depending on state may be wanted:
	fn actions(&self) -> ActionList {
		use PathToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(PathMessageDiscriminant; DragStart),
			Dragging => actions!(PathMessageDiscriminant; DragStop, PointerMove),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathToolFsmState {
	Ready,
	Dragging,
}

impl Default for PathToolFsmState {
	fn default() -> Self {
		PathToolFsmState::Ready
	}
}

#[derive(Default)]
struct PathToolData {
	manipulation_handler: ManipulationHandler,
	snap_handler: SnapHandler,
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Path(event) = event {
			use PathMessage::*;
			use PathToolFsmState::*;

			match (self, event) {
				(_, SelectionChanged) => {
					// Remove any residual overlays that might exist on selection change
					for shape in &mut data.manipulation_handler.selected_shapes {
						shape.remove_all_overlays(responses);
					}

					// This currently creates new VectorManipulatorShapes for every shape, which is not ideal
					// Atleast it is only on selection change for now
					data.manipulation_handler.selected_shapes = document.selected_visible_layers_vector_shapes(responses);

					self
				}
				(_, DocumentIsDirty) => {
					// Update the VectorManipulatorShapes by reference so they match the kurbo data
					for shape in &mut data.manipulation_handler.selected_shapes {
						shape.update_shape(document, responses);
					}
					self
				}
				(_, DragStart) => {
					// Select the first point within the threshold (in pixels)
					let select_threshold = SELECTION_THRESHOLD;
					if data.manipulation_handler.select_manipulator(input.mouse.position, select_threshold, responses) {
						responses.push_back(DocumentMessage::StartTransaction.into());
						data.snap_handler.start_snap(document, document.visible_layers());
						let snap_points = data
							.manipulation_handler
							.selected_shapes
							.iter()
							.flat_map(|shape| shape.anchors.iter().map(|anchor| anchor.point.position))
							.collect();
						data.snap_handler.add_snap_points(document, snap_points);
						Dragging
					} else {
						// Select shapes directly under our mouse
						let intersection = document.graphene_document.intersects_quad_root(Quad::from_box([input.mouse.position, input.mouse.position]));
						if !intersection.is_empty() {
							for shape in &mut data.manipulation_handler.selected_shapes {
								shape.remove_all_overlays(responses);
							}
							responses.push_back(
								DocumentMessage::SetSelectedLayers {
									replacement_selected_layers: intersection,
								}
								.into(),
							);
						}
						Ready
					}
				}
				(Dragging, PointerMove) => {
					let should_not_mirror = input.keyboard.get(Key::KeyAlt as usize);

					// Move the selected points by the mouse position
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
					let move_operation = data.manipulation_handler.move_selected_to(snapped_position, !should_not_mirror);
					responses.push_back(move_operation.into());
					Dragging
				}
				(_, DragStop) => {
					data.snap_handler.cleanup(responses);
					Ready
				}
				(_, Abort) => {
					for shape in &mut data.manipulation_handler.selected_shapes {
						shape.remove_all_overlays(responses);
					}
					Ready
				}
				(_, PointerMove) => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			PathToolFsmState::Ready => HintData(vec![
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::Lmb),
						label: String::from("Select Point"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Add/Remove Point (coming soon)"),
						plus: true,
					},
				]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Drag Selected"),
					plus: false,
				}]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![
							KeysGroup(vec![Key::KeyArrowUp]),
							KeysGroup(vec![Key::KeyArrowRight]),
							KeysGroup(vec![Key::KeyArrowDown]),
							KeysGroup(vec![Key::KeyArrowLeft]),
						],
						mouse: None,
						label: String::from("Nudge Selected (coming soon)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Big Increment Nudge"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyG])],
						mouse: None,
						label: String::from("Grab Selected (coming soon)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyR])],
						mouse: None,
						label: String::from("Rotate Selected (coming soon)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyS])],
						mouse: None,
						label: String::from("Scale Selected (coming soon)"),
						plus: false,
					},
				]),
			]),
			PathToolFsmState::Dragging => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
				mouse: None,
				label: String::from("Handle Mirroring Toggle"),
				plus: false,
			}])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}
