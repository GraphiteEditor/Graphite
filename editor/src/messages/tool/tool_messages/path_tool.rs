use crate::consts::{DRAG_THRESHOLD, SELECTION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::overlay_renderer::OverlayRenderer;
use crate::messages::tool::common_functionality::shape_editor::{ManipulatorPointInfo, OpposingHandleLengths, ShapeState};
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, HintData, HintGroup, HintInfo, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};

use document_legacy::intersection::Quad;
use document_legacy::LayerId;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use glam::{DMat2, DVec2};
use serde::{Deserialize, Serialize};

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
	InsertPoint,
	LineExtension,
	NudgeSelectedPoints {
		delta_x: f64,
		delta_y: f64,
	},
	PointerMove {
		alt_mirror_angle: Key,
		shift_mirror_distance: Key,
		line_extend: Key,
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
				LineExtension,
				DragStart,
				Delete,
				NudgeSelectedPoints,
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
			..Default::default()
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
	snap_manager: SnapManager,
	drag_start_pos: DVec2,
	dragged_manipulation: Option<(Vec<LayerId>, ManipulatorPointId)>,
	nearby_anchors: (DVec2, DVec2, DVec2),
	previous_mouse_position: DVec2,
	alt_debounce: bool,
	opposing_handle_lengths: Option<OpposingHandleLengths>,
	line_extension: bool,
}

impl PathToolData {
	fn refresh_overlays(&mut self, document: &DocumentMessageHandler, shape_editor: &mut ShapeState, shape_overlay: &mut OverlayRenderer, responses: &mut VecDeque<Message>) {
		// Set the previously selected layers to invisible
		for layer_path in document.all_layers() {
			shape_overlay.layer_overlay_visibility(&document.document_legacy, layer_path.to_vec(), false, responses);
		}

		// Render the new overlays
		for layer_path in shape_editor.selected_shape_state.keys() {
			shape_overlay.render_subpath_overlays(&shape_editor.selected_shape_state, &document.document_legacy, layer_path.to_vec(), responses);
		}

		self.opposing_handle_lengths = None;
	}
}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData {
			document,
			input,
			render_data,
			shape_editor,
			shape_overlay,
			..
		}: &mut ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Path(event) = event {
			match (self, event) {
				(_, PathToolMessage::SelectionChanged) => {
					// Set the newly targeted layers to visible
					let layer_paths = document.selected_visible_layers().map(|layer_path| layer_path.to_vec()).collect();
					shape_editor.set_selected_layers(layer_paths);

					tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);
					// This can happen in any state (which is why we return self)
					self
				}
				(_, PathToolMessage::DocumentIsDirty) => {
					// When the document has moved / needs to be redraw, re-render the overlays
					// TODO the overlay system should probably receive this message instead of the tool
					for layer_path in document.selected_visible_layers() {
						shape_overlay.render_subpath_overlays(&shape_editor.selected_shape_state, &document.document_legacy, layer_path.to_vec(), responses);
					}

					self
				}
				(_, PathToolMessage::LineExtension) => {
					// tool_data.line_extension = !tool_data.line_extension;
					self
				}
				// Mouse down
				(_, PathToolMessage::DragStart { add_to_selection }) => {
					let shift_pressed = input.keyboard.get(add_to_selection as usize);

					tool_data.opposing_handle_lengths = None;
					let selected_layers = shape_editor.selected_layers().cloned().collect();

					// Select the first point within the threshold (in pixels)
					if let Some(mut selected_points) = shape_editor.select_point(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD, shift_pressed) {
						responses.add(DocumentMessage::StartTransaction);

						tool_data
							.snap_manager
							.start_snap(document, input, document.bounding_boxes(Some(&selected_layers), None, render_data), true, true);

						// Do not snap against handles when anchor is selected
						let mut additional_selected_points = Vec::new();
						for point in selected_points.points.iter() {
							if point.point_id.manipulator_type == SelectedType::Anchor {
								additional_selected_points.push(ManipulatorPointInfo {
									shape_layer_path: point.shape_layer_path,
									point_id: ManipulatorPointId::new(point.point_id.group, SelectedType::InHandle),
								});
								additional_selected_points.push(ManipulatorPointInfo {
									shape_layer_path: point.shape_layer_path,
									point_id: ManipulatorPointId::new(point.point_id.group, SelectedType::OutHandle),
								});
							}
						}
						selected_points.points.extend(additional_selected_points);

						let include_handles: Vec<_> = selected_layers.iter().map(|x| x.as_slice()).collect();
						tool_data.snap_manager.add_all_document_handles(document, input, &include_handles, &[], &selected_points.points);

						tool_data.drag_start_pos = input.mouse.position;
						tool_data.previous_mouse_position = input.mouse.position - selected_points.offset;

						tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);

						tool_data.line_extension = false;

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
							if shift_pressed {
								responses.add(DocumentMessage::AddSelectedLayers { additional_layers: intersection });
							} else {
								// Selects the topmost layer when selecting intersecting shapes
								let top_most_intersection = intersection[intersection.len() - 1].clone();
								responses.add(DocumentMessage::SetSelectedLayers {
									replacement_selected_layers: vec![top_most_intersection.clone()],
								});
								tool_data.drag_start_pos = input.mouse.position;
								tool_data.previous_mouse_position = input.mouse.position;
								// Selects all the anchor points when clicking in a filled area of shape. If two shapes intersect we pick the topmost layer.
								shape_editor.select_all_anchors(&document.document_legacy, &top_most_intersection);
								return PathToolFsmState::Dragging;
							}
						} else {
							// Clear the previous selection if we didn't find anything
							if !input.keyboard.get(shift_pressed as usize) {
								responses.add(DocumentMessage::DeselectAllLayers);
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
						line_extend,
					},
				) => {
					// Determine when alt state changes
					let alt_pressed = input.keyboard.get(alt_mirror_angle as usize);

					// Only on alt down
					if alt_pressed && !tool_data.alt_debounce {
						tool_data.opposing_handle_lengths = None;
						shape_editor.toggle_handle_mirroring_on_selected(responses);
					}
					tool_data.alt_debounce = alt_pressed;

					// Determine when shift state changes
					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

					if shift_pressed {
						if tool_data.opposing_handle_lengths.is_none() {
							tool_data.opposing_handle_lengths = Some(shape_editor.opposing_handle_lengths(&document.document_legacy));
						}
					} else if let Some(opposing_handle_lengths) = &tool_data.opposing_handle_lengths {
						shape_editor.reset_opposing_handle_lengths(&document.document_legacy, opposing_handle_lengths, responses);
						tool_data.opposing_handle_lengths = None;
					}

					// Move the selected points by the mouse position
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let mut delta = snapped_position - tool_data.previous_mouse_position;

					let doc_transform = document.document_legacy.root.transform;

					if document.grid_enabled {
						let previous_mouse_pos_doc_space = doc_transform.inverse().transform_point2(tool_data.previous_mouse_position);
						let previous_mouse_pos_viewport_space = doc_transform.transform_point2(previous_mouse_pos_doc_space);
						let previous_mouse_pos_viewport_space_rounded = doc_transform.transform_point2(previous_mouse_pos_doc_space.round());

						if let Some(_point_under) = shape_editor.find_nearest_point_indices(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD) {
							delta = snapped_position - previous_mouse_pos_viewport_space;
						} else {
							delta = snapped_position - previous_mouse_pos_viewport_space_rounded;
						}
					}

					tool_data.line_extension = input.keyboard.get(line_extend as usize);

					if !tool_data.line_extension {
						shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
						tool_data.previous_mouse_position = snapped_position;
					}

					if tool_data.line_extension {
						if let Some(nearest_anchor) = shape_editor.find_nearest_point_indices(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD) {
							// Unwrapping should not error based on the fact that the function shape_editor.find_nearest_point_indices() in shape_editor.rs returns a existing anchor point
							let anchors_layer = document.document_legacy.layer(nearest_anchor.0.as_slice()).unwrap();
							let subpaths = anchors_layer.as_vector_data().unwrap().subpaths.first().unwrap();
							let anchor_subpath = subpaths.manipulator_from_id(nearest_anchor.1.group).unwrap();

							// Determine next and previous indices based on closed or non-closed
							let anchor_subpath_index = subpaths.manipulator_groups().iter().position(|&manu_group| manu_group == *anchor_subpath).unwrap_or_default();
							let mut index_prev: Option<usize> = Some(anchor_subpath_index);
							let mut index_next: Option<usize> = Some(anchor_subpath_index);
							if subpaths.closed() {
								if anchor_subpath_index == 0 {
									index_prev = Some(subpaths.len() - 1);
								} else {
									index_prev = Some(anchor_subpath_index - 1);
								}

								if anchor_subpath_index == (subpaths.len() - 1) {
									index_next = Some(0);
								} else {
									index_next = Some(anchor_subpath_index + 1);
								}
							} else if !subpaths.closed() {
								if anchor_subpath_index == 0 {
									index_prev = None;
									index_next = Some(anchor_subpath_index + 1);
								} else if anchor_subpath_index == subpaths.len() - 1 {
									index_prev = Some(anchor_subpath_index - 1);
									index_next = None;
								} else {
									index_prev = Some(anchor_subpath_index - 1);
									index_next = Some(anchor_subpath_index + 1);
								}
							}

							let prev_anchor_position = subpaths[index_prev.unwrap_or_default()].anchor;
							let next_anchor_position = subpaths[index_next.unwrap_or_default()].anchor;
							tool_data.nearby_anchors = (anchor_subpath.anchor, prev_anchor_position, next_anchor_position);
							tool_data.dragged_manipulation = Some(nearest_anchor);
						}
						match &tool_data.dragged_manipulation {
							Some((anchor_point_id, anchor_point_manipulator)) => {
								let anchor_start_position = tool_data.nearby_anchors.0;
								let anchor_prev_position = tool_data.nearby_anchors.1;
								let anchor_next_position = tool_data.nearby_anchors.2;
								// Based on the anchor were dragging obtain the subpath
								if anchor_point_manipulator.manipulator_type == SelectedType::Anchor {
									// Unwrapping should not error based on the fact that the function shape_editor.find_nearest_point_indices() in shape_editor.rs returns a existing anchor point (line 228)
									let anchors_layer = document.document_legacy.layer(&anchor_point_id.as_slice()).unwrap();
									let subpaths = anchors_layer.as_vector_data().unwrap().subpaths.first().unwrap();
									let anchor_subpath = subpaths.manipulator_from_id(anchor_point_manipulator.group).unwrap();

									// Given the subpath, determine if the selected anchor has (no handles, or if it has handles only if its handles are near the anchor)
									if (anchor_subpath.in_handle == None && anchor_subpath.out_handle == None)
										|| (anchor_subpath.in_handle == None
											&& anchor_subpath.out_handle != None && anchor_subpath.anchor - anchor_subpath.out_handle.unwrap_or_default() == DVec2::new(0.0, 0.0))
										|| (anchor_subpath.out_handle == None
											&& anchor_subpath.in_handle != None && anchor_subpath.anchor - anchor_subpath.in_handle.unwrap_or_default() == DVec2::new(0.0, 0.0))
										|| (anchor_subpath.in_handle != None
											&& anchor_subpath.out_handle != None && anchor_subpath.anchor - anchor_subpath.in_handle.unwrap_or_default() == DVec2::new(0.0, 0.0)
											&& anchor_subpath.anchor - anchor_subpath.out_handle.unwrap_or_default() == DVec2::new(0.0, 0.0))
									{
										let anchor_subpath_index = subpaths.manipulator_groups().iter().position(|&manu_group| manu_group == *anchor_subpath).unwrap_or_default();

										// Determine if the anchor is Bezier to determine if we need to translate the position from local to document space
										let scaling_factor = 100.0;
										let rounded_anchor_transform = DMat2 {
											x_axis: DVec2 {
												x: (anchors_layer.transform.matrix2.x_axis.x * scaling_factor).round() / scaling_factor,
												y: anchors_layer.transform.matrix2.x_axis.y,
											},
											y_axis: DVec2 {
												x: anchors_layer.transform.matrix2.y_axis.x,
												y: (anchors_layer.transform.matrix2.y_axis.y * scaling_factor).round() / scaling_factor,
											},
										};
										let is_bezier = rounded_anchor_transform.x_axis == DVec2 { x: 1.0, y: 0.0 } && rounded_anchor_transform.y_axis == DVec2 { x: 0.0, y: 1.0 };

										let input_pos_doc_space = doc_transform.inverse().transform_point2(input.mouse.position);

										// Translate the dragged and its neighboring anchors to document space
										let mut viewspace = document.document_legacy.generate_transform_relative_to_viewport(&anchor_point_id).ok().unwrap_or_default();
										let transform = document.document_legacy.multiply_transforms(&anchor_point_id).unwrap_or_default();

										let viewspace_start_position = viewspace.transform_point2(anchor_start_position);
										let docspace_start_position = doc_transform.inverse().transform_point2(viewspace_start_position);

										let viewspace_prev_anchor = viewspace.transform_point2(anchor_prev_position);
										let docspace_prev_anchor = doc_transform.inverse().transform_point2(viewspace_prev_anchor);

										let viewspace_next_anchor = viewspace.transform_point2(anchor_next_position);
										let docspace_next_anchor = doc_transform.inverse().transform_point2(viewspace_next_anchor);

										// For non-bezier, we have to translate the manipulation points from Local to Document space
										let mut docspace_anchor = anchor_subpath.anchor;

										if !is_bezier {
											let viewspace_anchor = viewspace.transform_point2(docspace_anchor);
											docspace_anchor = doc_transform.inverse().transform_point2(viewspace_anchor);
										}

										if delta != DVec2::new(0.0, 0.0) {
											let mut index_prev: Option<usize> = Some(anchor_subpath_index);
											let mut index_next: Option<usize> = Some(anchor_subpath_index);

											// Determine the previous and next anchor indices
											if subpaths.closed() {
												if anchor_subpath_index == 0 {
													index_prev = Some(subpaths.len() - 1);
												} else {
													index_prev = Some(anchor_subpath_index - 1);
												}

												if anchor_subpath_index == (subpaths.len() - 1) {
													index_next = Some(0);
												} else {
													index_next = Some(anchor_subpath_index + 1);
												}
											} else if !subpaths.closed() {
												if anchor_subpath_index == 0 {
													index_prev = None;
													index_next = Some(anchor_subpath_index + 1);
												} else if anchor_subpath_index == subpaths.len() - 1 {
													index_prev = Some(anchor_subpath_index - 1);
													index_next = None;
												} else {
													index_prev = Some(anchor_subpath_index - 1);
													index_next = Some(anchor_subpath_index + 1);
												}
											}
											if !subpaths.closed() {
												if let (Some(prev_index), Some(next_index)) = (index_prev, index_next) {
													let mut prev_anchor_subpath = subpaths[prev_index];
													let mut next_anchor_subpath = subpaths[next_index];

													// For non-bezier, we have to translate the manipulation points from Local to Document space
													if !is_bezier {
														let viewspace_prev_anchor = viewspace.transform_point2(prev_anchor_subpath.anchor);
														let docspace_prev_anchor = doc_transform.inverse().transform_point2(viewspace_prev_anchor);
														prev_anchor_subpath.anchor = docspace_prev_anchor;
														if let Some(prev_anchor_in_handle) = prev_anchor_subpath.in_handle {
															let viewspace_prev_anchor_in_handle = viewspace.transform_point2(prev_anchor_in_handle);
															let docspace_prev_anchor_in_handle = doc_transform.inverse().transform_point2(viewspace_prev_anchor_in_handle);
															if docspace_prev_anchor_in_handle == prev_anchor_subpath.anchor {
																prev_anchor_subpath.in_handle = Some(docspace_prev_anchor);
															}
														}
														if let Some(prev_anchor_out_handle) = prev_anchor_subpath.out_handle {
															let viewspace_prev_anchor_out_handle = viewspace.transform_point2(prev_anchor_out_handle);
															let docspace_prev_anchor_out_handle = doc_transform.inverse().transform_point2(viewspace_prev_anchor_out_handle);
															if docspace_prev_anchor_out_handle == prev_anchor_subpath.anchor {
																prev_anchor_subpath.out_handle = Some(docspace_prev_anchor);
															}
														}

														let viewspace_next_anchor = viewspace.transform_point2(next_anchor_subpath.anchor);
														let docspace_next_anchor = doc_transform.inverse().transform_point2(viewspace_next_anchor);
														next_anchor_subpath.anchor = docspace_next_anchor;
														if let Some(next_anchor_in_handle) = next_anchor_subpath.in_handle {
															let viewspace_next_anchor_in_handle = viewspace.transform_point2(next_anchor_in_handle);
															let docspace_next_anchor_in_handle = doc_transform.inverse().transform_point2(viewspace_next_anchor_in_handle);
															if docspace_next_anchor_in_handle == next_anchor_subpath.anchor {
																next_anchor_subpath.in_handle = Some(docspace_next_anchor);
															}
														}
														if let Some(next_anchor_out_handle) = next_anchor_subpath.out_handle {
															let viewspace_next_anchor_out_handle = viewspace.transform_point2(next_anchor_out_handle);
															let docspace_next_anchor_out_handle = doc_transform.inverse().transform_point2(viewspace_next_anchor_out_handle);
															if docspace_next_anchor_out_handle == next_anchor_subpath.anchor {
																next_anchor_subpath.out_handle = Some(docspace_next_anchor);
															}
														}
													}

													let mut dx = 0.0;
													let mut dy = 0.0;

													dx = docspace_prev_anchor.x - docspace_start_position.x;
													dy = -(docspace_prev_anchor.y - docspace_start_position.y);

													let mut slope_prev = dy / dx;
													// If divide by zero error occurs, update the value of infinite to a large slope
													if slope_prev == std::f64::INFINITY || slope_prev == std::f64::NEG_INFINITY {
														slope_prev = 99999999999999.0;
													}
													if slope_prev.abs() == 0.0 {
														slope_prev = 0.0000000000000000000000000001;
													}

													// x and y swap signs gets reversed
													dx = docspace_next_anchor.x - docspace_start_position.x;
													dy = -(docspace_next_anchor.y - docspace_start_position.y);

													// if our anchor is on the other side of the next anchor, flip direction
													if dy < 0.0 {
														dx *= -1.0;
														dy *= -1.0;
													}

													let mut slope_next = dy / dx;
													// If divide by zero error occurs, update the value of infinite to a large slope
													if slope_next == std::f64::INFINITY || slope_next == std::f64::NEG_INFINITY {
														slope_next = 99999999999999.0;
													}
													if slope_next.abs() == 0.0 {
														slope_next = 0.0000000000000000000000000001;
													}

													// Use the slope, anchor position, and the mouse position calculate the distance from the mouse to the infinite lines
													let dist_from_line_prev = (slope_prev * (input_pos_doc_space.x - docspace_start_position.x)
														+ (input_pos_doc_space.y - docspace_start_position.y) + 0.0)
														.abs() / (slope_prev * slope_prev + 1.0).sqrt();
													let dist_from_line_next = (slope_next * (input_pos_doc_space.x - docspace_start_position.x)
														+ (input_pos_doc_space.y - docspace_start_position.y) + 0.0)
														.abs() / (slope_next * slope_next + 1.0).sqrt();

													let mut intersection = docspace_start_position;
													if dist_from_line_prev >= dist_from_line_next {
														if next_anchor_subpath.anchor == next_anchor_subpath.in_handle.unwrap_or_default()
															&& next_anchor_subpath.anchor == next_anchor_subpath.out_handle.unwrap_or_default()
														{
															let b = -(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_next) * (input_pos_doc_space.x - docspace_start_position.x));
															let intersection_x = (b - 0.0) / ((slope_next) - (-1.0 / slope_next));
															let intersection_y = -(slope_next * (intersection_x)) + 0.0;
															let intersection = DVec2 {
																x: intersection_x + docspace_start_position.x,
																y: intersection_y + docspace_start_position.y,
															};
															if is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = viewspace.transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, true);
															} else if !is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = doc_transform.transform_vector2(new_delta);
																new_delta = viewspace.inverse().transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, false);
															}
															let mut viewspace_intersection = doc_transform.transform_point2(intersection);
															tool_data.previous_mouse_position = viewspace_intersection;
														}
														// Prevent stalling, where the other infinite line is closer to the cursor when the next anchor is curved
														else {
															if prev_anchor_subpath.anchor == prev_anchor_subpath.in_handle.unwrap_or_default()
																&& prev_anchor_subpath.anchor == prev_anchor_subpath.out_handle.unwrap_or_default()
															{
																let b =
																	-(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_prev) * (input_pos_doc_space.x - docspace_start_position.x));
																let intersection_x = (b - 0.0) / ((slope_prev) - (-1.0 / slope_prev));
																let intersection_y = -(slope_prev * (intersection_x)) + 0.0;
																intersection = DVec2 {
																	x: intersection_x + docspace_start_position.x,
																	y: intersection_y + docspace_start_position.y,
																};
															} else {
																shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
																tool_data.previous_mouse_position = snapped_position;
															}
														}
													} else if dist_from_line_prev < dist_from_line_next {
														if prev_anchor_subpath.anchor == prev_anchor_subpath.in_handle.unwrap_or_default()
															&& prev_anchor_subpath.anchor == prev_anchor_subpath.out_handle.unwrap_or_default()
														{
															let b = -(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_prev) * (input_pos_doc_space.x - docspace_start_position.x));
															let intersection_x = (b - 0.0) / ((slope_prev) - (-1.0 / slope_prev));
															let intersection_y = -(slope_prev * (intersection_x)) + 0.0;
															let intersection = DVec2 {
																x: intersection_x + docspace_start_position.x,
																y: intersection_y + docspace_start_position.y,
															};
															if is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = viewspace.transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, true);
															} else if !is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = doc_transform.transform_vector2(new_delta);
																new_delta = viewspace.inverse().transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, false);
															}
															let mut viewspace_intersection = doc_transform.transform_point2(intersection);
															tool_data.previous_mouse_position = viewspace_intersection;
														}
														// Prevent stalling, where the other infinite line is closer to the cursor when the next anchor is curved
														else {
															if next_anchor_subpath.anchor == next_anchor_subpath.in_handle.unwrap_or_default()
																&& next_anchor_subpath.anchor == next_anchor_subpath.out_handle.unwrap_or_default()
															{
																let b =
																	-(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_next) * (input_pos_doc_space.x - docspace_start_position.x));
																let intersection_x = (b - 0.0) / ((slope_next) - (-1.0 / slope_next));
																let intersection_y = -(slope_next * (intersection_x)) + 0.0;
																intersection = DVec2 {
																	x: intersection_x + docspace_start_position.x,
																	y: intersection_y + docspace_start_position.y,
																};
															} else {
																shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
																tool_data.previous_mouse_position = snapped_position;
															}
														}
													}
												} else if let (Some(prev_index), None) = (index_prev, index_next) {
													let mut prev_anchor_subpath = subpaths[prev_index];

													// For non-bezier, we have to translate the manipulation points from Local to Document space
													if !is_bezier {
														let viewspace_prev_anchor = viewspace.transform_point2(prev_anchor_subpath.anchor);
														let docspace_prev_anchor = doc_transform.inverse().transform_point2(viewspace_prev_anchor);
														prev_anchor_subpath.anchor = docspace_prev_anchor;
														if let Some(prev_anchor_in_handle) = prev_anchor_subpath.in_handle {
															let viewspace_prev_anchor_in_handle = viewspace.transform_point2(prev_anchor_in_handle);
															let docspace_prev_anchor_in_handle = doc_transform.inverse().transform_point2(viewspace_prev_anchor_in_handle);
															if docspace_prev_anchor_in_handle == prev_anchor_subpath.anchor {
																prev_anchor_subpath.in_handle = Some(docspace_prev_anchor);
															}
														}
														if let Some(prev_anchor_out_handle) = prev_anchor_subpath.out_handle {
															let viewspace_prev_anchor_out_handle = viewspace.transform_point2(prev_anchor_out_handle);
															let docspace_prev_anchor_out_handle = doc_transform.inverse().transform_point2(viewspace_prev_anchor_out_handle);
															if docspace_prev_anchor_out_handle == prev_anchor_subpath.anchor {
																prev_anchor_subpath.out_handle = Some(docspace_prev_anchor);
															}
														}
													}

													let mut dx = 0.0;
													let mut dy = 0.0;
													dx = docspace_prev_anchor.x - docspace_start_position.x;
													dy = -(docspace_prev_anchor.y - docspace_start_position.y);

													let mut slope_prev = dy / dx;
													// If divide by zero error occurs, update the value of infinite to a large slope
													if slope_prev == std::f64::INFINITY || slope_prev == std::f64::NEG_INFINITY {
														slope_prev = 99999999999999.0;
													}
													if slope_prev.abs() == 0.0 {
														slope_prev = 0.0000000000000000000000000001;
													}

													if prev_anchor_subpath.anchor == prev_anchor_subpath.in_handle.unwrap_or_default()
														&& prev_anchor_subpath.anchor == prev_anchor_subpath.out_handle.unwrap_or_default()
													{
														debug!("if");
														let b = -(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_prev) * (input_pos_doc_space.x - docspace_start_position.x));
														let intersection_x = (b - 0.0) / ((slope_prev) - (-1.0 / slope_prev));
														let intersection_y = -(slope_prev * (intersection_x)) + 0.0;
														let intersection = DVec2 {
															x: intersection_x + docspace_start_position.x,
															y: intersection_y + docspace_start_position.y,
														};
														if is_bezier {
															let mut new_delta = intersection - docspace_anchor;
															new_delta = viewspace.transform_vector2(new_delta);
															shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, true);
														} else if !is_bezier {
															let mut new_delta = intersection - docspace_anchor;
															new_delta = doc_transform.transform_vector2(new_delta);
															new_delta = viewspace.inverse().transform_vector2(new_delta);
															shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, false);
														}
														let mut viewspace_intersection = doc_transform.transform_point2(intersection);
														tool_data.previous_mouse_position = viewspace_intersection;
													}
												// else {
												// 	shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
												// 	tool_data.previous_mouse_position = snapped_position;
												// }
												} else if let (None, Some(next_index)) = (index_prev, index_next) {
													let mut next_anchor_subpath = subpaths[next_index];

													// For non-bezier, we have to translate the manipulation points from Local to Document space
													if !is_bezier {
														let viewspace_next_anchor = viewspace.transform_point2(next_anchor_subpath.anchor);
														let docspace_next_anchor = doc_transform.inverse().transform_point2(viewspace_next_anchor);
														next_anchor_subpath.anchor = docspace_next_anchor;
														if let Some(next_anchor_in_handle) = next_anchor_subpath.in_handle {
															let viewspace_next_anchor_in_handle = viewspace.transform_point2(next_anchor_in_handle);
															let docspace_next_anchor_in_handle = doc_transform.inverse().transform_point2(viewspace_next_anchor_in_handle);
															if docspace_next_anchor_in_handle == next_anchor_subpath.anchor {
																next_anchor_subpath.in_handle = Some(docspace_next_anchor);
															}
														}
														if let Some(next_anchor_out_handle) = next_anchor_subpath.out_handle {
															let viewspace_next_anchor_out_handle = viewspace.transform_point2(next_anchor_out_handle);
															let docspace_next_anchor_out_handle = doc_transform.inverse().transform_point2(viewspace_next_anchor_out_handle);
															if docspace_next_anchor_out_handle == next_anchor_subpath.anchor {
																next_anchor_subpath.out_handle = Some(docspace_next_anchor);
															}
														}
													}

													let mut dx = 0.0;
													let mut dy = 0.0;
													dx = docspace_next_anchor.x - docspace_start_position.x;
													dy = -(docspace_next_anchor.y - docspace_start_position.y);

													let mut slope_next = dy / dx;
													// If divide by zero error occurs, update the value of infinite to a large slope
													if slope_next == std::f64::INFINITY || slope_next == std::f64::NEG_INFINITY {
														slope_next = 99999999999999.0;
													}
													if slope_next.abs() == 0.0 {
														slope_next = 0.0000000000000000000000000001;
													}

													if next_anchor_subpath.anchor == next_anchor_subpath.in_handle.unwrap_or_default()
														&& next_anchor_subpath.anchor == next_anchor_subpath.out_handle.unwrap_or_default()
													{
														let b = -(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_next) * (input_pos_doc_space.x - docspace_start_position.x));
														let intersection_x = (b - 0.0) / ((slope_next) - (-1.0 / slope_next));
														let intersection_y = -(slope_next * (intersection_x)) + 0.0;
														let intersection = DVec2 {
															x: intersection_x + docspace_start_position.x,
															y: intersection_y + docspace_start_position.y,
														};
														if is_bezier {
															let mut new_delta = intersection - docspace_anchor;
															new_delta = viewspace.transform_vector2(new_delta);
															shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, true);
														} else if !is_bezier {
															let mut new_delta = intersection - docspace_anchor;
															new_delta = doc_transform.transform_vector2(new_delta);
															new_delta = viewspace.inverse().transform_vector2(new_delta);
															shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, false);
														}
														let mut viewspace_intersection = doc_transform.transform_point2(intersection);
														tool_data.previous_mouse_position = viewspace_intersection;
													} else {
														shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
														tool_data.previous_mouse_position = snapped_position;
													}
												}
											}
											// Dragging a closed-shape
											else if subpaths.closed() {
												debug!("closed");
												if let (Some(prev_index), Some(next_index)) = (index_prev, index_next) {
													let mut prev_anchor_subpath = subpaths[prev_index];
													let mut next_anchor_subpath = subpaths[next_index];

													// For non-bezier, we have to translate the manipulation points from Local to Document space
													if !is_bezier {
														let viewspace_prev_anchor = viewspace.transform_point2(prev_anchor_subpath.anchor);
														let docspace_prev_anchor = doc_transform.inverse().transform_point2(viewspace_prev_anchor);
														prev_anchor_subpath.anchor = docspace_prev_anchor;
														if let Some(prev_anchor_in_handle) = prev_anchor_subpath.in_handle {
															let viewspace_prev_anchor_in_handle = viewspace.transform_point2(prev_anchor_in_handle);
															let docspace_prev_anchor_in_handle = doc_transform.inverse().transform_point2(viewspace_prev_anchor_in_handle);
															if docspace_prev_anchor_in_handle == prev_anchor_subpath.anchor {
																prev_anchor_subpath.in_handle = Some(docspace_prev_anchor);
															}
														}
														if let Some(prev_anchor_out_handle) = prev_anchor_subpath.out_handle {
															let viewspace_prev_anchor_out_handle = viewspace.transform_point2(prev_anchor_out_handle);
															let docspace_prev_anchor_out_handle = doc_transform.inverse().transform_point2(viewspace_prev_anchor_out_handle);
															if docspace_prev_anchor_out_handle == prev_anchor_subpath.anchor {
																prev_anchor_subpath.out_handle = Some(docspace_prev_anchor);
															}
														}

														let viewspace_next_anchor = viewspace.transform_point2(next_anchor_subpath.anchor);
														let docspace_next_anchor = doc_transform.inverse().transform_point2(viewspace_next_anchor);
														next_anchor_subpath.anchor = docspace_next_anchor;
														if let Some(next_anchor_in_handle) = next_anchor_subpath.in_handle {
															let viewspace_next_anchor_in_handle = viewspace.transform_point2(next_anchor_in_handle);
															let docspace_next_anchor_in_handle = doc_transform.inverse().transform_point2(viewspace_next_anchor_in_handle);
															if docspace_next_anchor_in_handle == next_anchor_subpath.anchor {
																next_anchor_subpath.in_handle = Some(docspace_next_anchor);
															}
														}
														if let Some(next_anchor_out_handle) = next_anchor_subpath.out_handle {
															let viewspace_next_anchor_out_handle = viewspace.transform_point2(next_anchor_out_handle);
															let docspace_next_anchor_out_handle = doc_transform.inverse().transform_point2(viewspace_next_anchor_out_handle);
															if docspace_next_anchor_out_handle == next_anchor_subpath.anchor {
																next_anchor_subpath.out_handle = Some(docspace_next_anchor);
															}
														}
													}

													let mut dx = 0.0;
													let mut dy = 0.0;
													dx = docspace_prev_anchor.x - docspace_start_position.x;
													dy = -(docspace_prev_anchor.y - docspace_start_position.y);

													let mut slope_prev = dy / dx;
													// If divide by zero error occurs, update the value of infinite to a large slope
													if slope_prev == std::f64::INFINITY || slope_prev == std::f64::NEG_INFINITY {
														slope_prev = 99999999999999.0;
													}
													if slope_prev.abs() == 0.0 {
														slope_prev = 0.0000000000000000000000000001;
													}

													// x and y swap signs gets reversed
													dx = docspace_next_anchor.x - docspace_start_position.x;
													dy = -(docspace_next_anchor.y - docspace_start_position.y);

													// if our anchor is on the other side of the next anchor, flip direction
													if dy < 0.0 {
														dx *= -1.0;
														dy *= -1.0;
													}

													let mut slope_next = dy / dx;
													// If divide by zero error occurs, update the value of infinite to a large slope
													if slope_next == std::f64::INFINITY || slope_next == std::f64::NEG_INFINITY {
														slope_next = 99999999999999.0;
													}
													if slope_next.abs() == 0.0 {
														slope_next = 0.0000000000000000000000000001;
													}

													// Use the slope, anchor position, and the mouse position calculate the distance from the mouse to the infinite lines
													let dist_from_line_prev = (slope_prev * (input_pos_doc_space.x - docspace_start_position.x)
														+ (input_pos_doc_space.y - docspace_start_position.y) + 0.0)
														.abs() / (slope_prev * slope_prev + 1.0).sqrt();
													let dist_from_line_next = (slope_next * (input_pos_doc_space.x - docspace_start_position.x)
														+ (input_pos_doc_space.y - docspace_start_position.y) + 0.0)
														.abs() / (slope_next * slope_next + 1.0).sqrt();

													let mut intersection = docspace_start_position;
													if dist_from_line_prev >= dist_from_line_next {
														if next_anchor_subpath.anchor == next_anchor_subpath.in_handle.unwrap_or_default()
															&& next_anchor_subpath.anchor == next_anchor_subpath.out_handle.unwrap_or_default()
														{
															let b = -(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_next) * (input_pos_doc_space.x - docspace_start_position.x));
															let intersection_x = (b - 0.0) / ((slope_next) - (-1.0 / slope_next));
															let intersection_y = -(slope_next * (intersection_x)) + 0.0;
															let intersection = DVec2 {
																x: intersection_x + docspace_start_position.x,
																y: intersection_y + docspace_start_position.y,
															};
															if is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = viewspace.transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, true);
															} else if !is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = doc_transform.transform_vector2(new_delta);
																new_delta = viewspace.inverse().transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, false);
															}
														// let v = viewspace.transform_vector2(input.mouse.position);
														// tool_data.previous_mouse_position = v;
														}
														// Prevent stalling, where the other infinite line is closer to the cursor when the next anchor is curved
														else {
															if prev_anchor_subpath.anchor == prev_anchor_subpath.in_handle.unwrap_or_default()
																&& prev_anchor_subpath.anchor == prev_anchor_subpath.out_handle.unwrap_or_default()
															{
																let b =
																	-(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_prev) * (input_pos_doc_space.x - docspace_start_position.x));
																let intersection_x = (b - 0.0) / ((slope_prev) - (-1.0 / slope_prev));
																let intersection_y = -(slope_prev * (intersection_x)) + 0.0;
																intersection = DVec2 {
																	x: intersection_x + docspace_start_position.x,
																	y: intersection_y + docspace_start_position.y,
																};
															} else {
																shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
																tool_data.previous_mouse_position = snapped_position;
															}
														}
													} else if dist_from_line_prev < dist_from_line_next {
														if prev_anchor_subpath.anchor == prev_anchor_subpath.in_handle.unwrap_or_default()
															&& prev_anchor_subpath.anchor == prev_anchor_subpath.out_handle.unwrap_or_default()
														{
															let b = -(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_prev) * (input_pos_doc_space.x - docspace_start_position.x));
															let intersection_x = (b - 0.0) / ((slope_prev) - (-1.0 / slope_prev));
															let intersection_y = -(slope_prev * (intersection_x)) + 0.0;
															let intersection = DVec2 {
																x: intersection_x + docspace_start_position.x,
																y: intersection_y + docspace_start_position.y,
															};
															if is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = viewspace.transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, true);
															} else if !is_bezier {
																let mut new_delta = intersection - docspace_anchor;
																new_delta = doc_transform.transform_vector2(new_delta);
																new_delta = viewspace.inverse().transform_vector2(new_delta);
																shape_editor.move_selected_points(&document.document_legacy, new_delta, shift_pressed, responses, false);
															}
														// let v = viewspace.transform_vector2(input.mouse.position);
														// tool_data.previous_mouse_position = v;
														}
														// Prevent stalling, where the other infinite line is closer to the cursor when the next anchor is curved
														else {
															if next_anchor_subpath.anchor == next_anchor_subpath.in_handle.unwrap_or_default()
																&& next_anchor_subpath.anchor == next_anchor_subpath.out_handle.unwrap_or_default()
															{
																let b =
																	-(input_pos_doc_space.y - docspace_start_position.y) - ((-1.0 / slope_next) * (input_pos_doc_space.x - docspace_start_position.x));
																let intersection_x = (b - 0.0) / ((slope_next) - (-1.0 / slope_next));
																let intersection_y = -(slope_next * (intersection_x)) + 0.0;
																intersection = DVec2 {
																	x: intersection_x + docspace_start_position.x,
																	y: intersection_y + docspace_start_position.y,
																};
															} else {
																shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
																tool_data.previous_mouse_position = snapped_position;
															}
														}
													}
												}
											}
										}
									} else {
										shape_editor.move_selected_points(&document.document_legacy, delta, shift_pressed, responses, true);
										tool_data.previous_mouse_position = snapped_position;
									}
								}
							}
							None => {}
						}
					}

					PathToolFsmState::Dragging
				}

				// Mouse up
				(_, PathToolMessage::DragStop { shift_mirror_distance }) => {
					let nearest_point = shape_editor
						.find_nearest_point_indices(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD)
						.map(|(_, nearest_point)| nearest_point);
					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

					shape_editor.delete_selected_handles_with_zero_length(&document.document_legacy, &tool_data.opposing_handle_lengths, responses);

					if tool_data.drag_start_pos.distance(input.mouse.position) <= DRAG_THRESHOLD && !shift_pressed {
						let clicked_selected = shape_editor.selected_points().any(|&point| nearest_point == Some(point));
						if clicked_selected {
							shape_editor.deselect_all();
							shape_editor.select_point(&document.document_legacy, input.mouse.position, SELECTION_THRESHOLD, false);
						}
					}

					tool_data.dragged_manipulation = None;
					tool_data.snap_manager.cleanup(responses);
					PathToolFsmState::Ready
				}
				// Delete key
				(_, PathToolMessage::Delete) => {
					// Delete the selected points and clean up overlays
					responses.add(DocumentMessage::StartTransaction);
					shape_editor.delete_selected_points(responses);
					responses.add(PathToolMessage::SelectionChanged);
					for layer_path in document.all_layers() {
						shape_overlay.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}
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
					for layer_path in document.all_layers() {
						shape_overlay.clear_subpath_overlays(&document.document_legacy, layer_path.to_vec(), responses);
					}
					PathToolFsmState::Ready
				}
				(
					_,
					PathToolMessage::PointerMove {
						alt_mirror_angle: _,
						shift_mirror_distance: _,
						line_extend: _,
					},
				) => self,
				(_, PathToolMessage::NudgeSelectedPoints { delta_x, delta_y }) => {
					shape_editor.move_selected_points(&document.document_legacy, (delta_x, delta_y).into(), true, responses, true);
					PathToolFsmState::Ready
				}
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			PathToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Select Point"), HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
				HintGroup(vec![HintInfo::arrow_keys("Nudge Selected"), HintInfo::keys([Key::Shift], "10x").prepend_plus()]),
				HintGroup(vec![HintInfo::keys([Key::KeyG, Key::KeyR, Key::KeyS], "Grab/Rotate/Scale Selected")]),
			]),
			PathToolFsmState::Dragging => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Alt], "Split/Align Handles (Toggle)"),
				HintInfo::keys([Key::Shift], "Share Lengths of Aligned Handles"),
				HintInfo::keys([Key::KeyV], "Line Extend"),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
