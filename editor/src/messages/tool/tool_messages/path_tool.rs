use std::vec;

use super::tool_prelude::*;
use crate::consts::{DRAG_THRESHOLD, SELECTION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::tool::common_functionality::overlay_renderer::OverlayRenderer;
use crate::messages::tool::common_functionality::shape_editor::{ManipulatorPointInfo, OpposingHandleLengths, SelectedPointsInfo, ShapeState};
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::common_functionality::transformation_cage::{add_bounding_box, remove_bounding_box, update_bounding_box};

use document_legacy::LayerId;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

#[derive(Default)]
pub struct PathTool {
	fsm_state: PathToolFsmState,
	tool_data: PathToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
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
	DragStop {
		shift_mirror_distance: Key,
	},
	Enter {
		add_to_selection: Key,
	},
	InsertPoint,
	NudgeSelectedPoints {
		delta_x: f64,
		delta_y: f64,
	},
	PointerMove {
		alt: Key,
		shift: Key,
	},
}

impl ToolMetadata for PathTool {
	fn icon_name(&self) -> String {
		"VectorPathTool".into()
	}
	fn tooltip(&self) -> String {
		"Path Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Path
	}
}

impl LayoutHolder for PathTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for PathTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);
	}

	// Different actions depending on state may be wanted:
	fn actions(&self) -> ActionList {
		use PathToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStart,
				Delete,
				NudgeSelectedPoints,
				Enter,
			),
			Dragging => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
			),
			DrawingBox => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
				Enter
			),
		}
	}
}

impl ToolTransition for PathTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(PathToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(PathToolMessage::Abort.into()),
			selection_changed: Some(PathToolMessage::SelectionChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PathToolFsmState {
	#[default]
	Ready,
	Dragging,
	DrawingBox,
}

#[derive(Default)]
struct PathToolData {
	snap_manager: SnapManager,
	drag_start_pos: DVec2,
	previous_mouse_position: DVec2,
	alt_debounce: bool,
	opposing_handle_lengths: Option<OpposingHandleLengths>,
	drag_box_overlay_layer: Option<Vec<LayerId>>,
}

impl PathToolData {
	fn refresh_overlays(&mut self, document: &DocumentMessageHandler, shape_editor: &mut ShapeState, shape_overlay: &mut OverlayRenderer, responses: &mut VecDeque<Message>) {
		// Set the previously selected layers to invisible
		for layer in document.metadata().all_layers() {
			shape_overlay.layer_overlay_visibility(&document.document_legacy, layer, false, responses);
		}

		// Render the new overlays
		for &layer in shape_editor.selected_shape_state.keys() {
			shape_overlay.render_subpath_overlays(&shape_editor.selected_shape_state, &document.document_legacy, layer, responses);
		}

		self.opposing_handle_lengths = None;
	}

	fn mouse_down(
		&mut self,
		shift: bool,
		shape_editor: &mut ShapeState,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		shape_overlay: &mut OverlayRenderer,
		responses: &mut VecDeque<Message>,
	) -> PathToolFsmState {
		self.opposing_handle_lengths = None;
		let _selected_layers = shape_editor.selected_layers().cloned().collect::<Vec<_>>();

		// Select the first point within the threshold (in pixels)
		if let Some(selected_points) = shape_editor.select_point(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD, shift) {
			self.start_dragging_point(selected_points, input, document, responses);
			self.refresh_overlays(document, shape_editor, shape_overlay, responses);

			PathToolFsmState::Dragging
		}
		// We didn't find a point nearby, so consider selecting the nearest shape instead
		else if let Some(layer) = document.metadata().click(input.mouse.position) {
			if shift {
				responses.add(NodeGraphMessage::AddSelectNodes { nodes: vec![layer.to_node()] });
			} else {
				responses.add(NodeGraphMessage::SetSelectNodes { nodes: vec![layer.to_node()] });
			}
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = input.mouse.position;
			shape_editor.select_all_anchors(&document.document_legacy, layer);

			PathToolFsmState::Dragging
		} else {
			// Start drawing a box
			self.drag_start_pos = input.mouse.position;
			self.previous_mouse_position = input.mouse.position;
			self.drag_box_overlay_layer = Some(add_bounding_box(responses));

			PathToolFsmState::DrawingBox
		}
	}

	fn start_dragging_point(&mut self, mut selected_points: SelectedPointsInfo, input: &InputPreprocessorMessageHandler, _document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::StartTransaction);

		// TODO: enable snapping

		//self
		//	.snap_manager
		//	.start_snap(document, input, document.bounding_boxes(Some(&selected_layers), None, render_data), true, true);

		// Do not snap against handles when anchor is selected
		let mut additional_selected_points = Vec::new();
		for point in selected_points.points.iter() {
			if point.point_id.manipulator_type == SelectedType::Anchor {
				additional_selected_points.push(ManipulatorPointInfo {
					layer: point.layer,
					point_id: ManipulatorPointId::new(point.point_id.group, SelectedType::InHandle),
				});
				additional_selected_points.push(ManipulatorPointInfo {
					layer: point.layer,
					point_id: ManipulatorPointId::new(point.point_id.group, SelectedType::OutHandle),
				});
			}
		}
		selected_points.points.extend(additional_selected_points);

		//let include_handles: Vec<_> = selected_layers.iter().map(|x| x.as_slice()).collect();
		//self.snap_manager.add_all_document_handles(document, input, &include_handles, &[], &selected_points.points);

		self.drag_start_pos = input.mouse.position;
		self.previous_mouse_position = input.mouse.position - selected_points.offset;
	}

	fn drag(&mut self, shift: bool, alt: bool, shape_editor: &mut ShapeState, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		// Check if the alt key has just been pressed
		if alt && !self.alt_debounce {
			self.opposing_handle_lengths = None;
			shape_editor.toggle_handle_mirroring_on_selected(responses);
		}
		self.alt_debounce = alt;

		if shift {
			if self.opposing_handle_lengths.is_none() {
				self.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(&document.document_legacy));
			}
		} else if let Some(opposing_handle_lengths) = &self.opposing_handle_lengths {
			shape_editor.reset_opposing_handle_lengths(&document.document_legacy, opposing_handle_lengths, responses);
			self.opposing_handle_lengths = None;
		}

		// Move the selected points with the mouse
		let snapped_position = self.snap_manager.snap_position(responses, document, input.mouse.position);
		shape_editor.move_selected_points(&document.document_legacy, snapped_position - self.previous_mouse_position, shift, responses);
		self.previous_mouse_position = snapped_position;
	}
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			input,
			shape_editor,
			shape_overlay,
			..
		} = tool_action_data;
		let ToolMessage::Path(event) = event else {
			return self;
		};

		match (self, event) {
			(_, PathToolMessage::SelectionChanged) => {
				// Set the newly targeted layers to visible
				let target_layers = document.metadata().selected_layers().collect();
				shape_editor.set_selected_layers(target_layers);

				tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);
				// This can happen in any state (which is why we return self)
				self
			}
			(_, PathToolMessage::DocumentIsDirty) => {
				// When the document has moved / needs to be redraw, re-render the overlays
				// TODO the overlay system should probably receive this message instead of the tool
				for layer in document.metadata().selected_layers() {
					shape_overlay.render_subpath_overlays(&shape_editor.selected_shape_state, &document.document_legacy, layer, responses);
				}

				self
			}
			// Mouse down
			(_, PathToolMessage::DragStart { add_to_selection }) => {
				let shift = input.keyboard.get(add_to_selection as usize);

				tool_data.mouse_down(shift, shape_editor, document, input, shape_overlay, responses)
			}
			(PathToolFsmState::DrawingBox, PathToolMessage::PointerMove { .. }) => {
				tool_data.previous_mouse_position = input.mouse.position;
				update_bounding_box(tool_data.drag_start_pos, input.mouse.position, &tool_data.drag_box_overlay_layer, responses);

				PathToolFsmState::DrawingBox
			}
			(PathToolFsmState::Dragging, PathToolMessage::PointerMove { alt, shift }) => {
				let alt = input.keyboard.get(alt as usize);
				let shift = input.keyboard.get(shift as usize);
				tool_data.drag(shift, alt, shape_editor, document, input, responses);

				PathToolFsmState::Dragging
			}

			(PathToolFsmState::DrawingBox, PathToolMessage::Enter { add_to_selection }) => {
				let shift_pressed = input.keyboard.get(add_to_selection as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SetSelectNodes { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.document_legacy, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !shift_pressed);
					tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);
				};
				remove_bounding_box(tool_data.drag_box_overlay_layer.take(), responses);

				PathToolFsmState::Ready
			}

			// Mouse up
			(PathToolFsmState::DrawingBox, PathToolMessage::DragStop { shift_mirror_distance }) => {
				let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

				if tool_data.drag_start_pos == tool_data.previous_mouse_position {
					responses.add(NodeGraphMessage::SetSelectNodes { nodes: vec![] });
				} else {
					shape_editor.select_all_in_quad(&document.document_legacy, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !shift_pressed);
					tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);
				};
				remove_bounding_box(tool_data.drag_box_overlay_layer.take(), responses);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::DragStop { shift_mirror_distance }) => {
				let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

				let nearest_point = shape_editor
					.find_nearest_point_indices(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD)
					.map(|(_, nearest_point)| nearest_point);

				shape_editor.delete_selected_handles_with_zero_length(&document.document_legacy, &tool_data.opposing_handle_lengths, responses);

				if tool_data.drag_start_pos.distance(input.mouse.position) <= DRAG_THRESHOLD && !shift_pressed {
					let clicked_selected = shape_editor.selected_points().any(|&point| nearest_point == Some(point));
					if clicked_selected {
						shape_editor.deselect_all();
						shape_editor.select_point(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD, false);
						tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);
					}
				}

				tool_data.snap_manager.cleanup(responses);
				PathToolFsmState::Ready
			}

			// Delete key
			(_, PathToolMessage::Delete) => {
				// Delete the selected points and clean up overlays
				responses.add(DocumentMessage::StartTransaction);
				shape_editor.delete_selected_points(responses);
				responses.add(PathToolMessage::SelectionChanged);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::InsertPoint) => {
				// First we try and flip the sharpness (if they have clicked on an anchor)
				if !shape_editor.flip_sharp(&document.document_legacy, input.mouse.position, SELECTION_TOLERANCE, responses) {
					// If not, then we try and split the path that may have been clicked upon
					shape_editor.split(&document.document_legacy, input.mouse.position, SELECTION_TOLERANCE, responses);
				}

				self
			}
			(_, PathToolMessage::Abort) => {
				// TODO Tell overlay manager to remove the overlays
				shape_overlay.clear_all_overlays(responses);
				remove_bounding_box(tool_data.drag_box_overlay_layer.take(), responses);

				PathToolFsmState::Ready
			}
			(_, PathToolMessage::PointerMove { .. }) => self,
			(_, PathToolMessage::NudgeSelectedPoints { delta_x, delta_y }) => {
				shape_editor.move_selected_points(&document.document_legacy, (delta_x, delta_y).into(), true, responses);

				PathToolFsmState::Ready
			}
			(_, _) => PathToolFsmState::Ready,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let general_hint_data = HintData(vec![
			HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Point"), HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus()]),
			HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
			HintGroup(vec![HintInfo::arrow_keys("Nudge Selected"), HintInfo::keys([Key::Shift], "10x").prepend_plus()]),
			HintGroup(vec![HintInfo::keys([Key::KeyG, Key::KeyR, Key::KeyS], "Grab/Rotate/Scale Selected")]),
		]);

		let hint_data = match self {
			PathToolFsmState::Ready => general_hint_data,
			PathToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Alt], "Split/Align Handles (Toggle)"),
				HintInfo::keys([Key::Shift], "Share Lengths of Aligned Handles"),
			])]),
			PathToolFsmState::DrawingBox => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
				HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus(),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
