use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{Layout, LayoutGroup, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{Fsm, SignalToMessageMap, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::viewport_tools::vector_editor::overlay_renderer::OverlayRenderer;

use graphene::layers::style;
use graphene::layers::vector::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_shape::VectorShape};
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
		"Pen Tool (P)".into()
	}
	fn tool_type(&self) -> crate::viewport_tools::tool::ToolType {
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
	fn process_action(&mut self, action: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Pen(PenToolMessage::UpdateOptions(action)) = action {
			match action {
				PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.tool_data, tool_data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			PenToolFsmState::Ready => actions!(PenToolMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => actions!(PenToolMessageDiscriminant; DragStart, DragStop, PointerMove, Confirm, Abort),
		}
	}
}

impl ToolTransition for PenTool {
	fn signal_to_message_map(&self) -> SignalToMessageMap {
		SignalToMessageMap {
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
	snap_handler: SnapHandler,
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
						tool_data.overlay_renderer.render_vector_shape_overlays(&document.graphene_document, layer_path.to_vec(), responses);
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
					tool_data.snap_handler.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
					tool_data.snap_handler.add_all_document_handles(document, &[], &[], &[]);
					let snapped_position = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);

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
								vector_path: Default::default(),
								style: style::PathStyle::new(Some(style::Stroke::new(global_tool_data.primary_color, tool_data.weight)), style::Fill::None),
							}
							.into(),
						);
						responses.push_back(add_anchor(&tool_data.path, VectorAnchor::new(start_position)));
					}

					PenToolFsmState::DraggingHandle
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::DragStart) => PenToolFsmState::DraggingHandle,
				(PenToolFsmState::DraggingHandle, PenToolMessage::DragStop) => {
					// Add new point onto path
					if let Some(layer_path) = &tool_data.path {
						if let Some(vector_anchor) = get_vector_shape(layer_path, document).and_then(|shape| shape.anchors().last()) {
							if let Some(anchor) = &vector_anchor.points[ControlPointType::OutHandle] {
								responses.push_back(add_anchor(&tool_data.path, VectorAnchor::new(anchor.position)));
							}
						}
					}

					PenToolFsmState::PlacingAnchor
				}
				(PenToolFsmState::DraggingHandle, PenToolMessage::PointerMove { snap_angle, break_handle }) => {
					if let Some(layer_path) = &tool_data.path {
						let mouse = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);
						let mut pos = transform.inverse().transform_point2(mouse);
						if let Some(((&id, anchor), _previous)) = get_vector_shape(layer_path, document).and_then(last_2_anchors) {
							if let Some(anchor) = anchor.points[ControlPointType::Anchor as usize].as_ref() {
								pos = compute_snapped_angle(input, snap_angle, pos, anchor.position);
							}

							// Update points on current segment (to show preview of new handle)
							let msg = Operation::MoveVectorPoint {
								layer_path: layer_path.clone(),
								id,
								control_type: ControlPointType::OutHandle,
								position: pos.into(),
							};
							responses.push_back(msg.into());

							// Mirror handle of last segement
							if !input.keyboard.get(break_handle as usize) && get_vector_shape(layer_path, document).map(|shape| shape.anchors().len() > 1).unwrap_or_default() {
								if let Some(anchor) = anchor.points[ControlPointType::Anchor as usize].as_ref() {
									pos = anchor.position - (pos - anchor.position);
								}
								let msg = Operation::MoveVectorPoint {
									layer_path: layer_path.clone(),
									id,
									control_type: ControlPointType::InHandle,
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
						let mouse = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);
						let mut pos = transform.inverse().transform_point2(mouse);

						if let Some(((&id, _anchor), previous)) = get_vector_shape(layer_path, document).and_then(last_2_anchors) {
							if let Some(relative) = previous.as_ref().and_then(|(_, anchor)| anchor.points[ControlPointType::Anchor as usize].as_ref()) {
								pos = compute_snapped_angle(input, snap_angle, pos, relative.position);
							}

							for control_type in [ControlPointType::Anchor, ControlPointType::InHandle, ControlPointType::OutHandle] {
								let msg = Operation::MoveVectorPoint {
									layer_path: layer_path.clone(),
									id,
									control_type,
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
						if let Some(vector_shape) = (get_vector_shape(layer_path, document)).filter(|vector_shape| vector_shape.anchors().len() > 1) {
							if let Some(((&(mut id), mut anchor), previous)) = last_2_anchors(vector_shape) {
								// Remove the unplaced anchor if in anchor placing mode
								if self == PenToolFsmState::PlacingAnchor {
									let layer_path = layer_path.clone();
									let op = Operation::RemoveVectorAnchor { layer_path, id };
									responses.push_back(op.into());
									if let Some((&new_id, new_anchor)) = previous {
										id = new_id;
										anchor = new_anchor;
									}
								}

								// Remove the out handle if in dragging handle mode
								let op = Operation::MoveVectorPoint {
									layer_path: layer_path.clone(),
									id,
									control_type: ControlPointType::OutHandle,
									position: anchor.points[ControlPointType::Anchor as usize].as_ref().unwrap().position.into(),
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
						tool_data.overlay_renderer.clear_vector_shape_overlays(&document.graphene_document, layer_path.to_vec(), responses);
					}
					tool_data.path = None;
					tool_data.snap_handler.cleanup(responses);

					PenToolFsmState::Ready
				}
				(_, PenToolMessage::Abort) => {
					// Clean up overlays
					for layer_path in document.all_layers() {
						tool_data.overlay_renderer.clear_vector_shape_overlays(&document.graphene_document, layer_path.to_vec(), responses);
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
				mouse: Some(MouseMotion::Lmb),
				label: String::from("Draw Path"),
				plus: false,
			}])]),
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Add Handle"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Add Control Point"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyControl])],
					mouse: None,
					label: String::from("Snap 15°"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Break handle"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyEnter])],
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

/// Snap the angle of the line from relative to pos if the key is pressed
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

/// Pushes an anchor to the current layer via an [Operation]
fn add_anchor(layer_path: &Option<Vec<LayerId>>, anchor: VectorAnchor) -> Message {
	if let Some(layer_path) = layer_path {
		Operation::PushVectorAnchor {
			layer_path: layer_path.clone(),
			anchor,
		}
		.into()
	} else {
		Message::NoOp
	}
}

/// Gets the currently editing [VectorShape]
fn get_vector_shape<'a>(layer_path: &'a [LayerId], document: &'a DocumentMessageHandler) -> Option<&'a VectorShape> {
	document.graphene_document.layer(layer_path).ok().and_then(|layer| layer.as_vector_shape())
}

type AnchorRef<'a> = (&'a u64, &'a VectorAnchor);

/// Gets the last 2 [VectorAnchor] on the currently editing layer along with its id
fn last_2_anchors(vector_shape: &VectorShape) -> Option<(AnchorRef, Option<AnchorRef>)> {
	vector_shape.anchors().enumerate().last().map(|last| {
		(
			last,
			(vector_shape.anchors().len() > 1)
				.then(|| vector_shape.anchors().enumerate().nth(vector_shape.anchors().len() - 2))
				.flatten(),
		)
	})
}
