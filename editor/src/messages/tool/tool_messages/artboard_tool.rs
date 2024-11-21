use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::snapping;
use crate::messages::tool::common_functionality::snapping::SnapCandidatePoint;
use crate::messages::tool::common_functionality::snapping::SnapData;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::common_functionality::transformation_cage::*;

use graph_craft::document::NodeId;
use graphene_core::renderer::Quad;

use glam::{IVec2, Vec2Swizzles};

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
	NudgeSelected { delta_x: f64, delta_y: f64, resize: Key, resize_opposite_corner: Key },
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

		if let Some(bounds) = document.metadata().bounding_box_with_transform(layer, document.metadata().transform_to_document(layer)) {
			snapping::get_bbox_points(Quad::from_box(bounds), &mut self.snap_candidates, snapping::BBoxSnapValues::ARTBOARD, document);
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
		document.click_xray(input).find(|&layer| document.network_interface.is_artboard(&layer.to_node(), &[]))
	}

	fn select_artboard(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> bool {
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
			layer: self.selected_artboard.unwrap(),
			location: position.round().as_ivec2(),
			dimensions: size.round().as_ivec2(),
		});

		// TODO: Resize artboard children when resizing left/top edges so that they stay in the same viewport space
		// let old_top_left = bounds.bounds[0].round().as_ivec2();
		// let new_top_left = position.round().as_ivec2();
		// let top_left_delta = new_top_left - old_top_left;
		// if top_left_delta != IVec2::ZERO {
		// 	responses.add(GraphOperationMessage::TransformChange {
		// 		layer: self.selected_artboard.unwrap(),
		// 		transform: DAffine2::from_translation((-top_left_delta).into()),
		// 		transform_in: TransformIn::Local,
		// 		skip_rerender: false,
		// 	});
		// }
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

				self
			}
			(ArtboardToolFsmState::Ready { .. }, ArtboardToolMessage::PointerDown) => {
				let to_viewport = document.metadata().document_to_viewport;
				let to_document = to_viewport.inverse();
				tool_data.drag_start = to_document.transform_point2(input.mouse.position);
				tool_data.drag_current = to_document.transform_point2(input.mouse.position);

				let state = if let Some(selected_edges) = tool_data.check_dragging_bounds(input.mouse.position) {
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
				};
				responses.add(DocumentMessage::StartTransaction);
				state
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
					let mouse_delta = snap_drag(start, current, axis_align, snap_data, &mut tool_data.snap_manager, &tool_data.snap_candidates);

					let size = bounds.bounds[1] - bounds.bounds[0];
					let position = bounds.bounds[0] + bounds.transform.inverse().transform_vector2(mouse_delta);

					if tool_data.selected_artboard.unwrap() == LayerNodeIdentifier::ROOT_PARENT {
						log::error!("Selected artboard cannot be ROOT_PARENT");
						return ArtboardToolFsmState::Ready { hovered };
					}
					responses.add(GraphOperationMessage::ResizeArtboard {
						layer: tool_data.selected_artboard.unwrap(),
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
							layer: artboard,
							location: start.min(end).round().as_ivec2(),
							dimensions: (start.round() - end.round()).abs().as_ivec2(),
						});
					}
				} else {
					let id = NodeId::new();

					tool_data.selected_artboard = Some(LayerNodeIdentifier::new_unchecked(id));

					responses.add(GraphOperationMessage::NewArtboard {
						id,
						artboard: graphene_core::Artboard {
							graphic_group: graphene_core::GraphicGroup::EMPTY,
							label: String::from("Artboard"),
							location: start.round().as_ivec2(),
							dimensions: IVec2::splat(1),
							background: graphene_core::Color::WHITE,
							clip: false,
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
			(ArtboardToolFsmState::Drawing | ArtboardToolFsmState::ResizingBounds | ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerUp) => {
				responses.add(DocumentMessage::EndTransaction);

				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready { hovered }
			}
			(_, ArtboardToolMessage::UpdateSelectedArtboard) => {
				tool_data.selected_artboard = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_layers(document.metadata())
					.find(|layer| document.network_interface.is_artboard(&layer.to_node(), &[]));
				self
			}
			(_, ArtboardToolMessage::DeleteSelected) => {
				tool_data.selected_artboard.take();
				responses.add(DocumentMessage::DeleteSelectedLayers);

				ArtboardToolFsmState::Ready { hovered }
			}
			(
				_,
				ArtboardToolMessage::NudgeSelected {
					delta_x,
					delta_y,
					resize,
					resize_opposite_corner,
				},
			) => {
				let Some(bounds) = &mut tool_data.bounding_box_manager else {
					return ArtboardToolFsmState::Ready { hovered };
				};
				let Some(selected_artboard) = tool_data.selected_artboard else {
					return ArtboardToolFsmState::Ready { hovered };
				};
				if selected_artboard == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Selected artboard cannot be ROOT_PARENT");
					return ArtboardToolFsmState::Ready { hovered };
				}

				let resize = input.keyboard.key(resize);
				let resize_opposite_corner = input.keyboard.key(resize_opposite_corner);
				let [existing_top_left, existing_bottom_right] = bounds.bounds;

				// Nudge translation without resizing
				if !resize {
					let delta = DVec2::from_angle(-document.document_ptz.tilt()).rotate(DVec2::new(delta_x, delta_y));

					responses.add(GraphOperationMessage::ResizeArtboard {
						layer: selected_artboard,
						location: DVec2::new(existing_top_left.x + delta.x, existing_top_left.y + delta.y).round().as_ivec2(),
						dimensions: (existing_bottom_right - existing_top_left).round().as_ivec2(),
					});

					return ArtboardToolFsmState::Ready { hovered };
				}

				// Swap and negate coordinates as needed to match the resize direction that's closest to the current tilt angle
				let tilt = (document.document_ptz.tilt() + std::f64::consts::TAU) % std::f64::consts::TAU;
				let (delta_x, delta_y, opposite_x, opposite_y) = match ((tilt + std::f64::consts::FRAC_PI_4) / std::f64::consts::FRAC_PI_2).floor() as i32 % 4 {
					0 => (delta_x, delta_y, false, false),
					1 => (delta_y, -delta_x, false, true),
					2 => (-delta_x, -delta_y, true, true),
					3 => (-delta_y, delta_x, true, false),
					_ => unreachable!(),
				};

				let size = existing_bottom_right - existing_top_left;
				let enlargement = DVec2::new(
					if resize_opposite_corner != opposite_x { -delta_x } else { delta_x },
					if resize_opposite_corner != opposite_y { -delta_y } else { delta_y },
				);
				let enlargement_factor = (enlargement + size) / size;

				let position = DVec2::new(
					existing_top_left.x + if resize_opposite_corner != opposite_x { delta_x } else { 0. },
					existing_top_left.y + if resize_opposite_corner != opposite_y { delta_y } else { 0. },
				);
				let mut pivot = (existing_top_left * enlargement_factor - position) / (enlargement_factor - DVec2::ONE);
				if !pivot.x.is_finite() {
					pivot.x = 0.;
				}
				if !pivot.y.is_finite() {
					pivot.y = 0.;
				}
				let scale = DAffine2::from_scale(enlargement_factor);
				let pivot = DAffine2::from_translation(pivot);
				let transformation = pivot * scale * pivot.inverse();
				let document_to_viewport = document.navigation_handler.calculate_offset_transform(input.viewport_bounds.center(), &document.document_ptz);
				let to = document_to_viewport.inverse() * document.metadata().downstream_transform_to_viewport(selected_artboard);
				let original_transform = document.metadata().upstream_transform(selected_artboard.to_node());
				let new = to.inverse() * transformation * to * original_transform;

				responses.add(GraphOperationMessage::ResizeArtboard {
					layer: selected_artboard,
					location: position.round().as_ivec2(),
					dimensions: new.transform_vector2(existing_bottom_right - existing_top_left).round().as_ivec2(),
				});

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
