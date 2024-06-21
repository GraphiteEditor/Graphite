use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::snapping;
use crate::messages::tool::common_functionality::snapping::SnapCandidatePoint;
use crate::messages::tool::common_functionality::snapping::SnapData;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::common_functionality::transformation_cage::*;
use glam::{IVec2, Vec2Swizzles};
use graph_craft::document::NodeId;
use graphene_core::renderer::Quad;

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
			ArtboardToolFsmState::Ready { .. } => actions!(ArtboardToolMessageDiscriminant; PointerDown),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtboardToolFsmState {
	Ready { hovered: bool },
	Drawing,
	ResizingBounds,
	Dragging,
}

impl Default for ArtboardToolFsmState {
	fn default() -> Self {
		Self::Ready { hovered: false }
	}
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
	snap_candidates: Vec<SnapCandidatePoint>,
}

impl ArtboardToolData {
	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, _input: &InputPreprocessorMessageHandler) {
		self.snap_candidates.clear();
		let Some(layer) = self.selected_artboard else { return };
		// for layer in layer.children(document.metadata()) {
		// 	snapping::get_layer_snap_points(layer, &SnapData::new(document, input), &mut self.snap_candidates);
		// }

		if let Some(bounds) = document.metadata.bounding_box_with_transform(layer, document.metadata.transform_to_document(layer)) {
			let quad = Quad::from_box(bounds);
			snapping::get_bbox_points(quad, &mut self.snap_candidates, snapping::BBoxSnapValues::ARTBOARD, document);
		}
	}

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

	fn hovered_artboard(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) -> Option<LayerNodeIdentifier> {
		document
			.click_xray(input.mouse.position)
			.filter(|&layer| document.network.nodes.get(&layer.to_node()).map_or(false, |document_node| document_node.is_artboard()))
			.next()
	}

	fn select_artboard(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> bool {
		responses.add(DocumentMessage::StartTransaction);

		if let Some(intersection) = Self::hovered_artboard(document, input) {
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

	fn resize_artboard(&mut self, responses: &mut VecDeque<Message>, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, from_center: bool, constrain_square: bool) {
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
		let ignore = self.selected_artboard.map_or(Vec::new(), |layer| vec![layer]);
		let snap = Some(SizeSnapData {
			manager: &mut self.snap_manager,
			points: &mut self.snap_candidates,
			snap_data: SnapData::ignore(document, input, &ignore),
		});
		let (min, size) = movement.new_size(input.mouse.position, bounds.transform, center, constrain_square, snap);
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

		let hovered = ArtboardToolData::hovered_artboard(document, input).is_some();

		match (self, event) {
			(state, ArtboardToolMessage::Overlays(mut overlay_context)) => {
				if state != ArtboardToolFsmState::Drawing {
					if let Some(bounds) = tool_data.selected_artboard.and_then(|layer| document.metadata().bounding_box_document(layer)) {
						let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());
						bounding_box_manager.bounds = bounds;
						bounding_box_manager.transform = document.metadata().document_to_viewport;

						bounding_box_manager.render_overlays(&mut overlay_context);
					} else {
						tool_data.bounding_box_manager.take();
					}
				}
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);
				info!("Draw overlays");

				self
			}

			(ArtboardToolFsmState::Ready { .. }, ArtboardToolMessage::PointerDown) => {
				let to_viewport = document.metadata().document_to_viewport;
				let to_document = to_viewport.inverse();
				tool_data.drag_start = to_document.transform_point2(input.mouse.position);
				tool_data.drag_current = to_document.transform_point2(input.mouse.position);

				if let Some(selected_edges) = tool_data.check_dragging_bounds(input.mouse.position) {
					responses.add(DocumentMessage::StartTransaction);
					tool_data.start_resizing(selected_edges, document, input);
					tool_data.get_snap_candidates(document, input);
					ArtboardToolFsmState::ResizingBounds
				} else if tool_data.select_artboard(document, input, responses) {
					tool_data.get_snap_candidates(document, input);
					ArtboardToolFsmState::Dragging
				} else {
					tool_data.get_snap_candidates(document, input);
					let point = SnapCandidatePoint::handle(to_document.transform_point2(input.mouse.position));
					let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, None, false);
					tool_data.drag_start = snapped.snapped_point_document;
					tool_data.drag_current = snapped.snapped_point_document;

					ArtboardToolFsmState::Drawing
				}
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let from_center = input.keyboard.get(center as usize);
				let constrain_square = input.keyboard.get(constrain_axis_or_aspect as usize);
				tool_data.resize_artboard(responses, document, input, from_center, constrain_square);

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

					let ignore = tool_data.selected_artboard.map_or(Vec::new(), |layer| vec![layer]);
					let snap_data = SnapData::ignore(document, input, &ignore);
					let document_to_viewport = document.metadata().document_to_viewport;
					let [start, current] = [tool_data.drag_start, tool_data.drag_current].map(|point| document_to_viewport.transform_point2(point));
					let mouse_delta = snap_drag(start, current, axis_align, snap_data, &mut tool_data.snap_manager, &mut tool_data.snap_candidates);

					let size = bounds.bounds[1] - bounds.bounds[0];
					let position = bounds.bounds[0] + bounds.transform.inverse().transform_vector2(mouse_delta);

					if tool_data.selected_artboard.unwrap() == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("Selected artboard cannot be ROOT_PARENT");
						return ArtboardToolFsmState::Ready { hovered };
					}
					responses.add(GraphOperationMessage::ResizeArtboard {
						id: tool_data.selected_artboard.unwrap().to_node(),
						location: position.round().as_ivec2(),
						dimensions: size.round().as_ivec2(),
					});

					// The second term is added to prevent the slow change in position due to rounding errors.
					tool_data.drag_current += (document_to_viewport.inverse() * bounds.transform).transform_vector2(position.round() - bounds.bounds[0]);

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
				let to_viewport = document.metadata().document_to_viewport;
				let ignore = if let Some(layer) = tool_data.selected_artboard { vec![layer] } else { vec![] };
				let snap_data = SnapData::ignore(document, input, &ignore);
				let document_mouse = to_viewport.inverse().transform_point2(input.mouse.position);
				let snapped = tool_data.snap_manager.free_snap(&snap_data, &SnapCandidatePoint::handle(document_mouse), None, false);
				let snapped_mouse_position = to_viewport.transform_point2(snapped.snapped_point_document);
				tool_data.snap_manager.update_indicator(snapped);

				let mut start = to_viewport.transform_point2(tool_data.drag_start);
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

				let start = to_viewport.inverse().transform_point2(start);
				let size = to_viewport.inverse().transform_vector2(size);
				let end = start + size;

				if let Some(artboard) = tool_data.selected_artboard {
					if artboard == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("Selected artboard cannot be ROOT_PARENT");
					} else {
						responses.add(GraphOperationMessage::ResizeArtboard {
							id: artboard.to_node(),
							location: start.min(end).round().as_ivec2(),
							dimensions: (start.round() - end.round()).abs().as_ivec2(),
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

			(ArtboardToolFsmState::Ready { .. }, ArtboardToolMessage::PointerMove { .. }) => {
				let mut cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, false));

				if cursor == MouseCursorIcon::Default && !hovered {
					tool_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
					responses.add(OverlaysMessage::Draw);
					cursor = MouseCursorIcon::Crosshair;
				} else {
					tool_data.snap_manager.cleanup(responses);
				}

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				ArtboardToolFsmState::Ready { hovered }
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// AutoPanning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				ArtboardToolFsmState::ResizingBounds
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// AutoPanning
				tool_data.auto_panning.shift_viewport(input, responses);

				ArtboardToolFsmState::Dragging
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// AutoPanning
				tool_data.auto_panning.shift_viewport(input, responses);

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

				ArtboardToolFsmState::Ready { hovered }
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready { hovered }
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}
				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready { hovered }
			}
			(_, ArtboardToolMessage::UpdateSelectedArtboard) => {
				tool_data.selected_artboard = document.selected_nodes.selected_layers(document.metadata()).find(|layer| document.metadata().is_artboard(*layer));
				self
			}
			(_, ArtboardToolMessage::DeleteSelected) => {
				tool_data.selected_artboard.take();
				responses.add(NodeGraphMessage::DeleteSelectedNodes { reconnect: true });

				ArtboardToolFsmState::Ready { hovered }
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

				ArtboardToolFsmState::Ready { hovered }
			}
			(ArtboardToolFsmState::Dragging | ArtboardToolFsmState::Drawing | ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				tool_data.snap_manager.cleanup(responses);
				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready { hovered }
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			ArtboardToolFsmState::Ready { .. } => HintData(vec![
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
		if let Self::Ready { hovered: false } = self {
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
		} else {
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
		}
	}
}
