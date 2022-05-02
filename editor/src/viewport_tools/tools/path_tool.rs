use crate::consts::SELECTION_THRESHOLD;
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::viewport_tools::vector_editor::shape_editor::ShapeEditor;

use graphene::intersection::Quad;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct PathTool {
	fsm_state: PathToolFsmState,
	data: PathToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PathToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	SelectionChanged,

	// Tool-specific messages
	DragStart {
		add_to_selection: Key,
	},
	DragStop,
	PointerMove {
		alt_mirror_angle: Key,
		shift_mirror_distance: Key,
	},
}

impl PropertyHolder for PathTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for PathTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &(), data.2, responses);

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
			Ready => actions!(PathToolMessageDiscriminant; DragStart),
			Dragging => actions!(PathToolMessageDiscriminant; DragStop, PointerMove),
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
	shape_editor: ShapeEditor,
	snap_handler: SnapHandler,

	drag_start_pos: DVec2,
	alt_debounce: bool,
	shift_debounce: bool,
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		_tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Path(event) = event {
			use PathToolFsmState::*;
			use PathToolMessage::*;

			match (self, event) {
				// TODO: Capture a tool event instead of doing this?
				(_, SelectionChanged) => {
					// Remove any residual overlays that might exist on selection change
					data.shape_editor.remove_overlays(responses);

					// This currently creates new VectorManipulatorShapes for every shape, which is not ideal
					// At least it is only on selection change for now
					data.shape_editor.set_shapes_to_modify(document.selected_visible_layers_vector_shapes(responses));

					self
				}
				(_, DocumentIsDirty) => {
					// Update the VectorManipulatorShapes by reference so they match the kurbo data
					for shape in &mut data.shape_editor.shapes_to_modify {
						shape.update_shape(document, responses);
					}
					self
				}
				// Mouse down
				(_, DragStart { add_to_selection }) => {
					let add_to_selection = input.keyboard.get(add_to_selection as usize);

					// Select the first point within the threshold (in pixels)
					if data.shape_editor.select_point(input.mouse.position, SELECTION_THRESHOLD, add_to_selection, responses) {
						responses.push_back(DocumentMessage::StartTransaction.into());

						let ignore_document = data.shape_editor.shapes_to_modify.iter().map(|shape| shape.layer_path.clone()).collect::<Vec<_>>();
						data.snap_handler.start_snap(document, document.bounding_boxes(Some(&ignore_document), None), true, true);

						let include_handles = data.shape_editor.shapes_to_modify.iter().map(|shape| shape.layer_path.as_slice()).collect::<Vec<_>>();
						data.snap_handler.add_all_document_handles(document, &include_handles, &[]);

						data.drag_start_pos = input.mouse.position;
						Dragging
					}
					// We didn't find a point nearby, so consider selecting the nearest shape instead
					else {
						let selection_size = DVec2::new(2.0, 2.0);
						// Select shapes directly under our mouse
						let intersection = document
							.graphene_document
							.intersects_quad_root(Quad::from_box([input.mouse.position - selection_size, input.mouse.position + selection_size]));
						if !intersection.is_empty() {
							if add_to_selection {
								responses.push_back(DocumentMessage::AddSelectedLayers { additional_layers: intersection }.into());
							} else {
								responses.push_back(
									DocumentMessage::SetSelectedLayers {
										replacement_selected_layers: intersection,
									}
									.into(),
								);
							}
						} else {
							// Clear the previous selection if we didn't find anything
							if !input.keyboard.get(add_to_selection as usize) {
								responses.push_back(DocumentMessage::DeselectAllLayers.into());
							}
						}
						Ready
					}
				}
				// Dragging
				(
					Dragging,
					PointerMove {
						alt_mirror_angle,
						shift_mirror_distance,
					},
				) => {
					// Determine when alt state changes
					let alt_pressed = input.keyboard.get(alt_mirror_angle as usize);
					if alt_pressed != data.alt_debounce {
						data.alt_debounce = alt_pressed;
						// Only on alt down
						if alt_pressed {
							data.shape_editor.toggle_selected_mirror_angle();
						}
					}

					// Determine when shift state changes
					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);
					if shift_pressed != data.shift_debounce {
						data.shift_debounce = shift_pressed;
						data.shape_editor.toggle_selected_mirror_distance();
					}

					// Move the selected points by the mouse position
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
					data.shape_editor.move_selected_points(snapped_position - data.drag_start_pos, true, responses);
					Dragging
				}
				// Mouse up
				(_, DragStop) => {
					data.snap_handler.cleanup(responses);
					Ready
				}
				(_, Abort) => {
					data.shape_editor.remove_overlays(responses);
					Ready
				}
				(
					_,
					PointerMove {
						alt_mirror_angle: _,
						shift_mirror_distance: _,
					},
				) => self,
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
						label: String::from("Grow/Shrink Selection"),
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
			PathToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
					mouse: None,
					label: String::from("Split/Align Handles (Toggle)"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Share Lengths of Aligned Handles"),
					plus: false,
				},
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}
