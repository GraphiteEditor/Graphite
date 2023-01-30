use crate::consts::{SELECTION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::overlay_renderer::OverlayRenderer;
use crate::messages::tool::common_functionality::shape_editor::{ManipulatorPointInfo, ShapeEditor};
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::intersection::Quad;
use graphene_std::vector::consts::ManipulatorType;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct PathTool {
	fsm_state: PathToolFsmState,
	tool_data: PathToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
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
	InsertPoint,
	PointerMove {
		alt_mirror_angle: Key,
		shift_mirror_distance: Key,
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

impl PropertyHolder for PathTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for PathTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: ToolActionHandlerData<'a>) {
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
			),
			Dragging => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
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
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PathToolFsmState {
	#[default]
	Ready,
	Dragging,
}

#[derive(Default)]
struct PathToolData {
	shape_editor: ShapeEditor,
	overlay_renderer: OverlayRenderer,
	snap_manager: SnapManager,

	drag_start_pos: DVec2,
	alt_debounce: bool,
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _document_id, _global_tool_data, input, font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Path(event) = event {
			match (self, event) {
				(_, PathToolMessage::SelectionChanged) => {
					// Set the previously selected layers to invisible
					for layer_path in document.all_layers() {
						tool_data.overlay_renderer.layer_overlay_visibility(&document.document_legacy, layer_path.to_vec(), false, responses);
					}

					// Set the newly targeted layers to visible
					let layer_paths = document.selected_visible_layers().map(|layer_path| layer_path.to_vec()).collect();
					tool_data.shape_editor.set_selected_layers(layer_paths);
					// Render the new overlays
					for layer_path in tool_data.shape_editor.selected_layers() {
						tool_data.overlay_renderer.render_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}

					// This can happen in any state (which is why we return self)
					self
				}
				(_, PathToolMessage::DocumentIsDirty) => {
					// When the document has moved / needs to be redraw, re-render the overlays
					// TODO the overlay system should probably receive this message instead of the tool
					for layer_path in document.selected_visible_layers() {
						tool_data.overlay_renderer.render_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}

					self
				}
				// Mouse down
				(_, PathToolMessage::DragStart { add_to_selection }) => {
					let toggle_add_to_selection = input.keyboard.get(add_to_selection as usize);

					// Select the first point within the threshold (in pixels)
					if let Some(mut selected_points) = tool_data
						.shape_editor
						.select_point(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD, toggle_add_to_selection, responses)
					{
						responses.push_back(DocumentMessage::StartTransaction.into());

						let ignore_document = tool_data.shape_editor.selected_layers().clone();
						tool_data
							.snap_manager
							.start_snap(document, input, document.bounding_boxes(Some(&ignore_document), None, font_cache), true, true);

						// Do not snap against handles when anchor is selected
						let mut extension = Vec::new();
						for point in selected_points.points.iter() {
							if point.manipulator_type == ManipulatorType::Anchor {
								extension.push(ManipulatorPointInfo {
									manipulator_type: ManipulatorType::InHandle,
									..*point
								});
								extension.push(ManipulatorPointInfo {
									manipulator_type: ManipulatorType::OutHandle,
									..*point
								});
							}
						}
						selected_points.points.extend(extension);

						let include_handles = tool_data.shape_editor.selected_layers_ref();
						tool_data.snap_manager.add_all_document_handles(document, input, &include_handles, &[], &selected_points.points);

						tool_data.drag_start_pos = input.mouse.position - selected_points.offset;
						PathToolFsmState::Dragging
					}
					// We didn't find a point nearby, so consider selecting the nearest shape instead
					else {
						let selection_size = DVec2::new(2.0, 2.0);
						// Select shapes directly under our mouse
						let intersection = document
							.document_legacy
							.intersects_quad_root(Quad::from_box([input.mouse.position - selection_size, input.mouse.position + selection_size]), font_cache);
						if !intersection.is_empty() {
							if toggle_add_to_selection {
								responses.push_back(DocumentMessage::AddSelectedLayers { additional_layers: intersection }.into());
							} else {
								// selects the top most layer when selecting intersecting shapes
								let top_most_intersection = intersection[intersection.len() - 1].clone();
								responses.push_back(
									DocumentMessage::SetSelectedLayers {
										replacement_selected_layers: intersection,
									}
									.into(),
								);
								tool_data.drag_start_pos = input.mouse.position;
								// Selects all the anchor points when clicking in a filled area of shape. If two shapes intersect we pick the bottom-most layer
								tool_data.shape_editor.select_all_anchors(responses, top_most_intersection);
								return PathToolFsmState::Dragging;
							}
						} else {
							// Clear the previous selection if we didn't find anything
							if !input.keyboard.get(toggle_add_to_selection as usize) {
								responses.push_back(DocumentMessage::DeselectAllLayers.into());
							}
						}
						PathToolFsmState::Ready
					}
				}
				// Dragging
				(
					PathToolFsmState::Dragging,
					PathToolMessage::PointerMove {
						alt_mirror_angle,
						shift_mirror_distance,
					},
				) => {
					// Determine when alt state changes
					let alt_pressed = input.keyboard.get(alt_mirror_angle as usize);
					if alt_pressed != tool_data.alt_debounce {
						tool_data.alt_debounce = alt_pressed;
						// Only on alt down
						if alt_pressed {
							tool_data.shape_editor.toggle_handle_mirroring_on_selected(true, responses);
						}
					}

					// Determine when shift state changes
					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

					// Move the selected points by the mouse position
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					tool_data.shape_editor.move_selected_points(snapped_position - tool_data.drag_start_pos, shift_pressed, responses);
					tool_data.drag_start_pos = snapped_position;
					PathToolFsmState::Dragging
				}
				// Mouse up
				(_, PathToolMessage::DragStop) => {
					tool_data.snap_manager.cleanup(responses);
					PathToolFsmState::Ready
				}
				// Delete key
				(_, PathToolMessage::Delete) => {
					// Delete the selected points and clean up overlays
					responses.push_back(DocumentMessage::StartTransaction.into());
					tool_data.shape_editor.delete_selected_points(responses);
					responses.push_back(PathToolMessage::SelectionChanged.into());
					for layer_path in document.all_layers() {
						tool_data.overlay_renderer.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}
					PathToolFsmState::Ready
				}
				(_, PathToolMessage::InsertPoint) => {
					// First we try and flip the sharpness (if they have clicked on an anchor)
					if !tool_data.shape_editor.flip_sharp(&document.document_legacy, input.mouse.position, SELECTION_TOLERANCE, responses) {
						// If not, then we try and split the path that may have been clicked upon
						tool_data.shape_editor.split(&document.document_legacy, input.mouse.position, SELECTION_TOLERANCE, responses);
					}

					self
				}
				(_, PathToolMessage::Abort) => {
					// TODO Tell overlay manager to remove the overlays
					for layer_path in document.all_layers() {
						tool_data.overlay_renderer.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}
					PathToolFsmState::Ready
				}
				(
					_,
					PathToolMessage::PointerMove {
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
					HintInfo::mouse(MouseMotion::Lmb, "Select Point"),
					HintInfo::keys([Key::Shift], "Grow/Shrink Selection").prepend_plus(),
				]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
				HintGroup(vec![
					HintInfo::arrow_keys("Nudge Selected (coming soon)"),
					HintInfo::keys([Key::Shift], "Big Increment Nudge").prepend_plus(),
				]),
				HintGroup(vec![
					HintInfo::keys([Key::KeyG], "Grab Selected (coming soon)"),
					HintInfo::keys([Key::KeyR], "Rotate Selected (coming soon)"),
					HintInfo::keys([Key::KeyS], "Scale Selected (coming soon)"),
				]),
			]),
			PathToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Alt], "Split/Align Handles (Toggle)"),
				HintInfo::keys([Key::Shift], "Share Lengths of Aligned Handles"),
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}
