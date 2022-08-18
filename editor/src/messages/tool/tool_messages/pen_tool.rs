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
use graphene::layers::vector::subpath::Subpath;
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
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					// Create a new layer and prep snap system
					tool_data.path = Some(document.get_path_for_new_layer());
					tool_data.snap_manager.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
					tool_data.snap_manager.add_all_document_handles(document, &[], &[], &[]);
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
							ManipulatorGroup::new_with_handles(start_position, Some(start_position), Some(start_position)),
						));
					}

					PenToolFsmState::DraggingHandle
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::DragStart) => PenToolFsmState::DraggingHandle,
				(PenToolFsmState::DraggingHandle, PenToolMessage::DragStop) => {
					// Add new point onto path
					if let Some(layer_path) = &tool_data.path {
						if let Some(((&last_id, last_manipulator_group), previous)) = get_subpath(layer_path, document).and_then(last_2_manipulator_groups) {
							let last_anchor = &last_manipulator_group.points[ManipulatorType::Anchor];
							let last_in = &last_manipulator_group.points[ManipulatorType::InHandle];
							let first_manipulator = get_subpath(layer_path, document).and_then(|path| path.manipulator_groups().enumerate().next());
							let first_anchor = first_manipulator.and_then(|(_, group)| group.points[ManipulatorType::Anchor].as_ref());
							let first_id = first_manipulator.map(|(&id, _)| id);

							if let (Some(last_anchor), Some(last_in), Some(first_anchor), Some(first_id)) = (last_anchor, last_in, first_anchor, first_id) {
								let transformed_distance_between = transform.transform_point2(last_anchor.position).distance_squared(transform.transform_point2(first_anchor.position));

								if transformed_distance_between < crate::consts::SNAP_POINT_TOLERANCE.powi(2) && previous.is_some() {
									// Move the in handle of the first point to where the user has placed it
									let op = Operation::MoveManipulatorPoint {
										layer_path: layer_path.clone(),
										id: first_id,
										manipulator_type: ManipulatorType::InHandle,
										position: last_in.position.into(),
									};
									responses.push_back(op.into());

									// Stop the handles on the first point from mirroring
									let op = Operation::SetManipulatorHandleMirroring {
										layer_path: layer_path.clone(),
										id: first_id,
										distance: false,
										angle: false,
									};
									responses.push_back(op.into());

									// Remove the node that has just been placed
									let op = Operation::RemoveManipulatorGroup {
										layer_path: layer_path.clone(),
										id: last_id,
									};
									responses.push_back(op.into());

									// Push a close path node
									let manipulator_group = ManipulatorGroup::closed();
									let op = Operation::PushManipulatorGroup {
										layer_path: layer_path.clone(),
										manipulator_group,
									};
									responses.push_back(op.into());

									responses.push_back(DocumentMessage::CommitTransaction.into());

									// Clean up overlays
									for layer_path in document.all_layers() {
										tool_data.overlay_renderer.clear_subpath_overlays(&document.graphene_document, layer_path.to_vec(), responses);
									}
									tool_data.path = None;
									tool_data.snap_manager.cleanup(responses);

									return PenToolFsmState::Ready;
								}
							}
							if let Some(out_handle) = &last_manipulator_group.points[ManipulatorType::OutHandle] {
								responses.push_back(add_manipulator_group(&tool_data.path, ManipulatorGroup::new_with_anchor(out_handle.position)));
							}
						}
					}

					PenToolFsmState::PlacingAnchor
				}
				(PenToolFsmState::DraggingHandle, PenToolMessage::PointerMove { snap_angle, break_handle }) => {
					if let Some(layer_path) = &tool_data.path {
						let mouse = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
						let mut pos = transform.inverse().transform_point2(mouse);
						if let Some(((&id, manipulator_group), _previous)) = get_subpath(layer_path, document).and_then(last_2_manipulator_groups) {
							if let Some(anchor) = manipulator_group.points[ManipulatorType::Anchor].as_ref() {
								pos = compute_snapped_angle(input, snap_angle, pos, anchor.position);
							}

							// Update points on current segment (to show preview of new handle)
							let msg = Operation::MoveManipulatorPoint {
								layer_path: layer_path.clone(),
								id,
								manipulator_type: ManipulatorType::OutHandle,
								position: pos.into(),
							};
							responses.push_back(msg.into());

							// Mirror handle of last segment
							if !input.keyboard.get(break_handle as usize) && get_subpath(layer_path, document).map(|shape| shape.manipulator_groups().len() > 1).unwrap_or_default() {
								if let Some(anchor) = manipulator_group.points[ManipulatorType::Anchor].as_ref() {
									pos = anchor.position - (pos - anchor.position);
								}
								let msg = Operation::MoveManipulatorPoint {
									layer_path: layer_path.clone(),
									id,
									manipulator_type: ManipulatorType::InHandle,
									position: pos.into(),
								};
								responses.push_back(msg.into());
							}
						}
					}

					self
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerMove { snap_angle, .. }) => {
					if let Some(layer_path) = &tool_data.path {
						let mouse = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
						let mut pos = transform.inverse().transform_point2(mouse);

						// Snap to the first point (to show close path)
						if let Some(first) = get_subpath(layer_path, document).and_then(|path| path.first_point(ManipulatorType::Anchor)) {
							if mouse.distance_squared(transform.transform_point2(first.position)) < crate::consts::SNAP_POINT_TOLERANCE.powi(2) {
								pos = first.position;
							}
						}

						if let Some(((&id, _), previous)) = get_subpath(layer_path, document).and_then(last_2_manipulator_groups) {
							if let Some(relative) = previous.as_ref().and_then(|(_, manipulator_group)| manipulator_group.points[ManipulatorType::Anchor].as_ref()) {
								pos = compute_snapped_angle(input, snap_angle, pos, relative.position);
							}

							for manipulator_type in [ManipulatorType::Anchor, ManipulatorType::InHandle, ManipulatorType::OutHandle] {
								let msg = Operation::MoveManipulatorPoint {
									layer_path: layer_path.clone(),
									id,
									manipulator_type,
									position: pos.into(),
								};
								responses.push_back(msg.into());
							}
						}
					}

					self
				}
				(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Abort | PenToolMessage::Confirm) => {
					// Abort or commit the transaction to the undo history
					if let Some(layer_path) = tool_data.path.as_ref() {
						if let Some(subpath) = (get_subpath(layer_path, document)).filter(|subpath| subpath.manipulator_groups().len() > 1) {
							if let Some(((&(mut id), mut manipulator_group), previous)) = last_2_manipulator_groups(subpath) {
								// Remove the unplaced anchor if in anchor placing mode
								if self == PenToolFsmState::PlacingAnchor {
									let layer_path = layer_path.clone();
									let op = Operation::RemoveManipulatorGroup { layer_path, id };
									responses.push_back(op.into());
									if let Some((&new_id, new_manipulator_group)) = previous {
										id = new_id;
										manipulator_group = new_manipulator_group;
									}
								}

								// Remove the out handle if in dragging handle mode
								let op = Operation::MoveManipulatorPoint {
									layer_path: layer_path.clone(),
									id,
									manipulator_type: ManipulatorType::OutHandle,
									position: manipulator_group.points[ManipulatorType::Anchor].as_ref().unwrap().position.into(),
								};
								responses.push_back(op.into());
							}
						}

						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
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
fn add_manipulator_group(layer_path: &Option<Vec<LayerId>>, manipulator_group: ManipulatorGroup) -> Message {
	if let Some(layer_path) = layer_path {
		Operation::PushManipulatorGroup {
			layer_path: layer_path.clone(),
			manipulator_group,
		}
		.into()
	} else {
		Message::NoOp
	}
}

/// Gets the currently editing [Subpath].
fn get_subpath<'a>(layer_path: &'a [LayerId], document: &'a DocumentMessageHandler) -> Option<&'a Subpath> {
	document.graphene_document.layer(layer_path).ok().and_then(|layer| layer.as_subpath())
}

type ManipulatorGroupRef<'a> = (&'a u64, &'a ManipulatorGroup);

/// Gets the last 2 [ManipulatorGroup]s on the currently editing layer along with its ID.
fn last_2_manipulator_groups(subpath: &Subpath) -> Option<(ManipulatorGroupRef, Option<ManipulatorGroupRef>)> {
	subpath.manipulator_groups().enumerate().last().map(|last| {
		(
			last,
			(subpath.manipulator_groups().len() > 1)
				.then(|| subpath.manipulator_groups().enumerate().nth(subpath.manipulator_groups().len() - 2))
				.flatten(),
		)
	})
}
