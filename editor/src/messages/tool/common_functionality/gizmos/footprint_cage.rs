use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::transformation_cage::{BoundingBoxManager, SelectedEdges};
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::context::bake_footprint;
use graphene_std::platform_application_io::rasterize;
use graphene_std::renderer::Quad;
use graphene_std::transform::Footprint;

/// A transform cage that resizes and moves the area of the selected node with a footprint input.
#[derive(Clone, Debug, Default)]
pub struct FootprintCage {
	target: Option<CageTarget>,
	drag: Option<CageDrag>,
}

#[derive(Clone, Debug)]
struct CageTarget {
	footprint_input: InputConnector,
	area: Footprint,
	/// Spans the area, with the unit square as its bounds.
	bounding_box: BoundingBoxManager,
}

#[derive(Clone, Debug)]
struct CageDrag {
	footprint_input: InputConnector,
	original_area: Footprint,
	unit_square_to_viewport: DAffine2,
	kind: CageDragKind,
}

#[derive(Clone, Debug)]
enum CageDragKind {
	Resize(SelectedEdges),
	Move { grab: DVec2 },
}

impl FootprintCage {
	/// Places the cage on the area of the selected node with a footprint input, or removes it if it is hidden or there is none.
	pub fn refresh(&mut self, document: &DocumentMessageHandler, visible: bool) {
		self.target = visible.then(|| Self::find_target(document)).flatten();
	}

	fn find_target(document: &DocumentMessageHandler) -> Option<CageTarget> {
		let network_path = document.selection_network_path();
		let network_interface = &document.network_interface;

		let selected_nodes = network_interface.selected_nodes_in_nested_network(network_path)?;
		let mut footprint_inputs = selected_nodes.selected_nodes().filter_map(|&node_id| footprint_input(node_id, network_interface, network_path));
		let footprint_input = footprint_inputs.next()?;
		if footprint_inputs.next().is_some() {
			return None;
		}
		let node_id = footprint_input.node_id()?;

		let input = network_interface.input_from_connector(&footprint_input, network_path)?;
		let &TaggedValue::Footprint(area) = input.as_non_exposed_value()? else { return None };

		let unit_square_to_document = *document.metadata().footprint_areas.get(&node_id)?;
		let unit_square_to_viewport = document.metadata().document_to_viewport * unit_square_to_document;
		if unit_square_to_viewport.matrix2.determinant() == 0. {
			return None;
		}

		let bounding_box = BoundingBoxManager {
			bounds: [DVec2::ZERO, DVec2::ONE],
			transform: unit_square_to_viewport,
			..Default::default()
		};

		Some(CageTarget { footprint_input, area, bounding_box })
	}

	pub fn overlays(&mut self, overlay_context: &mut OverlayContext) {
		if let Some(target) = &mut self.target {
			target.bounding_box.render_overlays(overlay_context, true);
		}
	}

	/// The cursor for the part of the cage under the pointer, if any.
	pub fn cursor(&self, input: &InputPreprocessorMessageHandler) -> Option<MouseCursorIcon> {
		let target = self.target.as_ref()?;

		let edge_cursor = target.bounding_box.get_cursor(input, false, false, None);
		if edge_cursor != MouseCursorIcon::Default {
			return Some(edge_cursor);
		}

		target.contains(input.mouse.position).then_some(MouseCursorIcon::Move)
	}

	/// Begins resizing or moving the area if the pointer is on the cage, returning whether it did.
	pub fn begin_drag(&mut self, mouse: DVec2, responses: &mut VecDeque<Message>) -> bool {
		let Some(target) = &self.target else { return false };
		let bounding_box = &target.bounding_box;

		let kind = if let Some((top, bottom, left, right)) = bounding_box.check_selected_edges(mouse) {
			CageDragKind::Resize(SelectedEdges::new(top, bottom, left, right, bounding_box.bounds))
		} else if target.contains(mouse) {
			CageDragKind::Move {
				grab: bounding_box.transform.inverse().transform_point2(mouse),
			}
		} else {
			return false;
		};

		self.drag = Some(CageDrag {
			footprint_input: target.footprint_input,
			original_area: target.area,
			unit_square_to_viewport: bounding_box.transform,
			kind,
		});

		responses.add(DocumentMessage::StartTransaction);
		responses.add(DocumentMessage::NameInteraction { action: "Changed a Footprint".into() });
		true
	}

	/// Moves or resizes the area to follow the pointer, keeping its pixel density.
	pub fn update_drag(&mut self, mouse: DVec2, centered: bool, constrain: bool, responses: &mut VecDeque<Message>) {
		let Some(drag) = &self.drag else { return };

		let (position, size) = match &drag.kind {
			CageDragKind::Resize(edges) => {
				let center = drag.unit_square_to_viewport.transform_point2(DVec2::splat(0.5));
				let (position, size) = edges.new_size(mouse, drag.unit_square_to_viewport, centered.then_some(center), constrain, None);
				(position.min(position + size), size.abs())
			}
			CageDragKind::Move { grab } => {
				let pointer = drag.unit_square_to_viewport.inverse().transform_point2(mouse);
				(pointer - *grab, DVec2::ONE)
			}
		};

		let original_area = drag.original_area;
		let one_pixel = original_area.resolution.as_dvec2().recip();
		let size = size.max(one_pixel);

		let area = Footprint {
			transform: original_area.transform * DAffine2::from_scale_angle_translation(size, 0., position),
			resolution: (original_area.resolution.as_dvec2() * size).round().as_uvec2().max(UVec2::ONE),
			..original_area
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: drag.footprint_input,
			input: NodeInput::value(TaggedValue::Footprint(area), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Keeps the drag in place on the document while the viewport pans by `shift`.
	pub fn pan(&mut self, shift: DVec2) {
		if let Some(drag) = &mut self.drag {
			drag.unit_square_to_viewport.translation += shift;
		}
	}

	/// Finishes the drag, undoing its changes if it was aborted.
	pub fn end_drag(&mut self, aborted: bool, responses: &mut VecDeque<Message>) {
		if self.drag.take().is_none() {
			return;
		}

		responses.add(if aborted { DocumentMessage::AbortTransaction } else { DocumentMessage::EndTransaction });
	}
}

/// The input holding the footprint of a node that has one.
fn footprint_input(node_id: NodeId, network_interface: &NodeNetworkInterface, network_path: &[NodeId]) -> Option<InputConnector> {
	let reference = network_interface.reference(&node_id, network_path)?;

	if reference == DefinitionIdentifier::ProtoNode(bake_footprint::IDENTIFIER) {
		Some(InputConnector::node(node_id, bake_footprint::FootprintInput))
	} else if reference == DefinitionIdentifier::ProtoNode(rasterize::IDENTIFIER) {
		Some(InputConnector::node(node_id, rasterize::FootprintInput))
	} else {
		None
	}
}

impl CageTarget {
	fn contains(&self, viewport_point: DVec2) -> bool {
		(self.bounding_box.transform * Quad::from_box(self.bounding_box.bounds)).contains(viewport_point)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::test_prelude::*;

	#[tokio::test]
	async fn a_bake_footprint_node_in_a_layer_reports_where_its_area_lies() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 10., 10.).await;

		let layer = editor.active_document().metadata().all_layers().next().expect("drawing a rectangle should create a layer");
		let node_id = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(bake_footprint::IDENTIFIER)).await;
		editor.handle_message(NodeGraphMessage::MoveNodeToChainStart { node_id, parent: layer }).await;
		editor.eval_graph_until_finished().await.expect("the graph should evaluate");

		let document = editor.active_document();
		let input = document.network_interface.input_from_connector(&InputConnector::node(node_id, bake_footprint::FootprintInput), &[]);
		let Some(&TaggedValue::Footprint(area)) = input.and_then(NodeInput::as_non_exposed_value) else {
			panic!("the node should have a footprint value")
		};

		let metadata = document.metadata();
		let baked_area = metadata.footprint_areas.get(&node_id).expect("the render should report where the node baked its area");
		let layer_content_to_viewport = metadata.upstream_footprints.get(&layer.to_node()).expect("the render should report the layer's footprint").transform;
		let expected = metadata.rendered_document_to_viewport.inverse() * layer_content_to_viewport * area.transform;
		assert!(baked_area.abs_diff_eq(expected, 1e-9), "{baked_area:?} should be {expected:?}");
	}

	#[tokio::test]
	async fn a_baked_area_moves_with_a_transform_applied_after_it() {
		use graphene_std::transform_nodes::transform;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 10., 10.).await;

		let layer = editor.active_document().metadata().all_layers().next().expect("drawing a rectangle should create a layer");
		let bake_footprint_node = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(bake_footprint::IDENTIFIER)).await;
		editor
			.handle_message(NodeGraphMessage::MoveNodeToChainStart {
				node_id: bake_footprint_node,
				parent: layer,
			})
			.await;
		editor.eval_graph_until_finished().await.expect("the graph should evaluate");
		let untransformed = *editor.active_document().metadata().footprint_areas.get(&bake_footprint_node).expect("the area should be reported");

		let transform_node = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(transform::IDENTIFIER)).await;
		editor
			.handle_message(NodeGraphMessage::MoveNodeToChainStart {
				node_id: transform_node,
				parent: layer,
			})
			.await;
		let translation = DVec2::new(100., 50.);
		editor
			.handle_message(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(transform_node, transform::TranslationInput),
				input: NodeInput::value(TaggedValue::DVec2(translation), false),
			})
			.await;
		editor.eval_graph_until_finished().await.expect("the graph should evaluate");

		let transformed = *editor
			.active_document()
			.metadata()
			.footprint_areas
			.get(&bake_footprint_node)
			.expect("the area should still be reported");
		let expected = DAffine2::from_translation(translation) * untransformed;
		assert!(transformed.abs_diff_eq(expected, 1e-9), "{transformed:?} should be {expected:?}");
	}

	/// Drags the cage of a 'Bake Footprint' node in a layer between two points of its unit square, giving its area before and after.
	async fn drag_cage(from: DVec2, to: DVec2) -> (Footprint, Footprint) {
		use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
		use crate::messages::tool::utility_types::ToolType;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 10., 10.).await;

		let layer = editor.active_document().metadata().all_layers().next().expect("drawing a rectangle should create a layer");
		let node_id = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(bake_footprint::IDENTIFIER)).await;
		editor.handle_message(NodeGraphMessage::MoveNodeToChainStart { node_id, parent: layer }).await;

		let area = Footprint {
			transform: DAffine2::from_scale(DVec2::splat(100.)),
			resolution: UVec2::splat(200),
			..Default::default()
		};
		let footprint_input = InputConnector::node(node_id, bake_footprint::FootprintInput);
		editor
			.handle_message(NodeGraphMessage::SetInput {
				input_connector: footprint_input,
				input: NodeInput::value(TaggedValue::Footprint(area), false),
			})
			.await;
		editor.eval_graph_until_finished().await.expect("the graph should evaluate");
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node_id] }).await;

		let metadata = editor.active_document().metadata();
		let unit_square_to_viewport = metadata.document_to_viewport * *metadata.footprint_areas.get(&node_id).expect("the area should be reported");
		let (from, to) = (unit_square_to_viewport.transform_point2(from), unit_square_to_viewport.transform_point2(to));

		editor.select_tool(ToolType::Select).await;
		editor.handle_message(SelectToolMessage::Overlays { context: OverlayContext::default() }).await;
		editor.move_mouse(from.x, from.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(from.x, from.y, ModifierKeys::empty()).await;
		editor.move_mouse(to.x, to.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		editor.left_mouseup(to.x, to.y, ModifierKeys::empty()).await;

		let input = editor.active_document().network_interface.input_from_connector(&footprint_input, &[]);
		let Some(&TaggedValue::Footprint(dragged)) = input.and_then(NodeInput::as_non_exposed_value) else {
			panic!("the node should have a footprint value")
		};
		(area, dragged)
	}

	#[tokio::test]
	async fn dragging_the_right_edge_of_the_cage_widens_the_area_and_its_resolution() {
		let (area, dragged) = drag_cage(DVec2::new(1., 0.5), DVec2::new(1.5, 0.5)).await;

		let expected_transform = area.transform * DAffine2::from_scale(DVec2::new(1.5, 1.));
		assert!(dragged.transform.abs_diff_eq(expected_transform, 1e-9), "{:?} should be {expected_transform:?}", dragged.transform);
		assert_eq!(dragged.resolution, UVec2::new(300, 200));
	}

	#[tokio::test]
	async fn dragging_the_right_edge_onto_the_left_one_keeps_one_pixel_of_width() {
		let (area, dragged) = drag_cage(DVec2::new(1., 0.5), DVec2::new(0., 0.5)).await;

		let one_pixel = 1. / area.resolution.x as f64;
		let expected_transform = area.transform * DAffine2::from_scale(DVec2::new(one_pixel, 1.));
		assert!(dragged.transform.abs_diff_eq(expected_transform, 1e-9), "{:?} should be {expected_transform:?}", dragged.transform);
		assert_eq!(dragged.resolution, UVec2::new(1, 200));
	}
}
