use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::overlay_renderer::OverlayRenderer;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene::layers::style;
use graphene::layers::vector::consts::ManipulatorType;
use graphene::layers::vector::manipulator_group::ManipulatorGroup;
use graphene::LayerId;
use graphene::Operation;

use glam::{DAffine2, DVec2};
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
	#[remain::unsorted]
	SelectionChanged,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove {
		snap_angle: Key,
		break_handle: Key,
	},
	Undo,
	UpdateOptions(PenOptionsUpdate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	DraggingHandle,
	PlacingAnchor,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PenOptionsUpdate {
	LineWeight(f64),
}

impl ToolMetadata for PenTool {
	fn icon_name(&self) -> String {
		"VectorPenTool".into()
	}
	fn tooltip(&self) -> String {
		"Pen Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Pen
	}
}

impl PropertyHolder for PenTool {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: Some(self.options.line_weight),
				is_integer: false,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value.unwrap())).into()),
				..NumberInput::default()
			}))],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for PenTool {
	fn process_message(&mut self, message: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if message == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if message == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Pen(PenToolMessage::UpdateOptions(action)) = message {
			match action {
				PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		let new_state = self.fsm_state.transition(message, &mut self.tool_data, tool_data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			PenToolFsmState::Ready => actions!(PenToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				Confirm,
				Abort,
			),
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => actions!(PenToolMessageDiscriminant;
				DragStart,
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
		}
	}
}

impl ToolTransition for PenTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(PenToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(PenToolMessage::Abort.into()),
			selection_changed: Some(PenToolMessage::SelectionChanged.into()),
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
	overlay_renderer: OverlayRenderer,
	snap_manager: SnapManager,
	from_start: bool,
	should_mirror: bool,
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
		let transform = tool_data.path.as_ref().and_then(|path| document.graphene_document.multiply_transforms(path).ok()).unwrap_or_default();

		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(_, PenToolMessage::DocumentIsDirty) => {
					// When the document has moved / needs to be redraw, re-render the overlays
					// TODO the overlay system should probably receive this message instead of the tool
					for layer_path in document.selected_visible_layers() {
						tool_data.overlay_renderer.render_subpath_overlays(&document.graphene_document, layer_path.to_vec(), responses);
					}
					self
				}
				(_, PenToolMessage::SelectionChanged) => {
					// Set the previously selected layers to invisible
					for layer_path in document.all_layers() {
						tool_data.overlay_renderer.layer_overlay_visibility(&document.graphene_document, layer_path.to_vec(), false, responses);
					}
					self
				}
				(PenToolFsmState::Ready, PenToolMessage::DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());

					// Initalise snapping
					tool_data.snap_manager.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
					tool_data.snap_manager.add_all_document_handles(document, &[], &[], &[]);

					tool_data.should_mirror = false;

					if let Some((layer, from_start)) = should_extend(document, input.mouse.position, crate::consts::SNAP_POINT_TOLERANCE) {
						tool_data.path = Some(layer.to_vec());
						tool_data.from_start = from_start;
						return PenToolFsmState::DraggingHandle;
					}
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					// Create a new layer
					tool_data.path = Some(document.get_path_for_new_layer());
					tool_data.from_start = false;

					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

					// Get the position and set properties
					let transform = tool_data
						.path
						.as_ref()
						.and_then(|path| document.graphene_document.multiply_transforms(&path[..path.len() - 1]).ok())
						.unwrap_or_default();
					let start_position = transform.inverse().transform_point2(snapped_position);
					tool_data.weight = tool_options.line_weight;

					// Create the initial shape with a `bez_path` (only contains a moveto initially)
					if let Some(layer_path) = &tool_data.path {
						responses.push_back(
							Operation::AddShape {
								path: layer_path.clone(),
								transform: DAffine2::IDENTITY.to_cols_array(),
								insert_index: -1,
								subpath: Default::default(),
								style: style::PathStyle::new(Some(style::Stroke::new(global_tool_data.primary_color, tool_data.weight)), style::Fill::None),
							}
							.into(),
						);
						responses.push_back(add_manipulator_group(
							&tool_data.path,
							tool_data.from_start,
							ManipulatorGroup::new_with_handles(start_position, Some(start_position), Some(start_position)),
						));
					}

					PenToolFsmState::DraggingHandle
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::DragStart) => PenToolFsmState::DraggingHandle,
				(PenToolFsmState::DraggingHandle, PenToolMessage::DragStop) => {
					let mut process = || {
						// Get subpath
						let layer_path = tool_data.path.as_ref()?;
						let subpath = document.graphene_document.layer(layer_path).ok().and_then(|layer| layer.as_subpath())?;

						// Get manipulator groups
						let mut iter = subpath.manipulator_groups().enumerate();
						let (&last_id, last_manipulator_group) = if tool_data.from_start { iter.next()? } else { iter.next_back()? };
						let previous = if tool_data.from_start { iter.next() } else { iter.next_back() };
						let mut iter = subpath.manipulator_groups().enumerate();
						let (&first_id, first_manipulator_group) = if tool_data.from_start { iter.next_back()? } else { iter.next()? };

						// Get correct handle types
						let inwards_handle = if tool_data.from_start { ManipulatorType::OutHandle } else { ManipulatorType::InHandle };
						let outwards_handle = if tool_data.from_start { ManipulatorType::InHandle } else { ManipulatorType::OutHandle };

						// Get manipulator points
						let last_anchor = last_manipulator_group.points[ManipulatorType::Anchor].as_ref()?;
						let first_anchor = first_manipulator_group.points[ManipulatorType::Anchor].as_ref()?;
						let last_in = last_manipulator_group.points[inwards_handle].as_ref()?;

						let transformed_distance_between = transform.transform_point2(last_anchor.position).distance_squared(transform.transform_point2(first_anchor.position));

						// Close path
						if transformed_distance_between < crate::consts::SNAP_POINT_TOLERANCE.powi(2) && previous.is_some() {
							// Move the in handle of the first point to where the user has placed it
							let op = Operation::MoveManipulatorPoint {
								layer_path: layer_path.clone(),
								id: first_id,
								manipulator_type: inwards_handle,
								position: last_in.position.into(),
							};
							responses.push_back(op.into());

							// Stop the handles on the first point from mirroring
							let op = Operation::SetManipulatorHandleMirroring {
								layer_path: layer_path.clone(),
								id: first_id,
								mirror_distance: false,
								mirror_angle: false,
							};
							responses.push_back(op.into());

							// Remove the node that has just been placed
							let op = Operation::RemoveManipulatorGroup {
								layer_path: layer_path.clone(),
								id: last_id,
							};
							responses.push_back(op.into());

							// Push a close path node
							responses.push_back(add_manipulator_group(&tool_data.path, tool_data.from_start, ManipulatorGroup::closed()));

							responses.push_back(DocumentMessage::CommitTransaction.into());

							// Clean up overlays
							for layer_path in document.all_layers() {
								tool_data.overlay_renderer.clear_subpath_overlays(&document.graphene_document, layer_path.to_vec(), responses);
							}
							tool_data.path = None;
							tool_data.snap_manager.cleanup(responses);

							return Some(PenToolFsmState::Ready);
						}
						// Add a new manipulator for the next anchor that we will place
						if let Some(out_handle) = &last_manipulator_group.points[outwards_handle] {
							responses.push_back(add_manipulator_group(&tool_data.path, tool_data.from_start, ManipulatorGroup::new_with_anchor(out_handle.position)));
						}

						None
					};
					tool_data.should_mirror = true;
					process().unwrap_or(PenToolFsmState::PlacingAnchor)
				}
				(PenToolFsmState::DraggingHandle, PenToolMessage::PointerMove { snap_angle, break_handle }) => {
					let mut process = || {
						// Get subpath
						let layer_path = tool_data.path.as_ref()?;
						let subpath = document.graphene_document.layer(layer_path).ok().and_then(|layer| layer.as_subpath())?;

						// Get manipulator groups
						let mut iter = subpath.manipulator_groups().enumerate();
						let (&last_id, last_manipulator_group) = if tool_data.from_start { iter.next()? } else { iter.next_back()? };

						// Get correct handle types
						let inwards_handle = if tool_data.from_start { ManipulatorType::OutHandle } else { ManipulatorType::InHandle };
						let outwards_handle = if tool_data.from_start { ManipulatorType::InHandle } else { ManipulatorType::OutHandle };

						// Get manipulator points
						let last_anchor = last_manipulator_group.points[ManipulatorType::Anchor].as_ref()?;

						let mouse = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
						let pos = transform.inverse().transform_point2(mouse);
						let pos = compute_snapped_angle(input, snap_angle, pos, last_anchor.position);

						// Update points on current segment (to show preview of new handle)
						let msg = Operation::MoveManipulatorPoint {
							layer_path: layer_path.clone(),
							id: last_id,
							manipulator_type: outwards_handle,
							position: pos.into(),
						};
						responses.push_back(msg.into());

						// Mirror handle of last segment
						if !input.keyboard.get(break_handle as usize) && tool_data.should_mirror {
							let pos = last_anchor.position - (pos - last_anchor.position);

							let msg = Operation::MoveManipulatorPoint {
								layer_path: layer_path.clone(),
								id: last_id,
								manipulator_type: inwards_handle,
								position: pos.into(),
							};
							responses.push_back(msg.into());
						}

						Some(())
					};
					if process().is_none() {
						PenToolFsmState::Ready
					} else {
						self
					}
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerMove { snap_angle, .. }) => {
					let mut process = || {
						// Get subpath
						let layer_path = tool_data.path.as_ref()?;
						let subpath = document.graphene_document.layer(layer_path).ok().and_then(|layer| layer.as_subpath())?;

						// Get manipulator groups
						let mut iter = subpath.manipulator_groups().enumerate();
						let (&last_id, _last_manipulator_group) = if tool_data.from_start { iter.next()? } else { iter.next_back()? };
						let previous = if tool_data.from_start { iter.next() } else { iter.next_back() };
						let mut iter = subpath.manipulator_groups().enumerate();
						let (_first_id, first_manipulator_group) = if tool_data.from_start { iter.next_back()? } else { iter.next()? };

						// Get manipulator points
						let first_anchor = first_manipulator_group.points[ManipulatorType::Anchor].as_ref()?;

						let mouse = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
						let mut pos = transform.inverse().transform_point2(mouse);

						// Snap to the first point (to show close path)
						if mouse.distance_squared(transform.transform_point2(first_anchor.position)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2) {
							pos = first_anchor.position;
						}

						if let Some(relative) = previous.as_ref().and_then(|(_, manipulator_group)| manipulator_group.points[ManipulatorType::Anchor].as_ref()) {
							pos = compute_snapped_angle(input, snap_angle, pos, relative.position);
						}

						for manipulator_type in [ManipulatorType::Anchor, ManipulatorType::InHandle, ManipulatorType::OutHandle] {
							let msg = Operation::MoveManipulatorPoint {
								layer_path: layer_path.clone(),
								id: last_id,
								manipulator_type,
								position: pos.into(),
							};
							responses.push_back(msg.into());
						}

						Some(())
					};
					if process().is_none() {
						PenToolFsmState::Ready
					} else {
						self
					}
				}
				(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Abort | PenToolMessage::Confirm) => {
					// Abort or commit the transaction to the undo history
					let mut commit = || {
						// Get subpath
						let layer_path = tool_data.path.as_ref()?;
						let subpath = document.graphene_document.layer(layer_path).ok().and_then(|layer| layer.as_subpath())?;

						// If placing anchor we should abort if there are less than three manipulators (as the last one gets deleted)
						if self == PenToolFsmState::PlacingAnchor && subpath.manipulator_groups().len() < 3 {
							return None;
						}

						// Get manipulator groups
						let mut iter = subpath.manipulator_groups().enumerate();
						let (&(mut last_id), mut last_manipulator_group) = if tool_data.from_start { iter.next()? } else { iter.next_back()? };
						let previous = if tool_data.from_start { iter.next() } else { iter.next_back() };

						// Get correct handle types
						let outwards_handle = if tool_data.from_start { ManipulatorType::InHandle } else { ManipulatorType::OutHandle };

						// Cleanup if there are two or more manipulators
						if let Some((&previous_id, previous_manipulator_group)) = previous {
							// Remove the unplaced anchor if in anchor placing mode
							if self == PenToolFsmState::PlacingAnchor {
								let layer_path = layer_path.clone();
								let op = Operation::RemoveManipulatorGroup { layer_path, id: last_id };
								responses.push_back(op.into());
								last_id = previous_id;
								last_manipulator_group = previous_manipulator_group;
							}

							// Remove the out handle
							let op = Operation::MoveManipulatorPoint {
								layer_path: layer_path.clone(),
								id: last_id,
								manipulator_type: outwards_handle,
								position: last_manipulator_group.points[ManipulatorType::Anchor].as_ref()?.position.into(),
							};
							responses.push_back(op.into());

							responses.push_back(DocumentMessage::CommitTransaction.into());
						} else {
							// Abort if only one manipulator group has been placed
							return None;
						}

						Some(())
					};
					if commit().is_none() {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					// Clean up overlays
					for layer_path in document.all_layers() {
						tool_data.overlay_renderer.clear_subpath_overlays(&document.graphene_document, layer_path.to_vec(), responses);
					}
					tool_data.path = None;
					tool_data.snap_manager.cleanup(responses);

					PenToolFsmState::Ready
				}
				(_, PenToolMessage::Abort) => {
					// Clean up overlays
					for layer_path in document.all_layers() {
						tool_data.overlay_renderer.clear_subpath_overlays(&document.graphene_document, layer_path.to_vec(), responses);
					}
					self
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
				key_groups_mac: None,
				mouse: Some(MouseMotion::Lmb),
				label: String::from("Draw Path"),
				plus: false,
			}])]),
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Add Handle"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Add Anchor"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Control])],
					key_groups_mac: None,
					mouse: None,
					label: String::from("Snap 15°"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Shift])],
					key_groups_mac: None,
					mouse: None,
					label: String::from("Break Handle"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Enter])],
					key_groups_mac: None,
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

// TODO: Expand `pos` name below to the full word (position?)
/// Snap the angle of the line from relative to pos if the key is pressed.
fn compute_snapped_angle(input: &InputPreprocessorMessageHandler, key: Key, pos: DVec2, relative: DVec2) -> DVec2 {
	if input.keyboard.get(key as usize) {
		let delta = relative - pos;

		let length = delta.length();
		let mut angle = -delta.angle_between(DVec2::X);

		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;

		let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
		relative - rotated
	} else {
		pos
	}
}

/// Pushes a [ManipulatorGroup] to the current layer via an [Operation].
fn add_manipulator_group(layer_path: &Option<Vec<LayerId>>, start: bool, manipulator_group: ManipulatorGroup) -> Message {
	if let Some(layer_path) = layer_path {
		if start {
			Operation::PushFrontManipulatorGroup {
				layer_path: layer_path.clone(),
				manipulator_group,
			}
			.into()
		} else {
			Operation::PushManipulatorGroup {
				layer_path: layer_path.clone(),
				manipulator_group,
			}
			.into()
		}
	} else {
		Message::NoOp
	}
}

/// Determains if a path should be extended. Returns the path and if it is extending from the start if applicable.
fn should_extend(document: &DocumentMessageHandler, pos: DVec2, tolerance: f64) -> Option<(&[LayerId], bool)> {
	let mut best = None;
	let mut best_distance_squared = tolerance * tolerance;
	for layer_path in document.selected_layers() {
		let mut should_extend = || {
			let viewspace = document.graphene_document.generate_transform_relative_to_viewport(layer_path).ok()?;
			let subpath = document.graphene_document.layer(layer_path).ok().and_then(|layer| layer.as_subpath())?;
			let (&_first_id, first) = subpath.manipulator_groups().enumerate().next()?;
			let (&_last_id, last) = subpath.manipulator_groups().enumerate().next_back()?;
			if !last.is_close() {
				for (group, start) in [(first, true), (last, false)] {
					if let Some(point) = &group.points[ManipulatorType::Anchor] {
						let point_pos = viewspace.transform_point2(point.position);
						log::info!("Point {point_pos} pos {pos}");
						let distance_squared = point_pos.distance_squared(pos);
						if distance_squared < best_distance_squared {
							best = Some((layer_path, start));
							best_distance_squared = distance_squared;
						}
					}
				}
			}
			None::<()>
		};
		should_extend();
	}
	best
}
