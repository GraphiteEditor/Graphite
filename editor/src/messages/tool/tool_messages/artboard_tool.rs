use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::graph_modification_utils::is_layer_fed_by_node_of_name;
use crate::messages::tool::common_functionality::snapping::SnapCandidatePoint;
use crate::messages::tool::common_functionality::snapping::SnapData;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::common_functionality::transformation_cage::*;

use glam::{IVec2, Vec2Swizzles};
use graph_craft::document::NodeId;

#[derive(Default)]
pub struct ArtboardTool {
	fsm_state: ArtboardToolFsmState,
	data: ArtboardToolData,
}

#[impl_message(Message, ToolMessage, Artboard)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ArtboardToolMessage {
	// Standard messages
	Abort,
	Overlays(OverlayContext),

	// Tool-specific messages
	UpdateSelectedArtboard,
	DeleteSelected,
	NudgeSelected { delta_x: f64, delta_y: f64 },
	PointerDown,
	PointerMove { constrain_axis_or_aspect: Key, center: Key },
	PointerOutsideViewport { constrain_axis_or_aspect: Key, center: Key },
	PointerUp,
}

impl ToolMetadata for ArtboardTool {
	fn icon_name(&self) -> String {
		"GeneralArtboardTool".into()
	}
	fn tooltip(&self) -> String {
		"Artboard Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Artboard
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for ArtboardTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.data, tool_data, &(), responses, false);
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(ArtboardToolMessageDiscriminant;
			DeleteSelected,
			NudgeSelected,
			PointerMove,
		);

		let additional = match self.fsm_state {
			ArtboardToolFsmState::Ready => actions!(ArtboardToolMessageDiscriminant; PointerDown),
			_ => actions!(ArtboardToolMessageDiscriminant; PointerUp, Abort),
		};
		common.extend(additional);

		common
	}
}

impl LayoutHolder for ArtboardTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

impl ToolTransition for ArtboardTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(ArtboardToolMessage::Abort.into()),
			overlay_provider: Some(|overlay_context| ArtboardToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ArtboardToolFsmState {
	#[default]
	Ready,
	Drawing,
	ResizingBounds,
	Dragging,
}

#[derive(Clone, Debug, Default)]
struct ArtboardToolData {
	bounding_box_manager: Option<BoundingBoxManager>,
	selected_artboard: Option<LayerNodeIdentifier>,
	snap_manager: SnapManager,
	cursor: MouseCursorIcon,
	drag_start: DVec2,
	drag_current: DVec2,
	auto_panning: AutoPanning,
}

impl ArtboardToolData {
	fn check_dragging_bounds(&mut self, cursor: DVec2) -> Option<(bool, bool, bool, bool)> {
		let bounding_box = self.bounding_box_manager.as_mut()?;
		let edges = bounding_box.check_selected_edges(cursor)?;
		let (top, bottom, left, right) = edges;
		let selected_edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
		bounding_box.opposite_pivot = selected_edges.calculate_pivot();
		bounding_box.selected_edges = Some(selected_edges);

		Some(edges)
	}

	fn start_resizing(&mut self, _selected_edges: (bool, bool, bool, bool), _document: &DocumentMessageHandler, _input: &InputPreprocessorMessageHandler) {
		if let Some(bounds) = &mut self.bounding_box_manager {
			bounds.center_of_transformation = bounds.transform.transform_point2((bounds.bounds[0] + bounds.bounds[1]) / 2.);
		}
	}

	fn select_artboard(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> bool {
		responses.add(DocumentMessage::StartTransaction);

		let mut intersections = document
			.click_xray(input.mouse.position)
			.filter(|&layer| document.network.nodes.get(&layer.to_node()).map_or(false, |document_node| document_node.is_artboard()));

		if let Some(intersection) = intersections.next() {
			self.selected_artboard = Some(intersection);

			if let Some(bounds) = document.metadata().bounding_box_document(intersection) {
				let bounding_box_manager = self.bounding_box_manager.get_or_insert(BoundingBoxManager::default());
				bounding_box_manager.bounds = bounds;
				bounding_box_manager.transform = document.metadata().document_to_viewport;
			}

			responses.add_front(NodeGraphMessage::SelectedNodesSet { nodes: vec![intersection.to_node()] });

			true
		} else {
			self.selected_artboard = None;

			responses.add(PropertiesPanelMessage::Clear);

			false
		}
	}

	fn resize_artboard(&mut self, responses: &mut VecDeque<Message>, _document: &DocumentMessageHandler, mouse_position: DVec2, from_center: bool, constrain_square: bool) {
		let Some(bounds) = &self.bounding_box_manager else {
			return;
		};
		let Some(movement) = &bounds.selected_edges else {
			return;
		};
		if self.selected_artboard.unwrap() == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Selected artboard cannot be ROOT_PARENT");
			return;
		}

		let center = from_center.then_some(bounds.center_of_transformation);
		let (min, size) = movement.new_size(mouse_position, bounds.transform, center, constrain_square, None);
		let max = min + size;
		let position = min.min(max);
		let size = (max - min).abs();

		responses.add(GraphOperationMessage::ResizeArtboard {
			id: self.selected_artboard.unwrap().to_node(),
			location: position.round().as_ivec2(),
			dimensions: size.round().as_ivec2(),
		});
	}
}

impl Fsm for ArtboardToolFsmState {
	type ToolData = ArtboardToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData { document, input, .. } = tool_action_data;

		let ToolMessage::Artboard(event) = event else {
			return self;
		};

		match (self, event) {
			(state, ArtboardToolMessage::Overlays(mut overlay_context)) if state != ArtboardToolFsmState::Drawing => {
				if let Some(bounds) = tool_data.selected_artboard.and_then(|layer| document.metadata().bounding_box_document(layer)) {
					let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());
					bounding_box_manager.bounds = bounds;
					bounding_box_manager.transform = document.metadata().document_to_viewport;

					bounding_box_manager.render_overlays(&mut overlay_context);
				} else {
					tool_data.bounding_box_manager.take();
				}

				self
			}

			(ArtboardToolFsmState::Ready, ArtboardToolMessage::PointerDown) => {
				tool_data.drag_start = input.mouse.position;
				tool_data.drag_current = input.mouse.position;

				if let Some(selected_edges) = tool_data.check_dragging_bounds(input.mouse.position) {
					responses.add(DocumentMessage::StartTransaction);
					tool_data.start_resizing(selected_edges, document, input);

					ArtboardToolFsmState::ResizingBounds
				} else if tool_data.select_artboard(document, input, responses) {
					ArtboardToolFsmState::Dragging
				} else {
					ArtboardToolFsmState::Drawing
				}
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let from_center = input.keyboard.get(center as usize);
				let constrain_square = input.keyboard.get(constrain_axis_or_aspect as usize);
				let mouse_position = input.mouse.position;
				let to_viewport = document.metadata().document_to_viewport;

				let present_artboard = match tool_data.selected_artboard {
					Some(value) => value,
					None => LayerNodeIdentifier::default(),
				};
				let ignore_slice: &[LayerNodeIdentifier] = &[present_artboard];
				let snap_data = SnapData::ignore(document, input, ignore_slice);
				let document_mouse = to_viewport.inverse().transform_point2(input.mouse.position);
				let snapped = tool_data.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), None, false);
				let snapped_mouse_position = to_viewport.transform_point2(snapped.snapped_point_document);
				tool_data.snap_manager.update_indicator(snapped);

				tool_data.resize_artboard(responses, document, snapped_mouse_position, from_center, constrain_square);

				// AutoPanning
				let messages = [
					ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
					ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				ArtboardToolFsmState::ResizingBounds
			}

			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				if let Some(ref mut bounds) = &mut tool_data.bounding_box_manager {
					let axis_align = input.keyboard.get(constrain_axis_or_aspect as usize);

					let mouse_position = axis_align_drag(axis_align, input.mouse.position, tool_data.drag_start);

					let size = bounds.bounds[1] - bounds.bounds[0];
					let position = bounds.bounds[0] + bounds.transform.inverse().transform_vector2(mouse_position - tool_data.drag_current);

					if tool_data.selected_artboard.unwrap() == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("Selected artboard cannot be ROOT_PARENT");
						return ArtboardToolFsmState::Ready;
					}
					responses.add(GraphOperationMessage::ResizeArtboard {
						id: tool_data.selected_artboard.unwrap().to_node(),
						location: position.round().as_ivec2(),
						dimensions: size.round().as_ivec2(),
					});

					// The second term is added to prevent the slow change in position due to rounding errors.
					tool_data.drag_current = mouse_position + bounds.transform.transform_vector2(position.round() - position);

					// Update bounds if another `PointerMove` message comes before `ResizeArtboard` is finished.
					bounds.bounds[0] = position.round();
					bounds.bounds[1] = position.round() + size.round();

					// AutoPanning
					let messages = [
						ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
						ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
					];
					tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);
				}
				ArtboardToolFsmState::Dragging
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let mouse_position = input.mouse.position;
				let to_viewport = document.metadata().document_to_viewport;
				let present_artboard = match tool_data.selected_artboard {
					Some(value) => value,
					None => LayerNodeIdentifier::default(),
				};

				let ignore_slice: &[LayerNodeIdentifier] = &[present_artboard];
				let snap_data = SnapData::ignore(document, input, ignore_slice);
				let document_mouse = to_viewport.inverse().transform_point2(input.mouse.position);
				let snapped = tool_data.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), None, false);
				let snapped_mouse_position = to_viewport.transform_point2(snapped.snapped_point_document);
				tool_data.snap_manager.update_indicator(snapped);
				let mut start = tool_data.drag_start;
				let mut size = snapped_mouse_position - start;
				let root_transform = document.metadata().document_to_viewport.inverse();

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
				let end = start + size;
				let size = (start - end).abs();
				let start = start.min(end);

				if let Some(artboard) = tool_data.selected_artboard {
					if artboard == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("Selected artboard cannot be ROOT_PARENT");
					} else {
						responses.add(GraphOperationMessage::ResizeArtboard {
							id: artboard.to_node(),
							location: start.round().as_ivec2(),
							dimensions: size.round().as_ivec2(),
						});
					}
				} else {
					let id = NodeId(generate_uuid());

					tool_data.selected_artboard = Some(LayerNodeIdentifier::new_unchecked(id));

					responses.add(GraphOperationMessage::NewArtboard {
						id,
						artboard: graphene_core::Artboard {
							graphic_group: graphene_core::GraphicGroup::EMPTY,
							location: start.round().as_ivec2(),
							dimensions: IVec2::splat(1),
							background: graphene_core::Color::WHITE,
							clip: false,
							alias: None,
						},
					})
				}

				// AutoPanning
				let messages = [
					ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
					ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				ArtboardToolFsmState::Drawing
			}

			(ArtboardToolFsmState::Ready, ArtboardToolMessage::PointerMove { .. }) => {
				let cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, false));

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				ArtboardToolFsmState::Ready
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// AutoPanning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				ArtboardToolFsmState::ResizingBounds
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// AutoPanning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_current += shift;
					tool_data.drag_start += shift;
				}

				ArtboardToolFsmState::Dragging
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// AutoPanning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_start += shift;
				}

				ArtboardToolFsmState::Drawing
			}
			(state, ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }) => {
				// AutoPanning
				let messages = [
					ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
					ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				ArtboardToolFsmState::Ready
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}
				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready
			}
			(_, ArtboardToolMessage::UpdateSelectedArtboard) => {
				tool_data.selected_artboard = document.selected_nodes.selected_layers(document.metadata()).find(|layer| document.metadata().is_artboard(*layer));
				self
			}
			(_, ArtboardToolMessage::DeleteSelected) => {
				tool_data.selected_artboard.take();
				responses.add(NodeGraphMessage::DeleteSelectedNodes { reconnect: true });
				ArtboardToolFsmState::Ready
			}
			(_, ArtboardToolMessage::NudgeSelected { delta_x, delta_y }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					if tool_data.selected_artboard.unwrap() == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("Selected artboard cannot be ROOT_PARENT");
					} else {
						responses.add(GraphOperationMessage::ResizeArtboard {
							id: tool_data.selected_artboard.unwrap().to_node(),
							location: DVec2::new(bounds.bounds[0].x + delta_x, bounds.bounds[0].y + delta_y).round().as_ivec2(),
							dimensions: (bounds.bounds[1] - bounds.bounds[0]).round().as_ivec2(),
						});
					}
				}

				ArtboardToolFsmState::Ready
			}
			(ArtboardToolFsmState::Dragging | ArtboardToolFsmState::Drawing | ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				// ArtboardTool currently doesn't implement snapping
				// tool_data.snap_manager.cleanup(responses);

				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			ArtboardToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Artboard")]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Move Artboard")]),
				HintGroup(vec![HintInfo::keys([Key::Backspace], "Delete Artboard")]),
			]),
			ArtboardToolFsmState::Dragging => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain to Axis")]),
			]),
			ArtboardToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
			ArtboardToolFsmState::ResizingBounds => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Preserve Aspect Ratio"), HintInfo::keys([Key::Alt], "From Center")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
