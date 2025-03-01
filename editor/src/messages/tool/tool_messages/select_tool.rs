#![allow(clippy::too_many_arguments)]

use super::tool_prelude::*;
use crate::consts::{
	COLOR_OVERLAY_BLUE, COLOR_OVERLAY_GREEN, COLOR_OVERLAY_RED, COMPASS_ROSE_HOVER_RING_DIAMETER, DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD, RESIZE_HANDLE_SIZE, ROTATE_INCREMENT,
	SELECTION_DRAG_ANGLE, SELECTION_TOLERANCE,
};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{AlignAggregate, AlignAxis, FlipAxis, GroupFolderType};
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, NodeNetworkInterface, NodeTemplate};
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::preferences::SelectionMode;
use crate::messages::tool::common_functionality::compass_rose::{Axis, CompassRose};
use crate::messages::tool::common_functionality::graph_modification_utils::{get_text, is_layer_fed_by_node_of_name};
use crate::messages::tool::common_functionality::pivot::Pivot;
use crate::messages::tool::common_functionality::shape_editor::SelectionShapeType;
use crate::messages::tool::common_functionality::snapping::{self, SnapCandidatePoint, SnapData, SnapManager};
use crate::messages::tool::common_functionality::transformation_cage::*;
use crate::messages::tool::common_functionality::{auto_panning::AutoPanning, measure};

use bezier_rs::Subpath;
use graph_craft::document::NodeId;
use graphene_core::renderer::Quad;
use graphene_core::text::load_face;
use graphene_std::renderer::Rect;
use graphene_std::vector::misc::BooleanOperation;

use glam::DMat2;
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

#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SelectOptionsUpdate {
	NestedSelectionBehavior(NestedSelectionBehavior),
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
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

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SelectToolPointerKeys {
	pub axis_align: Key,
	pub snap_angle: Key,
	pub center: Key,
	pub duplicate: Key,
}

#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SelectToolMessage {
	// Standard messages
	Abort,
	Overlays(OverlayContext),

	// Tool-specific messages
	DragStart {
		extend_selection: Key,
		remove_from_selection: Key,
		select_deepest: Key,
		lasso_select: Key,
		skew: Key,
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
				MenuListEntry::new(format!("{mode:?}"))
					.label(mode.to_string())
					.on_commit(move |_| SelectToolMessage::SelectOptions(SelectOptionsUpdate::NestedSelectionBehavior(*mode)).into())
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

	fn turn_widgets(&self, disabled: bool) -> impl Iterator<Item = WidgetHolder> {
		[(-90., "TurnNegative90", "Turn -90°"), (90., "TurnPositive90", "Turn 90°")]
			.into_iter()
			.map(move |(degrees, icon, name)| {
				IconButton::new(icon, 24)
					.tooltip(name)
					.on_update(move |_| DocumentMessage::RotateSelectedLayers { degrees }.into())
					.disabled(disabled)
					.widget_holder()
			})
	}

	fn boolean_widgets(&self, selected_count: usize) -> impl Iterator<Item = WidgetHolder> {
		let operations = BooleanOperation::list();
		let icons = BooleanOperation::icons();
		operations.into_iter().zip(icons).map(move |(operation, icon)| {
			IconButton::new(icon, 24)
				.tooltip(operation.to_string())
				.disabled(selected_count == 0)
				.on_update(move |_| {
					let group_folder_type = GroupFolderType::BooleanOperation(operation);
					DocumentMessage::GroupSelectedLayers { group_folder_type }.into()
				})
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
		// widgets.push(
		// 	PopoverButton::new()
		// 		.popover_layout(vec![
		// 			LayoutGroup::Row {
		// 				widgets: vec![TextLabel::new("Align").bold(true).widget_holder()],
		// 			},
		// 			LayoutGroup::Row {
		// 				widgets: vec![TextLabel::new("Coming soon").widget_holder()],
		// 			},
		// 		])
		// 		.disabled(disabled)
		// 		.widget_holder(),
		// );

		// Flip
		let disabled = self.tool_data.selected_layers_count == 0;
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.flip_widgets(disabled));

		// Turn
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.turn_widgets(disabled));

		// Boolean
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.extend(self.boolean_widgets(self.tool_data.selected_layers_count));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for SelectTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Select(SelectToolMessage::SelectOptions(SelectOptionsUpdate::NestedSelectionBehavior(nested_selection_behavior))) = message {
			self.tool_data.nested_selection_behavior = nested_selection_behavior;
			responses.add(ToolMessage::UpdateHints);
		}

		if matches!(message, ToolMessage::Select(SelectToolMessage::PointerMove(_))) && !self.tool_data.has_dragged {
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
		let mut common = actions!(SelectToolMessageDiscriminant;
			PointerMove,
			Abort,
			EditLayer,
			Enter,
		);

		let additional = match self.fsm_state {
			SelectToolFsmState::Ready { .. } => actions!(SelectToolMessageDiscriminant; DragStart),
			_ => actions!(SelectToolMessageDiscriminant; DragStop),
		};
		common.extend(additional);

		common
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SelectToolFsmState {
	Ready { selection: NestedSelectionBehavior },
	Drawing { selection_shape: SelectionShapeType },
	Dragging { axis: Axis, using_compass: bool },
	ResizingBounds,
	SkewingBounds { skew: Key },
	RotatingBounds,
	DraggingPivot,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		let selection = NestedSelectionBehavior::Deepest;
		SelectToolFsmState::Ready { selection }
	}
}

#[derive(Clone, Debug, Default)]
struct SelectToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	lasso_polygon: Vec<ViewportPosition>,
	selection_mode: Option<SelectionMode>,
	layers_dragging: Vec<LayerNodeIdentifier>,
	layer_selected_on_start: Option<LayerNodeIdentifier>,
	select_single_layer: Option<LayerNodeIdentifier>,
	has_dragged: bool,
	axis_align: bool,
	non_duplicated_layers: Option<Vec<LayerNodeIdentifier>>,
	bounding_box_manager: Option<BoundingBoxManager>,
	snap_manager: SnapManager,
	cursor: MouseCursorIcon,
	pivot: Pivot,
	compass_rose: CompassRose,
	skew_edge: EdgeBool,
	nested_selection_behavior: NestedSelectionBehavior,
	selected_layers_count: usize,
	selected_layers_changed: bool,
	snap_candidates: Vec<SnapCandidatePoint>,
	auto_panning: AutoPanning,
}

impl SelectToolData {
	fn get_snap_candidates(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {
		self.snap_candidates.clear();
		for &layer in &self.layers_dragging {
			if (self.snap_candidates.len() as f64) < document.snapping_state.tolerance {
				snapping::get_layer_snap_points(layer, &SnapData::new(document, input), &mut self.snap_candidates);
			}
			if let Some(bounds) = document.metadata().bounding_box_with_transform(layer, DAffine2::IDENTITY) {
				let quad = document.metadata().transform_to_document(layer) * Quad::from_box(bounds);
				snapping::get_bbox_points(quad, &mut self.snap_candidates, snapping::BBoxSnapValues::BOUNDING_BOX, document);
			}
		}
	}

	pub fn selection_quad(&self) -> Quad {
		let bbox = self.selection_box();
		Quad::from_box(bbox)
	}

	pub fn calculate_selection_mode_from_direction(&mut self) -> SelectionMode {
		let bbox: [DVec2; 2] = self.selection_box();
		let above_threshold = bbox[1].distance_squared(bbox[0]) > DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD.powi(2);

		if self.selection_mode.is_none() && above_threshold {
			let mode = if bbox[1].x < bbox[0].x {
				SelectionMode::Touched
			} else {
				// This also covers the case where they're equal: the area is zero, so we use `Enclosed` to ensure the selection ends up empty, as nothing will be enclosed by an empty area
				SelectionMode::Enclosed
			};
			self.selection_mode = Some(mode);
		}

		self.selection_mode.unwrap_or(SelectionMode::Touched)
	}

	pub fn selection_box(&self) -> [DVec2; 2] {
		if self.drag_current == self.drag_start {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[self.drag_start - tolerance, self.drag_start + tolerance]
		} else {
			[self.drag_start, self.drag_current]
		}
	}

	pub fn intersect_lasso_no_artboards(&self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) -> Vec<LayerNodeIdentifier> {
		if self.lasso_polygon.len() < 2 {
			return Vec::new();
		}
		let polygon = Subpath::from_anchors_linear(self.lasso_polygon.clone(), true);
		document.intersect_polygon_no_artboards(polygon, input).collect()
	}

	pub fn is_layer_inside_lasso_polygon(&self, layer: &LayerNodeIdentifier, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) -> bool {
		if self.lasso_polygon.len() < 2 {
			return false;
		}
		let polygon = Subpath::from_anchors_linear(self.lasso_polygon.clone(), true);
		document.is_layer_fully_inside_polygon(layer, input, polygon)
	}

	/// Duplicates the currently dragging layers. Called when Alt is pressed and the layers have not yet been duplicated.
	fn start_duplicates(&mut self, document: &mut DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.non_duplicated_layers = Some(self.layers_dragging.clone());
		let mut new_dragging = Vec::new();

		// Get the shallowest unique layers and sort by their index relative to parent for ordered processing
		let mut layers = document.network_interface.shallowest_unique_layers(&[]).collect::<Vec<_>>();

		layers.sort_by_key(|layer| {
			let Some(parent) = layer.parent(document.metadata()) else { return usize::MAX };
			DocumentMessageHandler::get_calculated_insert_index(document.metadata(), &SelectedNodes(vec![layer.to_node()]), parent)
		});

		for layer in layers.into_iter().rev() {
			let Some(parent) = layer.parent(document.metadata()) else { continue };

			// Moves the layer back to its starting position.
			responses.add(GraphOperationMessage::TransformChange {
				layer,
				transform: DAffine2::from_translation(self.drag_start - self.drag_current),
				transform_in: TransformIn::Viewport,
				skip_rerender: true,
			});

			// Copy the layer
			let mut copy_ids = HashMap::new();
			let node_id = layer.to_node();
			copy_ids.insert(node_id, NodeId(0));

			document
				.network_interface
				.upstream_flow_back_from_nodes(vec![layer.to_node()], &[], FlowType::LayerChildrenUpstreamFlow)
				.enumerate()
				.for_each(|(index, node_id)| {
					copy_ids.insert(node_id, NodeId((index + 1) as u64));
				});

			let nodes = document.network_interface.copy_nodes(&copy_ids, &[]).collect::<Vec<(NodeId, NodeTemplate)>>();

			let insert_index = DocumentMessageHandler::get_calculated_insert_index(document.metadata(), &SelectedNodes(vec![layer.to_node()]), parent);

			let new_ids: HashMap<_, _> = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();

			let layer_id = *new_ids.get(&NodeId(0)).expect("Node Id 0 should be a layer");
			let layer = LayerNodeIdentifier::new_unchecked(layer_id);
			new_dragging.push(layer);
			responses.add(NodeGraphMessage::AddNodes { nodes, new_ids });
			responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index });
		}
		let nodes = new_dragging.iter().map(|layer| layer.to_node()).collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		responses.add(NodeGraphMessage::RunDocumentGraph);
		self.layers_dragging = new_dragging;
	}

	/// Removes the duplicated layers. Called when Alt is released and the layers have previously been duplicated.
	fn stop_duplicates(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(original) = self.non_duplicated_layers.take() else {
			return;
		};

		// Delete the duplicated layers
		for layer in document.network_interface.shallowest_unique_layers(&[]) {
			responses.add(NodeGraphMessage::DeleteNodes {
				node_ids: vec![layer.to_node()],
				delete_children: true,
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
		let nodes = original
			.iter()
			.filter_map(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					Some(layer.to_node())
				} else {
					log::error!("ROOT_PARENT cannot be part of non_duplicated_layers");
					None
				}
			})
			.collect();
		responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
		responses.add(NodeGraphMessage::RunDocumentGraph);
		responses.add(NodeGraphMessage::SelectedNodesUpdated);
		responses.add(NodeGraphMessage::SendGraph);
		self.layers_dragging = original;
	}
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData { document, input, font_cache, .. } = tool_action_data;

		let ToolMessage::Select(event) = event else { return self };
		match (self, event) {
			(_, SelectToolMessage::Overlays(mut overlay_context)) => {
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

				let selected_layers_count = document.network_interface.selected_nodes(&[]).unwrap().selected_unlocked_layers(&document.network_interface).count();
				tool_data.selected_layers_changed = selected_layers_count != tool_data.selected_layers_count;
				tool_data.selected_layers_count = selected_layers_count;

				// Outline selected layers, but not artboards
				for layer in document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
				{
					overlay_context.outline(document.metadata().layer_outline(layer), document.metadata().transform_to_viewport(layer));

					if is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text") {
						let (text, font, typesetting) = get_text(layer, &document.network_interface).expect("Text layer should have text when interacting with the Text tool in `interact()`");

						let buzz_face = font_cache.get(font).map(|data| load_face(data));
						let far = graphene_core::text::bounding_box(text, buzz_face, typesetting);
						let quad = Quad::from_box([DVec2::ZERO, far]);
						let transformed_quad = document.metadata().transform_to_viewport(layer) * quad;

						overlay_context.dashed_quad(transformed_quad, None, Some(4.), Some(4.), Some(0.5));
					}
				}

				// Update bounds
				let transform = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
					.map(|layer| document.metadata().transform_to_viewport(layer));

				let mut transform = transform.unwrap_or(DAffine2::IDENTITY);
				let mut transform_tampered = false;
				// Check if the matrix is not invertible
				if transform.matrix2.determinant() == 0. {
					transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
					transform_tampered = true;
				}

				let bounds = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
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
					bounding_box_manager.transform_tampered = transform_tampered;

					bounding_box_manager.render_overlays(&mut overlay_context);
				} else {
					tool_data.bounding_box_manager.take();
				}

				let angle = bounds
					.map(|bounds| transform * Quad::from_box(bounds))
					.map_or(0., |quad| (quad.top_left() - quad.top_right()).to_angle());

				let mouse_position = input.mouse.position;
				let compass_rose_state = tool_data.compass_rose.compass_rose_state(mouse_position, angle);

				let show_hover_ring = if let SelectToolFsmState::Dragging { axis, using_compass } = self {
					using_compass && !axis.is_constraint()
				} else {
					compass_rose_state.is_ring()
				};

				let dragging_bounds = tool_data
					.bounding_box_manager
					.as_mut()
					.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
					.is_some();

				let rotating_bounds = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box| bounding_box.check_rotate(input.mouse.position))
					.unwrap_or_default();

				let is_resizing_or_rotating = matches!(self, SelectToolFsmState::ResizingBounds | SelectToolFsmState::SkewingBounds { .. } | SelectToolFsmState::RotatingBounds);

				if let Some(bounds) = tool_data.bounding_box_manager.as_mut() {
					let edges = bounds.check_selected_edges(input.mouse.position);
					let is_skewing = matches!(self, SelectToolFsmState::SkewingBounds { .. });
					let is_near_square = edges.is_some_and(|hover_edge| bounds.over_extended_edge_midpoint(input.mouse.position, hover_edge));
					if is_skewing || (dragging_bounds && is_near_square && !is_resizing_or_rotating) {
						bounds.render_skew_gizmos(&mut overlay_context, tool_data.skew_edge);
					}
					if !is_skewing && dragging_bounds {
						if let Some(edges) = edges {
							tool_data.skew_edge = bounds.get_closest_edge(edges, input.mouse.position);
						}
					}
				}

				let might_resize_or_rotate = dragging_bounds || rotating_bounds;
				let can_get_into_other_states = might_resize_or_rotate && !matches!(self, SelectToolFsmState::Dragging { .. });

				let show_compass = !(can_get_into_other_states || is_resizing_or_rotating);
				let show_compass_with_ring = bounds.map(|bounds| transform * Quad::from_box(bounds)).and_then(|quad| {
					const MIN_ARROWS_TO_RESIZE_HANDLE_DISTANCE: f64 = 4.;
					(show_compass && quad.all_sides_at_least_width(COMPASS_ROSE_HOVER_RING_DIAMETER + RESIZE_HANDLE_SIZE + MIN_ARROWS_TO_RESIZE_HANDLE_DISTANCE))
						.then_some(
							matches!(self, SelectToolFsmState::Dragging { .. })
								.then_some(show_hover_ring)
								.or((quad.contains(mouse_position)).then_some(show_hover_ring)),
						)
						.flatten()
				});

				// Update pivot
				tool_data.pivot.update_pivot(document, &mut overlay_context, angle);

				// Update compass rose
				tool_data.compass_rose.refresh_position(document);
				let compass_center = tool_data.compass_rose.compass_rose_position();
				overlay_context.compass_rose(compass_center, angle, show_compass_with_ring);

				let axis_state = if let SelectToolFsmState::Dragging { axis, .. } = self {
					Some((axis, false))
				} else {
					compass_rose_state.axis_type().and_then(|axis| axis.is_constraint().then_some((axis, true)))
				};

				if show_compass_with_ring.is_some() {
					if let Some((axis, hover)) = axis_state {
						if axis.is_constraint() {
							let e0 = tool_data
								.bounding_box_manager
								.as_ref()
								.map(|bounding_box_manager| bounding_box_manager.transform * Quad::from_box(bounding_box_manager.bounds))
								.map_or(DVec2::X, |quad| (quad.top_left() - quad.top_right()).normalize_or(DVec2::X));

							let (direction, color) = match axis {
								Axis::X => (e0, COLOR_OVERLAY_RED),
								Axis::Y => (e0.perp(), COLOR_OVERLAY_GREEN),
								_ => unreachable!(),
							};

							let viewport_diagonal = input.viewport_bounds.size().length();

							let color = if !hover {
								color
							} else {
								let color_string = &graphene_std::Color::from_rgb_str(color.strip_prefix('#').unwrap()).unwrap().with_alpha(0.25).rgba_hex();
								&format!("#{}", color_string)
							};
							overlay_context.line(compass_center - direction * viewport_diagonal, compass_center + direction * viewport_diagonal, Some(color));
						}
					}
				}

				if axis_state.is_none_or(|(axis, _)| !axis.is_constraint()) && tool_data.axis_align {
					let mouse_position = mouse_position - tool_data.drag_start;
					let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
					let angle = -mouse_position.angle_to(DVec2::X);
					let snapped_angle = (angle / snap_resolution).round() * snap_resolution;

					let mut other = graphene_std::Color::from_rgb_str(COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap()).unwrap().with_alpha(0.25).rgba_hex();
					other.insert(0, '#');
					let other = other.as_str();

					let extension = tool_data.drag_current - tool_data.drag_start;
					let origin = compass_center - extension;
					let viewport_diagonal = input.viewport_bounds.size().length();

					let edge = DVec2::from_angle(snapped_angle) * viewport_diagonal;
					let perp = edge.perp();

					overlay_context.line(origin - edge * viewport_diagonal, origin + edge * viewport_diagonal, Some(COLOR_OVERLAY_BLUE));
					overlay_context.line(origin - perp * viewport_diagonal, origin + perp * viewport_diagonal, Some(other));
				}

				// Check if the tool is in selection mode
				if let Self::Drawing { selection_shape } = self {
					// Get the updated selection box bounds
					let quad = Quad::from_box([tool_data.drag_start, tool_data.drag_current]);

					let selection_mode = match tool_action_data.preferences.get_selection_mode() {
						SelectionMode::Directional => tool_data.calculate_selection_mode_from_direction(),
						selection_mode => selection_mode,
					};

					// Draw outline visualizations on the layers to be selected
					let mut draw_layer_outline = |layer| overlay_context.outline(document.metadata().layer_outline(layer), document.metadata().transform_to_viewport(layer));
					let intersection: Vec<LayerNodeIdentifier> = match selection_shape {
						SelectionShapeType::Box => document.intersect_quad_no_artboards(quad, input).collect(),
						SelectionShapeType::Lasso => tool_data.intersect_lasso_no_artboards(document, input),
					};
					if selection_mode == SelectionMode::Enclosed {
						let is_inside = |layer: &LayerNodeIdentifier| match selection_shape {
							SelectionShapeType::Box => document.is_layer_fully_inside(layer, quad),
							SelectionShapeType::Lasso => tool_data.is_layer_inside_lasso_polygon(layer, document, input),
						};
						for layer in intersection.into_iter().filter(is_inside) {
							draw_layer_outline(layer);
						}
					} else {
						for layer in intersection {
							draw_layer_outline(layer);
						}
					}

					// Update the selection box
					let mut fill_color = graphene_std::Color::from_rgb_str(crate::consts::COLOR_OVERLAY_BLUE.strip_prefix('#').unwrap())
						.unwrap()
						.with_alpha(0.05)
						.rgba_hex();
					fill_color.insert(0, '#');
					let fill_color = Some(fill_color.as_str());

					let polygon = &tool_data.lasso_polygon;

					match (selection_shape, selection_mode) {
						(SelectionShapeType::Box, SelectionMode::Enclosed) => overlay_context.dashed_quad(quad, fill_color, Some(4.), Some(4.), Some(0.5)),
						(SelectionShapeType::Lasso, SelectionMode::Enclosed) => overlay_context.dashed_polygon(polygon, fill_color, Some(4.), Some(4.), Some(0.5)),
						(SelectionShapeType::Box, _) => overlay_context.quad(quad, fill_color),
						(SelectionShapeType::Lasso, _) => overlay_context.polygon(polygon, fill_color),
					}
				}
				// Only highlight layers if the viewport is not being panned (middle mouse button is pressed)
				// TODO: Don't use `Key::Mmb` directly, instead take it as a variable from the input mappings list like in all other places
				else if !input.keyboard.get(Key::MouseMiddle as usize) {
					// Get the layer the user is hovering over
					let click = document.click(input);
					let not_selected_click = click.filter(|&hovered_layer| !document.network_interface.selected_nodes(&[]).unwrap().selected_layers_contains(hovered_layer, document.metadata()));
					if let Some(layer) = not_selected_click {
						overlay_context.outline(document.metadata().layer_outline(layer), document.metadata().transform_to_viewport(layer));

						// Measure with Alt held down
						// TODO: Don't use `Key::Alt` directly, instead take it as a variable from the input mappings list like in all other places
						if !matches!(self, Self::ResizingBounds { .. }) && input.keyboard.get(Key::Alt as usize) {
							let hovered_bounds = document
								.metadata()
								.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer));

							if let [Some(selected_bounds), Some(hovered_bounds)] = [bounds, hovered_bounds].map(|rect| rect.map(Rect::from_box)) {
								measure::overlay(selected_bounds, hovered_bounds, transform, document.metadata().document_to_viewport, &mut overlay_context);
							}
						}
					}
				}

				self
			}
			(_, SelectToolMessage::EditLayer) => {
				// Edit the clicked layer
				if let Some(intersect) = document.click(input) {
					match tool_data.nested_selection_behavior {
						NestedSelectionBehavior::Shallowest => edit_layer_shallowest_manipulation(document, intersect, responses),
						NestedSelectionBehavior::Deepest => edit_layer_deepest_manipulation(intersect, &document.network_interface, responses),
					}
				}

				self
			}
			(
				SelectToolFsmState::Ready { .. },
				SelectToolMessage::DragStart {
					extend_selection,
					remove_from_selection,
					select_deepest,
					lasso_select,
					skew,
				},
			) => {
				tool_data.drag_start = input.mouse.position;
				tool_data.drag_current = input.mouse.position;
				tool_data.selection_mode = None;

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

				let mut selected: Vec<_> = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.collect();
				let intersection_list = document.click_list(input).collect::<Vec<_>>();
				let intersection = document.find_deepest(&intersection_list);

				// If the user is dragging the bounding box bounds, go into ResizingBounds mode.
				// If the user is dragging the rotate trigger, go into RotatingBounds mode.
				// If the user clicks on a layer that is in their current selection, go into the dragging mode.
				// If the user clicks on new shape, make that layer their new selection.
				// Otherwise enter the box select mode
				let bounds = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box_manager| bounding_box_manager.transform * Quad::from_box(bounding_box_manager.bounds));

				let angle = bounds.map_or(0., |quad| (quad.top_left() - quad.top_right()).to_angle());
				let mouse_position = input.mouse.position;
				let compass_rose_state = tool_data.compass_rose.compass_rose_state(mouse_position, angle);
				let is_over_pivot = tool_data.pivot.is_over(mouse_position);

				let show_compass = bounds.is_some_and(|quad| quad.all_sides_at_least_width(COMPASS_ROSE_HOVER_RING_DIAMETER) && quad.contains(mouse_position));
				let can_grab_compass_rose = compass_rose_state.can_grab() && show_compass;
				let is_flat_layer = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box_manager| bounding_box_manager.transform_tampered)
					.unwrap_or(true);

				let state =
				// Dragging the pivot
				if is_over_pivot {
					responses.add(DocumentMessage::StartTransaction);

					// tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(), true, true);
					// tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);

					SelectToolFsmState::DraggingPivot
				}
				// Dragging one (or two, forming a corner) of the transform cage bounding box edges
				else if dragging_bounds.is_some() && !is_flat_layer {
					responses.add(DocumentMessage::StartTransaction);

					tool_data.layers_dragging = selected;

					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						bounds.original_bound_transform = bounds.transform;

						tool_data.layers_dragging.retain(|layer| {
							if *layer != LayerNodeIdentifier::ROOT_PARENT {
								document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
							} else {
								log::error!("ROOT_PARENT should not be part of layers_dragging");
								false
							}
						});

						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut bounds.center_of_transformation,
							&tool_data.layers_dragging,
							responses,
							&document.network_interface,
							None,
							&ToolType::Select,
							None
						);
						bounds.center_of_transformation = selected.mean_average_of_pivots();

						// Check if we're hovering over a skew triangle
						let edges = bounds.check_selected_edges(input.mouse.position);
						if let Some(edges) = edges {
							let closest_edge = bounds.get_closest_edge(edges, input.mouse.position);
							if bounds.check_skew_handle(input.mouse.position, closest_edge) {
								tool_data.get_snap_candidates(document, input);
								return SelectToolFsmState::SkewingBounds { skew };
							}
						}
					}
					tool_data.get_snap_candidates(document, input);
					SelectToolFsmState::ResizingBounds
				}
				// Dragging the selected layers around to transform them
				else if can_grab_compass_rose || intersection.is_some_and(|intersection| selected.iter().any(|selected_layer| intersection.starts_with(*selected_layer, document.metadata()))) {
					responses.add(DocumentMessage::StartTransaction);

					if input.keyboard.key(select_deepest) || tool_data.nested_selection_behavior == NestedSelectionBehavior::Deepest {
						tool_data.select_single_layer = intersection;
					} else {
						tool_data.select_single_layer = intersection.and_then(|intersection| intersection.ancestors(document.metadata()).find(|ancestor| selected.contains(ancestor)));
					}

					tool_data.layers_dragging = selected;

					tool_data.get_snap_candidates(document, input);
					let (axis, using_compass) = {
						let axis_state = compass_rose_state.axis_type().filter(|_| can_grab_compass_rose);
						(axis_state.unwrap_or_default(), axis_state.is_some())
					};
					SelectToolFsmState::Dragging { axis, using_compass }
				}
				// Dragging near the transform cage bounding box to rotate it
				else if rotating_bounds {
					responses.add(DocumentMessage::StartTransaction);

					if let Some(bounds) = &mut tool_data.bounding_box_manager {
						tool_data.layers_dragging.retain(|layer| {
							if *layer != LayerNodeIdentifier::ROOT_PARENT {
								document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
							} else {
								log::error!("ROOT_PARENT should not be part of layers_dragging");
								false
							}
						});
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut bounds.center_of_transformation,
							&selected,
							responses,
							&document.network_interface,
							None,
							&ToolType::Select,
							None
						);

						bounds.center_of_transformation = selected.mean_average_of_pivots();
					}

					tool_data.layers_dragging = selected;

					SelectToolFsmState::RotatingBounds
				}
				// Dragging a selection box
				else {
					tool_data.layers_dragging = selected;
					if !input.keyboard.key(extend_selection) && !input.keyboard.key(remove_from_selection) {
						responses.add(DocumentMessage::DeselectAllLayers);
						tool_data.layers_dragging.clear();
					}

					if let Some(intersection) = intersection {
						tool_data.layer_selected_on_start = Some(intersection);
						selected = intersection_list;

						match tool_data.nested_selection_behavior {
							NestedSelectionBehavior::Shallowest if !input.keyboard.key(select_deepest) => drag_shallowest_manipulation(responses, selected, tool_data, document),
							_ => drag_deepest_manipulation(responses, selected, tool_data, document),
						}
						tool_data.get_snap_candidates(document, input);

						responses.add(DocumentMessage::StartTransaction);
						SelectToolFsmState::Dragging { axis: Axis::None, using_compass: false }
					} else {
						let selection_shape = if input.keyboard.key(lasso_select) { SelectionShapeType::Lasso } else { SelectionShapeType::Box };
						SelectToolFsmState::Drawing { selection_shape }
					}
				};
				tool_data.non_duplicated_layers = None;

				state
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Dragging { axis, using_compass }, SelectToolMessage::PointerMove(modifier_keys)) => {
				tool_data.has_dragged = true;

				if input.keyboard.key(modifier_keys.duplicate) && tool_data.non_duplicated_layers.is_none() {
					tool_data.start_duplicates(document, responses);
				} else if !input.keyboard.key(modifier_keys.duplicate) && tool_data.non_duplicated_layers.is_some() {
					tool_data.stop_duplicates(document, responses);
				}

				tool_data.axis_align = input.keyboard.key(modifier_keys.axis_align) && !axis.is_constraint();

				// Ignore the non duplicated layers if the current layers have not spawned yet.
				let layers_exist = tool_data.layers_dragging.iter().all(|&layer| document.metadata().click_targets(layer).is_some());
				let ignore = tool_data.non_duplicated_layers.as_ref().filter(|_| !layers_exist).unwrap_or(&tool_data.layers_dragging);

				let snap_data = SnapData::ignore(document, input, ignore);
				let (start, current) = (tool_data.drag_start, tool_data.drag_current);
				let mouse_delta = snap_drag(start, current, tool_data.axis_align, snap_data, &mut tool_data.snap_manager, &tool_data.snap_candidates);
				let e0 = tool_data
					.bounding_box_manager
					.as_ref()
					.map(|bounding_box_manager| bounding_box_manager.transform * Quad::from_box(bounding_box_manager.bounds))
					.map_or(DVec2::X, |quad| (quad.top_left() - quad.top_right()).normalize_or(DVec2::X));
				let mouse_delta = match axis {
					Axis::X => mouse_delta.project_onto(e0),
					Axis::Y => mouse_delta.project_onto(e0.perp()),
					Axis::None => mouse_delta,
				};

				// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
				for layer in document.network_interface.shallowest_unique_layers(&[]) {
					responses.add_front(GraphOperationMessage::TransformChange {
						layer,
						transform: DAffine2::from_translation(mouse_delta),
						transform_in: TransformIn::Viewport,
						skip_rerender: false,
					});
				}
				tool_data.drag_current += mouse_delta;

				// AutoPanning
				let messages = [
					SelectToolMessage::PointerOutsideViewport(modifier_keys.clone()).into(),
					SelectToolMessage::PointerMove(modifier_keys).into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				SelectToolFsmState::Dragging { axis, using_compass }
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

						tool_data.layers_dragging.retain(|layer| {
							if *layer != LayerNodeIdentifier::ROOT_PARENT {
								document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
							} else {
								log::error!("ROOT_PARENT should not be part of layers_dragging");
								false
							}
						});
						let selected = &tool_data.layers_dragging;
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut pivot,
							selected,
							responses,
							&document.network_interface,
							None,
							&ToolType::Select,
							None,
						);

						selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse(), None);

						// AutoPanning
						let messages = [
							SelectToolMessage::PointerOutsideViewport(modifier_keys.clone()).into(),
							SelectToolMessage::PointerMove(modifier_keys).into(),
						];
						tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);
					}
				}
				SelectToolFsmState::ResizingBounds
			}
			(SelectToolFsmState::SkewingBounds { skew }, SelectToolMessage::PointerMove(_)) => {
				if let Some(ref mut bounds) = &mut tool_data.bounding_box_manager {
					if let Some(movement) = &mut bounds.selected_edges {
						let free_movement = input.keyboard.key(skew);
						let transformation = movement.skew_transform(input.mouse.position, bounds.original_bound_transform, free_movement);

						tool_data.layers_dragging.retain(|layer| {
							if *layer != LayerNodeIdentifier::ROOT_PARENT {
								document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
							} else {
								log::error!("ROOT_PARENT should not be part of layers_dragging");
								false
							}
						});
						let selected = &tool_data.layers_dragging;
						let mut pivot = DVec2::ZERO;
						let mut selected = Selected::new(
							&mut bounds.original_transforms,
							&mut pivot,
							selected,
							responses,
							&document.network_interface,
							None,
							&ToolType::Select,
							None,
						);

						selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse(), None);
					}
				}
				SelectToolFsmState::SkewingBounds { skew }
			}
			(SelectToolFsmState::RotatingBounds, SelectToolMessage::PointerMove(modifier_keys)) => {
				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					let angle = {
						let start_offset = tool_data.drag_start - bounds.center_of_transformation;
						let end_offset = input.mouse.position - bounds.center_of_transformation;

						start_offset.angle_to(end_offset)
					};

					let snapped_angle = if input.keyboard.key(modifier_keys.snap_angle) {
						let snap_resolution = ROTATE_INCREMENT.to_radians();
						(angle / snap_resolution).round() * snap_resolution
					} else {
						angle
					};

					let delta = DAffine2::from_angle(snapped_angle);

					tool_data.layers_dragging.retain(|layer| {
						if *layer != LayerNodeIdentifier::ROOT_PARENT {
							document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
						} else {
							log::error!("ROOT_PARENT should not be part of replacement_selected_layers");
							false
						}
					});
					let mut selected = Selected::new(
						&mut bounds.original_transforms,
						&mut bounds.center_of_transformation,
						&tool_data.layers_dragging,
						responses,
						&document.network_interface,
						None,
						&ToolType::Select,
						None,
					);

					selected.update_transforms(delta, None, None);
				}

				SelectToolFsmState::RotatingBounds
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::PointerMove(modifier_keys)) => {
				let mouse_position = input.mouse.position;
				let snapped_mouse_position = mouse_position;
				tool_data.pivot.set_viewport_position(snapped_mouse_position, document, responses);

				// AutoPanning
				let messages = [
					SelectToolMessage::PointerOutsideViewport(modifier_keys.clone()).into(),
					SelectToolMessage::PointerMove(modifier_keys).into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				SelectToolFsmState::DraggingPivot
			}
			(SelectToolFsmState::Drawing { selection_shape }, SelectToolMessage::PointerMove(modifier_keys)) => {
				tool_data.drag_current = input.mouse.position;
				responses.add(OverlaysMessage::Draw);

				if selection_shape == SelectionShapeType::Lasso {
					extend_lasso(&mut tool_data.lasso_polygon, tool_data.drag_current);
				}

				// AutoPanning
				let messages = [
					SelectToolMessage::PointerOutsideViewport(modifier_keys.clone()).into(),
					SelectToolMessage::PointerMove(modifier_keys).into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				SelectToolFsmState::Drawing { selection_shape }
			}
			(SelectToolFsmState::Ready { .. }, SelectToolMessage::PointerMove(_)) => {
				let dragging_bounds = tool_data
					.bounding_box_manager
					.as_mut()
					.and_then(|bounding_box| bounding_box.check_selected_edges(input.mouse.position))
					.is_some();

				let mut cursor = tool_data
					.bounding_box_manager
					.as_ref()
					.map_or(MouseCursorIcon::Default, |bounds| bounds.get_cursor(input, true, dragging_bounds, Some(tool_data.skew_edge)));

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

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Dragging { axis, using_compass }, SelectToolMessage::PointerOutsideViewport(_)) => {
				// AutoPanning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_current += shift;
					tool_data.drag_start += shift;
				}

				SelectToolFsmState::Dragging { axis, using_compass }
			}
			(SelectToolFsmState::ResizingBounds | SelectToolFsmState::SkewingBounds { .. }, SelectToolMessage::PointerOutsideViewport(_)) => {
				// AutoPanning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					if let Some(ref mut bounds) = &mut tool_data.bounding_box_manager {
						bounds.center_of_transformation += shift;
						bounds.original_bound_transform.translation += shift;
					}
				}

				self
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::PointerOutsideViewport(_)) => {
				// AutoPanning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				self
			}
			(SelectToolFsmState::Drawing { .. }, SelectToolMessage::PointerOutsideViewport(_)) => {
				// AutoPanning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, responses) {
					tool_data.drag_start += shift;
				}

				self
			}
			(state, SelectToolMessage::PointerOutsideViewport(modifier_keys)) => {
				// AutoPanning
				let messages = [
					SelectToolMessage::PointerOutsideViewport(modifier_keys.clone()).into(),
					SelectToolMessage::PointerMove(modifier_keys).into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(SelectToolFsmState::Dragging { .. }, SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::AbortTransaction,
					false => DocumentMessage::EndTransaction,
				};
				tool_data.axis_align = false;
				tool_data.snap_manager.cleanup(responses);
				responses.add_front(response);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Dragging { .. }, SelectToolMessage::DragStop { remove_from_selection }) => {
				// Deselect layer if not snap dragging
				responses.add(DocumentMessage::EndTransaction);
				tool_data.axis_align = false;

				if !tool_data.has_dragged && input.keyboard.key(remove_from_selection) && tool_data.layer_selected_on_start.is_none() {
					// When you click on the layer with remove from selection key (shift) pressed, we deselect all nodes that are children.
					let quad = tool_data.selection_quad();
					let intersection = document.intersect_quad_no_artboards(quad, input);

					if let Some(path) = intersection.last() {
						let replacement_selected_layers: Vec<_> = document
							.network_interface
							.selected_nodes(&[])
							.unwrap()
							.selected_layers(document.metadata())
							.filter(|&layer| !path.starts_with(layer, document.metadata()))
							.collect();

						tool_data.layers_dragging.clear();
						tool_data.layers_dragging.extend(replacement_selected_layers.iter());

						responses.add(NodeGraphMessage::SelectedNodesSet {
							nodes: replacement_selected_layers
								.iter()
								.filter_map(|layer| {
									if *layer != LayerNodeIdentifier::ROOT_PARENT {
										Some(layer.to_node())
									} else {
										log::error!("ROOT_PARENT cannot be part of replacement_selected_layers");
										None
									}
								})
								.collect(),
						});
					}
				} else if let Some(selecting_layer) = tool_data.select_single_layer.take() {
					// Previously, we may have had many layers selected. If the user clicks without dragging, we should just select the one layer that has been clicked.
					if !tool_data.has_dragged {
						if selecting_layer == LayerNodeIdentifier::ROOT_PARENT {
							log::error!("selecting_layer should not be ROOT_PARENT");
						} else {
							responses.add(NodeGraphMessage::SelectedNodesSet {
								nodes: vec![selecting_layer.to_node()],
							});
						}
					}
				}

				tool_data.has_dragged = false;
				tool_data.layer_selected_on_start = None;

				tool_data.snap_manager.cleanup(responses);
				tool_data.select_single_layer = None;

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::ResizingBounds | SelectToolFsmState::SkewingBounds { .. }, SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::AbortTransaction,
					false => DocumentMessage::EndTransaction,
				};
				responses.add(response);

				tool_data.snap_manager.cleanup(responses);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::RotatingBounds, SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::AbortTransaction,
					false => DocumentMessage::EndTransaction,
				};
				responses.add(response);

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::DraggingPivot, SelectToolMessage::DragStop { .. } | SelectToolMessage::Enter) => {
				let response = match input.mouse.position.distance(tool_data.drag_start) < 10. * f64::EPSILON {
					true => DocumentMessage::AbortTransaction,
					false => DocumentMessage::EndTransaction,
				};
				responses.add(response);

				tool_data.snap_manager.cleanup(responses);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Drawing { selection_shape }, SelectToolMessage::DragStop { remove_from_selection }) => {
				let quad = tool_data.selection_quad();

				let selection_mode = match tool_action_data.preferences.get_selection_mode() {
					SelectionMode::Directional => tool_data.calculate_selection_mode_from_direction(),
					selection_mode => selection_mode,
				};

				let intersection: Vec<LayerNodeIdentifier> = match selection_shape {
					SelectionShapeType::Box => document.intersect_quad_no_artboards(quad, input).collect(),
					SelectionShapeType::Lasso => tool_data.intersect_lasso_no_artboards(document, input),
				};
				let new_selected: HashSet<_> = if selection_mode == SelectionMode::Enclosed {
					let is_inside = |layer: &LayerNodeIdentifier| match selection_shape {
						SelectionShapeType::Box => document.is_layer_fully_inside(layer, quad),
						SelectionShapeType::Lasso => tool_data.is_layer_inside_lasso_polygon(layer, document, input),
					};
					intersection.into_iter().filter(is_inside).collect()
				} else {
					intersection.into_iter().collect()
				};

				let current_selected: HashSet<_> = document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()).collect();
				let negative_selection = input.keyboard.key(remove_from_selection);
				let selection_modified = new_selected != current_selected;
				// Negative selection when both Shift and Ctrl are pressed
				if negative_selection {
					let updated_selection = current_selected
						.into_iter()
						.filter(|layer| !new_selected.iter().any(|selected| layer.starts_with(*selected, document.metadata())))
						.collect();
					tool_data.layers_dragging = updated_selection;
				} else if selection_modified {
					let parent_selected: HashSet<_> = new_selected
						.into_iter()
						.map(|layer| {
							// Find the parent node
							layer.ancestors(document.metadata()).filter(not_artboard(document)).last().unwrap_or(layer)
						})
						.collect();
					tool_data.layers_dragging.extend(parent_selected.iter().copied());
				}

				if negative_selection || selection_modified {
					responses.add(NodeGraphMessage::SelectedNodesSet {
						nodes: tool_data
							.layers_dragging
							.iter()
							.filter_map(|layer| {
								if *layer != LayerNodeIdentifier::ROOT_PARENT {
									Some(layer.to_node())
								} else {
									log::error!("ROOT_PARENT cannot be part of tool_data.layers_dragging");
									None
								}
							})
							.collect(),
					});
				}

				tool_data.lasso_polygon.clear();

				responses.add(OverlaysMessage::Draw);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Ready { .. }, SelectToolMessage::Enter) => {
				let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
				let mut selected_layers = selected_nodes.selected_layers(document.metadata());

				if let Some(layer) = selected_layers.next() {
					// Check that only one layer is selected
					if selected_layers.next().is_none() && is_layer_fed_by_node_of_name(layer, &document.network_interface, "Text") {
						responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
						responses.add(TextToolMessage::EditSelected);
					}
				}

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(SelectToolFsmState::Dragging { .. }, SelectToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				tool_data.axis_align = false;
				responses.add(OverlaysMessage::Draw);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
			}
			(_, SelectToolMessage::Abort) => {
				tool_data.layers_dragging.retain(|layer| {
					if *layer != LayerNodeIdentifier::ROOT_PARENT {
						document.network_interface.network(&[]).unwrap().nodes.contains_key(&layer.to_node())
					} else {
						false
					}
				});

				if let Some(bounds) = &mut tool_data.bounding_box_manager {
					bounds.original_transforms.clear();
				}

				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				responses.add(OverlaysMessage::Draw);

				let selection = tool_data.nested_selection_behavior;
				SelectToolFsmState::Ready { selection }
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
				self.update_hints(responses, tool_data);
				true
			}
			ToolMessage::UpdateCursor => {
				self.update_cursor(responses);
				true
			}
			_ => false,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>, tool_data: &Self::ToolData) {
		match self {
			SelectToolFsmState::Ready { selection } => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Drag Selected")]),
					HintGroup({
						let mut hints = vec![HintInfo::mouse(MouseMotion::Lmb, "Select Object"), HintInfo::keys([Key::Shift], "Extend Selection").prepend_plus()];
						if *selection == NestedSelectionBehavior::Shallowest {
							hints.extend([HintInfo::keys([Key::Accel], "Deepest").prepend_plus(), HintInfo::mouse(MouseMotion::LmbDouble, "Deepen Selection")]);
						}
						hints
					}),
					HintGroup(vec![
						HintInfo::mouse(MouseMotion::LmbDrag, "Select Area"),
						HintInfo::keys([Key::Shift], "Extend").prepend_plus(),
						HintInfo::keys([Key::Alt], "Subtract").prepend_plus(),
						HintInfo::keys([Key::Control], "Lasso").prepend_plus(),
					]),
					HintGroup(vec![HintInfo::multi_keys([[Key::KeyG], [Key::KeyR], [Key::KeyS]], "Grab/Rotate/Scale Selected")]),
					HintGroup(vec![
						HintInfo::arrow_keys("Nudge Selected"),
						HintInfo::keys([Key::Shift], "10x").prepend_plus(),
						HintInfo::keys([Key::Alt], "Resize Corner").prepend_plus(),
						HintInfo::keys([Key::Control], "Other Corner").prepend_plus(),
					]),
					HintGroup(vec![
						HintInfo::keys_and_mouse([Key::Alt], MouseMotion::LmbDrag, "Move Duplicate"),
						HintInfo::keys([Key::Control, Key::KeyD], "Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
					]),
				]);
				responses.add(FrontendMessage::UpdateInputHints { hint_data });
			}
			SelectToolFsmState::Dragging { axis, using_compass } if tool_data.has_dragged => {
				let mut hint_data = vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![
						HintInfo::keys([Key::Alt], "Move Duplicate"),
						HintInfo::keys([Key::Control, Key::KeyD], "Place Duplicate").add_mac_keys([Key::Command, Key::KeyD]),
					]),
				];

				if !(*using_compass && axis.is_constraint()) {
					hint_data.push(HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain to Axis")]));
				};
				let hint_data = HintData(hint_data);
				responses.add(FrontendMessage::UpdateInputHints { hint_data });
			}
			SelectToolFsmState::Drawing { .. } if tool_data.drag_start != tool_data.drag_current => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Shift], "Extend"), HintInfo::keys([Key::Alt], "Subtract")]),
					// TODO: Re-select deselected layers during drag when Shift is pressed, and re-deselect if Shift is released before drag ends.
					// TODO: (See https://discord.com/channels/731730685944922173/1216976541947531264/1321360311298818048)
					// HintGroup(vec![HintInfo::keys([Key::Shift], "Extend Selection")])
				]);
				responses.add(FrontendMessage::UpdateInputHints { hint_data });
			}
			SelectToolFsmState::Drawing { .. } | SelectToolFsmState::Dragging { .. } => {}
			SelectToolFsmState::ResizingBounds => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Alt], "From Pivot"), HintInfo::keys([Key::Shift], "Preserve Aspect Ratio")]),
				]);
				responses.add(FrontendMessage::UpdateInputHints { hint_data });
			}
			SelectToolFsmState::RotatingBounds => {
				let hint_data = HintData(vec![
					HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
					HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments")]),
				]);
				responses.add(FrontendMessage::UpdateInputHints { hint_data });
			}
			SelectToolFsmState::DraggingPivot | SelectToolFsmState::SkewingBounds { .. } => {
				let hint_data = HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]);
				responses.add(FrontendMessage::UpdateInputHints { hint_data });
			}
		}
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn not_artboard(document: &DocumentMessageHandler) -> impl Fn(&LayerNodeIdentifier) -> bool + '_ {
	|&layer| layer != LayerNodeIdentifier::ROOT_PARENT && !document.network_interface.is_artboard(&layer.to_node(), &[])
}

fn drag_shallowest_manipulation(responses: &mut VecDeque<Message>, selected: Vec<LayerNodeIdentifier>, tool_data: &mut SelectToolData, document: &DocumentMessageHandler) {
	for layer in selected {
		let ancestor = layer
			.ancestors(document.metadata())
			.filter(not_artboard(document))
			.find(|&ancestor| document.network_interface.selected_nodes(&[]).unwrap().selected_layers_contains(ancestor, document.metadata()));

		let new_selected = ancestor.unwrap_or_else(|| layer.ancestors(document.metadata()).filter(not_artboard(document)).last().unwrap_or(layer));
		tool_data.layers_dragging.retain(|layer| !layer.ancestors(document.metadata()).any(|ancestor| ancestor == new_selected));
		tool_data.layers_dragging.push(new_selected);
	}

	responses.add(NodeGraphMessage::SelectedNodesSet {
		nodes: tool_data
			.layers_dragging
			.iter()
			.filter_map(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					Some(layer.to_node())
				} else {
					log::error!("ROOT_PARENT cannot be part of tool_data.layers_dragging");
					None
				}
			})
			.collect(),
	});
}

fn drag_deepest_manipulation(responses: &mut VecDeque<Message>, selected: Vec<LayerNodeIdentifier>, tool_data: &mut SelectToolData, document: &DocumentMessageHandler) {
	tool_data.layers_dragging.append(&mut vec![document.find_deepest(&selected).unwrap_or(
		LayerNodeIdentifier::ROOT_PARENT
			.children(document.metadata())
			.next()
			.expect("ROOT_PARENT should have a layer child when clicking"),
	)]);
	responses.add(NodeGraphMessage::SelectedNodesSet {
		nodes: tool_data
			.layers_dragging
			.iter()
			.filter_map(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					Some(layer.to_node())
				} else {
					log::error!("ROOT_PARENT cannot be part of tool_data.layers_dragging");
					None
				}
			})
			.collect(),
	});
}

/// Called when you double click on the layer of the shallowest layer.
/// If possible, the direct sibling of an old selected layer is the new selected layer.
/// Otherwise, the first non-parent ancestor is selected.
fn edit_layer_shallowest_manipulation(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	let Some(new_selected) = layer.ancestors(document.metadata()).filter(not_artboard(document)).find(|ancestor| {
		ancestor
			.parent(document.metadata())
			.is_some_and(|parent| document.network_interface.selected_nodes(&[]).unwrap().selected_layers_contains(parent, document.metadata()))
	}) else {
		return;
	};

	if new_selected == LayerNodeIdentifier::ROOT_PARENT {
		log::error!("new_selected cannot be ROOT_PARENT");
		return;
	}

	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![new_selected.to_node()] });
}

/// Called when a double click on a layer in deep select mode.
/// If the layer is text, the text tool is selected.
fn edit_layer_deepest_manipulation(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface, responses: &mut VecDeque<Message>) {
	if is_layer_fed_by_node_of_name(layer, network_interface, "Text") {
		responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text });
		responses.add(TextToolMessage::EditSelected);
	}
}

pub fn extend_lasso(lasso_polygon: &mut Vec<DVec2>, point: DVec2) {
	if lasso_polygon.len() < 2 {
		lasso_polygon.push(point);
	} else {
		let last_points = lasso_polygon.last_chunk::<2>().unwrap();
		let distance = last_points[0].distance_squared(last_points[1]);

		if distance < SELECTION_TOLERANCE.powi(2) {
			lasso_polygon.pop();
		}
		lasso_polygon.push(point);
	}
}
