#![allow(clippy::too_many_arguments)]

use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::consts::{DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS, DRAG_BEYOND_VIEWPORT_SPEED_FACTOR, ROTATE_SNAP_ANGLE, SELECTION_TOLERANCE};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis};
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::tool::common_functionality::graph_modification_utils::is_layer_fed_by_node_of_name;
use crate::messages::tool::common_functionality::pivot::Pivot;
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnappedPoint};
use crate::messages::tool::common_functionality::transformation_cage::*;

use graph_craft::document::{NodeId, NodeNetwork};
use graphene_core::renderer::Quad;

use std::fmt;

#[derive(Default)]
pub struct SelectTool {
	fsm_state: SelectToolFsmState,
	tool_data: SelectToolData,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct SelectOptions {
	nested_selection_behavior: NestedSelectionBehavior,
}

#[remain::sorted]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum SelectOptionsUpdate {
	NestedSelectionBehavior(NestedSelectionBehavior),
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum NestedSelectionBehavior {
	#[default]
	Deepest,
	Shallowest,
}

impl fmt::Display for NestedSelectionBehavior {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			NestedSelectionBehavior::Deepest => write!(f, "Deep Select"),
			NestedSelectionBehavior::Shallowest => write!(f, "Shallow Select"),
		}
	}
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub struct SelectToolPointerKeys {
	pub axis_align: Key,
	pub snap_angle: Key,
	pub center: Key,
	pub duplicate: Key,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum SelectToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	Overlays(OverlayContext),

	// Tool-specific messages
	DragStart {
		add_to_selection: Key,
		select_deepest: Key,
	},
	DragStop {
		remove_from_selection: Key,
	},
	EditLayer,
	Enter,
	PointerMove(SelectToolPointerKeys),
	PointerOutsideViewport(SelectToolPointerKeys),
	SelectOptions(SelectOptionsUpdate),
	SetPivot {
		position: PivotPosition,
	},
	ShiftViewport,
}

impl ToolMetadata for SelectTool {
	fn icon_name(&self) -> String {
		"GeneralSelectTool".into()
	}
	fn tooltip(&self) -> String {
		"Select Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Select
	}
}

impl SelectTool {
	fn deep_selection_widget(&self) -> WidgetHolder {
		let layer_selection_behavior_entries = [NestedSelectionBehavior::Deepest, NestedSelectionBehavior::Shallowest]
			.iter()
			.map(|mode| {
				MenuListEntry::new(mode.to_string())
					.value(mode.to_string())
					.on_update(move |_| SelectToolMessage::SelectOptions(SelectOptionsUpdate::NestedSelectionBehavior(*mode)).into())
			})
			.collect();

		DropdownInput::new(vec![layer_selection_behavior_entries])
			.selected_index(Some((self.tool_data.nested_selection_behavior == NestedSelectionBehavior::Shallowest) as u32))
			.tooltip("Choose if clicking nested layers directly selects the deepest, or selects the shallowest and deepens by double clicking")
			.widget_holder()
	}

	fn pivot_widget(&self, disabled: bool) -> WidgetHolder {
		PivotInput::new(self.tool_data.pivot.to_pivot_position())
			.on_update(|pivot_input: &PivotInput| SelectToolMessage::SetPivot { position: pivot_input.position }.into())
			.disabled(disabled)
			.widget_holder()
	}

	fn alignment_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetHolder> {
		[AlignAxis::X, AlignAxis::Y]
			.into_iter()
			.flat_map(|axis| [(axis, AlignAggregate::Min), (axis, AlignAggregate::Center), (axis, AlignAggregate::Max)])
			.map(move |(axis, aggregate)| {
				let (icon, tooltip) = match (axis, aggregate) {
					(AlignAxis::X, AlignAggregate::Min) => ("AlignLeft", "Align Left"),
					(AlignAxis::X, AlignAggregate::Center) => ("AlignHorizontalCenter", "Align Horizontal Center"),
					(AlignAxis::X, AlignAggregate::Max) => ("AlignRight", "Align Right"),
					(AlignAxis::Y, AlignAggregate::Min) => ("AlignTop", "Align Top"),
					(AlignAxis::Y, AlignAggregate::Center) => ("AlignVerticalCenter", "Align Vertical Center"),
					(AlignAxis::Y, AlignAggregate::Max) => ("AlignBottom", "Align Bottom"),
				};
				IconButton::new(icon, 24)
					.tooltip(tooltip)
					.on_update(move |_| DocumentMessage::AlignSelectedLayers { axis, aggregate }.into())
					.disabled(disabled)
					.widget_holder()
			})
	}

	fn flip_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetHolder> {
		[(FlipAxis::X, "Horizontal"), (FlipAxis::Y, "Vertical")].into_iter().map(move |(flip_axis, name)| {
			IconButton::new("Flip".to_string() + name, 24)
				.tooltip("Flip ".to_string() + name)
				.on_update(move |_| DocumentMessage::FlipSelectedLayers { flip_axis }.into())
				.disabled(disabled)
				.widget_holder()
		})
	}

	fn boolean_widgets(&self) -> impl Iterator<Item = WidgetHolder> {
		["Union", "Subtract Front", "Subtract Back", "Intersect", "Difference"].into_iter().map(|name| {
			IconButton::new(format!("Boolean{}", name.replace(' ', "")), 24)
				.tooltip(format!("Boolean {name} (coming soon)"))
				.on_update(|_| DialogMessage::RequestComingSoonDialog { issue: Some(1091) }.into())
				.widget_holder()
		})
	}
}

impl LayoutHolder for SelectTool {
	fn layout(&self) -> Layout {
		let mut widgets = Vec::new();

		// Select mode (Deep/Shallow)
		widgets.push(self.deep_selection_widget());

		// Pivot
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(self.pivot_widget(self.tool_data.selected_layers_count == 0));

		// Align
		let disabled = self.tool_data.selected_layers_count < 2;
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.alignment_widgets(disabled));
		widgets.push(PopoverButton::new("Align", "Coming soon").disabled(disabled).widget_holder());

		// Flip
		let disabled = self.tool_data.selected_layers_count == 0;
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.flip_widgets(disabled));

		// Boolean
		if self.tool_data.selected_layers_count >= 2 {
			widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
			widgets.extend(self.boolean_widgets());
		}

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for SelectTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Select(SelectToolMessage::SelectOptions(SelectOptionsUpdate::NestedSelectionBehavior(nested_selection_behavior))) = message {
			self.tool_data.nested_selection_behavior = nested_selection_behavior;
			responses.add(ToolMessage::UpdateHints);
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, false);

		if self.tool_data.pivot.should_refresh_pivot_position() || self.tool_data.selected_layers_changed {
			// Send the layout containing the updated pivot position (a bit ugly to do it here not in the fsm but that doesn't have SelectTool)
			self.send_layout(responses, LayoutTarget::ToolOptions);
			self.tool_data.selected_layers_changed = false;
		}
	}

	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(SelectToolMessageDiscriminant;
				DragStart,
				PointerMove,
				Abort,
				EditLayer,
				Enter,
			),
			_ => actions!(SelectToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Abort,
				EditLayer,
				Enter,
			),
		}
	}
}

impl ToolTransition for SelectTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(SelectToolMessage::Abort.into()),
			overlay_provider: Some(|overlay_context| SelectToolMessage::Overlays(overlay_context).into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum SelectToolFsmState {
	#[default]
	Ready,
	Dragging,
	DrawingBox,
	ResizingBounds,
	RotatingBounds,
	DraggingPivot,
}

#[derive(Clone, Debug, Default)]
struct SelectToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	layers_dragging: Vec<LayerNodeIdentifier>,
	layer_selected_on_start: Option<LayerNodeIdentifier>,
	select_single_layer: Option<LayerNodeIdentifier>,
	has_dragged: bool,
	non_duplicated_layers: Option<Vec<LayerNodeIdentifier>>,
	bounding_box_manager: Option<BoundingBoxManager>,
	snap_manager: SnapManager,
	cursor: MouseCursorIcon,
	pivot: Pivot,
	nested_selection_behavior: NestedSelectionBehavior,
	selected_layers_count: usize,
	selected_layers_changed: bool,
	snap_candidates: Vec<SnapCandidatePoint>,
	subscribed_to_animation_frame: bool,
}

impl SelectToolData {
	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		self.snap_candidates.clear();
		for &layer in &self.layers_dragging {
			if (self.snap_candidates.len() as f64) < document.snapping_state.tolerance {
				snapping::get_layer_snap_points(layer, &SnapData::new(document, input), &mut self.snap_candidates);
			}
			if let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) {
				let quad = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
				snapping::get_bbox_points(quad, &mut self.snap_candidates, snapping::BBoxSnapValues::BOUNDING_BOX, document);
			}
		}
	}

	fn selection_quad(&self) -> Quad {
		let bbox = self.selection_box();
		Quad::from_box(bbox)
	}

	fn selection_box(&self) -> [DVec2; 2] {
		if self.drag_current == self.drag_start {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[self.drag_start - tolerance, self.drag_start + tolerance]
		} else {
			[self.drag_start, self.drag_current]
		}
	}

	/// Duplicates the currently dragging layers. Called when Alt is pressed and the layers have not yet been duplicated.
	fn start_duplicates(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.non_duplicated_layers = Some(self.layers_dragging.clone());
		let mut new_dragging = Vec::new();
		for layer_ancestors in document.metadata().shallowest_unique_layers(self.layers_dragging.iter().copied().rev()) {
			let Some(layer) = layer_ancestors.last().copied() else { continue };
			let Some(parent) = layer.parent(&document.metadata) else { continue };

			// Copy the layer
			let node = layer.to_node();
			let Some(node) = document.network().nodes.get(&node).and_then(|node| node.inputs.first()).and_then(|input| input.as_node()) else {
				continue;
			};

			let nodes = NodeGraphMessageHandler::copy_nodes(
				document.network(),
				&document
					.network()
					.upstream_flow_back_from_nodes(vec![node], false)
					.enumerate()
					.map(|(index, (_, node_id))| (node_id, NodeId(index as u64)))
					.collect(),
			)
			.collect();

			// Moves the layer back to its starting position.
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform: DAffine2::from_translation(self.drag_start - self.drag_current),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

			let id = NodeId(generate_uuid());
			let insert_index = -1;
			let layer = LayerNodeIdentifier::new_unchecked(id);
			responses.add(GraphOperationMessage::NewCustomLayer { id, nodes, parent, insert_index });
			new_dragging.push(layer);
		}
		let nodes = new_dragging.iter().map(|layer| layer.to_node()).collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		self.layers_dragging = new_dragging;
	}

	/// Removes the duplicated layers. Called when Alt is released and the layers have previously been duplicated.
	fn stop_duplicates(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(original) = self.non_duplicated_layers.take() else {
			return;
		};

		// Delete the duplicated layers
		for layer_ancestors in document.metadata().shallowest_unique_layers(self.layers_dragging.iter().copied()) {
			responses.add(GraphOperationMessage::DeleteLayer {
				id: layer_ancestors.last().unwrap().to_node(),
			});
		}

		for &layer in &original {
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform: DAffine2::from_translation(self.drag_current - self.drag_start),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});
		}
		let nodes = original.iter().map(|layer| layer.to_node()).collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		self.layers_dragging = original;
	}
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData { document, input, .. } = tool_action_data;

		let ToolMessage::Select(event) = event else {
			return self;
		};
		match (self, event) {
			(_, SelectToolMessage::Overlays(mut overlay_context)) => {
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

				let selected_layers_count = document.selected_nodes.selected_layers(document.metadata()).count();
				tool_data.selected_layers_changed = selected_layers_count != tool_data.selected_layers_count;
				tool_data.selected_layers_count = selected_layers_count;

				// Outline selected layers
				for layer in document.selected_nodes.selected_visible_layers(document.network(), document.metadata()) {
					overlay_context.outline(document.metadata().layer_outline(layer), document.metadata().transform_to_viewport(layer));
				}

				// Get the layer the user is hovering over
				let click = document.click(input.mouse.position, &document.network);
				let not_selected_click = click.filter(|&hovered_layer| !document.selected_nodes.selected_layers_contains(hovered_layer, document.metadata()));
				if let Some(layer) = not_selected_click {
					overlay_context.outline(document.metadata().layer_outline(layer), document.metadata().transform_to_viewport(layer));
				}

				// Update bounds
				let transform = document
					.selected_nodes
					.selected_visible_layers(document.network(), document.metadata())
					.next()
					.map(|layer| document.metadata().transform_to_viewport(layer));
				let transform = transform.unwrap_or(DAffine2::IDENTITY);
				let bounds = document
					.selected_nodes
					.selected_visible_layers(document.network(), document.metadata())
					.filter_map(|layer| {
						document
							.metadata()
							.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer))
					})
					.reduce(graphene_core::renderer::Quad::combine_bounds);
				if let Some(bounds) = bounds {
					let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());

					bounding_box_manager.bounds = bounds;
					bounding_box_manager.transform = transform;

					bounding_box_manager.render_overlays(&mut overlay_context);
				} else {
					tool_data.bounding_box_manager.take();
				}

				// Update pivot
				tool_data.pivot.update_pivot(document, &mut overlay_context);

				// Update dragging box
				if self == Self::DrawingBox {
					overlay_context.quad(Quad::from_box([tool_data.drag_start, tool_data.drag_current]));
				}

				self
			}
			(_, SelectToolMessage::EditLayer) => {
				// Edit the clicked layer
				if let Some(intersect) = document.click(input.mouse.position, &document.network) {
					match tool_data.nested_selection_behavior {
						NestedSelectionBehavior::Shallowest => edit_layer_shallowest_manipulation(document, intersect, responses),
						NestedSelectionBehavior::Deepest => edit_layer_deepest_manipulation(intersect, &document.network, responses),
					}
				}

				self
			}
			(SelectToolFsmState::Ready, SelectToolMessage::DragStart { add_to_selection, select_deepest }) => {
				tool_data.drag_start = input.mouse.position;
				tool_data.drag_current = input.mouse.position;

				let dragging_bounds = tool_data.bounding_box_manager.as_mut().and_then(|bounding_box| {
					let edges = bounding_box.check_selected_edges(input.mouse.position);

					bounding_box.selected_edges = edges.map(|(top, bottom, left, right)| {
						let selected_edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
						bounding_box.opposite_pivot = selected_edges.calculate_pivot();
						selected_edges
					});

					edges
				});

				let rotating_bounds = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box| bounding_box.check_rotate(input.mouse.position))
					.unwrap_or_default();

				let mut selected: Vec<_> = document.selected_nodes.selected_visible_layers(document.network(), document.metadata()).collect();
				let intersection = document.click(input.mouse.position, &document.network);

				// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
				// If the user is dragging the rotate trigger, go into RotatingBounds mode.
				// If the user clicks on a layer that is in their current selection, go into the dragging mode.
				// If the user clicks on new shape, make that layer their new selection.
				// Otherwise enter the box select mode

				let state =
				// Dragging the pivot
				if tool_data.pivot.is_over(input.mouse.position) {
					responses.add(DocumentMessage::StartTransaction);

					//tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(), true, true);
					//tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

					SelectToolFsmState::DraggingPivot
				}
				// Dragging one (or two, forming a corner) of the transform cage bounding box edges
				else if let Some(_selected_edges) = dragging_bounds {
					responses.add(DocumentMessage::StartTransaction);

					tool_data.layers_dragging = selected;

					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.original_bound_transform = bounds.transform;

						tool_data.layers_dragging.retain(|layer| document.network.nodes.contains_key(&layer.to_node()));
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut bounds.center_of_transformation,
							&tool_data.layers_dragging,
							responses,
							&document.network,
							&document.metadata,
							None,
							&ToolType::Select,
						);
						bounds.center_of_transformation = selected.mean_average_of_pivots();
					}
					tool_data.get_snap_candidates(document, input);

					SelectToolFsmState::ResizingBounds
				}
				// Dragging near the transform cage bounding box to rotate it
				else if rotating_bounds {
					responses.add(DocumentMessage::StartTransaction);

					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						tool_data.layers_dragging.retain(|layer| document.network().nodes.contains_key(&layer.to_node()));
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut bounds.center_of_transformation,
							&selected,
							responses,
							&document.network,
							&document.metadata,
							None,
							&ToolType::Select,
						);

						bounds.center_of_transformation = selected.mean_average_of_pivots();
					}

					tool_data.layers_dragging = selected;

					SelectToolFsmState::RotatingBounds
				}
				// Dragging the selected layers around to transform them
				else if intersection.is_some_and(|intersection| selected.iter().any(|selected_layer| intersection.starts_with(*selected_layer, document.metadata()))) {
					responses.add(DocumentMessage::StartTransaction);

					if tool_data.nested_selection_behavior == NestedSelectionBehavior::Deepest {
						tool_data.select_single_layer = intersection;
					} else {
						tool_data.select_single_layer = intersection.and_then(|intersection| intersection.ancestors(&document.metadata).find(|ancestor| selected.contains(ancestor)));
					}

					tool_data.layers_dragging = selected;

					tool_data.get_snap_candidates(document, input);

					SelectToolFsmState::Dragging
				}
				// Dragging a selection box
				else {
					tool_data.layers_dragging = selected;

					if !input.keyboard.key(add_to_selection) {
						responses.add(DocumentMessage::DeselectAllLayers);
						tool_data.layers_dragging.clear();
					}

					if let Some(intersection) = intersection {
						responses.add(DocumentMessage::StartTransaction);

						tool_data.layer_selected_on_start = Some(intersection);
						selected = vec![intersection];

						match tool_data.nested_selection_behavior {
							NestedSelectionBehavior::Shallowest if !input.keyboard.key(select_deepest) => drag_shallowest_manipulation(responses, selected, tool_data, document),
							_ => drag_deepest_manipulation(responses, selected, tool_data),
						}
						tool_data.get_snap_candidates(document, input);
						SelectToolFsmState::Dragging
					} else {
						// Deselect all layers if using shallowest selection behavior
						// Necessary since for shallowest mode, we need to know the current selected layers to determine the next
						if tool_data.nested_selection_behavior == NestedSelectionBehavior::Shallowest {
							responses.add(DocumentMessage::DeselectAllLayers);
							tool_data.layers_dragging.clear();
						}
						SelectToolFsmState::DrawingBox
					}
				};
				tool_data.non_duplicated_layers = None;

				state
			}
			(SelectToolFsmState::Dragging, SelectToolMessage::PointerMove(modifier_keys)) => {
				tool_data.has_dragged = true;

				if input.keyboard.key(modifier_keys.duplicate) && tool_data.non_duplicated_layers.is_none() {
					tool_data.start_duplicates(document, responses);
				} else if !input.keyboard.key(modifier_keys.duplicate) && tool_data.non_duplicated_layers.is_some() {
					tool_data.stop_duplicates(document, responses);
				}

				let axis_align = input.keyboard.key(modifier_keys.axis_align);
				let mouse_position = axis_align_drag(axis_align, input.mouse.position, tool_data.drag_start);
				let total_mouse_delta_document = document.metadata.document_to_viewport.inverse().transform_vector2(mouse_position - tool_data.drag_start);

				// Ignore the non duplicated layers if the current layers have not spawned yet.
				let layers_exist = tool_data.layers_dragging.iter().all(|&layer| document.metadata().click_target(layer).is_some());
				let ignore = tool_data.non_duplicated_layers.as_ref().filter(|_| !layers_exist).unwrap_or(&tool_data.layers_dragging);

				let snap_data = SnapData::ignore(document, input, ignore);
				let mouse_delta_document = document.metadata.document_to_viewport.inverse().transform_vector2(mouse_position - tool_data.drag_current);
				let mut offset = mouse_delta_document;
				let mut best_snap = SnappedPoint::infinite_snap(document.metadata.document_to_viewport.inverse().transform_point2(mouse_position));

				for point in &mut tool_data.snap_candidates {
					point.document_point += total_mouse_delta_document;
					let snapped = if axis_align {
						let constraint = SnapConstraint::Line {
							origin: point.document_point,
							direction: total_mouse_delta_document.normalize(),
						};
						tool_data.snap_manager.constrained_snap(&snap_data, point, constraint, None)
					} else {
						tool_data.snap_manager.free_snap(&snap_data, point, None, false)
					};
					if best_snap.other_snap_better(&snapped) {
						offset = snapped.snapped_point_document - point.document_point + mouse_delta_document;
						best_snap = snapped;
					}
					point.document_point -= total_mouse_delta_document;
				}
				tool_data.snap_manager.update_indicator(best_snap);

				let mouse_delta = document.metadata.document_to_viewport.transform_vector2(offset);

				// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
				for layer_ancestors in document.metadata().shallowest_unique_layers(tool_data.layers_dragging.iter().copied()) {
					responses.add_front(GraphOperationMessage::TransformChange {
						layer: *layer_ancestors.last().unwrap(),
						transform: DAffine2::from_translation(mouse_delta),
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});
				}
				tool_data.drag_current += mouse_delta;

				setup_pointer_outside_edge_event(input.mouse.position, input.viewport_bounds.size(), tool_data, modifier_keys, responses);

				SelectToolFsmState::Dragging
			}
			(SelectToolFsmState::ResizingBounds, SelectToolMessage::PointerMove(modifier_keys)) => {
				if let Some(ref mut bounds) = &mut tool_data.bounding_box_manager {
					if let Some(movement) = &mut bounds.selected_edges {
						let (center, constrain) = (input.keyboard.key(modifier_keys.center), input.keyboard.key(modifier_keys.axis_align));

						let center = center.then_some(bounds.center_of_transformation);
						let snap = Some(SizeSnapData {
							manager: &mut tool_data.snap_manager,
							points: &mut tool_data.snap_candidates,
							snap_data: SnapData::ignore(document, input, &tool_data.layers_dragging),
						});
						let (position, size) = movement.new_size(input.mouse.position, bounds.original_bound_transform, center, constrain, snap);
						let (delta, mut pivot) = movement.bounds_to_scale_transform(position, size);

						let pivot_transform = DAffine2::from_translation(pivot);
						let transformation = pivot_transform * delta * pivot_transform.inverse();

						tool_data.layers_dragging.retain(|layer| document.network().nodes.contains_key(&layer.to_node()));
						let selected = &tool_data.layers_dragging;
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut pivot,
							selected,
							responses,
							&document.network,
							&document.metadata,
							None,
							&ToolType::Select,
						);

						selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse());

						setup_pointer_outside_edge_event(input.mouse.position, input.viewport_bounds.size(), tool_data, modifier_keys, responses);
					}
				}
				SelectToolFsmState::ResizingBounds
			}
			(SelectToolFsmState::RotatingBounds, SelectToolMessage::PointerMove(modifier_keys)) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					let angle = {
						let start_offset = tool_data.drag_start - bounds.center_of_transformation;
						let end_offset = input.mouse.position - bounds.center_of_transformation;

						start_offset.angle_between(end_offset)
					};

					let snapped_angle = if input.keyboard.key(modifier_keys.snap_angle) {
						let snap_resolution = ROTATE_SNAP_ANGLE.to_radians();
						(angle / snap_resolution).round() * snap_resolution
					} else {
						angle
					};

					let delta = DAffine2::from_angle(snapped_angle);

					tool_data.layers_dragging.retain(|layer| document.network().nodes.contains_key(&layer.to_node()));
					let mut selected = Selected::new(
						&mut bounds.original_transforms,
						&mut bounds.center_of_transformation,
						&tool_data.layers_dragging,
						responses,
						&document.network,
						&document.metadata,
						None,
						&ToolType::Select,
					);

					selected.update_transforms(delta);
				}

				SelectToolFsmState::RotatingBounds
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::PointerMove(modifier_keys)) => {
				let mouse_position = input.mouse.position;
				let snapped_mouse_position = mouse_position; //tool_data.snap_manager.snap_position(responses, document, mouse_position);
				tool_data.pivot.set_viewport_position(snapped_mouse_position, document, responses);

				setup_pointer_outside_edge_event(mouse_position, input.viewport_bounds.size(), tool_data, modifier_keys, responses);

				SelectToolFsmState::DraggingPivot
			}
			(SelectToolFsmState::DrawingBox, SelectToolMessage::PointerMove(modifier_keys)) => {
				tool_data.drag_current = input.mouse.position;
				responses.add(OverlaysMessage::Draw);

				setup_pointer_outside_edge_event(input.mouse.position, input.viewport_bounds.size(), tool_data, modifier_keys, responses);

				SelectToolFsmState::DrawingBox
			}
			(SelectToolFsmState::Ready, SelectToolMessage::PointerMove(_)) => {
				let mut cursor = tool_data.bounding_box_manager.as_ref().map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, true));

				// Dragging the pivot overrules the other operations
				if tool_data.pivot.is_over(input.mouse.position) {
					cursor = MouseCursorIcon::Move;
				}

				// Generate the hover outline
				responses.add(OverlaysMessage::Draw);

				if tool_data.cursor != cursor {
					tool_data.cursor = cursor;
					responses.add(FrontendMessage::UpdateMouseCursor { cursor });
				}

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::Dragging, SelectToolMessage::PointerOutsideViewport(modifier_keys)) => {
				responses.add(SelectToolMessage::PointerMove(modifier_keys));

				if let Some(shift) = shift_viewport_if_mouse_beyond_edge(input.mouse.position, input.viewport_bounds.size(), responses) {
					tool_data.drag_current += shift;
					tool_data.drag_start += shift;
				}

				SelectToolFsmState::Dragging
			}
			(SelectToolFsmState::ResizingBounds | SelectToolFsmState::DraggingPivot | SelectToolFsmState::DrawingBox, SelectToolMessage::PointerOutsideViewport(modifier_keys)) => {
				responses.add(SelectToolMessage::PointerMove(modifier_keys));

				responses.add(SelectToolMessage::ShiftViewport);

				self
			}
			(SelectToolFsmState::ResizingBounds, SelectToolMessage::ShiftViewport) => {
				if let Some(shift) = shift_viewport_if_mouse_beyond_edge(input.mouse.position, input.viewport_bounds.size(), responses) {
					if let Some(ref mut bounds) = &mut tool_data.bounding_box_manager {
						bounds.center_of_transformation += shift;
						bounds.original_bound_transform.translation += shift;
					}
				}

				self
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::ShiftViewport) => {
				let _ = shift_viewport_if_mouse_beyond_edge(input.mouse.position, input.viewport_bounds.size(), responses);

				self
			}
			(SelectToolFsmState::DrawingBox, SelectToolMessage::ShiftViewport) => {
				if let Some(shift) = shift_viewport_if_mouse_beyond_edge(input.mouse.position, input.viewport_bounds.size(), responses) {
					tool_data.drag_start += shift;
				}

				self
			}
			(state, SelectToolMessage::PointerOutsideViewport(modifier_keys)) => {
				unsubscribe_animation_frame(tool_data, modifier_keys, responses);

				state
			}
			(SelectToolFsmState::Dragging, SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::Undo,
					false => DocumentMessage::CommitTransaction,
				};
				tool_data.snap_manager.cleanup(responses);
				responses.add_front(response);

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::Dragging, SelectToolMessage::DragStop { remove_from_selection }) => {
				// Deselect layer if not snap dragging
				if !tool_data.has_dragged && input.keyboard.key(remove_from_selection) && tool_data.layer_selected_on_start.is_none() {
					let quad = tool_data.selection_quad();
					let intersection = document.intersect_quad(quad, &document.network);

					if let Some(path) = intersection.last() {
						let replacement_selected_layers: Vec<_> = document
							.selected_nodes
							.selected_layers(document.metadata())
							.filter(|&layer| !path.starts_with(layer, document.metadata()))
							.collect();

						tool_data.layers_dragging.clear();
						tool_data.layers_dragging.extend(replacement_selected_layers.iter());

						responses.add(NodeGraphMessage::SelectedNodesSet {
							nodes: replacement_selected_layers.iter().map(|layer| layer.to_node()).collect(),
						});
					}
				} else if let Some(selecting_layer) = tool_data.select_single_layer.take() {
					if !tool_data.has_dragged {
						responses.add(NodeGraphMessage::SelectedNodesSet {
							nodes: vec![selecting_layer.to_node()],
						});
					}
				}

				tool_data.has_dragged = false;
				tool_data.layer_selected_on_start = None;

				responses.add(DocumentMessage::CommitTransaction);
				tool_data.snap_manager.cleanup(responses);
				tool_data.select_single_layer = None;

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::ResizingBounds, SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::Undo,
					false => DocumentMessage::CommitTransaction,
				};
				responses.add(response);

				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::RotatingBounds, SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::Undo,
					false => DocumentMessage::CommitTransaction,
				};
				responses.add(response);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::Undo,
					false => DocumentMessage::CommitTransaction,
				};
				responses.add(response);

				tool_data.snap_manager.cleanup(responses);

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::DrawingBox, SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter) => {
				let quad = tool_data.selection_quad();
				let new_selected: HashSet<_> = document.intersect_quad(quad, &document.network).collect();
				let current_selected: HashSet<_> = document.selected_nodes.selected_layers(document.metadata()).collect();
				if new_selected != current_selected {
					tool_data.layers_dragging = new_selected.into_iter().collect();
					responses.add(DocumentMessage::StartTransaction);
					responses.add(NodeGraphMessage::SelectedNodesSet {
						nodes: tool_data.layers_dragging.iter().map(|layer| layer.to_node()).collect(),
					});
				}
				responses.add(OverlaysMessage::Draw);

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::Ready, SelectToolMessage::Enter) => {
				let mut selected_layers = document.selected_nodes.selected_layers(document.metadata());

				if let Some(layer) = selected_layers.next() {
					// Check that only one layer is selected
					if selected_layers.next().is_none() && is_layer_fed_by_node_of_name(layer, &document.network, "Text") {
						responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
						responses.add(TextToolMessage::EditSelected);
					}
				}

				SelectToolFsmState::Ready
			}
			(SelectToolFsmState::Dragging, SelectToolMessage::Abort) => {
				tool_data.snap_manager.cleanup(responses);
				responses.add(DocumentMessage::Undo);
				responses.add(OverlaysMessage::Draw);

				SelectToolFsmState::Ready
			}
			(_, SelectToolMessage::Abort) => {
				tool_data.layers_dragging.retain(|layer| document.network().nodes.contains_key(&layer.to_node()));
				if let Some(mut bounding_box_overlays) = tool_data.bounding_box_manager.take() {
					let mut selected = Selected::new(
						&mut bounding_box_overlays.original_transforms,
						&mut bounding_box_overlays.opposite_pivot,
						&tool_data.layers_dragging,
						responses,
						&document.network,
						&document.metadata,
						None,
						&ToolType::Select,
					);

					selected.revert_operation();
				}

				responses.add(OverlaysMessage::Draw);

				tool_data.snap_manager.cleanup(responses);
				SelectToolFsmState::Ready
			}
			(_, SelectToolMessage::SetPivot { position }) => {
				responses.add(DocumentMessage::StartTransaction);

				let pos: Option<DVec2> = position.into();
				tool_data.pivot.set_normalized_position(pos.unwrap(), document, responses);

				self
			}
			_ => self,
		}
	}

	fn standard_tool_messages(&self, message: &ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut Self::ToolData) -> bool {
		// Check for standard hits or cursor events
		match message {
			ToolMessage::UpdateHints => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
					HintGroup(vec![HintInfo::keys([Key::KeyG, Key::KeyR, Key::KeyS], "Grab/Rotate/Scale Selected")]),
					HintGroup({
						let mut hints = vec![HintInfo::mouse(MouseMotion::Lmb, "Select Object"), HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus()];
						if tool_data.nested_selection_behavior == NestedSelectionBehavior::Shallowest {
							hints.extend([HintInfo::keys([Key::Accel], "Deepest").prepend_plus(), HintInfo::mouse(MouseMotion::LmbDouble, "Deepen Selection")]);
						}
						hints
					}),
					HintGroup(vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
						HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus(),
					]),
					HintGroup(vec![
						HintInfo::arrow_keys("Nudge Selected"),
						HintInfo::keys([Key::Shift], "10x").prepend_plus(),
						HintInfo::keys([Key::Alt], "Resize Corner").prepend_plus(),
						HintInfo::keys([Key::Control], "Opp. Corner").prepend_plus(),
					]),
					HintGroup(vec![
						HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "Move Duplicate"),
						HintInfo::keys([Key::Control, Key::KeyD], "Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
					]),
				]);

				responses.add(FrontendMessage::UpdateInputHints { hint_data });
				self.update_hints(responses);
				true
			}
			ToolMessage::UpdateCursor => {
				self.update_cursor(responses);
				true
			}
			_ => false,
		}
	}

	fn update_hints(&self, _responses: &mut VecDeque<Message>) {}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn not_artboard(document: &DocumentMessageHandler) -> impl Fn(&LayerNodeIdentifier) -> bool + '_ {
	|&layer| !document.metadata.is_artboard(layer)
}

fn drag_shallowest_manipulation(responses: &mut VecDeque<Message>, selected: Vec<LayerNodeIdentifier>, tool_data: &mut SelectToolData, document: &DocumentMessageHandler) {
	for layer in selected {
		let ancestor = layer
			.ancestors(document.metadata())
			.filter(not_artboard(document))
			.find(|&ancestor| document.selected_nodes.selected_layers_contains(ancestor, document.metadata()));

		let new_selected = ancestor.unwrap_or_else(|| {
			layer
				.ancestors(document.metadata())
				.take_while(|&layer| layer != LayerNodeIdentifier::ROOT)
				.filter(not_artboard(document))
				.last()
				.unwrap_or(layer)
		});
		tool_data.layers_dragging.retain(|layer| !layer.ancestors(document.metadata()).any(|ancestor| ancestor == new_selected));
		tool_data.layers_dragging.push(new_selected);
	}

	responses.add(NodeGraphMessage::SelectedNodesSet {
		nodes: tool_data.layers_dragging.iter().map(|layer| layer.to_node()).collect(),
	});
}

fn drag_deepest_manipulation(responses: &mut VecDeque<Message>, mut selected: Vec<LayerNodeIdentifier>, tool_data: &mut SelectToolData) {
	tool_data.layers_dragging.append(&mut selected);
	responses.add(NodeGraphMessage::SelectedNodesSet {
		nodes: tool_data.layers_dragging.iter().map(|layer| layer.to_node()).collect(),
	});
}

fn edit_layer_shallowest_manipulation(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	if document.selected_nodes.selected_layers_contains(layer, document.metadata()) {
		responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Path });
		return;
	}

	let Some(new_selected) = layer.ancestors(document.metadata()).filter(not_artboard(document)).find(|ancestor| {
		ancestor
			.parent(document.metadata())
			.is_some_and(|parent| document.selected_nodes.selected_layers_contains(parent, document.metadata()))
	}) else {
		return;
	};

	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![new_selected.to_node()] });
}

fn edit_layer_deepest_manipulation(layer: LayerNodeIdentifier, document_network: &NodeNetwork, responses: &mut VecDeque<Message>) {
	if is_layer_fed_by_node_of_name(layer, document_network, "Text") {
		responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
		responses.add(TextToolMessage::EditSelected);
	} else if is_layer_fed_by_node_of_name(layer, document_network, "Shape") {
		responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Path });
	}
}

/// Shifts the viewport when the mouse reaches the edge of the viewport.
///
/// If the mouse was beyond any edge, it returns the amount shifted. Otherwise it returns None.
/// The shift is proportional to the distance between edge and mouse. It is also guaranteed to be integral.
fn shift_viewport_if_mouse_beyond_edge(mouse_position: DVec2, viewport_size: DVec2, responses: &mut VecDeque<Message>) -> Option<DVec2> {
	let mouse_position = mouse_position.clamp(
		DVec2::ZERO - DVec2::splat(DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS),
		viewport_size + DVec2::splat(DRAG_BEYOND_VIEWPORT_MAX_OVEREXTENSION_PIXELS),
	);
	let mouse_position_percent = mouse_position / viewport_size;

	let mut shift_percent = DVec2::ZERO;

	if mouse_position_percent.x < 0. {
		shift_percent.x = -mouse_position_percent.x;
	} else if mouse_position_percent.x > 1. {
		shift_percent.x = 1. - mouse_position_percent.x;
	}

	if mouse_position_percent.y < 0. {
		shift_percent.y = -mouse_position_percent.y;
	} else if mouse_position_percent.y > 1. {
		shift_percent.y = 1. - mouse_position_percent.y;
	}

	if shift_percent.x == 0. && shift_percent.y == 0. {
		return None;
	}

	let delta = (shift_percent * DRAG_BEYOND_VIEWPORT_SPEED_FACTOR * viewport_size).round();
	responses.add(NavigationMessage::TranslateCanvas { delta });
	Some(delta)
}

fn setup_pointer_outside_edge_event(mouse_position: DVec2, viewport_size: DVec2, tool_data: &mut SelectToolData, modifier_keys: SelectToolPointerKeys, responses: &mut VecDeque<Message>) {
	let is_pointer_outside_edge = mouse_position.x < 0. || mouse_position.x > viewport_size.x || mouse_position.y < 0. || mouse_position.y > viewport_size.y;

	match is_pointer_outside_edge {
		true => subscribe_animation_frame(tool_data, modifier_keys, responses),
		false => unsubscribe_animation_frame(tool_data, modifier_keys, responses),
	}
}

fn subscribe_animation_frame(tool_data: &mut SelectToolData, modifier_keys: SelectToolPointerKeys, responses: &mut VecDeque<Message>) {
	if !tool_data.subscribed_to_animation_frame {
		tool_data.subscribed_to_animation_frame = true;

		responses.add(BroadcastMessage::SubscribeEvent {
			on: BroadcastEvent::AnimationFrame,
			send: Box::new(SelectToolMessage::PointerOutsideViewport(modifier_keys).into()),
		});
	}
}

fn unsubscribe_animation_frame(tool_data: &mut SelectToolData, modifier_keys: SelectToolPointerKeys, responses: &mut VecDeque<Message>) {
	if tool_data.subscribed_to_animation_frame {
		tool_data.subscribed_to_animation_frame = false;

		responses.add(BroadcastMessage::UnsubscribeEvent {
			on: BroadcastEvent::AnimationFrame,
			message: Box::new(SelectToolMessage::PointerOutsideViewport(modifier_keys).into()),
		});
	}
}
