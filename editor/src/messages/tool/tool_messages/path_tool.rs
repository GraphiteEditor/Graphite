use std::vec;

use crate::consts::{DRAG_THRESHOLD, SELECTION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::overlay_renderer::OverlayRenderer;
use crate::messages::tool::common_functionality::shape_editor::{ManipulatorPointInfo, OpposingHandleLengths, ShapeState};
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::common_functionality::transformation_cage::{add_bounding_box, transform_from_box};
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, HintData, HintGroup, HintInfo, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};

use document_legacy::document::Document;
use document_legacy::intersection::Quad;
use document_legacy::{LayerId, Operation};
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use glam::{DAffine2, DVec2};
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
	Enter {
		add_to_selection: Key,
	},
	InsertPoint,
	ManipulatorAngleMadeSharp,
	ManipulatorAngleMadeSmooth,
	NudgeSelectedPoints {
		delta_x: f64,
		delta_y: f64,
	},
	PointerMove {
		alt_mirror_angle: Key,
		shift_mirror_distance: Key,
	},
	SelectedPointUpdated,
	SelectedPointXChanged {
		new_x: f64,
	},
	SelectedPointYChanged {
		new_y: f64,
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
		let coordinates = self.tool_data.selection_status.as_one().as_ref().map(|point| point.coordinates);
		let manipulator_angle = self.tool_data.selection_status.as_one().as_ref().map(|point| point.manipulator_angle);
		let (x, y) = coordinates.map(|point| (Some(point.x), Some(point.y))).unwrap_or((None, None));

		let x_location = NumberInput::new(x)
			.unit(" px")
			.label("X")
			.min_width(120)
			.disabled(x.is_none())
			.min(-((1u64 << std::f64::MANTISSA_DIGITS) as f64))
			.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				let new_x = number_input.value.unwrap_or(x.unwrap());
				PathToolMessage::SelectedPointXChanged { new_x }.into()
			})
			.widget_holder();

		let y_location = NumberInput::new(y)
			.unit(" px")
			.label("Y")
			.min_width(120)
			.disabled(y.is_none())
			.min(-((1u64 << std::f64::MANTISSA_DIGITS) as f64))
			.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
			.on_update(move |number_input: &NumberInput| {
				let new_y = number_input.value.unwrap_or(y.unwrap());
				PathToolMessage::SelectedPointYChanged { new_y }.into()
			})
			.widget_holder();

		let seperator = Separator::new(SeparatorType::Related).widget_holder();

		let seperator_2 = Separator::new(SeparatorType::Unrelated).widget_holder();

		let mut index = manipulator_angle.map(|angle| angle as u32).unwrap_or(0);

		if let Some(many) = self.tool_data.selection_status.as_many() {
			index = many.manipulator_angle as u32;
		}

		let options = vec![
			RadioEntryData::new("Smooth").on_update(|_| PathToolMessage::ManipulatorAngleMadeSmooth.into()),
			RadioEntryData::new("Sharp").on_update(|_| PathToolMessage::ManipulatorAngleMadeSharp.into()),
		];

		let manipulator_angle_radio = RadioInput::new(options).disabled(self.tool_data.selection_status.is_none()).selected_index(index).widget_holder();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![x_location, seperator, y_location, seperator_2, manipulator_angle_radio],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for PathTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let updating_point = message == ToolMessage::Path(PathToolMessage::SelectedPointUpdated);

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);

		if updating_point {
			self.send_layout(responses, LayoutTarget::ToolOptions);
		}
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
	selection_status: SelectionStatus,
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

					responses.add(PathToolMessage::SelectedPointUpdated);
					// This can happen in any state (which is why we return self)
					self
				}
				(_, PathToolMessage::DocumentIsDirty) => {
					// When the document has moved / needs to be redraw, re-render the overlays
					// TODO the overlay system should probably receive this message instead of the tool
					for layer_path in document.selected_visible_layers() {
						shape_overlay.render_subpath_overlays(&shape_editor.selected_shape_state, &document.document_legacy, layer_path.to_vec(), responses);
					}

					responses.add(PathToolMessage::SelectedPointUpdated);

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

						responses.add(PathToolMessage::SelectedPointUpdated);
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
							// an empty intersection means that the user is drawing a box
							tool_data.drag_start_pos = input.mouse.position;
							tool_data.previous_mouse_position = input.mouse.position;

							tool_data.drag_box_overlay_layer = Some(add_bounding_box(responses));
							return PathToolFsmState::DrawingBox;
						}

						PathToolFsmState::Ready
					}
				}
				(PathToolFsmState::DrawingBox, PathToolMessage::PointerMove { .. }) => {
					tool_data.previous_mouse_position = input.mouse.position;

					responses.add_front(DocumentMessage::Overlays(
						Operation::SetLayerTransformInViewport {
							path: tool_data.drag_box_overlay_layer.clone().unwrap(),
							transform: transform_from_box(tool_data.drag_start_pos, tool_data.previous_mouse_position, DAffine2::IDENTITY).to_cols_array(),
						}
						.into(),
					));
					PathToolFsmState::DrawingBox
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
					shape_editor.move_selected_points(&document.document_legacy, snapped_position - tool_data.previous_mouse_position, shift_pressed, responses);
					tool_data.previous_mouse_position = snapped_position;
					PathToolFsmState::Dragging
				}

				(PathToolFsmState::DrawingBox, PathToolMessage::Enter { add_to_selection }) => {
					let shift_pressed = input.keyboard.get(add_to_selection as usize);

					if tool_data.drag_start_pos == tool_data.previous_mouse_position {
						responses.add(DocumentMessage::DeselectAllLayers);
					} else {
						shape_editor.select_all_in_quad(&document.document_legacy, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !shift_pressed);
						tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);
					};

					responses.add_front(DocumentMessage::Overlays(
						Operation::DeleteLayer {
							path: tool_data.drag_box_overlay_layer.take().unwrap(),
						}
						.into(),
					));

					responses.add(PathToolMessage::SelectedPointUpdated);
					PathToolFsmState::Ready
				}

				// Mouse up
				(PathToolFsmState::DrawingBox, PathToolMessage::DragStop { shift_mirror_distance }) => {
					let shift_pressed = input.keyboard.get(shift_mirror_distance as usize);

					if tool_data.drag_start_pos == tool_data.previous_mouse_position {
						responses.add(DocumentMessage::DeselectAllLayers);
					} else {
						shape_editor.select_all_in_quad(&document.document_legacy, [tool_data.drag_start_pos, tool_data.previous_mouse_position], !shift_pressed);
						tool_data.refresh_overlays(document, shape_editor, shape_overlay, responses);
					};

					responses.add_front(DocumentMessage::Overlays(
						Operation::DeleteLayer {
							path: tool_data.drag_box_overlay_layer.take().unwrap(),
						}
						.into(),
					));

					responses.add(PathToolMessage::SelectedPointUpdated);
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
						}
					}

					responses.add(PathToolMessage::SelectedPointUpdated);
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

					responses.add(PathToolMessage::SelectedPointUpdated);
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
					},
				) => self,
				(_, PathToolMessage::NudgeSelectedPoints { delta_x, delta_y }) => {
					shape_editor.move_selected_points(&document.document_legacy, (delta_x, delta_y).into(), true, responses);
					PathToolFsmState::Ready
				}
				(_, PathToolMessage::SelectedPointXChanged { new_x }) => {
					if let Some(&SingleSelectedPoint { coordinates, id, ref layer_path, .. }) = tool_data.selection_status.as_one() {
						shape_editor.reposition_control_point(&id, responses, &document.document_legacy, DVec2::new(new_x, coordinates.y), layer_path);
					}
					PathToolFsmState::Ready
				}
				(_, PathToolMessage::SelectedPointYChanged { new_y }) => {
					if let Some(&SingleSelectedPoint { coordinates, id, ref layer_path, .. }) = tool_data.selection_status.as_one() {
						shape_editor.reposition_control_point(&id, responses, &document.document_legacy, DVec2::new(coordinates.x, new_y), layer_path);
					}
					PathToolFsmState::Ready
				}
				(_, PathToolMessage::SelectedPointUpdated) => {
					tool_data.selection_status = get_selection_status(&document.document_legacy, shape_editor);
					self
				}
				(_, PathToolMessage::ManipulatorAngleMadeSmooth) => {
					responses.add(DocumentMessage::StartTransaction);
					shape_editor.set_handle_mirroring_on_selected(true, responses);
					for group in shape_editor.selected_manipulator_groups().iter() {
						shape_editor.blink_manipulator_group(&group, responses, &document.document_legacy);
					}
					responses.add(DocumentMessage::CommitTransaction);
					PathToolFsmState::Ready
				}
				(_, PathToolMessage::ManipulatorAngleMadeSharp) => {
					responses.add(DocumentMessage::StartTransaction);
					shape_editor.set_handle_mirroring_on_selected(false, responses);
					responses.add(DocumentMessage::CommitTransaction);
					PathToolFsmState::Ready
				}
				(_, _) => PathToolFsmState::Ready,
			}
		} else {
			self
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

#[derive(Debug, PartialEq, Default)]
enum SelectionStatus {
	#[default]
	None,
	One(SingleSelectedPoint),
	Many(ManySelectedPoints),
}

impl SelectionStatus {
	fn as_one(&self) -> Option<&SingleSelectedPoint> {
		if let SelectionStatus::One(one) = self {
			Some(one)
		} else {
			None
		}
	}

	fn as_many(&self) -> Option<&ManySelectedPoints> {
		if let SelectionStatus::Many(many) = self {
			Some(many)
		} else {
			None
		}
	}

	fn is_none(&self) -> bool {
		self == &SelectionStatus::None
	}
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum ManipulatorAngle {
	Smooth = 0,
	Sharp = 1,
	Mixed = 2,
}

#[derive(Debug, PartialEq)]
struct ManySelectedPoints {
	manipulator_angle: ManipulatorAngle,
}

#[derive(Debug, PartialEq)]
struct SingleSelectedPoint {
	coordinates: DVec2,
	id: ManipulatorPointId,
	layer_path: Vec<u64>,
	manipulator_angle: ManipulatorAngle,
}

// If there is one selected and only one manipulator group this yields the selected control point,
// if only one handle is selected it will yield that handle, otherwise it will yield the groups anchor.
fn get_selection_status(document: &Document, shape_state: &mut ShapeState) -> SelectionStatus {
	// Check to see if only one manipulator group is selected
	let selection_layers: Vec<_> = shape_state.selected_shape_state.iter().take(2).map(|(k, v)| (k, v.selected_points_count())).collect();
	if let [(layer, 1)] = selection_layers[..] {
		let Some(layer_data) = document.layer(layer).ok() else { return SelectionStatus::None };
		let Some(vector_data) = layer_data.as_vector_data() else { return SelectionStatus::None };
		let Some(point) = shape_state.selected_points().next() else {
			return SelectionStatus::None;
		};

		let Some(group) = vector_data.manipulator_from_id(point.group) else {
			return SelectionStatus::None;
		};
		let Some(local_position) = point.manipulator_type.get_position(group) else {
			return SelectionStatus::None;
		};

		let manipulator_angle = if vector_data.mirror_angle.contains(&point.group) {
			ManipulatorAngle::Smooth
		} else {
			ManipulatorAngle::Sharp
		};

		return SelectionStatus::One(SingleSelectedPoint {
			coordinates: layer_data.transform.transform_point2(local_position) + layer_data.pivot,
			layer_path: layer.clone(),
			id: *point,
			manipulator_angle,
		});
	};

	let groups = shape_state.selected_manipulator_groups();
	if let Some(first) = groups.get(0) {
		let first_is_smooth = first.is_smooth(&document);
		let mixed = groups.iter().any(|point| first_is_smooth != point.is_smooth(&document));
		let manipulator_angle = match (first_is_smooth, mixed) {
			(_, true) => ManipulatorAngle::Mixed,
			(false, false) => ManipulatorAngle::Sharp,
			_ => ManipulatorAngle::Smooth,
		};
		return SelectionStatus::Many(ManySelectedPoints { manipulator_angle });
	}

	SelectionStatus::None
}
