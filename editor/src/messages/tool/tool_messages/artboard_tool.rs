use super::tool_prelude::*;
use crate::consts::SELECTION_DRAG_ANGLE;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::measure;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::common_functionality::snap_session::{
	BoxFeatures, Drawn, FeatureMap, Locus, Moving, PointOptions, SnapSession, SnapView, nudge_translation, nudges_land_on_grid, snapping_hints,
};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::common_functionality::utility_functions::nudge_resize_bounds;
use graph_craft::document::NodeId;
use graphene_std::renderer::{Quad, Rect};

#[derive(Default, ExtractField)]
pub struct ArtboardTool {
	fsm_state: ArtboardToolFsmState,
	data: ArtboardToolData,
}

#[impl_message(Message, ToolMessage, Artboard)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ArtboardToolMessage {
	// Standard messages
	Abort,
	Overlays { context: OverlayContext },

	// Tool-specific messages
	UpdateSelectedArtboard,
	DeleteSelected,
	NudgeSelected { delta_x: f64, delta_y: f64, resize: Key, resize_opposite: Key },
	PointerDown,
	PointerMove { constrain_axis_or_aspect: Key, center: Key },
	PointerOutsideViewport { constrain_axis_or_aspect: Key, center: Key },
	PointerUp,
}

impl ToolMetadata for ArtboardTool {
	fn icon_name(&self) -> String {
		"GeneralArtboardTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Artboard Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Artboard
	}
}

impl ToolEditTarget for ArtboardTool {
	fn edit_target(&self) -> EditTarget {
		// An artboard layer is itself the Artboard node, which the chain lookup starts from
		EditTarget::network_node("Artboard", Insertion::Never)
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for ArtboardTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let previous_state = self.fsm_state;
		self.fsm_state.process_event(message, &mut self.data, context, &(), responses, false);

		// However a drawing, a drag, or a resize ends, its snapping gesture ends with it
		let in_gesture = matches!(previous_state, ArtboardToolFsmState::Drawing | ArtboardToolFsmState::Dragging | ArtboardToolFsmState::ResizingBounds);
		if in_gesture && self.fsm_state != previous_state {
			self.data.slide = None;
			context.snap_session.end_gesture();
			responses.add(OverlaysMessage::Draw);
		}
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
		Layout::default()
	}
}

impl ToolTransition for ArtboardTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(ArtboardToolMessage::Abort.into()),
			overlay_provider: Some(|context| ArtboardToolMessage::Overlays { context }.into()),
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

/// The artboard being dragged, which slides by however far the pointer asks from where it took hold.
#[derive(Clone, Debug)]
struct Slide {
	/// Where the pointer took hold, in document space.
	grip: DVec2,
	/// The corners, the middles of the edges, and the center of the artboard, each from where it began in document space.
	features: Vec<FeatureMap>,
	/// The corner nearest where the pointer took hold, which is the feature that lands on the grid.
	lead: usize,
	/// Which of the features make up the artboard's box.
	box_features: BoxFeatures,
	/// Where the artboard's top left corner began, in document space.
	start_location: DVec2,
	/// The artboard's width and height.
	size: DVec2,
}

#[derive(Clone, Debug, Default)]
struct ArtboardToolData {
	bounding_box_manager: Option<BoundingBoxManager>,
	selected_artboard: Option<LayerNodeIdentifier>,
	cursor: MouseCursorIcon,
	slide: Option<Slide>,
	auto_panning: AutoPanning,
	dragging_current_artboard_location: glam::DVec2,
	draw: Resize,
}

impl ArtboardToolData {
	/// Readies the selected artboard to slide from where the pointer took hold.
	fn prepare_slide(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		self.slide = None;
		let Some(artboard) = self.selected_artboard else { return };
		let Some(bounds) = document.metadata().bounding_box_document(artboard) else { return };

		let grip = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);
		let corners = Quad::from_box(bounds).0;
		let middles = [0, 1, 2, 3].map(|i| (corners[i] + corners[(i + 1) % 4]) / 2.);
		let center = (bounds[0] + bounds[1]) / 2.;
		let features = corners.into_iter().chain(middles).chain([center]).map(FeatureMap::sliding_from).collect();

		let lead = (0..4)
			.min_by(|&a, &b| corners[a].distance_squared(grip).total_cmp(&corners[b].distance_squared(grip)))
			.unwrap_or_default();

		// The artboard is upright, so its first corner carries both low sides and its third both high sides
		let box_features = BoxFeatures {
			corners: [0, 1, 2, 3],
			sides: [[Some(0), Some(2)]; 2],
		};

		self.slide = Some(Slide {
			grip,
			features,
			lead,
			box_features,
			start_location: bounds[0],
			size: bounds[1] - bounds[0],
		});
	}

	/// Slides the artboard by however far the pointer asks from where it took hold, keeping to an axis while the constrain key is held.
	fn slide_artboard(&mut self, responses: &mut VecDeque<Message>, view: SnapView, snap_session: &mut SnapSession, artboard: LayerNodeIdentifier, constrain_axis: bool) {
		let Some(slide) = &self.slide else { return };
		let input = view.input;
		let document_to_viewport = view.document.metadata().document_to_viewport;

		let pointer = document_to_viewport.inverse().transform_point2(input.mouse.position);
		let free = pointer - slide.grip;

		// The constrain key holds the drag to the nearest axis from where it began, as seen in the viewport
		let axis_aligned = || {
			let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
			let angle = DVec2::X.try_angle_to(input.mouse.position - document_to_viewport.transform_point2(slide.grip))?;
			Some(DVec2::from_angle((angle / snap_resolution).round() * snap_resolution))
		};
		let held_direction = constrain_axis.then(axis_aligned).flatten();
		let required = held_direction.and_then(|direction| Locus::line(DVec2::ZERO, document_to_viewport.inverse().transform_vector2(direction)));

		let options = PointOptions {
			required,
			box_features: Some(slide.box_features),
			..Default::default()
		};
		let slid = snap_session.snap_features(view, &slide.features, slide.lead, free, options);

		let location = (slide.start_location + slid).round();
		let dimensions = slide.size.round();
		responses.add(GraphOperationMessage::ResizeArtboard {
			layer: artboard,
			location,
			dimensions,
		});

		// Keeps the cage with the artboard until the move has rendered
		if let Some(bounds) = &mut self.bounding_box_manager {
			bounds.bounds = [location, location + dimensions];
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
			self.dragging_current_artboard_location = bounds.bounds[0].round();
		}
	}

	/// The selected artboard, unless it's locked or hidden, which puts it out of reach of the tool as on the canvas.
	fn editable_artboard(&self, document: &DocumentMessageHandler) -> Option<LayerNodeIdentifier> {
		let artboard = self.selected_artboard?;
		let selected_nodes = document.network_interface.selected_nodes();
		let editable = selected_nodes.layer_visible(artboard, &document.network_interface) && !selected_nodes.layer_locked(artboard, &document.network_interface);
		editable.then_some(artboard)
	}

	fn hovered_artboard(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler) -> Option<LayerNodeIdentifier> {
		document.click_xray(input, viewport).find(|&layer| document.network_interface.is_artboard(&layer.to_node(), &[]))
	}

	fn select_artboard(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, viewport: &ViewportMessageHandler, responses: &mut VecDeque<Message>) -> bool {
		if let Some(intersection) = Self::hovered_artboard(document, input, viewport) {
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

			responses.add(InspectorMessage::Clear);

			false
		}
	}

	fn resize_artboard(&mut self, responses: &mut VecDeque<Message>, view: SnapView, snap_session: &mut SnapSession, from_center: bool, constrain_square: bool) {
		let document = view.document;
		let Some(bounds) = &self.bounding_box_manager else {
			return;
		};
		let Some(movement) = &bounds.selected_edges else {
			return;
		};
		let Some(selected_artboard) = self.selected_artboard else {
			warn!("Attempted to resize artboard with no selected artboard");
			self.bounding_box_manager.take(); // Remove the bounding box manager if there is no artboard.
			return; // Just do nothing instead of crashing since the state likely isn't too broken.
		};
		if selected_artboard == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Selected artboard cannot be ROOT_PARENT");
			return;
		}

		let center = from_center.then_some(bounds.center_of_transformation);
		let snap = Some(SizeSnapData { session: snap_session, view });
		let (min, size) = movement.new_size(view.input.mouse.position, bounds.transform, center, constrain_square, snap);
		let max = min + size;
		let position = min.min(max);
		let size = (max - min).abs();

		responses.add(GraphOperationMessage::ResizeArtboard {
			layer: selected_artboard,
			location: position.round(),
			dimensions: size.round(),
		});

		let translation = position.round() - self.dragging_current_artboard_location;
		self.dragging_current_artboard_location = position.round();
		for child in selected_artboard.children(document.metadata()) {
			let local_translation = document.metadata().downstream_transform_to_document(child).inverse().transform_vector2(-translation);
			responses.add(GraphOperationMessage::TransformChange {
				layer: child,
				transform: DAffine2::from_translation(local_translation),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			});
		}
	}
}

impl Fsm for ArtboardToolFsmState {
	type ToolData = ArtboardToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionMessageContext, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionMessageContext {
			document,
			document_id,
			input,
			viewport,
			snap_session,
			..
		} = tool_action_data;

		let hovered = ArtboardToolData::hovered_artboard(document, input, viewport).is_some();

		let ToolMessage::Artboard(event) = event else { return self };
		match (self, event) {
			(state, ArtboardToolMessage::Overlays { context: mut overlay_context }) => {
				let display_transform_cage = overlay_context.visibility_settings.transform_cage();
				if display_transform_cage && state != ArtboardToolFsmState::Drawing {
					if let Some(bounds) = tool_data.editable_artboard(document).and_then(|layer| document.metadata().bounding_box_document(layer)) {
						let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());
						bounding_box_manager.bounds = bounds;
						bounding_box_manager.transform = document.metadata().document_to_viewport;

						bounding_box_manager.render_overlays(&mut overlay_context, true);
					} else {
						// Without an editable artboard (one deleted, locked, or hidden), discard the bounding box
						tool_data.bounding_box_manager.take();
					}
				} else {
					tool_data.bounding_box_manager.take();
				}

				// Measure with Alt held down between selected artboard and hovered layers/artboards
				// TODO: Don't use `Key::Alt` directly, instead take it as a variable from the input mappings list like in all other places
				let alt_pressed = input.keyboard.get(Key::Alt as usize);
				let quick_measurement_enabled = overlay_context.visibility_settings.quick_measurement();
				let not_resizing = !matches!(state, ArtboardToolFsmState::ResizingBounds);

				if quick_measurement_enabled
					&& not_resizing && alt_pressed
					&& let Some(selected_artboard) = tool_data.selected_artboard
					&& let Some(selected_bounds) = document.metadata().bounding_box_document(selected_artboard).map(Rect::from_box)
				{
					// Find the topmost artboard under the pointer and the topmost other layer above it or within it in one pass
					let (mut hovered_artboard, mut hovered_layer): (Option<LayerNodeIdentifier>, _) = (None, None);
					for layer in document.click_xray(input, viewport) {
						let is_artboard = document.network_interface.is_artboard(&layer.to_node(), &[]);

						match hovered_artboard {
							// Beneath the topmost artboard, only what lies within it shows, since its background hides the rest
							Some(artboard) => {
								if hovered_layer.is_none() && !is_artboard && artboard.is_ancestor_of(document.metadata(), &layer) {
									hovered_layer = Some(layer);
								}
								break;
							}
							None if is_artboard => hovered_artboard = Some(layer),
							None => hovered_layer = hovered_layer.or(Some(layer)),
						}
					}

					// Another artboard is measured to first, and over the selected one, the topmost other layer under the pointer instead
					let measured_layer = match hovered_artboard {
						Some(artboard) if artboard != selected_artboard => Some(artboard),
						_ => hovered_layer,
					};

					if let Some(hovered_bounds) = measured_layer.and_then(|layer| document.metadata().bounding_box_document(layer)).map(Rect::from_box) {
						let document_to_viewport = document.metadata().document_to_viewport;
						measure::overlay(selected_bounds, hovered_bounds, document_to_viewport, &mut overlay_context);
					}
				}

				self
			}
			(ArtboardToolFsmState::Ready { .. }, ArtboardToolMessage::PointerDown) => {
				let resize_edges = tool_data.editable_artboard(document).and_then(|_| tool_data.check_dragging_bounds(input.mouse.position));
				let state = if let Some(selected_edges) = resize_edges {
					tool_data.start_resizing(selected_edges, document, input);

					// The artboard's own box follows the resize, while what is inside it stays put and can be snapped to
					let moving = Moving {
						edited: tool_data.selected_artboard.into_iter().collect(),
						..Default::default()
					};
					snap_session.begin_gesture(SnapView::new(document, *document_id, input), moving);

					ArtboardToolFsmState::ResizingBounds
				} else if tool_data.select_artboard(document, input, viewport, responses) {
					tool_data.prepare_slide(document, input);

					let moving = Moving {
						layers: tool_data.selected_artboard.into_iter().collect(),
						..Default::default()
					};
					snap_session.begin_gesture(SnapView::new(document, *document_id, input), moving);

					ArtboardToolFsmState::Dragging
				} else {
					let drawn = Drawn {
						parent: LayerNodeIdentifier::ROOT_PARENT,
						artboard: true,
					};
					tool_data.draw.start(snap_session, SnapView::new(document, *document_id, input), drawn);

					ArtboardToolFsmState::Drawing
				};
				responses.add(DocumentMessage::StartTransaction);
				state
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let from_center = input.keyboard.get(center as usize);
				let constrain_square = input.keyboard.get(constrain_axis_or_aspect as usize);
				tool_data.resize_artboard(responses, SnapView::new(document, *document_id, input), snap_session, from_center, constrain_square);

				// Auto-panning
				let messages = [
					ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
					ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				ArtboardToolFsmState::ResizingBounds
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let Some(artboard) = tool_data.selected_artboard else { return ArtboardToolFsmState::Dragging };
				if artboard == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Selected artboard cannot be ROOT_PARENT");
					return ArtboardToolFsmState::Ready { hovered };
				}

				let constrain_axis = input.keyboard.key(constrain_axis_or_aspect);
				tool_data.slide_artboard(responses, SnapView::new(document, *document_id, input), snap_session, artboard, constrain_axis);

				// Auto-panning
				let messages = [
					ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
					ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				ArtboardToolFsmState::Dragging
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }) => {
				let view = SnapView::new(document, *document_id, input);
				let [start, end] = tool_data.draw.calculate_points_ignore_layer(snap_session, view, center, constrain_axis_or_aspect, true);
				let viewport_to_document = document.metadata().document_to_viewport.inverse();
				let [start, end] = [start, end].map(|point| viewport_to_document.transform_point2(point));
				if let Some(artboard) = tool_data.selected_artboard {
					assert_ne!(artboard, LayerNodeIdentifier::ROOT_PARENT, "Selected artboard cannot be ROOT_PARENT");

					responses.add(GraphOperationMessage::ResizeArtboard {
						layer: artboard,
						location: start.min(end).round(),
						dimensions: (start.round() - end.round()).abs(),
					});
				} else {
					let id = NodeId::new();

					tool_data.selected_artboard = Some(LayerNodeIdentifier::new_unchecked(id));

					responses.add(GraphOperationMessage::NewArtboard {
						id,
						location: start.min(end).round(),
						dimensions: (start.round() - end.round()).abs(),
						background: graphene_std::Color::WHITE,
						clip: true,
					})
				}

				// Auto-panning
				let messages = [
					ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
					ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				ArtboardToolFsmState::Drawing
			}

			(ArtboardToolFsmState::Ready { .. }, ArtboardToolMessage::PointerMove { .. }) => {
				let mut cursor = tool_data
					.bounding_box_manager
					.as_ref()
					.map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, false, false, None));

				// Shows where a press would start an artboard, unless the press would move or resize one instead
				if cursor == MouseCursorIcon::Default && !hovered {
					let pointer = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position);
					snap_session.snap_point(SnapView::new(document, *document_id, input), pointer, PointOptions::default());
					cursor = MouseCursorIcon::Crosshair;
				} else {
					snap_session.forget();
				}
				responses.add(OverlaysMessage::Draw);

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				ArtboardToolFsmState::Ready { hovered }
			}
			(ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, viewport, responses);

				ArtboardToolFsmState::ResizingBounds
			}
			(ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				tool_data.auto_panning.shift_viewport(input, viewport, responses);

				ArtboardToolFsmState::Dragging
			}
			(ArtboardToolFsmState::Drawing, ArtboardToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				tool_data.auto_panning.shift_viewport(input, viewport, responses);

				ArtboardToolFsmState::Drawing
			}
			(state, ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }) => {
				// Auto-panning
				let messages = [
					ArtboardToolMessage::PointerOutsideViewport { constrain_axis_or_aspect, center }.into(),
					ArtboardToolMessage::PointerMove { constrain_axis_or_aspect, center }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(ArtboardToolFsmState::Drawing | ArtboardToolFsmState::ResizingBounds | ArtboardToolFsmState::Dragging, ArtboardToolMessage::PointerUp) => {
				responses.add(DocumentMessage::EndTransaction);

				tool_data.draw.cleanup();

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(OverlaysMessage::Draw);

				ArtboardToolFsmState::Ready { hovered }
			}
			(_, ArtboardToolMessage::UpdateSelectedArtboard) => {
				tool_data.selected_artboard = document
					.network_interface
					.selected_nodes()
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
					resize_opposite,
				},
			) => {
				let Some(bounds) = &tool_data.bounding_box_manager else {
					return ArtboardToolFsmState::Ready { hovered };
				};
				let Some(selected_artboard) = tool_data.editable_artboard(document) else {
					return ArtboardToolFsmState::Ready { hovered };
				};
				if selected_artboard == LayerNodeIdentifier::ROOT_PARENT {
					log::error!("Selected artboard cannot be ROOT_PARENT");
					return ArtboardToolFsmState::Ready { hovered };
				}

				let [existing_top_left, existing_bottom_right] = bounds.bounds;
				let tilt = document.document_ptz.tilt();

				// The resize key switches from nudging the artboard's position to resizing its box, anchored to the opposite edge by the other key
				let resize = input.keyboard.key(resize);
				let (location, dimensions) = if resize {
					let resize_opposite = input.keyboard.key(resize_opposite);
					let resized = nudge_resize_bounds(existing_top_left, existing_bottom_right, DVec2::new(delta_x, delta_y), tilt, resize_opposite);
					(resized.min.round(), (resized.max - resized.min).round())
				} else {
					let delta = nudge_translation(document, DVec2::new(delta_x, delta_y), Quad::from_box(bounds.bounds).0);

					// A nudge onto the grid rounds to whole pixels in its direction, so a grid line under half a pixel ahead doesn't round it back to where it was
					let moved = existing_top_left + delta;
					let onward = |value: f64, step: f64| match step.partial_cmp(&0.) {
						Some(std::cmp::Ordering::Greater) => (value - 1e-9).ceil(),
						Some(std::cmp::Ordering::Less) => (value + 1e-9).floor(),
						_ => value.round(),
					};
					let location = if nudges_land_on_grid(document) {
						DVec2::new(onward(moved.x, delta.x), onward(moved.y, delta.y))
					} else {
						moved.round()
					};
					(location, (existing_bottom_right - existing_top_left).round())
				};

				responses.add(DocumentMessage::AddTransaction);
				responses.add(GraphOperationMessage::ResizeArtboard {
					layer: selected_artboard,
					location,
					dimensions,
				});

				ArtboardToolFsmState::Ready { hovered }
			}
			(ArtboardToolFsmState::Dragging | ArtboardToolFsmState::Drawing | ArtboardToolFsmState::ResizingBounds, ArtboardToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				tool_data.draw.cleanup();
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
				snapping_hints(),
			]),
			ArtboardToolFsmState::Dragging => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain to Axis")]),
				snapping_hints(),
			]),
			ArtboardToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")]),
				snapping_hints(),
			]),
			ArtboardToolFsmState::ResizingBounds => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "Preserve Aspect Ratio"), HintInfo::keys([Key::Alt], "From Center")]),
				snapping_hints(),
			]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		if let Self::Ready { hovered: false } = self {
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
		} else {
			responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
		}
	}
}

#[cfg(test)]
mod test_artboard {
	use crate::consts::NUDGE_AMOUNT;
	use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
	use crate::messages::portfolio::document::utility_types::misc::GridType;
	pub use crate::test_utils::test_prelude::*;
	use graphene_std::Artboard;
	use graphene_std::list::List;

	async fn get_artboards(editor: &mut EditorTestUtils) -> List<Artboard> {
		let instrumented = match editor.eval_graph_until_finished().await {
			Ok(instrumented) => instrumented,
			Err(e) => panic!("Failed to evaluate graph: {e}"),
		};
		instrumented
			.grab_all_input::<graphene_std::graphic::extend::NewInput, List<Artboard>>(&editor.runtime)
			.flatten()
			.collect()
	}

	#[derive(Debug, PartialEq)]
	struct ArtboardLayoutDocument {
		position: DVec2,
		dimensions: DVec2,
	}
	impl ArtboardLayoutDocument {
		pub fn new(position: impl Into<DVec2>, dimensions: impl Into<DVec2>) -> Self {
			Self {
				position: position.into(),
				dimensions: dimensions.into(),
			}
		}
	}

	/// Check if all of the artboards exist in any ordering
	async fn has_artboards(editor: &mut EditorTestUtils, mut expected: Vec<ArtboardLayoutDocument>) {
		let artboards = get_artboards(editor).await;
		let artboards = (0..artboards.len())
			.map(|index| {
				let location: DVec2 = artboards.attribute_cloned_or_default(graphene_std::ATTR_LOCATION, index);
				let dimensions: DVec2 = artboards.attribute_cloned_or_default(graphene_std::ATTR_DIMENSIONS, index);
				ArtboardLayoutDocument::new(location, dimensions)
			})
			.collect::<Vec<_>>();
		assert_eq!(artboards.len(), expected.len(), "incorrect len: actual {:?}, expected {:?}", artboards, expected);

		for artboard in artboards {
			let Some(index) = expected.iter().position(|expected| *expected == artboard) else {
				panic!("found {:?} that did not match any expected artboards\nexpected {:?}", artboard, expected);
			};
			expected.remove(index);
		}
	}

	#[tokio::test]
	async fn artboard_draw_simple() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 10.1, 10.8, 19.9, 0.2, ModifierKeys::empty()).await;
		has_artboards(&mut editor, vec![ArtboardLayoutDocument::new((10., 0.), (10., 11.))]).await;
	}

	#[tokio::test]
	async fn artboard_snapping() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.set_viewport_size(DVec2::splat(-1000.), DVec2::splat(1000.)).await; // Necessary for doing snapping since snaps outside of the viewport are discarded
		editor.drag_tool(ToolType::Artboard, 10., 10., 20., 20., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Artboard, 10., 50., 19., 60., ModifierKeys::empty()).await;
		has_artboards(
			&mut editor,
			vec![ArtboardLayoutDocument::new((10., 10.), (10., 10.)), ArtboardLayoutDocument::new((10., 50.), (10., 10.))],
		)
		.await;
	}

	#[tokio::test]
	async fn artboard_draw_square() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 10., 10., -10., 11., ModifierKeys::SHIFT).await;
		has_artboards(&mut editor, vec![ArtboardLayoutDocument::new((-10., 10.), (20., 20.))]).await;
	}

	#[tokio::test]
	async fn artboard_draw_square_rotated() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				// 45 degree rotation of content clockwise
				angle_radians: f64::consts::FRAC_PI_4,
			})
			.await;
		// Viewport coordinates
		editor.drag_tool(ToolType::Artboard, 0., 0., 0., 10., ModifierKeys::SHIFT).await;
		let desired_size = DVec2::splat(f64::consts::FRAC_1_SQRT_2 * 10.);

		has_artboards(&mut editor, vec![ArtboardLayoutDocument::new(DVec2::new(0., 0.), desired_size.round())]).await;
	}

	#[tokio::test]
	async fn artboard_draw_center_square_rotated() {
		let mut editor = EditorTestUtils::create();

		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				// 45 degree rotation of content clockwise
				angle_radians: f64::consts::FRAC_PI_4,
			})
			.await;
		// Viewport coordinates
		editor.drag_tool(ToolType::Artboard, 0., 0., 0., 10., ModifierKeys::SHIFT | ModifierKeys::ALT).await;
		let desired_location = DVec2::splat(f64::consts::FRAC_1_SQRT_2 * -10.).round();
		let desired_size = DVec2::splat(f64::consts::FRAC_1_SQRT_2 * 20.).round();
		has_artboards(&mut editor, vec![ArtboardLayoutDocument::new(desired_location, desired_size)]).await;
	}

	#[tokio::test]
	async fn artboard_delete() {
		let mut editor = EditorTestUtils::create();

		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 10.1, 10.8, 19.9, 0.2, ModifierKeys::default()).await;
		editor.press(Key::Delete, ModifierKeys::default()).await;

		has_artboards(&mut editor, vec![]).await;
	}

	#[tokio::test]
	async fn artboard_cancel() {
		let mut editor = EditorTestUtils::create();

		editor.new_document().await;

		editor.drag_tool_cancel_rmb(ToolType::Artboard).await;
		has_artboards(&mut editor, vec![]).await;
	}

	#[tokio::test]
	async fn artboard_move() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 10., 10., 20., 22., ModifierKeys::empty()).await; // Artboard to drag
		editor.drag_tool(ToolType::Artboard, 15., 15., 65., 65., ModifierKeys::empty()).await; // Drag from the middle by (50,50)

		has_artboards(&mut editor, vec![ArtboardLayoutDocument::new((60., 60.), (10., 12.))]).await;
	}
	#[tokio::test]
	async fn artboard_move_snapping() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.set_viewport_size(DVec2::splat(-1000.), DVec2::splat(1000.)).await; // Necessary for doing snapping since snaps outside of the viewport are discarded
		editor.drag_tool(ToolType::Artboard, 10., 10., 20., 22., ModifierKeys::empty()).await; // Artboard to drag
		editor.drag_tool(ToolType::Artboard, 70., 0., 80., 100., ModifierKeys::empty()).await; // Artboard to snap to
		editor.drag_tool(ToolType::Artboard, 15., 15., 15. + 49., 15., ModifierKeys::empty()).await; // Drag the artboard so it should snap to the edge

		has_artboards(
			&mut editor,
			vec![ArtboardLayoutDocument::new((60., 10.), (10., 12.)), ArtboardLayoutDocument::new((70., 0.), (10., 100.))],
		)
		.await;
	}

	#[tokio::test]
	async fn an_artboard_edge_resized_to_near_the_edge_of_another_artboard_lands_on_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 100., 100., 200., 200., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Artboard, 300., 100., 400., 200., ModifierKeys::empty()).await;

		// Drawing the overlays lays the cage around the second artboard, and its left edge is brought a few pixels from the first artboard's right edge
		editor.handle_message(ArtboardToolMessage::Overlays { context: OverlayContext::default() }).await;
		editor.drag_path(&[DVec2::new(300., 150.), DVec2::new(203., 156.)], ModifierKeys::empty()).await;

		has_artboards(
			&mut editor,
			vec![ArtboardLayoutDocument::new((100., 100.), (100., 100.)), ArtboardLayoutDocument::new((200., 100.), (200., 100.))],
		)
		.await;
	}

	#[tokio::test]
	async fn first_artboard() {
		let mut editor = EditorTestUtils::create();

		editor.new_document().await;
		// Put rectangles in before making the artboard
		editor.drag_tool(ToolType::Rectangle, 10., 10., 20., 16., ModifierKeys::empty()).await;
		let rectangle1 = editor.get_selected_layer().await.unwrap();
		editor.drag_tool(ToolType::Rectangle, 15., 15., 25., 25., ModifierKeys::empty()).await;
		let rectangle2 = editor.get_selected_layer().await.unwrap();

		// Where the rectangles ended up, which the second one snapping to the first may have changed
		let drawn_bounds = [rectangle1, rectangle2].map(|layer| editor.active_document().metadata().bounding_box_document(layer).unwrap());

		// Put the artboard in
		editor.drag_tool(ToolType::Artboard, 0., 0., 100., 100., ModifierKeys::empty()).await;
		has_artboards(&mut editor, vec![ArtboardLayoutDocument::new((0., 0.), (100., 100.))]).await;
		let document = editor.active_document();

		// artboard
		// ├── rectangle1
		// └── rectangle2
		let artboard = document.metadata().all_layers().next().unwrap();
		assert_eq!(artboard.first_child(document.metadata()), Some(rectangle2));
		assert_eq!(rectangle2.next_sibling(document.metadata()), Some(rectangle1));

		// The document bounding boxes should remain the same (content shouldn't shift)
		assert_eq!([rectangle1, rectangle2].map(|layer| document.metadata().bounding_box_document(layer).unwrap()), drawn_bounds);
	}

	/// The least and greatest corners of a layer's bounding box in the document.
	fn bounds(editor: &EditorTestUtils, layer: LayerNodeIdentifier) -> [DVec2; 2] {
		editor.active_document().metadata().bounding_box_document(layer).expect("the layer should have a bounding box")
	}

	fn assert_bounds(editor: &EditorTestUtils, layer: LayerNodeIdentifier, expected: [DVec2; 2]) {
		let [min, max] = bounds(editor, layer);
		let [expected_min, expected_max] = expected;
		assert!(
			min.distance(expected_min) < 1e-6 && max.distance(expected_max) < 1e-6,
			"the layer spans {min} to {max} rather than {expected_min} to {expected_max}"
		);
	}

	/// Draws an artboard between two positions, which are the same in the viewport and the document, and returns its layer.
	async fn draw_artboard(editor: &mut EditorTestUtils, from: DVec2, to: DVec2) -> LayerNodeIdentifier {
		editor.drag_tool(ToolType::Artboard, from.x, from.y, to.x, to.y, ModifierKeys::empty()).await;

		let metadata = editor.active_document().metadata();
		let is_drawn = |layer: &LayerNodeIdentifier| metadata.bounding_box_document(*layer).is_some_and(|[min, max]| min.distance(from) < 1e-6 && max.distance(to) < 1e-6);
		metadata.all_layers().find(is_drawn).expect("the artboard should be where it was drawn")
	}

	/// Presses the Artboard tool at a viewport position, which takes hold of the artboard there.
	async fn press_at(editor: &mut EditorTestUtils, position: DVec2) {
		editor.select_tool(ToolType::Artboard).await;
		editor.move_mouse(position.x, position.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(position.x, position.y, ModifierKeys::empty()).await;
	}

	async fn move_to(editor: &mut EditorTestUtils, position: DVec2, modifier_keys: ModifierKeys) {
		editor.move_mouse(position.x, position.y, modifier_keys, MouseKeys::LEFT).await;
	}

	async fn drag(editor: &mut EditorTestUtils, from: DVec2, to: DVec2, modifier_keys: ModifierKeys) {
		press_at(editor, from).await;
		move_to(editor, to, modifier_keys).await;
		editor.left_mouseup(to.x, to.y, modifier_keys).await;
	}

	#[tokio::test]
	async fn an_artboard_dragged_nearly_level_with_one_far_beside_it_lines_up_with_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_artboard(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		let artboard = draw_artboard(&mut editor, DVec2::new(400., 300.), DVec2::new(440., 380.)).await;

		// Its top comes four pixels below the other's top, with hundreds of pixels between them
		let grab = DVec2::new(420., 340.);
		drag(&mut editor, grab, grab + DVec2::new(7., -196.), ModifierKeys::empty()).await;

		assert_bounds(&editor, artboard, [DVec2::new(407., 100.), DVec2::new(447., 180.)]);
	}

	#[tokio::test]
	async fn an_artboard_dragged_nearly_halfway_between_two_others_lands_with_the_same_gap_on_both_sides() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_artboard(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		draw_artboard(&mut editor, DVec2::new(300., 100.), DVec2::new(360., 160.)).await;
		let artboard = draw_artboard(&mut editor, DVec2::new(200., 112.), DVec2::new(240., 148.)).await;

		// Halfway is ten further along, where the gap on either side comes to fifty
		let grab = DVec2::new(220., 130.);
		drag(&mut editor, grab, grab + DVec2::new(7., 0.), ModifierKeys::empty()).await;

		assert_bounds(&editor, artboard, [DVec2::new(210., 112.), DVec2::new(250., 148.)]);
	}

	#[tokio::test]
	async fn an_artboard_dragged_near_where_its_own_content_was_does_not_snap_to_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(100., 100.), DVec2::new(300., 300.)).await;
		editor.draw_rect(150., 150., 180., 170.).await;
		let rectangle = editor.get_selected_layer().await.expect("the drawn rectangle should be selected");
		assert_eq!(rectangle.parent(editor.active_document().metadata()), Some(artboard));
		assert_bounds(&editor, rectangle, [DVec2::new(150., 150.), DVec2::new(180., 170.)]);

		// Its top left corner is brought a few pixels from where the rectangle's top left corner was
		let grab = DVec2::new(250., 250.);
		drag(&mut editor, grab, grab + DVec2::new(53., 52.), ModifierKeys::empty()).await;

		assert_bounds(&editor, artboard, [DVec2::new(153., 152.), DVec2::new(353., 352.)]);
	}

	#[tokio::test]
	async fn an_artboard_dragged_with_shift_moves_along_one_axis_only() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(300., 300.), DVec2::new(400., 380.)).await;

		// Mostly rightward, it stays at the same height
		let grab = DVec2::new(350., 340.);
		drag(&mut editor, grab, grab + DVec2::new(70., 23.), ModifierKeys::SHIFT).await;
		assert_bounds(&editor, artboard, [DVec2::new(370., 300.), DVec2::new(470., 380.)]);

		// Mostly downward, it stays at the same left and right
		let grab = DVec2::new(420., 340.);
		drag(&mut editor, grab, grab + DVec2::new(-18., 60.), ModifierKeys::SHIFT).await;
		assert_bounds(&editor, artboard, [DVec2::new(370., 360.), DVec2::new(470., 440.)]);
	}

	#[tokio::test]
	async fn escape_during_a_drag_puts_the_artboard_back_and_ends_the_snapping_gesture() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_artboard(&mut editor, DVec2::new(100., 100.), DVec2::new(160., 160.)).await;
		let artboard = draw_artboard(&mut editor, DVec2::new(300., 300.), DVec2::new(360., 360.)).await;
		let shows_an_attempt = |editor: &EditorTestUtils| editor.editor.dispatcher.message_handlers.tool_message_handler.snap_session.shown().is_some();

		let (grab, away) = (DVec2::new(330., 330.), DVec2::new(400., 420.));
		press_at(&mut editor, grab).await;
		move_to(&mut editor, away, ModifierKeys::empty()).await;
		assert_bounds(&editor, artboard, [DVec2::new(370., 390.), DVec2::new(430., 450.)]);
		assert!(shows_an_attempt(&editor));

		// Ending the gesture lets go of the drag's latest attempt to snap
		editor.press(Key::Escape, ModifierKeys::empty()).await;
		editor.left_mouseup(away.x, away.y, ModifierKeys::empty()).await;
		assert_bounds(&editor, artboard, [DVec2::new(300., 300.), DVec2::new(360., 360.)]);
		assert!(!shows_an_attempt(&editor));

		// Its top left corner is brought a few pixels from the other's bottom right corner
		drag(&mut editor, grab, grab + DVec2::new(-137., -142.), ModifierKeys::empty()).await;
		assert_bounds(&editor, artboard, [DVec2::new(160., 160.), DVec2::new(220., 220.)]);
	}

	#[tokio::test]
	async fn an_artboard_dragged_far_from_anything_moves_as_far_as_the_pointer_rounded_to_whole_units() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(300., 300.), DVec2::new(400., 380.)).await;

		let grab = DVec2::new(350.4, 340.2);
		drag(&mut editor, grab, grab + DVec2::new(123.3, -47.6), ModifierKeys::empty()).await;

		assert_bounds(&editor, artboard, [DVec2::new(423., 252.), DVec2::new(523., 332.)]);
	}

	#[tokio::test]
	async fn an_artboard_drawn_a_few_pixels_short_of_level_with_another_ends_level_with_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		draw_artboard(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 200.)).await;

		// Its bottom comes four pixels above the other's bottom, with hundreds of pixels between them
		editor.drag_tool(ToolType::Artboard, 400., 130., 460., 196., ModifierKeys::empty()).await;

		let metadata = editor.active_document().metadata();
		let is_new = |layer: &LayerNodeIdentifier| metadata.bounding_box_document(*layer).is_some_and(|[min, _]| min.x > 300.);
		let artboard = metadata.all_layers().find(is_new).expect("the artboard drawn last should be beside the other");
		assert_bounds(&editor, artboard, [DVec2::new(400., 130.), DVec2::new(460., 200.)]);
	}

	#[tokio::test]
	async fn an_artboard_drawn_nearly_level_with_artwork_inside_another_artboard_is_not_lined_up_with_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let other = draw_artboard(&mut editor, DVec2::new(0., 0.), DVec2::new(300., 300.)).await;
		editor.draw_rect(100., 100., 160., 160.).await;
		let rectangle = editor.get_selected_layer().await.expect("the drawn rectangle should be selected");
		assert_eq!(rectangle.parent(editor.active_document().metadata()), Some(other));
		assert_bounds(&editor, rectangle, [DVec2::new(100., 100.), DVec2::new(160., 160.)]);

		// Its bottom comes four pixels below the rectangle's bottom, which only boxes sharing its parent would line it up with
		let artboard = draw_artboard(&mut editor, DVec2::new(500., 110.), DVec2::new(560., 164.)).await;
		assert_bounds(&editor, artboard, [DVec2::new(500., 110.), DVec2::new(560., 164.)]);
	}

	#[tokio::test]
	async fn an_artboard_nudged_with_grid_snapping_on_lands_on_the_next_grid_line() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(103., 32.), DVec2::new(163., 76.)).await;

		let state = &mut editor.active_document_mut().snapping_state;
		state.grid_snapping = true;
		state.grid.grid_type = GridType::Rectangular { spacing: DVec2::splat(10.) };

		// Drawing the overlays lays the cage around the artboard, which the nudge moves
		editor.handle_message(ArtboardToolMessage::Overlays { context: OverlayContext::default() }).await;
		editor
			.handle_message(ArtboardToolMessage::NudgeSelected {
				delta_x: 0.,
				delta_y: NUDGE_AMOUNT,
				resize: Key::Alt,
				resize_opposite: Key::Control,
			})
			.await;

		assert_bounds(&editor, artboard, [DVec2::new(103., 36.), DVec2::new(163., 80.)]);
	}

	#[tokio::test]
	async fn an_artboard_nudged_toward_a_grid_line_less_than_half_a_pixel_ahead_still_moves() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(21., 30.), DVec2::new(81., 70.)).await;

		let state = &mut editor.active_document_mut().snapping_state;
		state.grid_snapping = true;
		state.grid.grid_type = GridType::Rectangular { spacing: DVec2::splat(10.) };
		state.grid.origin = DVec2::new(0.5, 0.);

		// Lines run half a pixel off the whole pixels the artboard keeps to
		editor.handle_message(ArtboardToolMessage::Overlays { context: OverlayContext::default() }).await;
		for expected_left in [20., 10.] {
			let nudge_left = ArtboardToolMessage::NudgeSelected {
				delta_x: -NUDGE_AMOUNT,
				delta_y: 0.,
				resize: Key::Alt,
				resize_opposite: Key::Control,
			};
			editor.handle_message(nudge_left).await;
			editor.handle_message(ArtboardToolMessage::Overlays { context: OverlayContext::default() }).await;

			assert_bounds(&editor, artboard, [DVec2::new(expected_left, 30.), DVec2::new(expected_left + 60., 70.)]);
		}
	}

	#[tokio::test]
	async fn an_artboard_nudged_on_a_tilted_canvas_with_grid_snapping_off_moves_to_the_nearest_whole_pixels() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(103., 32.), DVec2::new(163., 76.)).await;

		// Rightward on the screen runs a third of a pixel upward in the document, too little to leave its row
		editor.handle_message(NavigationMessage::CanvasTiltSet { angle_radians: 20_f64.to_radians() }).await;
		editor.handle_message(ArtboardToolMessage::Overlays { context: OverlayContext::default() }).await;
		editor
			.handle_message(ArtboardToolMessage::NudgeSelected {
				delta_x: NUDGE_AMOUNT,
				delta_y: 0.,
				resize: Key::Alt,
				resize_opposite: Key::Control,
			})
			.await;

		assert_bounds(&editor, artboard, [DVec2::new(104., 32.), DVec2::new(164., 76.)]);
	}

	#[tokio::test]
	async fn a_locked_artboard_selected_in_the_layers_panel_cant_be_nudged_or_resized() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 160.)).await;
		editor
			.handle_message(NodeGraphMessage::SetLocked {
				node_id: artboard.to_node(),
				network_path: Vec::new(),
				locked: true,
			})
			.await;

		// Selected in the Layers panel, as the canvas leaves locked artboards out of reach
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![artboard.to_node()] }).await;
		editor.handle_message(ArtboardToolMessage::Overlays { context: OverlayContext::default() }).await;

		editor.press(Key::ArrowRight, ModifierKeys::SHIFT).await;
		assert_bounds(&editor, artboard, [DVec2::new(100., 100.), DVec2::new(200., 160.)]);

		// Where its bottom edge's handle would be
		drag(&mut editor, DVec2::new(150., 160.), DVec2::new(150., 220.), ModifierKeys::empty()).await;
		assert_bounds(&editor, artboard, [DVec2::new(100., 100.), DVec2::new(200., 160.)]);
	}

	#[tokio::test]
	async fn an_artboard_locked_while_its_cage_is_shown_cant_be_nudged_or_resized_before_the_overlays_redraw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let artboard = draw_artboard(&mut editor, DVec2::new(100., 100.), DVec2::new(200., 160.)).await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![artboard.to_node()] }).await;
		editor.handle_message(ArtboardToolMessage::Overlays { context: OverlayContext::default() }).await;

		// No overlay pass follows the lock, so the cage drawn before it is still there to grab
		editor
			.handle_message(NodeGraphMessage::SetLocked {
				node_id: artboard.to_node(),
				network_path: Vec::new(),
				locked: true,
			})
			.await;

		editor.press(Key::ArrowRight, ModifierKeys::SHIFT).await;
		assert_bounds(&editor, artboard, [DVec2::new(100., 100.), DVec2::new(200., 160.)]);

		drag(&mut editor, DVec2::new(150., 160.), DVec2::new(150., 220.), ModifierKeys::empty()).await;
		assert_bounds(&editor, artboard, [DVec2::new(100., 100.), DVec2::new(200., 160.)]);
	}
}
