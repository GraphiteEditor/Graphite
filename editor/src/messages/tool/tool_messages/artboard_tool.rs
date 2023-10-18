use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::messages::tool::common_functionality::graph_modification_utils::is_artboard;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::common_functionality::transformation_cage::*;

use document_legacy::document_metadata::LayerNodeIdentifier;

use document_legacy::layers::RenderData;
use glam::{IVec2, Vec2Swizzles};

#[derive(Default)]
pub struct ArtboardTool {
	fsm_state: ArtboardToolFsmState,
	data: ArtboardToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Artboard)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum ArtboardToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	DeleteSelected,
	NudgeSelected {
		delta_x: f64,
		delta_y: f64,
	},
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
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Artboard
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for ArtboardTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.data, tool_data, &(), responses, false);
	}

	advertise_actions!(ArtboardToolMessageDiscriminant;
		PointerDown,
		PointerUp,
		PointerMove,
		DeleteSelected,
		NudgeSelected,
		Abort,
	);
}

impl LayoutHolder for ArtboardTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

impl ToolTransition for ArtboardTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(ArtboardToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(ArtboardToolMessage::Abort.into()),
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
	bounding_box_overlays: Option<BoundingBoxOverlays>,
	selected_artboard: Option<LayerNodeIdentifier>,
	snap_manager: SnapManager,
	cursor: MouseCursorIcon,
	drag_start: DVec2,
	drag_current: DVec2,
}

impl ArtboardToolData {
	fn refresh_overlays(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let current_artboard = self.selected_artboard.and_then(|layer| document.metadata().bounding_box_document(layer));
		match (current_artboard, self.bounding_box_overlays.take()) {
			(None, Some(bounding_box_overlays)) => bounding_box_overlays.delete(responses),
			(Some(bounds), paths) => {
				let mut bounding_box_overlays = paths.unwrap_or_else(|| BoundingBoxOverlays::new(responses));

				bounding_box_overlays.bounds = bounds;
				bounding_box_overlays.transform = document.metadata().document_to_viewport;

				bounding_box_overlays.transform(responses);

				self.bounding_box_overlays = Some(bounding_box_overlays);

				responses.add(OverlaysMessage::Rerender);
			}
			_ => {}
		};
	}

	fn check_dragging_bounds(&mut self, cursor: DVec2) -> Option<(bool, bool, bool, bool)> {
		let bounding_box = self.bounding_box_overlays.as_mut()?;
		let edges = bounding_box.check_selected_edges(cursor)?;
		let (top, bottom, left, right) = edges;
		let selected_edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
		bounding_box.opposite_pivot = selected_edges.calculate_pivot();
		bounding_box.selected_edges = Some(selected_edges);

		Some(edges)
	}

	fn start_resizing(&mut self, selected_edges: (bool, bool, bool, bool), document: &DocumentMessageHandler, render_data: &RenderData, input: &InputPreprocessorMessageHandler) {
		let snap_x = selected_edges.2 || selected_edges.3;
		let snap_y = selected_edges.0 || selected_edges.1;

		let artboard = self.selected_artboard.unwrap();
		self.snap_manager
			.start_snap(document, input, document.bounding_boxes(None, Some(artboard.to_node()), render_data), snap_x, snap_y);
		self.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

		if let Some(bounds) = &mut self.bounding_box_overlays {
			bounds.center_of_transformation = (bounds.bounds[0] + bounds.bounds[1]) / 2.;
		}
	}

	fn select_artboard(&mut self, document: &DocumentMessageHandler, render_data: &RenderData, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> bool {
		responses.add(DocumentMessage::StartTransaction);

		let mut intersections = document
			.document_legacy
			.metadata
			.click_xray(input.mouse.position)
			.filter(|&layer| is_artboard(layer, &document.document_legacy));

		responses.add(BroadcastEvent::DocumentIsDirty);
		if let Some(intersection) = intersections.next() {
			self.selected_artboard = Some(intersection);

			self.snap_manager
				.start_snap(document, input, document.bounding_boxes(None, Some(intersection.to_node()), render_data), true, true);
			self.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

			true
		} else {
			self.selected_artboard = None;

			responses.add(PropertiesPanelMessage::ClearSelection);

			false
		}
	}

	fn resize_artboard(&mut self, responses: &mut VecDeque<Message>, document: &DocumentMessageHandler, mouse_position: DVec2, from_center: bool, constrain_square: bool) {
		let Some(bounds) = &self.bounding_box_overlays else {
			return;
		};
		let Some(movement) = &bounds.selected_edges else {
			return;
		};

		let snapped_mouse_position: DVec2 = self.snap_manager.snap_position(responses, document, mouse_position);

		let (position, size) = movement.new_size(snapped_mouse_position, bounds.transform, from_center, bounds.center_of_transformation, constrain_square);
		responses.add(GraphOperationMessage::ResizeArtboard {
			id: self.selected_artboard.unwrap().to_node(),
			location: position.round().as_ivec2(),
			dimensions: size.round().as_ivec2(),
		});

		responses.add(BroadcastEvent::DocumentIsDirty);
	}
}

impl Fsm for ArtboardToolFsmState {
	type ToolData = ArtboardToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData { document, input, render_data, .. } = tool_action_data;

		let ToolMessage::Artboard(event) = event else {
			return self;
		};

		match (self, event) {
			(state, ArtboardToolMessage::DocumentIsDirty) if state != ArtboardToolFsmState::Drawing => {
				tool_data.refresh_overlays(document, responses);

				self
			}
			(ArtboardToolFsmState::Ready, ArtboardToolMessage::PointerDown) => {
				tool_data.drag_start = input.mouse.position;
				tool_data.drag_current = input.mouse.position;

				if let Some(selected_edges) = tool_data.check_dragging_bounds(input.mouse.position) {
					responses.add(DocumentMessage::StartTransaction);
					tool_data.start_resizing(selected_edges, document, render_data, input);

					ArtboardToolFsmState::ResizingBounds
				} else if tool_data.select_artboard(document, render_data, input, responses) {
					ArtboardToolFsmState::Dragging
				} else {
					ArtboardToolFsmState::Drawing
				}
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let from_center = input.keyboard.get(center as usize);
				let constrain_square = input.keyboard.get(constrain_axis_or_aspect as usize);
				let mouse_position = input.mouse.position;
				tool_data.resize_artboard(responses, document, mouse_position, from_center, constrain_square);

				ArtboardToolFsmState::ResizingBounds
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, .. }) => {
				if let Some(bounds) = &tool_data.bounding_box_overlays {
					let axis_align = input.keyboard.get(constrain_axis_or_aspect as usize);

					let mouse_position = axis_align_drag(axis_align, input.mouse.position, tool_data.drag_start);
					let mouse_delta = mouse_position - tool_data.drag_current;

					let snap = bounds.evaluate_transform_handle_positions().into_iter().collect();
					let closest_move = tool_data.snap_manager.snap_layers(responses, document, snap, mouse_delta);

					let size = bounds.bounds[1] - bounds.bounds[0];

					let position = bounds.bounds[0] + bounds.transform.inverse().transform_vector2(mouse_position - tool_data.drag_current + closest_move);

					responses.add(GraphOperationMessage::ResizeArtboard {
						id: tool_data.selected_artboard.unwrap().to_node(),
						location: position.round().as_ivec2(),
						dimensions: size.round().as_ivec2(),
					});

					responses.add(BroadcastEvent::DocumentIsDirty);

					tool_data.drag_current = mouse_position + closest_move;
				}
				ArtboardToolFsmState::Dragging
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let mouse_position = input.mouse.position;
				let snapped_mouse_position = tool_data.snap_manager.snap_position(responses, document, mouse_position);

				let root_transform = document.metadata().document_to_viewport.inverse();

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

				if let Some(artboard) = tool_data.selected_artboard {
					responses.add(GraphOperationMessage::ResizeArtboard {
						id: artboard.to_node(),
						location: start.round().as_ivec2(),
						dimensions: size.round().as_ivec2(),
					});
				} else {
					let id = generate_uuid();
					tool_data.selected_artboard = Some(LayerNodeIdentifier::new_unchecked(id));

					tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(None, Some(id), render_data), true, true);
					tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

					responses.add(GraphOperationMessage::NewArtboard {
						id,
						artboard: graphene_core::Artboard {
							graphic_group: graphene_core::GraphicGroup::EMPTY,
							location: start.round().as_ivec2(),
							dimensions: IVec2::splat(1),
							background: graphene_core::Color::WHITE,
							clip: false,
						},
					})
				}

				// Have to put message here instead of when Artboard is created
				// This might result in a few more calls but it is not reliant on the order of messages

				responses.add(BroadcastEvent::DocumentIsDirty);

				ArtboardToolFsmState::Drawing
			}
			(ArtboardToolFsmState::Ready, ArtboardToolMessage::PointerMove { .. }) => {
				let cursor = tool_data.bounding_box_overlays.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, false));

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				ArtboardToolFsmState::Ready
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_overlays {
					bounds.original_transforms.clear();
				}

				ArtboardToolFsmState::Ready
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_overlays {
					bounds.original_transforms.clear();
				}

				responses.add(BroadcastEvent::DocumentIsDirty);

				ArtboardToolFsmState::Ready
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerUp) => {
				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_overlays {
					bounds.original_transforms.clear();
				}

				ArtboardToolFsmState::Ready
			}
			(_, ArtboardToolMessage::DeleteSelected) => {
				if let Some(artboard) = tool_data.selected_artboard.take() {
					let id = artboard.to_node();
					responses.add(GraphOperationMessage::DeleteLayer { id });

					responses.add(BroadcastEvent::DocumentIsDirty);
				}
				ArtboardToolFsmState::Ready
			}
			(_, ArtboardToolMessage::NudgeSelected { delta_x, delta_y }) => {
				if let Some(bounds) = &mut tool_data.bounding_box_overlays {
					responses.add(GraphOperationMessage::ResizeArtboard {
						id: tool_data.selected_artboard.unwrap().to_node(),
						location: DVec2::new(bounds.bounds[0].x + delta_x, bounds.bounds[0].y + delta_y).round().as_ivec2(),
						dimensions: (bounds.bounds[1] - bounds.bounds[0]).round().as_ivec2(),
					});
				}

				ArtboardToolFsmState::Ready
			}
			(_, ArtboardToolMessage::Abort) => {
				if let Some(bounding_box_overlays) = tool_data.bounding_box_overlays.take() {
					bounding_box_overlays.delete(responses);
				}

				// Register properties when switching back to other tools
				responses.add(PropertiesPanelMessage::SetActiveLayers {
					paths: document.selected_layers().map(|path| path.to_vec()).collect(),
				});

				tool_data.snap_manager.cleanup(responses);
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
			ArtboardToolFsmState::Dragging => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain to Axis")])]),
			ArtboardToolFsmState::Drawing | ArtboardToolFsmState::ResizingBounds => {
				HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")])])
			}
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
