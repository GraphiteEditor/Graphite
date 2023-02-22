use crate::consts::{SELECTION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::overlay_renderer::OverlayRenderer;
use crate::messages::tool::common_functionality::shape_editor::{ManipulatorPointInfo, ShapeEditor};
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::common_functionality::transformation_cage::axis_align_drag;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use bezier_rs::ManipulatorGroup;
use document_legacy::intersection::Quad;
use document_legacy::Operation;
use graphene_core::vector::manipulator_point::ManipulatorPoint;
use graphene_std::vector::consts::ManipulatorType;

use glam::{DAffine2, DVec2};
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
	BeginGrab,
	BeginRotate,
	BeginScale,
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
				BeginGrab,
				BeginRotate,BeginScale,
			),
			Dragging => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
				BeginGrab,
				BeginRotate,
				BeginScale,
			),
			Rotating => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
				BeginGrab,
				BeginRotate,
				BeginScale,
			),
			Scaling => actions!(PathToolMessageDiscriminant;
				InsertPoint,
				DragStop,
				PointerMove,
				Delete,
				BeginGrab,
				BeginRotate,
				BeginScale,
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
	Rotating,
	Scaling,
}

#[derive(Default)]
struct PathToolData {
	shape_editor: ShapeEditor,
	overlay_renderer: OverlayRenderer,
	snap_manager: SnapManager,
	drag_start_pos: DVec2,
	previous_mouse_position: DVec2,
	grs_mouse_start: DVec2,
	alt_debounce: bool,
	grs_initial_points: Vec<DVec2>,
	factor: f64,
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _document_id, _global_tool_data, input, render_data): ToolActionHandlerData,
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
							.start_snap(document, input, document.bounding_boxes(Some(&ignore_document), None, render_data), true, true);

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

						tool_data.drag_start_pos = input.mouse.position;
						tool_data.previous_mouse_position = input.mouse.position;

						PathToolFsmState::Dragging
					}
					// We didn't find a point nearby, so consider selecting the nearest shape instead
					else {
						let selection_size = DVec2::new(2.0, 2.0);
						// Select shapes directly under our mouse
						let intersection = document
							.document_legacy
							.intersects_quad_root(Quad::from_box([input.mouse.position - selection_size, input.mouse.position + selection_size]), render_data);
						if !intersection.is_empty() {
							if toggle_add_to_selection {
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
							if !input.keyboard.get(toggle_add_to_selection as usize) {
								responses.push_back(DocumentMessage::DeselectAllLayers.into());
							}
						}

						PathToolFsmState::Ready
					}
				}
				//Rotating
				(
					PathToolFsmState::Rotating,
					PathToolMessage::PointerMove {
						alt_mirror_angle,
						shift_mirror_distance,
					},
				) => {
					let path = tool_data.shape_editor.selected_layers_ref();
					let viewspace = &mut document.document_legacy.generate_transform_relative_to_viewport(path[0]).ok().unwrap();
					// let to = document.generate_transform_relative_to_viewport(path[0]);

					let points = tool_data.shape_editor.selected_points(&document.document_legacy);

					//make sure that the pivot is in viewspace - fixed with the transofmr_point
					let mut count: usize = 0;
					let pivot = points
						.map(|point| {
							count += 1;
							viewspace.transform_point2(point.position)
						})
						.sum::<DVec2>() / count as f64;

					//drag start is in pixels // center is in relative pos
					let vector_from_mouse_start = pivot - tool_data.grs_mouse_start;
					let vector_from_mouse_current = pivot - input.mouse.position;
					let angle = vector_from_mouse_start.angle_between(vector_from_mouse_current);
					// tool_data.grs_mouse_start = input.mouse.position;
					let delta = DAffine2::from_translation(pivot) * DAffine2::from_angle(angle) * DAffine2::from_translation(-pivot);

					//convert pivot position from viewport space (pixels) into layer space using one of the funcs -> DAffine2
					//modify that matrix (viewspace) inverse to go back to layer then  multiplying it by rotation matrix based on angle -> DAffine2
					let layerspace_rotation = viewspace.inverse() * delta;
					let points = tool_data.shape_editor.selected_points(&document.document_legacy);
					let subpath = document.document_legacy.layer(path[0]).ok().and_then(|layer| layer.as_subpath());

					for point in points {
						let mut group_id = 0;
						for man_group in subpath.unwrap().manipulator_groups().enumerate() {
							let points_in_group = man_group.1.selected_points();
							for p in points_in_group {
								if p.position == point.position {
									group_id = *man_group.0;
								}
							}
						}
						// self.transform_operation = TransformOperation::Rotating(rotation.increment_amount(change));
						let viewport_point = viewspace.transform_point2(point.position);

						// transform the new point from layer position to viewspace position
						let new_pos = layerspace_rotation.transform_point2(viewport_point);
						//TODO: check is this the right pos?
						debug!("group_id rot {}", group_id);
						let op = Operation::MoveManipulatorPoint {
							layer_path: path[0].to_vec(),
							id: group_id, //manGroupID
							manipulator_type: point.manipulator_type,
							position: new_pos.into(),
						}
						.into();
						responses.push_back(op);
					}

					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let axis_aligned_position = axis_align_drag(shift_pressed, snapped_position, tool_data.drag_start_pos);
					tool_data.previous_mouse_position = axis_aligned_position;

					PathToolFsmState::Rotating
				}
				(
					PathToolFsmState::Scaling,
					PathToolMessage::PointerMove {
						alt_mirror_angle,
						shift_mirror_distance,
					},
				) => {
					let path = tool_data.shape_editor.selected_layers_ref();
					let viewspace = &mut document.document_legacy.generate_transform_relative_to_viewport(path[0]).ok().unwrap();
					let points = tool_data.shape_editor.selected_points(&document.document_legacy);
					let pivot = tool_data.grs_initial_points.iter().map(|point| viewspace.transform_point2(*point)).sum::<DVec2>() / tool_data.grs_initial_points.len() as f64;

					//drag start is in pixels // center is in relative pos
					let change = {
						let previous_frame_dist = (tool_data.previous_mouse_position - pivot).length();
						let current_frame_dist = (input.mouse.position - pivot).length();
						let start_transform_dist = (tool_data.grs_mouse_start - pivot).length();
						debug!("previous_frame_dist {}", previous_frame_dist);
						debug!("current_frame_dist {}", current_frame_dist);
						debug!("start_transform_dist {}", start_transform_dist);	
						(current_frame_dist - previous_frame_dist) / start_transform_dist
					};
					tool_data.factor += change;

					debug!("Change {}", change);
					let pivot_matrix = DAffine2::from_translation(pivot);
					let delta = pivot_matrix * DAffine2::from_scale(DVec2::splat(tool_data.factor)) * pivot_matrix.inverse();

					//modify that matrix (viewspace) inverse to go back to layer then  multiplying it by rotation matrix based on angle -> DAffine2
					let layerspace_rotation = viewspace.inverse() * delta;
					// TODO: make this work for multiple selected layers not just one
					let subpath = document.document_legacy.layer(path[0]).ok().and_then(|layer| layer.as_subpath());

					for (point, initial_point) in points.zip(tool_data.grs_initial_points.iter()) {
						// let mut group_id= None ;
						// for man_group in subpath.unwrap().manipulator_groups().iter() {
						// 	group_id = man_group.selected_points().find(|p| p.position == point.position);
						// }
						debug!("Pos {:?}", point.position);
						let group_id = subpath
							.unwrap()
							.manipulator_groups()
							.iter()
							.enumerate()
							.find_map(|(index, group)| group.points().any(|manip_point| manip_point == point).then_some(index));

						let viewport_point = viewspace.transform_point2(*initial_point);
						debug!("Group ID {:?}", group_id);
						let new_pos_viewport = layerspace_rotation.transform_point2(viewport_point);
						info!("Old viewport {viewport_point} new point {new_pos_viewport}");
						let new_pos = viewspace.inverse().transform_point2(new_pos_viewport);
						debug!("New Pos {:?}", new_pos);

						let op = Operation::MoveManipulatorPoint {
							layer_path: path[0].to_vec(),
							id: group_id.unwrap() as u64 + 1, //the +1 is to compensate for the enumerate() starting at 0 and group ids starting at 1
							manipulator_type: point.manipulator_type,
							position: new_pos_viewport.into(),
						}
						.into();
						responses.push_back(op);
					}

					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let axis_aligned_position = axis_align_drag(shift_pressed, snapped_position, tool_data.drag_start_pos);
					tool_data.previous_mouse_position = axis_aligned_position;

					PathToolFsmState::Scaling
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

					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let axis_aligned_position = axis_align_drag(shift_pressed, snapped_position, tool_data.drag_start_pos);
					tool_data
						.shape_editor
						.move_selected_points(axis_aligned_position - tool_data.previous_mouse_position, shift_pressed, responses);
					tool_data.previous_mouse_position = axis_aligned_position;
					PathToolFsmState::Dragging
				}

				(_, PathToolMessage::BeginGrab) => {
					tool_data.previous_mouse_position = input.mouse.position;
					tool_data.drag_start_pos = input.mouse.position;

					PathToolFsmState::Dragging
				}
				(_, PathToolMessage::BeginRotate) => {
					// TODO: need start pos of mouse to calculate angle
					tool_data.grs_mouse_start = input.mouse.position;

					let path = tool_data.shape_editor.selected_layers_ref();
					let viewspace = &mut document.document_legacy.generate_transform_relative_to_viewport(path[0]).ok().unwrap();

					PathToolFsmState::Rotating
				}
				(_, PathToolMessage::BeginScale) => {
					tool_data.previous_mouse_position = input.mouse.position;

					let points: Vec<_> = tool_data.shape_editor.selected_points(&document.document_legacy).map(|point| point.position).collect();

					tool_data.grs_initial_points = points;
					tool_data.grs_mouse_start = input.mouse.position;

					PathToolFsmState::Scaling
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
				HintGroup(vec![HintInfo::arrow_keys("Nudge Selected (coming soon)"), HintInfo::keys([Key::Shift], "10x").prepend_plus()]),
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
			PathToolFsmState::Rotating => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Alt], "Split/Align Handles (Toggle)"),
				HintInfo::keys([Key::Shift], "Share Lengths of Aligned Handles"),
			])]),
			PathToolFsmState::Scaling => HintData(vec![HintGroup(vec![
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
