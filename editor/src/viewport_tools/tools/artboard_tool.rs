use crate::consts::SELECTION_TOLERANCE;
use crate::document::utility_types::TargetDocument;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{Fsm, SignalToMessageMap, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};

use graphene::intersection::Quad;

use super::shared::transformation_cage::*;

use glam::{DVec2, Vec2Swizzles};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ArtboardTool {
	fsm_state: ArtboardToolFsmState,
	data: ArtboardToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Artboard)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum ArtboardToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	DeleteSelected,
	PointerDown,
	PointerMove {
		constrain_axis_or_aspect: Key,
		center: Key,
	},
	PointerUp,
}

impl ToolMetadata for ArtboardTool {
	fn icon_name(&self) -> String {
		"GeneralArtboardTool".into()
	}
	fn tooltip(&self) -> String {
		"Artboard Tool".into()
	}
	fn tool_type(&self) -> crate::viewport_tools::tool::ToolType {
		ToolType::Artboard
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for ArtboardTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.data, data, &(), responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	advertise_actions!(ArtboardToolMessageDiscriminant; PointerDown, PointerUp, PointerMove, DeleteSelected, Abort);
}

impl PropertyHolder for ArtboardTool {}

impl ToolTransition for ArtboardTool {
	fn signal_to_message_map(&self) -> SignalToMessageMap {
		SignalToMessageMap {
			document_dirty: Some(ArtboardToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(ArtboardToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtboardToolFsmState {
	Ready,
	Drawing,
	ResizingBounds,
	Dragging,
}

impl Default for ArtboardToolFsmState {
	fn default() -> Self {
		ArtboardToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct ArtboardToolData {
	bounding_box_overlays: Option<BoundingBoxOverlays>,
	selected_board: Option<LayerId>,
	snap_handler: SnapHandler,
	cursor: MouseCursorIcon,
	drag_start: DVec2,
	drag_current: DVec2,
}

impl Fsm for ArtboardToolFsmState {
	type ToolData = ArtboardToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _global_tool_data, input, font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Artboard(event) = event {
			match (self, event) {
				(ArtboardToolFsmState::Ready | ArtboardToolFsmState::ResizingBounds | ArtboardToolFsmState::Dragging, ArtboardToolMessage::DocumentIsDirty) => {
					let mut buffer = Vec::new();
					match (
						tool_data.selected_board.map(|path| document.artboard_bounding_box_and_transform(&[path], font_cache)).unwrap_or(None),
						tool_data.bounding_box_overlays.take(),
					) {
						(None, Some(bounding_box_overlays)) => bounding_box_overlays.delete(&mut buffer),
						(Some((bounds, transform)), paths) => {
							let mut bounding_box_overlays = paths.unwrap_or_else(|| BoundingBoxOverlays::new(&mut buffer));

							bounding_box_overlays.bounds = bounds;
							bounding_box_overlays.transform = transform;

							bounding_box_overlays.transform(&mut buffer);

							tool_data.bounding_box_overlays = Some(bounding_box_overlays);

							responses.push_back(OverlaysMessage::Rerender.into());
							responses.push_back(
								PropertiesPanelMessage::SetActiveLayers {
									paths: vec![vec![tool_data.selected_board.unwrap()]],
									document: TargetDocument::Artboard,
								}
								.into(),
							);
						}
						_ => {}
					};
					buffer.into_iter().rev().for_each(|message| responses.push_front(message));
					self
				}
				(ArtboardToolFsmState::Ready, ArtboardToolMessage::PointerDown) => {
					tool_data.drag_start = input.mouse.position;
					tool_data.drag_current = input.mouse.position;

					let dragging_bounds = if let Some(bounding_box) = &mut tool_data.bounding_box_overlays {
						let edges = bounding_box.check_selected_edges(input.mouse.position);

						bounding_box.selected_edges = edges.map(|(top, bottom, left, right)| {
							let edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
							bounding_box.pivot = edges.calculate_pivot();
							edges
						});

						edges
					} else {
						None
					};

					if let Some(selected_edges) = dragging_bounds {
						let snap_x = selected_edges.2 || selected_edges.3;
						let snap_y = selected_edges.0 || selected_edges.1;

						tool_data
							.snap_handler
							.start_snap(document, document.bounding_boxes(None, Some(tool_data.selected_board.unwrap()), font_cache), snap_x, snap_y);
						tool_data.snap_handler.add_all_document_handles(document, &[], &[]);

						ArtboardToolFsmState::ResizingBounds
					} else {
						let tolerance = DVec2::splat(SELECTION_TOLERANCE);
						let quad = Quad::from_box([input.mouse.position - tolerance, input.mouse.position + tolerance]);
						let intersection = document.artboard_message_handler.artboards_graphene_document.intersects_quad_root(quad, font_cache);

						responses.push_back(BroadcastSignal::DocumentIsDirty.into());
						if let Some(intersection) = intersection.last() {
							tool_data.selected_board = Some(intersection[0]);

							tool_data
								.snap_handler
								.start_snap(document, document.bounding_boxes(None, Some(intersection[0]), font_cache), true, true);
							tool_data.snap_handler.add_all_document_handles(document, &[], &[]);

							responses.push_back(
								PropertiesPanelMessage::SetActiveLayers {
									paths: vec![intersection.clone()],
									document: TargetDocument::Artboard,
								}
								.into(),
							);

							ArtboardToolFsmState::Dragging
						} else {
							let id = generate_uuid();
							tool_data.selected_board = Some(id);

							tool_data.snap_handler.start_snap(document, document.bounding_boxes(None, Some(id), font_cache), true, true);
							tool_data.snap_handler.add_all_document_handles(document, &[], &[]);

							responses.push_back(
								ArtboardMessage::AddArtboard {
									id: Some(id),
									position: (0., 0.),
									size: (0., 0.),
								}
								.into(),
							);

							responses.push_back(PropertiesPanelMessage::ClearSelection.into());

							ArtboardToolFsmState::Drawing
						}
					}
				}
				(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
					if let Some(bounds) = &tool_data.bounding_box_overlays {
						if let Some(movement) = &bounds.selected_edges {
							let from_center = input.keyboard.get(center as usize);
							let constrain_square = input.keyboard.get(constrain_axis_or_aspect as usize);

							let mouse_position = input.mouse.position;
							let snapped_mouse_position = tool_data.snap_handler.snap_position(responses, document, mouse_position);

							let [position, size] = movement.new_size(snapped_mouse_position, bounds.transform, from_center, constrain_square);
							let position = movement.center_position(position, size, from_center);

							responses.push_back(
								ArtboardMessage::ResizeArtboard {
									artboard: tool_data.selected_board.unwrap(),
									position: position.round().into(),
									size: size.round().into(),
								}
								.into(),
							);

							responses.push_back(BroadcastSignal::DocumentIsDirty.into());
						}
					}
					ArtboardToolFsmState::ResizingBounds
				}
				(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, .. }) => {
					if let Some(bounds) = &tool_data.bounding_box_overlays {
						let axis_align = input.keyboard.get(constrain_axis_or_aspect as usize);

						let mouse_position = axis_align_drag(axis_align, input.mouse.position, tool_data.drag_start);
						let mouse_delta = mouse_position - tool_data.drag_current;

						let snap = bounds.evaluate_transform_handle_positions().into_iter().collect();
						let closest_move = tool_data.snap_handler.snap_layers(responses, document, snap, mouse_delta);

						let size = bounds.bounds[1] - bounds.bounds[0];

						let position = bounds.bounds[0] + bounds.transform.inverse().transform_vector2(mouse_position - tool_data.drag_current + closest_move);

						responses.push_back(
							ArtboardMessage::ResizeArtboard {
								artboard: tool_data.selected_board.unwrap(),
								position: position.round().into(),
								size: size.round().into(),
							}
							.into(),
						);

						responses.push_back(BroadcastSignal::DocumentIsDirty.into());

						tool_data.drag_current = mouse_position + closest_move;
					}
					ArtboardToolFsmState::Dragging
				}
				(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
					let mouse_position = input.mouse.position;
					let snapped_mouse_position = tool_data.snap_handler.snap_position(responses, document, mouse_position);

					let root_transform = document.graphene_document.root.transform.inverse();

					let mut start = tool_data.drag_start;
					let mut size = snapped_mouse_position - start;
					// Constrain axis
					if input.keyboard.get(constrain_axis_or_aspect as usize) {
						size = size.abs().max(size.abs().yx()) * size.signum();
					}
					// From center
					if input.keyboard.get(center as usize) {
						start -= size;
						size *= 2.;
					}

					let start = root_transform.transform_point2(start);
					let size = root_transform.transform_vector2(size);

					responses.push_back(
						ArtboardMessage::ResizeArtboard {
							artboard: tool_data.selected_board.unwrap(),
							position: start.round().into(),
							size: size.round().into(),
						}
						.into(),
					);

					// Have to put message here instead of when Artboard is created
					// This might result in a few more calls but it is not reliant on the order of messages
					responses.push_back(
						PropertiesPanelMessage::SetActiveLayers {
							paths: vec![vec![tool_data.selected_board.unwrap()]],
							document: TargetDocument::Artboard,
						}
						.into(),
					);

					responses.push_back(BroadcastSignal::DocumentIsDirty.into());

					ArtboardToolFsmState::Drawing
				}
				(ArtboardToolFsmState::Ready, ArtboardToolMessage::PointerMove { .. }) => {
					let cursor = tool_data.bounding_box_overlays.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, false));

					if tool_data.cursor != cursor {
						tool_data.cursor = cursor;
						responses.push_back(FrontendMessage::UpdateMouseCursor { cursor }.into());
					}

					ArtboardToolFsmState::Ready
				}
				(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerUp) => {
					tool_data.snap_handler.cleanup(responses);

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					ArtboardToolFsmState::Ready
				}
				(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerUp) => {
					tool_data.snap_handler.cleanup(responses);

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					responses.push_back(BroadcastSignal::DocumentIsDirty.into());

					ArtboardToolFsmState::Ready
				}
				(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerUp) => {
					tool_data.snap_handler.cleanup(responses);

					if let Some(bounds) = &mut tool_data.bounding_box_overlays {
						bounds.original_transforms.clear();
					}

					ArtboardToolFsmState::Ready
				}
				(_, ArtboardToolMessage::DeleteSelected) => {
					if let Some(artboard) = tool_data.selected_board.take() {
						responses.push_back(ArtboardMessage::DeleteArtboard { artboard }.into());
						responses.push_back(BroadcastSignal::DocumentIsDirty.into());
					}
					ArtboardToolFsmState::Ready
				}
				(_, ArtboardToolMessage::Abort) => {
					if let Some(bounding_box_overlays) = tool_data.bounding_box_overlays.take() {
						bounding_box_overlays.delete(responses);
					}

					// Register properties when switching back to other tools
					responses.push_back(
						PropertiesPanelMessage::SetActiveLayers {
							paths: document.selected_layers().map(|path| path.to_vec()).collect(),
							document: TargetDocument::Artwork,
						}
						.into(),
					);

					tool_data.snap_handler.cleanup(responses);
					ArtboardToolFsmState::Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			ArtboardToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Draw Artboard"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Move Artboard"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyBackspace])],
					mouse: None,
					label: String::from("Delete Artboard"),
					plus: false,
				}]),
			]),
			ArtboardToolFsmState::Dragging => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyShift])],
				mouse: None,
				label: String::from("Constrain to Axis"),
				plus: false,
			}])]),
			ArtboardToolFsmState::Drawing | ArtboardToolFsmState::ResizingBounds => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Constrain Square"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
					mouse: None,
					label: String::from("From Center"),
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
