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
use crate::viewport_tools::vector_editor::overlay_renderer::OverlayRenderer;
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
	Delete,
	DragStart {
		add_to_selection: Key,
	},
	DragStop,
	PointerMove {
		alt_mirror_angle: Key,
		shift_mirror_distance: Key,
	},
	SelectPoint,
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
			Ready => actions!(PathToolMessageDiscriminant; DragStart, SelectPoint, Delete),
			Dragging => actions!(PathToolMessageDiscriminant; DragStop, PointerMove, Delete),
			PointSelected => actions!(PathToolMessageDiscriminant; SelectPoint, Delete/*TODO: Delete */),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathToolFsmState {
	Ready,
	Dragging,
	PointSelected,
}

impl Default for PathToolFsmState {
	fn default() -> Self {
		PathToolFsmState::Ready
	}
}

#[derive(Default)]
struct PathToolData {
	shape_editor: ShapeEditor,
	overlay_renderer: OverlayRenderer,
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
				(_, SelectionChanged) => {
					// TODO Tell overlay renderer to clear / updates the overlays
					for layer_path in document.all_layers() {
						data.overlay_renderer.layer_overlay_visibility(&document.graphene_document, layer_path.to_vec(), false, responses);
					}

					let layer_paths = document.selected_visible_layers().map(|layer_path| layer_path.to_vec()).collect();
					data.shape_editor.set_target_layers(layer_paths);
					self
				}
				(_, DocumentIsDirty) => {
					// TODO This should be handled by the document not by the tool, but this is a stop gap
					for layer_path in document.selected_visible_layers() {
						data.overlay_renderer.render_vector_shape_overlays(&document.graphene_document, layer_path.to_vec(), responses);
					}

					self
				}
				// Mouse down
				(_, DragStart { add_to_selection }) => {
					let add_to_selection = input.keyboard.get(add_to_selection as usize);

					// Select the first point within the threshold (in pixels)
					if data
						.shape_editor
						.select_point(&document.graphene_document, input.mouse.position, SELECTION_THRESHOLD, add_to_selection, responses)
					{
						responses.push_back(DocumentMessage::StartTransaction.into());
						data.snap_handler.start_snap(document, document.bounding_boxes(None, None), true, true);
						let snap_points = data
							.shape_editor
							.selected_anchors(&document.graphene_document)
							.flat_map(|anchor| anchor.points[0].as_ref())
							.map(|point| point.position)
							.collect();
						data.snap_handler.add_snap_points(document, snap_points);
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
							data.shape_editor.toggle_selected_mirror_angle(&document.graphene_document, &responses);
						}
					}

					// Determine when shift state changes
					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);
					if shift_pressed != data.shift_debounce {
						data.shift_debounce = shift_pressed;
						data.shape_editor.toggle_selected_mirror_distance(&document.graphene_document, &responses);
					}

					// Move the selected points by the mouse position
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
					data.shape_editor
						.move_selected_points(&document.graphene_document, snapped_position - data.drag_start_pos, true, &responses);
					Dragging
				}
				// DoubleClick
				(_, Delete) => {
					// Select the first point within the threshold (in pixels)
					if data.shape_editor.select_point(&document.graphene_document, input.mouse.position, SELECTION_THRESHOLD, false, responses) {
						responses.push_back(DocumentMessage::StartTransaction.into());
						data.shape_editor.delete_selected_points(&document.graphene_document, responses);
						responses.push_back(SelectionChanged.into());
					}
					Ready
				}
				// Mouse up
				(_, DragStop) => {
					data.snap_handler.cleanup(responses);
					Ready
				}
				(_, Abort) | (_, SelectPoint) => {
					// TODO Tell overlay manager to remove the overlays
					//data.shape_editor.remove_overlays();
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
			PathToolFsmState::PointSelected => HintData(vec![HintGroup(vec![
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
