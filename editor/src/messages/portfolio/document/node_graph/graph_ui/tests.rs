use super::*;
use crate::messages::dialog::simple_dialogs::ARTWORK;
use crate::messages::input_mapper::utility_types::pointer::EditorPointerState;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{self, resolve_document_node_type};
use crate::messages::portfolio::document::node_graph::utility_types::{ContextMenuData, NodeEvaluationError};
use crate::messages::portfolio::document::utility_types::network_interface::{DocumentNodeClickTargets, NodeTypeClickTargets, Ports};
use crate::messages::portfolio::ingest::utility_types::IngestAction;
use crate::messages::portfolio::utility_types::PanelType;
use crate::test_utils::test_prelude::*;
use glam::DAffine2;
use graph_ui::metrics::{CONNECTOR_SIZE, DOT_OUTPUT_ARROW, GRID_SIZE, IMPORT_EXPORT_NAME_RAISE, LAYER_LEFT_MARGIN, STACK_CONNECTOR_SIZE};
use graph_ui::replicated_map::ReplicatedMap;
use graph_ui::scene::GraphUiScene;
use graphene_std::vector::click_target::ClickTarget;
use kurbo::PathEl;
use std::time::{Duration, Instant};

async fn engine_drawn_graph(rectangles: usize) -> EditorTestUtils {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.set_viewport_size(DVec2::ZERO, DVec2::new(1200., 800.)).await;
	for index in 0..rectangles {
		let left = 120. * index as f64;
		editor.draw_rect(left, 0., left + 100., 100.).await;
	}
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	editor
}

fn scene(editor: &EditorTestUtils) -> &GraphUiScene {
	editor.portfolio_message_handler().executor.graph_ui_scene()
}

fn renders(editor: &EditorTestUtils) -> u64 {
	editor.portfolio_message_handler().executor.current_execution_id()
}

fn first_layer(editor: &EditorTestUtils) -> Layer {
	scene(editor).layers().iter().next().cloned().unwrap()
}

fn middle([min, max]: [DVec2; 2]) -> DVec2 {
	(min + max) / 2.
}

fn pointer_down(right_click: bool) -> NodeGraphMessage {
	NodeGraphMessage::PointerDown {
		shift_click: false,
		control_click: false,
		alt_click: false,
		right_click,
	}
}

fn rectangle_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER)
}

fn fill_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::fill::IDENTIFIER)
}

fn merge_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::Network("Merge".to_string())
}

fn redirect_dot_definition() -> DefinitionIdentifier {
	DefinitionIdentifier::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER)
}

#[tokio::test]
async fn the_executor_holds_a_card_for_each_node_of_the_open_network() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	editor.draw_rect(0., 0., 100., 100.).await;
	let card = |editor: &EditorTestUtils, node| editor.portfolio_message_handler().executor.graph_ui_scene().cards().get(&node).cloned();
	let rectangle = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER);

	let node = editor.create_node_by_name_at(rectangle, 3, 4).await;
	assert_eq!(card(&editor, node), None, "the graph is closed");

	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	assert_eq!(card(&editor, node).map(|card| card.cell), Some(IVec2::new(3, 4)));
	assert_eq!(card(&editor, node).unwrap().icon, "NodeRectangle", "the icon of the node's definition");
	assert_eq!(card(&editor, first_layer(&editor).key), None, "a layer has no card");

	assert_eq!(card(&editor, node).unwrap().secondary_inputs, []);
	let input_connector = InputConnector::node_at_index(node, 1);
	let expose = NodeGraphMessage::ExposeInput {
		input_connector,
		set_to_exposed: true,
		start_transaction: true,
	};
	editor.handle_message(expose).await;
	let names = |card: Card| card.secondary_inputs.into_iter().map(|input| input.name).collect::<Vec<_>>();
	assert_eq!(card(&editor, node).map(names), Some(vec!["Width".to_string()]), "the exposed input has a row");
	assert_eq!(card(&editor, node).unwrap().primary_output.map(|output| output.connected), Some(false));
	assert_eq!(card(&editor, node).unwrap().primary_input, None);
	let pointer = editor
		.create_node_by_name_at(DefinitionIdentifier::ProtoNode(graphene_std::animation::pointer_position::IDENTIFIER), 10, 4)
		.await;
	assert_eq!(card(&editor, pointer).unwrap().primary_input, None, "a node without inputs has no input connector");

	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;
	assert_eq!(card(&editor, node).unwrap().selection, Selection::Active, "the node is selected");
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() }).await;
	assert_eq!(card(&editor, node).unwrap().selection, Selection::None, "the selection is cleared");

	assert!(card(&editor, node).unwrap().visible);
	editor
		.handle_message(NodeGraphMessage::ToggleVisibility {
			node_id: node,
			network_path: Vec::new(),
		})
		.await;
	assert!(!card(&editor, node).unwrap().visible, "the node is hidden");

	assert!(!card(&editor, node).unwrap().previewed);
	editor.handle_message(NodeGraphMessage::TogglePreview { node_id: node }).await;
	assert!(card(&editor, node).unwrap().previewed, "the node is previewed");

	editor
		.handle_message(NodeGraphMessage::DeleteNodes {
			node_ids: vec![node],
			delete_children: false,
		})
		.await;
	assert_eq!(card(&editor, node), None, "the node is deleted");
}

#[test]
fn every_node_icon_can_be_drawn_where_the_branding_is() {
	let icon = graph_ui::icons::icon;
	let names = document_node_definitions::collect_node_icons().into_iter().map(|(_, name)| name);
	let missing: Vec<String> = names.chain(["NodeCustomNode".to_string()]).filter(|name| icon(name, Default::default()).is_none()).collect();
	let branding = std::path::Path::new("../branding/assets").is_dir();
	assert!(!branding || missing.is_empty(), "{missing:?}");
}

#[tokio::test]
async fn the_executor_holds_each_layer_with_its_name_and_width() {
	let mut editor = engine_drawn_graph(1).await;
	let layer = first_layer(&editor);
	let key = layer.key;
	let network_interface = &editor.active_document().network_interface;
	let expected = (
		network_interface.position(&key, &[]),
		network_interface.display_name(&key, &[]),
		network_interface.layer_width(&key, &[]),
	);
	let chain_width = network_interface.chain_width(&key, &[]);
	assert_eq!((Some(layer.cell), layer.name, Some(layer.width)), expected);
	assert!(chain_width > 0 && layer.chain_width == chain_width, "the shape nodes are chained into the layer");
	assert!(!layer.has_left_input_wire, "nothing feeds the chain from the left");
	assert!(layer.visible && !layer.locked);
	assert_eq!(layer.selection, Selection::Active, "the drawn layer is selected in the network the graph shows");
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: Vec::new() }).await;
	let selection = |editor: &EditorTestUtils| scene(editor).layers().get(&key).map(|layer| layer.selection);
	assert_eq!(selection(&editor), Some(Selection::None), "the selection is cleared");
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![key] }).await;
	assert_eq!(selection(&editor), Some(Selection::Active), "the layer is selected again");

	editor
		.handle_message(NodeGraphMessage::ToggleVisibility {
			node_id: key,
			network_path: Vec::new(),
		})
		.await;
	editor
		.handle_message(NodeGraphMessage::SetLocked {
			node_id: key,
			network_path: Vec::new(),
			locked: true,
		})
		.await;
	let layer = scene(&editor).layers().get(&key).cloned().unwrap();
	assert!(!layer.visible && layer.locked, "the layer is hidden and locked");

	let name = "A Layer With a Name Long Enough to Widen It".to_string();
	editor
		.handle_message(NodeGraphMessage::SetDisplayName {
			node_id: key,
			network_path: Vec::new(),
			alias: name.clone(),
			skip_adding_history_step: false,
		})
		.await;
	let layer = scene(&editor).layers().get(&key).cloned().unwrap();
	assert_eq!(layer.name, name);
	assert!(layer.width > expected.2.unwrap(), "the layer widens to fit its name");
}

#[tokio::test]
async fn the_layers_know_what_their_connectors_are_wired_to() {
	let mut editor = engine_drawn_graph(2).await;
	let layers: Vec<Layer> = scene(&editor).layers().iter().cloned().collect();
	let [lower, upper] = [true, false].map(|lower| layers.iter().find(|layer| layer.primary_output_connected_to_layer == lower).cloned().unwrap());

	assert!(upper.primary_input_connected_to_layer, "the upper layer stacks on the lower one");
	assert!(!lower.primary_input_connected_to_layer, "nothing is stacked below the lower layer");
	assert_eq!(upper.primary_input.as_ref().map(|input| input.connected), Some(true));
	assert_eq!(lower.primary_output.as_ref().map(|output| output.connected), Some(true));
	assert_eq!(upper.primary_output.as_ref().map(|output| output.connected), Some(true), "the upper layer feeds the export");
	for layer in [&lower, &upper] {
		assert_eq!(
			layer.secondary_input.as_ref().map(|input| input.connected),
			Some(true),
			"the shape chain feeds the input on the left side"
		);
		assert!(!layer.previewed);
	}

	editor.handle_message(NodeGraphMessage::TogglePreview { node_id: lower.key }).await;
	let previewed = |key| scene(&editor).layers().get(&key).map(|layer| layer.previewed);
	assert_eq!([previewed(lower.key), previewed(upper.key)], [Some(true), Some(false)], "the lower layer is previewed");
}

#[tokio::test]
async fn the_executor_holds_a_dot_for_each_redirect_dot_and_no_card() {
	let mut editor = engine_drawn_graph(0).await;
	let dot = editor.create_node_by_name_at(redirect_dot_definition(), 5, 6).await;
	assert_eq!(scene(&editor).cards().get(&dot), None, "a redirect dot has no card");
	let facts = |editor: &EditorTestUtils| scene(editor).dots().get(&dot).map(|dot| (dot.cell, dot.data_type, dot.wired));
	assert_eq!(facts(&editor), Some((IVec2::new(5, 6), DataType::General, false)));

	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 5).await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(rectangle, 0),
			input_connector: InputConnector::node_at_index(dot, 0),
		})
		.await;
	assert_eq!(facts(&editor).map(|(_, _, wired)| wired), Some(true), "a wire runs into the dot");

	let looks = |editor: &EditorTestUtils| scene(editor).dots().get(&dot).map(|dot| (dot.selected, dot.previewed));
	assert_eq!(looks(&editor), Some((false, false)));
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![dot] }).await;
	assert_eq!(looks(&editor), Some((true, false)), "the dot is selected");
	editor.handle_message(NodeGraphMessage::TogglePreview { node_id: dot }).await;
	assert_eq!(looks(&editor).map(|(_, previewed)| previewed), Some(true), "the dot is previewed");
}

fn frame_config(editor: &EditorTestUtils) -> GraphUiConfig {
	config(editor.active_document()).expect("the graph is open")
}

fn wire(editor: &EditorTestUtils, key: WireKey) -> Option<Wire> {
	scene(editor).wires().get(&key).cloned()
}

fn routed(editor: &EditorTestUtils, input: InputConnector) -> Option<kurbo::BezPath> {
	let (path, _, _) = editor.active_document().network_interface.vector_wire_from_input(&input, GraphWireStyle::Direct, &[])?;
	Some(path)
}

#[tokio::test]
async fn the_executor_holds_the_wire_into_each_connected_input_where_the_network_interface_routes_it() {
	let mut editor = engine_drawn_graph(0).await;
	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let fill = editor.create_node_by_name_at(fill_definition(), 10, 2).await;
	let input = InputConnector::node_at_index(fill, 0);
	let key = WireKey::Input { node: fill, index: 0 };
	assert_eq!(wire(&editor, key), None, "nothing feeds the input yet");

	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(fill, 0),
			input_connector: InputConnector::Export(0),
		})
		.await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(rectangle, 0),
			input_connector: input,
		})
		.await;
	let wired = wire(&editor, key).expect("a wire runs into the input");
	assert_eq!(Some(wired.path.clone()), routed(&editor, input));
	assert_eq!(wired.data_type, DataType::Vector, "the type of the output feeding it");
	assert!(!wired.list, "a single shape");

	editor.dispatch(NodeGraphMessage::ShiftNodePosition { node_id: rectangle, x: 0, y: 3 });
	let moved = wire(&editor, key).expect("the input is still wired");
	assert_ne!(moved.path, wired.path, "the wire follows the node it comes from");
	assert_eq!(Some(moved.path), routed(&editor, input));

	editor.handle_message(NodeGraphMessage::DisconnectInput { input_connector: input }).await;
	editor.handle_message(NodeGraphMessage::SendGraph).await;
	assert_eq!(wire(&editor, key), None, "the input is disconnected");
}

#[tokio::test]
async fn the_wire_from_a_layer_into_the_stack_input_of_another_is_thick() {
	let editor = engine_drawn_graph(2).await;
	let upper = scene(&editor)
		.layers()
		.iter()
		.find(|layer| layer.primary_input_connected_to_layer)
		.expect("one layer stacks on the other")
		.key;

	let stack = InputConnector::node_at_index(upper, 0);
	let stack_wire = wire(&editor, WireKey::Input { node: upper, index: 0 }).expect("a wire runs into the stack input");
	let (_, center_line, thick) = editor.active_document().network_interface.vector_wire_from_input(&stack, GraphWireStyle::Direct, &[]).unwrap();
	assert!(thick && stack_wire.thick, "the wire from the layer below is thick");
	assert_eq!(stack_wire.center_line, center_line);
	let chain_wire = wire(&editor, WireKey::Input { node: upper, index: 1 }).expect("the shape chain feeds the input on the left side");
	assert!(!chain_wire.thick, "the wire from the chain is thin");
}

#[tokio::test]
async fn a_previewed_node_has_a_wire_to_the_export_until_the_preview_ends() {
	let mut editor = engine_drawn_graph(1).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 10).await;
	assert_eq!(wire(&editor, WireKey::Preview), None, "nothing is previewed");

	editor.handle_message(NodeGraphMessage::TogglePreview { node_id: node }).await;
	let preview = wire(&editor, WireKey::Preview).expect("the previewed node has a wire to the export");
	let network_interface = &editor.active_document().network_interface;
	let (output, path, _, _) = network_interface.vector_wire_to_preview(GraphWireStyle::Direct, &GlideOffsets::default(), &[]).unwrap();
	assert_eq!((output, &preview.path), (OutputConnector::node(node, 0), &path));

	editor.handle_message(NodeGraphMessage::TogglePreview { node_id: node }).await;
	assert_eq!(wire(&editor, WireKey::Preview), None, "the preview has ended");
}

#[tokio::test]
async fn the_wire_a_dragged_node_would_be_inserted_on_is_marked_until_the_drag_is_aborted() {
	let mut editor = engine_drawn_graph(0).await;
	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let wired = editor.create_node_by_name_at(fill_definition(), 20, 0).await;
	let dragged = editor.create_node_by_name_at(fill_definition(), 10, 4).await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(rectangle, 0),
			input_connector: InputConnector::node_at_index(wired, 0),
		})
		.await;
	let target = WireKey::Input { node: wired, index: 0 };
	let marked = |editor: &EditorTestUtils| wire(editor, target).map(|wire| wire.insertion_target);
	assert_eq!(marked(&editor), Some(false));

	let from = middle(scene(&editor).cards().get(&dragged).unwrap().body());
	let to = from - DVec2::new(0., 4. * GRID_SIZE);
	point_at(&mut editor, from, MouseKeys::empty());
	press_at(&mut editor, from);
	point_at(&mut editor, to, MouseKeys::LEFT);
	assert_eq!(
		editor.active_document().network_interface.position(&dragged, &[]),
		Some(IVec2::new(10, 0)),
		"the node is dragged onto the wire"
	);
	assert_eq!(marked(&editor), Some(true), "the node would be inserted on the wire");

	editor.dispatch(pointer_down(true));
	assert_eq!(marked(&editor), Some(false), "the drag is aborted");
}

#[tokio::test]
async fn the_wire_dragged_out_of_an_output_is_drawn_until_the_drag_is_aborted() {
	let mut editor = engine_drawn_graph(0).await;
	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let output = editor.active_document().network_interface.output_position(&OutputConnector::node(rectangle, 0), &[]).unwrap();
	let loose_end = output + DVec2::new(96., 48.);

	point_at(&mut editor, output, MouseKeys::empty());
	press_at(&mut editor, output);
	let before = renders(&editor);
	point_at(&mut editor, loose_end, MouseKeys::LEFT);
	assert!(renders(&editor) > before, "the graph is drawn again");
	let wire = frame_config(&editor).wire_in_progress.expect("a wire is dragged out of the output");
	let [PathEl::MoveTo(start), .., PathEl::LineTo(end)] = wire.path else { panic!("{:?}", wire.path) };
	assert!(
		start.distance((output.x, output.y).into()) < 1e-9 && end.distance((loose_end.x, loose_end.y).into()) < 1e-9,
		"{start:?} {end:?}"
	);

	editor.dispatch(pointer_down(true));
	assert_eq!(frame_config(&editor).wire_in_progress, None, "the drag is aborted");
}

#[tokio::test]
async fn the_wire_from_an_artboard_carries_a_list() {
	let mut editor = engine_drawn_graph(0).await;
	editor
		.handle_message(GraphOperationMessage::NewArtboard {
			id: NodeId::new(),
			location: DVec2::ZERO,
			dimensions: DVec2::new(400., 300.),
			background: Color::WHITE,
			clip: false,
		})
		.await;
	assert_eq!(wire(&editor, WireKey::Export(0)).map(|wire| wire.list), Some(true));
}

#[tokio::test]
async fn the_wire_into_the_export_follows_the_export_as_the_graph_is_panned() {
	let mut editor = engine_drawn_graph(1).await;
	let export = InputConnector::Export(0);
	let before = wire(&editor, WireKey::Export(0)).expect("the layer feeds the export");
	assert_eq!(Some(before.path.clone()), routed(&editor, export));

	editor.dispatch(NavigationMessage::CanvasPan { delta: DVec2::new(-480., 0.) });
	let panned = wire(&editor, WireKey::Export(0)).expect("the layer still feeds the export");
	assert_ne!(panned.path, before.path, "the export keeps its place in the viewport, so the wire reaches further");
	assert_eq!(Some(panned.path), routed(&editor, export));
}

fn network_connectors(editor: &EditorTestUtils, kind: NetworkConnectorKind) -> Vec<NetworkConnector> {
	scene(editor).network_connectors().iter().filter(|row| row.kind == kind).cloned().collect()
}

fn network_connector(editor: &EditorTestUtils, kind: NetworkConnectorKind, index: usize) -> NetworkConnector {
	scene(editor).network_connectors().get(&(kind, index)).cloned().unwrap()
}

fn renamed_row(editor: &EditorTestUtils, kind: NetworkConnectorKind) -> Option<usize> {
	network_connectors(editor, kind).iter().find(|row| row.renaming).map(|row| row.index)
}

#[tokio::test]
async fn the_executor_holds_the_imports_and_exports_with_their_connectors_where_the_network_interface_places_them() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let names = |rows: &[NetworkConnector]| rows.iter().map(|row| row.connector.as_ref().map(|connector| connector.name.clone())).collect::<Vec<_>>();

	let exports = network_connectors(&editor, NetworkConnectorKind::Export);
	assert_eq!(names(&exports), [Some("Canvas".to_string())], "the document network shows the canvas as its export");
	assert!(network_connectors(&editor, NetworkConnectorKind::Import).is_empty(), "the document network has no imports");
	assert!(!exports[0].editable);

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	let network_path = [merge];
	let [imports, exports] = [NetworkConnectorKind::Import, NetworkConnectorKind::Export].map(|kind| network_connectors(&editor, kind));
	let network_interface = &editor.active_document().network_interface;
	assert!(imports.iter().chain(&exports).all(|row| row.editable));
	let with_connectors = |rows: &[NetworkConnector]| rows.iter().filter(|row| row.connector.is_some()).map(|row| (row.index, row.anchor())).collect::<Vec<_>>();
	let placed_imports = (0..network_interface.number_of_imports(&network_path))
		.filter_map(|index| Some((index, network_interface.output_position(&OutputConnector::Import(index), &network_path)?)))
		.collect::<Vec<_>>();
	let placed_exports = (0..network_interface.number_of_exports(&network_path))
		.filter_map(|index| Some((index, network_interface.input_position(&InputConnector::Export(index), &network_path)?)))
		.collect::<Vec<_>>();
	assert!(placed_imports.len() + placed_exports.len() >= 2);
	assert_eq!(with_connectors(&imports), placed_imports);
	assert_eq!(with_connectors(&exports), placed_exports);

	editor.dispatch(NavigationMessage::CanvasPan { delta: DVec2::new(-480., 0.) });
	let panned = network_connector(&editor, NetworkConnectorKind::Export, 0);
	assert_ne!(panned.position, exports[0].position, "the exports keep their place in the viewport as the graph is panned");
	let positions = editor.active_document().network_interface.import_export_position(&network_path);
	assert_eq!(Some(panned.position), positions.map(|(_, export)| export));
}

#[tokio::test]
async fn the_connectors_for_adding_imports_and_exports_are_drawn_where_the_network_interface_places_them() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let drawn_where_placed = |editor: &EditorTestUtils| {
		for (kind, is_import) in [(NetworkConnectorKind::Import, true), (NetworkConnectorKind::Export, false)] {
			let placed = editor.active_document().network_interface.empty_import_export_connectors(is_import, &[merge]);
			let rows = network_connectors(editor, kind);
			let drawn: Vec<(usize, DVec2)> = rows.iter().filter(|row| row.connector.is_none()).map(|row| (row.index, row.anchor())).collect();
			assert_eq!(drawn, placed, "{kind:?}");
		}
	};
	drawn_where_placed(&editor);

	editor.handle_message(NodeGraphMessage::ExposePrimaryExport { exposed: false }).await;
	assert_eq!(network_connector(&editor, NetworkConnectorKind::Export, 0).connector, None, "the primary export is hidden");
	drawn_where_placed(&editor);
}

#[tokio::test]
async fn the_bar_where_a_dragged_import_would_be_dropped_is_drawn_until_the_drag_is_aborted() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.active_document_mut().network_interface.make_editable(&[merge]);
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	let names = editor.active_document().network_interface.import_export_name_bounds(&[merge]);
	let name = names.iter().find(|(name, _)| matches!(name, ImportOrExport::Import(_))).expect("an import with a name").1;
	let on_the_name = middle(name);

	point_at(&mut editor, on_the_name, MouseKeys::empty());
	press_at(&mut editor, on_the_name);
	assert_eq!(frame_config(&editor).reorder_bar, None, "nothing moves until the pointer does");
	let before = renders(&editor);
	point_at(&mut editor, on_the_name + DVec2::new(0., 72.), MouseKeys::LEFT);
	let reorder_bar = frame_config(&editor).reorder_bar;
	let Some((NetworkConnectorKind::Import, anchor)) = reorder_bar else { panic!("{reorder_bar:?}") };
	assert_eq!(anchor.x, network_connectors(&editor, NetworkConnectorKind::Import)[0].anchor().x, "in the column of the imports");
	assert!(renders(&editor) > before, "the graph is drawn again with the bar");

	let before = renders(&editor);
	editor.dispatch(pointer_down(true));
	assert_eq!(frame_config(&editor).reorder_bar, None, "the drag is aborted");
	assert!(renders(&editor) > before, "the graph is drawn again without the bar");
}

#[tokio::test]
async fn the_box_dragged_out_over_the_background_is_drawn_until_the_box_selection_is_aborted() {
	let mut editor = engine_drawn_graph(0).await;
	let drawn = |editor: &EditorTestUtils| frame_config(editor).box_selection;

	editor.move_mouse(100., 120., ModifierKeys::empty(), MouseKeys::empty()).await;
	editor.handle_message(pointer_down(false)).await;
	assert_eq!(drawn(&editor), None, "nothing is drawn until the pointer moves");
	let before = renders(&editor);
	let editor_mouse_state = EditorPointerState {
		editor_position: DVec2::new(300., 250.),
		mouse_keys: MouseKeys::LEFT,
		..Default::default()
	};
	editor.dispatch(InputPreprocessorMessage::PointerMove {
		editor_mouse_state,
		modifier_keys: ModifierKeys::empty(),
	});
	assert_eq!(drawn(&editor), Some([DVec2::new(100., 120.), DVec2::new(300., 250.)]));
	assert!(renders(&editor) > before, "the graph is drawn again with the box");

	editor.dispatch(NavigationMessage::CanvasPan { delta: DVec2::new(-40., 0.) });
	let document = editor.active_document();
	let (start, _) = document.node_graph_handler.box_selection_start.unwrap();
	let to_viewport = document.network_interface.network_metadata(&[]).unwrap().persistent_metadata.navigation_metadata.node_graph_to_viewport;
	let start = to_viewport.transform_point2(start).max(DVec2::ZERO).floor();
	assert_ne!(start, DVec2::new(100., 120.), "the graph is panned");
	assert_eq!(drawn(&editor), Some([start, DVec2::new(300., 250.)]), "the box starts where it started in the graph");

	let before = renders(&editor);
	editor.dispatch(pointer_down(true));
	assert_eq!(drawn(&editor), None, "the box selection is aborted");
	assert!(renders(&editor) > before, "the graph is drawn again without the box");
}

#[tokio::test]
async fn the_executor_holds_a_badge_for_each_node_with_an_error_where_the_node_graph_shows_it() {
	let mut editor = engine_drawn_graph(1).await;
	let wrong = editor.create_node_by_name_at(rectangle_definition(), 13, 22).await;
	let feeding = editor.create_node_by_name_at(rectangle_definition(), 4, 23).await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(wrong, 0),
			input_connector: InputConnector::Export(0),
		})
		.await;
	let width = InputConnector::node_at_index(wrong, 1);
	let expose = NodeGraphMessage::ExposeInput {
		input_connector: width,
		set_to_exposed: true,
		start_transaction: true,
	};
	editor.handle_message(expose).await;
	let badges = |editor: &EditorTestUtils| scene(editor).errors().iter().cloned().collect::<Vec<_>>();
	assert_eq!(badges(&editor), []);

	editor.dispatch(NodeGraphMessage::CreateWire {
		output_connector: OutputConnector::node(feeding, 0),
		input_connector: width,
	});
	let _ = editor.eval_graph_until_finished().await;
	let badge = badges(&editor).into_iter().find(|badge| badge.key == wrong).expect("a shape does not fit the width");
	assert_eq!((badge.cell, badge.on_layer), (IVec2::new(13, 22), false), "the badge stands on the top left corner of the node's cell");
	assert_eq!(changed_by_a_rebuild(&mut editor), [0; 6], "the scene holds the badges a rebuild gives");

	editor.dispatch(NodeGraphMessage::DisconnectInput { input_connector: width });
	let _ = editor.eval_graph_until_finished().await;
	assert_eq!(badges(&editor), [], "the error is fixed");

	let layer = first_layer(&editor).key;
	let failed = |node| NodeEvaluationError {
		node_path: vec![node],
		message: "Failed".to_string(),
	};
	editor.dispatch(NodeGraphMessage::UpdateEvaluationErrors {
		errors: vec![failed(wrong), failed(layer)],
	});
	assert_eq!(badges(&editor).len(), 2, "errors of an evaluation reach the scene");
	assert_eq!(changed_by_a_rebuild(&mut editor), [0; 6], "the scene holds the badges a rebuild gives");
	let on_the_layer = badges(&editor).into_iter().find(|badge| badge.key == layer).unwrap();
	let cell = editor.active_document().network_interface.position(&layer, &[]).unwrap();
	assert_eq!((on_the_layer.cell, on_the_layer.on_layer), (cell, true), "the badge of a layer stands on its thumbnail");
}

#[tokio::test]
async fn the_selection_of_a_nested_network_is_inactive_while_the_layers_panel_is_active() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let merge = editor.create_node_by_name_at(DefinitionIdentifier::Network("Merge".to_string()), 0, 0).await;
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	let node = editor.portfolio_message_handler().executor.graph_ui_scene().cards().iter().next().unwrap().key;
	editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![node] }).await;
	let selection = |editor: &EditorTestUtils| editor.portfolio_message_handler().executor.graph_ui_scene().cards().get(&node).unwrap().selection;
	assert_eq!(selection(&editor), Selection::Active);

	editor.handle_message(DocumentMessage::SetActivePanel { active_panel: PanelType::Layers }).await;
	assert_eq!(selection(&editor), Selection::Inactive, "the Layers panel selects in the document network");
	editor.handle_message(DocumentMessage::SetActivePanel { active_panel: PanelType::Document }).await;
	assert_eq!(selection(&editor), Selection::Active, "the graph selects in its open network again");
}

#[tokio::test]
async fn a_change_to_the_open_nested_network_reaches_the_scene_while_the_layers_panel_is_active() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let merge = editor.create_node_by_name_at(DefinitionIdentifier::Network("Merge".to_string()), 0, 0).await;
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let node = editor.portfolio_message_handler().executor.graph_ui_scene().cards().iter().next().unwrap().key;
	editor.handle_message(DocumentMessage::SetActivePanel { active_panel: PanelType::Layers }).await;

	editor
		.handle_message(NodeGraphMessage::ToggleVisibility {
			node_id: node,
			network_path: vec![merge],
		})
		.await;
	let visible = editor.portfolio_message_handler().executor.graph_ui_scene().cards().get(&node).unwrap().visible;
	assert!(!visible, "the card of the hidden node is rebuilt");
}

#[tokio::test]
async fn errors_in_the_open_nested_network_reach_the_scene_while_the_layers_panel_is_active() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	let node = scene(&editor).cards().iter().next().unwrap().key;
	editor.handle_message(DocumentMessage::SetActivePanel { active_panel: PanelType::Layers }).await;
	editor.dispatch(NodeGraphMessage::UpdateEvaluationErrors {
		errors: vec![NodeEvaluationError {
			node_path: vec![merge, node],
			message: "Failed".to_string(),
		}],
	});
	assert_eq!(scene(&editor).errors().len(), 1);
}

#[tokio::test]
async fn a_press_in_a_panel_that_keeps_the_selection_network_rebuilds_no_rows() {
	let mut editor = EditorTestUtils::create();
	editor.new_document().await;
	let rectangle = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER);
	let node = editor.create_node_by_name_at(rectangle, 0, 0).await;
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	editor.portfolio_message_handler_mut().executor.update_graph_ui(|scene| assert!(scene.cards.remove(&node)));

	for active_panel in [PanelType::Document, PanelType::Layers] {
		editor.dispatch(DocumentMessage::SetActivePanel { active_panel });
		let cards = editor.portfolio_message_handler().executor.graph_ui_scene().cards();
		assert!(cards.is_empty(), "{active_panel:?}: a rebuild would bring back the removed card");
	}
}

#[tokio::test]
async fn a_layer_panned_out_of_the_graph_view_is_out_of_view_until_panned_back() {
	let mut editor = engine_drawn_graph(1).await;
	let layer = first_layer(&editor).key;
	let in_view = |editor: &EditorTestUtils| editor.active_document().layers_in_view.all_in_view().contains(&layer);
	assert!(in_view(&editor), "the layer is in the graph's view");

	editor.dispatch(NavigationMessage::CanvasPan { delta: DVec2::new(-4800., 0.) });
	assert!(!in_view(&editor), "the layer is panned out of view");
	editor.dispatch(NavigationMessage::CanvasPan { delta: DVec2::new(4800., 0.) });
	assert!(in_view(&editor), "the layer is panned back into view");

	editor.handle_message(DocumentMessage::GraphViewOverlay { open: false }).await;
	editor.dispatch(NodeGraphMessage::UpdateNodeGraphWidth);
	assert!(!in_view(&editor), "a closed graph has no layers in view");
}

fn pointer_state(editor: &EditorTestUtils, point: DVec2, mouse_keys: MouseKeys) -> EditorPointerState {
	let document = editor.active_document();
	let network_metadata = document.network_interface.network_metadata(document.breadcrumb_network_path()).unwrap();
	let to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
	EditorPointerState {
		editor_position: to_viewport.transform_point2(point),
		mouse_keys,
		..Default::default()
	}
}

fn point_at(editor: &mut EditorTestUtils, point: DVec2, mouse_keys: MouseKeys) -> Vec<FrontendMessage> {
	let (editor_mouse_state, modifier_keys) = (pointer_state(editor, point, mouse_keys), ModifierKeys::empty());
	editor.dispatch(InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys })
}

fn press_at(editor: &mut EditorTestUtils, point: DVec2) -> Vec<FrontendMessage> {
	let (editor_mouse_state, modifier_keys) = (pointer_state(editor, point, MouseKeys::LEFT), ModifierKeys::empty());
	editor.dispatch(InputPreprocessorMessage::PointerDown { editor_mouse_state, modifier_keys })
}

fn release_at(editor: &mut EditorTestUtils, point: DVec2) -> Vec<FrontendMessage> {
	let (editor_mouse_state, modifier_keys) = (pointer_state(editor, point, MouseKeys::empty()), ModifierKeys::empty());
	editor.dispatch(InputPreprocessorMessage::PointerUp { editor_mouse_state, modifier_keys })
}

fn tooltips(messages: Vec<FrontendMessage>) -> Vec<(String, String)> {
	let tooltip = |message| match message {
		FrontendMessage::UpdateGraphTooltip { label, description } => Some((label, description)),
		_ => None,
	};
	messages.into_iter().filter_map(tooltip).collect()
}

fn the_tooltip(messages: Vec<FrontendMessage>) -> (String, String) {
	let mut sent = tooltips(messages);
	assert_eq!(sent.len(), 1, "one tooltip is sent: {sent:?}");
	sent.remove(0)
}

fn label(text: &str) -> (String, String) {
	(text.to_string(), String::new())
}

#[tokio::test]
async fn the_tooltip_of_the_part_of_a_card_under_the_pointer_is_sent_when_the_pointer_reaches_another_part() {
	let mut editor = engine_drawn_graph(0).await;
	let card = ("Rectangle".to_string(), resolve_document_node_type(&rectangle_definition()).unwrap().description.trim().to_string());
	let node = editor.create_node_by_name_at(rectangle_definition(), 3, 4).await;
	let expose = NodeGraphMessage::ExposeInput {
		input_connector: InputConnector::node_at_index(node, 1),
		set_to_exposed: true,
		start_transaction: true,
	};
	editor.handle_message(expose).await;
	let [min, max] = scene(&editor).cards().get(&node).unwrap().body();
	let header = DVec2::new((min.x + max.x) / 2., min.y + GRID_SIZE / 2.);
	let network_interface = &editor.active_document().network_interface;
	let width = network_interface.displayed_input_name_and_description(&node, 1, &[]);
	let output = network_interface.frontend_output_from_connector(&OutputConnector::node(node, 0), &[]).unwrap();
	let output_position = network_interface.output_position(&OutputConnector::node(node, 0), &[]).unwrap();

	assert_eq!(the_tooltip(point_at(&mut editor, header, MouseKeys::empty())), card, "the card");
	assert_eq!(
		tooltips(point_at(&mut editor, header + DVec2::new(1., 1.), MouseKeys::empty())),
		[],
		"the pointer is still over the card"
	);
	assert_eq!(
		the_tooltip(point_at(&mut editor, header + DVec2::new(0., GRID_SIZE), MouseKeys::empty())),
		width,
		"the name of the exposed input's row"
	);
	assert_eq!(
		the_tooltip(point_at(&mut editor, DVec2::new(max.x - 4., header.y + GRID_SIZE), MouseKeys::empty())),
		card,
		"beside the name of the row"
	);

	let (output_label, description) = the_tooltip(point_at(&mut editor, output_position, MouseKeys::empty()));
	assert_eq!(output_label, output.name, "the output");
	let types = output.possible_types.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join("\n");
	assert!(!types.is_empty() && description.ends_with(&format!("Possible Types:\n{types}")), "{description}");
	assert!(!description.contains("Wired Type"), "the output is not wired: {description}");

	let mut clicked = press_at(&mut editor, output_position);
	clicked.extend(release_at(&mut editor, output_position));
	assert_eq!(tooltips(clicked), [], "a click on the part leaves its tooltip as it is");

	assert_eq!(
		the_tooltip(point_at(&mut editor, DVec2::new(-2000., 0.), MouseKeys::empty())),
		label(""),
		"the pointer has left the graph"
	);
}

#[tokio::test]
async fn the_buttons_and_the_body_of_a_layer_have_tooltips() {
	let mut editor = engine_drawn_graph(1).await;
	let row = first_layer(&editor);
	let (layer, visibility, grip, thumbnail) = (row.key, middle(row.visibility()), middle(row.grip()), middle(row.thumbnail()));

	assert_eq!(the_tooltip(point_at(&mut editor, visibility, MouseKeys::empty())), label("Hide"));
	editor
		.handle_message(NodeGraphMessage::ToggleVisibility {
			node_id: layer,
			network_path: Vec::new(),
		})
		.await;
	assert_eq!(
		tooltips(point_at(&mut editor, visibility + DVec2::new(1., 1.), MouseKeys::empty())),
		[],
		"the tooltip stays on the same button"
	);
	point_at(&mut editor, thumbnail, MouseKeys::empty());
	assert_eq!(
		the_tooltip(point_at(&mut editor, visibility, MouseKeys::empty())),
		label("Show"),
		"back on the button of the hidden layer"
	);
	let grip_description = "Drag only this layer without pushing others outside the stack".to_string();
	assert_eq!(the_tooltip(point_at(&mut editor, grip, MouseKeys::empty())), (String::new(), grip_description));

	editor
		.handle_message(NodeGraphMessage::SetLocked {
			node_id: layer,
			network_path: Vec::new(),
			locked: true,
		})
		.await;
	let lock = middle(first_layer(&editor).lock().unwrap());
	assert_eq!(the_tooltip(point_at(&mut editor, lock, MouseKeys::empty())), label("Unlock"));

	let network_interface = &editor.active_document().network_interface;
	let (name, implementation_name) = (network_interface.display_name(&layer, &[]), network_interface.implementation_name(&layer, &[]));
	let (label, _) = the_tooltip(point_at(&mut editor, thumbnail, MouseKeys::empty()));
	assert_eq!(label, format!("{implementation_name} ({name})"), "the layer's thumbnail");
}

#[tokio::test]
async fn an_error_badge_hides_the_tooltip_of_the_card_it_covers() {
	let mut editor = engine_drawn_graph(0).await;
	let covered = editor.create_node_by_name_at(rectangle_definition(), 3, 8).await;
	let failing = editor.create_node_by_name_at(rectangle_definition(), 3, 10).await;
	let card = scene(&editor).cards().get(&covered).unwrap().body();
	editor.dispatch(NodeGraphMessage::UpdateEvaluationErrors {
		errors: vec![NodeEvaluationError {
			node_path: vec![failing],
			message: "Failed".to_string(),
		}],
	});
	let badge = scene(&editor).errors().get(&failing).cloned().unwrap();
	let [min, _] = badge.body(badge.text_size());
	let on_both = min + DVec2::new(30., 4.);
	assert!(on_both.cmpge(card[0]).all() && on_both.cmple(card[1]).all(), "the badge stands over the card above its node");

	let beside = DVec2::new((card[0].x + card[1].x) / 2. + 20., on_both.y);
	assert_eq!(the_tooltip(point_at(&mut editor, beside, MouseKeys::empty())).0, "Rectangle", "beside the badge");
	assert_eq!(the_tooltip(point_at(&mut editor, on_both, MouseKeys::empty())), label(""), "on the badge");
}

#[tokio::test]
async fn the_tooltip_under_the_pointer_is_sent_again_when_its_document_is_selected_again() {
	let mut editor = engine_drawn_graph(1).await;
	let first = editor.portfolio_message_handler().active_document_id().unwrap();
	let visibility = middle(first_layer(&editor).visibility());
	assert_eq!(the_tooltip(point_at(&mut editor, visibility, MouseKeys::empty())), label("Hide"));

	editor.new_document().await;
	editor.handle_message(DocumentMessage::GraphViewOverlay { open: true }).await;
	let selected = editor.dispatch(PortfolioMessage::SelectDocument { document_id: first });
	assert_eq!(
		tooltips(selected).last(),
		Some(&label("Hide")),
		"the graph element the documents share showed the other document's tooltip"
	);
}

fn name_fields(messages: Vec<FrontendMessage>) -> Vec<(Option<GraphNameField>, bool)> {
	let name_field = |message| match message {
		FrontendMessage::UpdateGraphNameField { field, opens } => Some((field, opens)),
		_ => None,
	};
	messages.into_iter().filter_map(name_field).collect()
}

#[tokio::test]
async fn a_double_click_on_the_name_of_a_layer_opens_a_field_over_the_name_that_renames_the_layer() {
	let mut editor = engine_drawn_graph(1).await;
	let layer = first_layer(&editor).key;
	let [name, _] = first_layer(&editor).name_box();
	let on_the_name = name + DVec2::new(4., GRID_SIZE / 2.);
	let field = |text: &str| GraphNameField {
		name: text.to_string(),
		x: name.x,
		top: name.y,
		align_right: false,
		dimmed: false,
	};
	let original = first_layer(&editor).name;

	point_at(&mut editor, on_the_name, MouseKeys::empty());
	let opened = editor.dispatch(NodeGraphMessage::EnterNestedNetwork);
	assert_eq!(name_fields(opened), [(Some(field(&original)), true)], "the field opens over the drawn name");
	assert!(first_layer(&editor).renaming, "the layer leaves its name undrawn");
	assert_eq!(name_fields(editor.dispatch(NodeGraphMessage::EnterNestedNetwork)), [], "another double-click on the name");
	let panned = editor.dispatch(NavigationMessage::CanvasPan { delta: DVec2::new(-48., 0.) });
	assert_eq!(name_fields(panned), [], "the field pans with the graph");

	let renamed = editor.dispatch(NodeGraphMessage::EndEditName { name: Some("Renamed".to_string()) });
	assert_eq!(name_fields(renamed), [(None, false)]);
	assert_eq!((first_layer(&editor).name, first_layer(&editor).renaming), ("Renamed".to_string(), false), "the layer is renamed");

	point_at(&mut editor, on_the_name, MouseKeys::empty());
	assert_eq!(name_fields(editor.dispatch(NodeGraphMessage::EnterNestedNetwork)), [(Some(field("Renamed")), true)]);
	assert_eq!(name_fields(editor.dispatch(NodeGraphMessage::EndEditName { name: None })), [(None, false)]);
	assert_eq!(first_layer(&editor).name, "Renamed", "the edit is canceled");

	editor.dispatch(NodeGraphMessage::EnterNestedNetwork);
	let closed = editor.dispatch(DocumentMessage::GraphViewOverlay { open: false });
	assert_eq!(name_fields(closed), [(None, false)], "closing the graph ends the edit");
	assert_eq!(editor.active_document().node_graph_handler.graph_ui.editing_name, None);

	editor.dispatch(DocumentMessage::GraphViewOverlay { open: true });
	editor.dispatch(NodeGraphMessage::EnterNestedNetwork);
	let mut deleted = editor.dispatch(NodeGraphMessage::DeleteNodes {
		node_ids: vec![layer],
		delete_children: false,
	});
	deleted.extend(editor.dispatch(NodeGraphMessage::SendGraph));
	assert_eq!(name_fields(deleted), [(None, false)], "the renamed layer is deleted");
}

#[tokio::test]
async fn the_names_of_the_imports_and_exports_of_a_subnetwork_are_renamed_in_the_same_field() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let canvas = editor.dispatch(NodeGraphMessage::BeginEditName { name: GraphName::Export(0) });
	assert_eq!(name_fields(canvas), [], "the export of the document network keeps its name");

	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let network_interface = &editor.active_document().network_interface;
	let (index, bounds) = network_interface
		.import_export_name_bounds(&[merge])
		.into_iter()
		.find_map(|(name, bounds)| match name {
			ImportOrExport::Export(index) => Some((index, bounds)),
			ImportOrExport::Import(_) => None,
		})
		.expect("an export with a name");
	let original = network_interface.displayed_export_name(index, &[merge]);
	let field = |editor: &EditorTestUtils, kind: NetworkConnectorKind, index: usize, name: &str| {
		let [top_left, _] = network_connector(editor, kind, index).name_box(0.);
		GraphNameField {
			name: name.to_string(),
			x: top_left.x,
			top: top_left.y,
			align_right: kind == NetworkConnectorKind::Import,
			dimmed: false,
		}
	};

	point_at(&mut editor, middle(bounds), MouseKeys::empty());
	let opened = editor.dispatch(NodeGraphMessage::EnterNestedNetwork);
	assert_eq!(
		name_fields(opened),
		[(Some(field(&editor, NetworkConnectorKind::Export, index, &original)), true)],
		"a double-click on the export's name"
	);
	assert_eq!(renamed_row(&editor, NetworkConnectorKind::Export), Some(index), "the export leaves its name undrawn");

	let before = field(&editor, NetworkConnectorKind::Export, index, &original);
	let panned = editor.dispatch(NavigationMessage::CanvasPan { delta: DVec2::new(-48., 0.) });
	let after = field(&editor, NetworkConnectorKind::Export, index, &original);
	assert_ne!(after, before, "the exports keep their place in the viewport as the graph is panned");
	assert_eq!(name_fields(panned), [(Some(after), false)], "the field follows the export");

	let renamed = editor.dispatch(NodeGraphMessage::EndEditName { name: Some("Result".to_string()) });
	assert_eq!(name_fields(renamed), [(None, false)]);
	assert_eq!(editor.active_document().network_interface.displayed_export_name(index, &[merge]), "Result");
	assert_eq!(renamed_row(&editor, NetworkConnectorKind::Export), None);

	let imports = network_connectors(&editor, NetworkConnectorKind::Import);
	let import = imports.iter().find(|row| row.connector.is_some()).expect("an import").index;
	let name = editor.active_document().network_interface.displayed_import_name(import, &[merge]);
	let opened = editor.dispatch(NodeGraphMessage::BeginEditName { name: GraphName::Import(import) });
	assert_eq!(
		name_fields(opened),
		[(Some(field(&editor, NetworkConnectorKind::Import, import, &name)), true)],
		"the menu's Rename on an import"
	);
	let unchanged = editor.dispatch(NodeGraphMessage::EndEditName { name: Some(name.clone()) });
	assert_eq!(name_fields(unchanged), [(None, false)]);
	assert_eq!(editor.active_document().network_interface.displayed_import_name(import, &[merge]), name);
}

#[tokio::test]
async fn the_rename_of_an_export_is_dropped_once_another_export_takes_its_place() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let edited = editor.active_document().network_interface.number_of_exports(&[merge]);
	for name in ["Edited", "Next"] {
		editor.handle_message(NodeGraphMessage::AddExport).await;
		let index = ImportOrExport::Export(editor.active_document().network_interface.number_of_exports(&[merge]) - 1);
		editor.handle_message(NodeGraphMessage::SetImportExportName { name: name.to_string(), index }).await;
	}
	let export_name = |editor: &EditorTestUtils| editor.active_document().network_interface.displayed_export_name(edited, &[merge]);
	assert_eq!(export_name(&editor), "Edited");

	let opened = editor.dispatch(NodeGraphMessage::BeginEditName { name: GraphName::Export(edited) });
	assert_eq!(
		name_fields(opened).into_iter().map(|(field, _)| field.map(|field| field.name)).collect::<Vec<_>>(),
		[Some("Edited".to_string())]
	);
	let removed = editor.dispatch(NodeGraphMessage::RemoveExport { export_index: edited });
	assert_eq!(export_name(&editor), "Next", "the next export takes the place of the removed one");
	assert_eq!(name_fields(removed), [(None, false)], "the field closes rather than show the next export");
	assert_eq!(renamed_row(&editor, NetworkConnectorKind::Export), None, "the next export's name is drawn");

	editor.dispatch(NodeGraphMessage::EndEditName { name: Some("Typed".to_string()) });
	assert_eq!(export_name(&editor), "Next", "the text typed into the closed field renames nothing");
}

#[tokio::test]
async fn an_export_whose_rename_is_dropped_as_its_row_is_built_shows_its_name() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let edited = editor.active_document().network_interface.number_of_exports(&[merge]);
	editor.handle_message(NodeGraphMessage::AddExport).await;
	let index = ImportOrExport::Export(edited);
	editor.handle_message(NodeGraphMessage::SetImportExportName { name: "Edited".to_string(), index }).await;
	editor.dispatch(NodeGraphMessage::BeginEditName { name: GraphName::Export(edited) });
	assert_eq!(renamed_row(&editor, NetworkConnectorKind::Export), Some(edited));

	editor.active_document_mut().network_interface.set_import_export_name("Other".to_string(), index, &[merge]);
	let closed = editor.dispatch(PortfolioMessage::UpdateGraphUi { part: GraphUiPart::NetworkConnectors });
	assert_eq!(name_fields(closed), [(None, false)], "the field over a name that changed closes");
	assert_eq!(renamed_row(&editor, NetworkConnectorKind::Export), None, "the new name is drawn");
}

#[tokio::test]
async fn leaving_the_network_of_a_renamed_export_drops_the_rename() {
	let mut editor = engine_drawn_graph(0).await;
	let outer = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: outer }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let inner = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: inner }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let opened = editor.dispatch(NodeGraphMessage::BeginEditName { name: GraphName::Export(0) });
	assert_eq!(name_fields(opened).len(), 1, "the field opens over the export's name");

	let left = editor.dispatch(DocumentMessage::ExitNestedNetwork { steps_back: 1 });
	assert_eq!(name_fields(left), [(None, false)], "the field closes rather than show the outer network's export of the same name");
	assert_eq!(editor.active_document().node_graph_handler.graph_ui.editing_name, None);
}

#[tokio::test]
async fn the_menus_of_an_import_and_an_export_carry_their_descriptions_and_where_the_popovers_editing_them_point() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	editor.handle_message(DocumentMessage::MakeNetworkEditable).await;
	let network_interface = &editor.active_document().network_interface;
	let names = network_interface.import_export_name_bounds(&[merge]);
	let (imports, exports) = network_interface.import_export_position(&[merge]).unwrap();
	let row_center = |index: usize| if index == 0 { 0. } else { (index + 1) as f64 * GRID_SIZE };
	let sides = [
		(true, imports, "just right of the import's connector", 7.),
		(false, exports, "just left of the export's connector", -5.),
	];

	for (import, position, placement, beside_connector) in sides {
		let (index, bounds) = names
			.iter()
			.find_map(|&(name, bounds)| match name {
				ImportOrExport::Import(index) if import => Some((index, bounds)),
				ImportOrExport::Export(index) if !import => Some((index, bounds)),
				_ => None,
			})
			.expect("a name on each side");
		let named = if import { ImportOrExport::Import(index) } else { ImportOrExport::Export(index) };
		let what_it_is_for = format!("What {named:?} is for");
		editor
			.handle_message(NodeGraphMessage::SetImportExportDescription {
				description: what_it_is_for.clone(),
				index: named,
			})
			.await;

		point_at(&mut editor, middle(bounds), MouseKeys::empty());
		editor.dispatch(pointer_down(true));
		let menu = editor.active_document().node_graph_handler.context_menu.clone().map(|menu| menu.context_menu_data);
		let Some(ContextMenuData::ModifyImportExport {
			is_import,
			index: menu_index,
			description,
			description_anchor,
			..
		}) = menu
		else {
			panic!("the menu of {named:?}: {menu:?}")
		};
		assert_eq!((is_import, menu_index as usize), (import, index));
		assert_eq!(description, what_it_is_for);
		let anchor = position.as_dvec2() + DVec2::new(beside_connector, row_center(index));
		assert_eq!(description_anchor, anchor.into(), "{placement}, on the row of {named:?}");
	}
}

#[tokio::test]
async fn the_tooltip_of_a_layer_renamed_under_the_pointer_is_worded_with_its_new_name() {
	let mut editor = engine_drawn_graph(1).await;
	let row = first_layer(&editor);
	let [name, _] = row.name_box();
	let on_the_name = name + DVec2::new(4., GRID_SIZE / 2.);
	let implementation_name = editor.active_document().network_interface.implementation_name(&row.key, &[]);
	let worded = |name: &str| format!("{implementation_name} ({name})");

	assert_eq!(the_tooltip(point_at(&mut editor, on_the_name, MouseKeys::empty())).0, worded(&row.name));
	editor.dispatch(NodeGraphMessage::EnterNestedNetwork);

	let renamed = editor.dispatch(NodeGraphMessage::EndEditName { name: Some("Box".to_string()) });
	assert_eq!(the_tooltip(renamed).0, worded("Box"), "the pointer stays over the renamed layer");
	assert_eq!(
		tooltips(point_at(&mut editor, on_the_name + DVec2::new(1., 0.), MouseKeys::empty())),
		[],
		"the new name is already sent"
	);
}

#[tokio::test]
async fn the_tooltip_of_an_input_a_wire_is_dropped_on_under_the_pointer_tells_the_wired_type() {
	let mut editor = engine_drawn_graph(0).await;
	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let fill = editor.create_node_by_name_at(fill_definition(), 10, 4).await;
	let network_interface = &editor.active_document().network_interface;
	let output = network_interface.output_position(&OutputConnector::node(rectangle, 0), &[]).unwrap();
	let input = network_interface.input_position(&InputConnector::node_at_index(fill, 0), &[]).unwrap();
	let input_name = network_interface.frontend_input_from_connector(&InputConnector::node_at_index(fill, 0), &[]).unwrap().name;

	point_at(&mut editor, output, MouseKeys::empty());
	press_at(&mut editor, output);
	point_at(&mut editor, middle([output, input]), MouseKeys::LEFT);
	let (label, unwired) = the_tooltip(point_at(&mut editor, input, MouseKeys::LEFT));
	assert!(label == input_name && !unwired.contains("Wired Type"), "the wire is dragged onto the input: {label}\n{unwired}");

	let (label, wired) = the_tooltip(release_at(&mut editor, input));
	assert!(label == input_name && wired.contains("Wired Type"), "the wire is dropped on the input: {label}\n{wired}");
}

#[tokio::test]
async fn a_redirect_dot_has_the_tooltip_of_its_output_all_over() {
	let mut editor = engine_drawn_graph(0).await;
	let dot = editor.create_node_by_name_at(redirect_dot_definition(), 5, 6).await;
	let center = scene(&editor).dots().get(&dot).unwrap().center();
	let output = editor.active_document().network_interface.frontend_output_from_connector(&OutputConnector::node(dot, 0), &[]).unwrap();

	assert_eq!(the_tooltip(point_at(&mut editor, center, MouseKeys::empty())).0, output.name);
	assert_eq!(tooltips(point_at(&mut editor, center + DVec2::new(10., 0.), MouseKeys::empty())), [], "where a wire is pulled out");
	assert_eq!(tooltips(point_at(&mut editor, center - DVec2::new(10., 0.), MouseKeys::empty())), [], "where a wire is pulled in");
}

#[tokio::test]
async fn the_names_of_the_imports_and_the_connectors_for_adding_them_have_tooltips() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.active_document_mut().network_interface.make_editable(&[merge]);
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	let network_interface = &editor.active_document().network_interface;
	let (index, bounds) = network_interface
		.import_export_name_bounds(&[merge])
		.into_iter()
		.find_map(|(name, bounds)| match name {
			ImportOrExport::Import(index) => Some((index, bounds)),
			ImportOrExport::Export(_) => None,
		})
		.expect("an import with a name");
	let name = (network_interface.displayed_import_name(index, &[merge]), network_interface.import_description(index, &[merge]));
	let (add_index, add_center) = *network_interface.empty_import_export_connectors(true, &[merge]).last().unwrap();
	assert!(add_index > 0);

	assert_eq!(the_tooltip(point_at(&mut editor, middle(bounds), MouseKeys::empty())), name, "the name of the import");
	let add_import = (
		"Add Import".to_string(),
		"Drag a wire from here and connect it to an input, or drop one here from an input.".to_string(),
	);
	assert_eq!(the_tooltip(point_at(&mut editor, add_center, MouseKeys::empty())), add_import, "the connector for adding an import");
}

#[tokio::test]
async fn the_connector_a_press_would_grab_is_hovered_while_the_pointer_is_over_it_and_no_button_is_held() {
	let mut editor = engine_drawn_graph(0).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let output = editor.active_document().network_interface.output_position(&OutputConnector::node(node, 0), &[]).unwrap();
	let hovered = |editor: &EditorTestUtils| scene(editor).cards().get(&node).and_then(|card| card.primary_output.as_ref()).unwrap().highlight;
	assert_eq!(hovered(&editor), Highlight::None);

	point_at(&mut editor, output, MouseKeys::empty());
	assert_eq!(hovered(&editor), Highlight::Hovered, "the pointer is over the output");
	let before = renders(&editor);
	point_at(&mut editor, output + DVec2::new(1., 1.), MouseKeys::empty());
	assert_eq!(hovered(&editor), Highlight::Hovered, "the pointer is still over the output");
	assert_eq!(renders(&editor), before, "the graph is not drawn again while the same connector stays hovered");

	point_at(&mut editor, output + DVec2::new(0., 48.), MouseKeys::empty());
	assert_eq!(hovered(&editor), Highlight::None, "the pointer has left the output");
	assert!(renders(&editor) > before, "the graph is drawn again without the hover");

	point_at(&mut editor, output, MouseKeys::empty());
	assert_eq!(hovered(&editor), Highlight::Hovered);
	point_at(&mut editor, DVec2::new(-2000., 0.), MouseKeys::empty());
	assert_eq!(hovered(&editor), Highlight::None, "the pointer has left the graph");

	point_at(&mut editor, output, MouseKeys::empty());
	assert_eq!(hovered(&editor), Highlight::Hovered);
	press_at(&mut editor, output);
	assert_ne!(hovered(&editor), Highlight::Hovered, "a held button hides the hover");
	release_at(&mut editor, output);
	assert_eq!(hovered(&editor), Highlight::Hovered, "the hover is back once the button is released");
}

#[tokio::test]
async fn a_hovered_export_is_highlighted_in_its_row_as_the_rows_built_again_would_have_it() {
	let mut editor = engine_drawn_graph(0).await;
	let merge = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	editor.active_document_mut().network_interface.make_editable(&[merge]);
	editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
	let exports = network_connectors(&editor, NetworkConnectorKind::Export);
	let export = exports.iter().find(|row| row.connector.is_some()).expect("an export");
	let add = exports.iter().rfind(|row| row.connector.is_none()).expect("a connector for adding an export");
	let highlights = |editor: &EditorTestUtils| {
		[export.index, add.index].map(|index| {
			let row = network_connector(editor, NetworkConnectorKind::Export, index);
			row.connector.map_or(row.add_highlight, |connector| connector.highlight)
		})
	};

	point_at(&mut editor, export.anchor(), MouseKeys::empty());
	assert_eq!(highlights(&editor), [Highlight::Hovered, Highlight::None], "the pointer is over the export");
	assert_eq!(changed_by_a_rebuild(&mut editor), [0; 6]);

	point_at(&mut editor, add.anchor(), MouseKeys::empty());
	assert_eq!(highlights(&editor), [Highlight::None, Highlight::Hovered], "the pointer is over the connector for adding an export");
	assert_eq!(changed_by_a_rebuild(&mut editor), [0; 6]);

	point_at(&mut editor, DVec2::new(-2000., 0.), MouseKeys::empty());
	assert_eq!(highlights(&editor), [Highlight::None; 2], "the pointer has left the graph");
	assert_eq!(changed_by_a_rebuild(&mut editor), [0; 6]);
}

#[tokio::test]
async fn a_redirect_dot_grows_where_it_would_be_dragged_and_shows_its_output_where_a_wire_would_be_pulled_out() {
	let mut editor = engine_drawn_graph(0).await;
	let dot = editor.create_node_by_name_at(redirect_dot_definition(), 5, 6).await;
	let center = scene(&editor).dots().get(&dot).unwrap().center();
	let looks = |editor: &EditorTestUtils| scene(editor).dots().get(&dot).map(|dot| (dot.highlight, dot.output_highlight)).unwrap();
	assert_eq!(looks(&editor), (Highlight::None, Highlight::None));

	point_at(&mut editor, center, MouseKeys::empty());
	assert_eq!(looks(&editor), (Highlight::Hovered, Highlight::None), "the pointer is over the dot");
	point_at(&mut editor, center + DVec2::new(0., 48.), MouseKeys::empty());
	assert_eq!(looks(&editor), (Highlight::None, Highlight::None), "the pointer has left the dot");
	point_at(&mut editor, center + DVec2::new(10., 0.), MouseKeys::empty());
	assert_eq!(looks(&editor), (Highlight::None, Highlight::Hovered), "the pointer is right of the dot, where a wire is pulled out");
	point_at(&mut editor, center - DVec2::new(10., 0.), MouseKeys::empty());
	assert_eq!(looks(&editor), (Highlight::None, Highlight::None), "the pointer is left of the unwired dot, where a wire is pulled in");

	point_at(&mut editor, center, MouseKeys::empty());
	press_at(&mut editor, center);
	point_at(&mut editor, center + DVec2::new(48., 0.), MouseKeys::LEFT);
	assert_eq!(looks(&editor).0, Highlight::Hovered, "the pressed dot stays grown while it is dragged");
	release_at(&mut editor, center + DVec2::new(48., 0.));
	point_at(&mut editor, center + DVec2::new(48., 48.), MouseKeys::empty());
	assert_eq!(looks(&editor), (Highlight::None, Highlight::None), "the dot is released");
}

#[tokio::test]
async fn a_hovered_layer_button_shows_what_a_click_would_do_until_it_is_clicked_and_the_pointer_leaves() {
	let mut editor = engine_drawn_graph(1).await;
	let hovered = |editor: &EditorTestUtils| {
		let layer = first_layer(editor);
		(layer.hovered_button, layer.hovered_button_clicked)
	};
	let visibility = middle(first_layer(&editor).visibility());
	let grip = middle(first_layer(&editor).grip());

	point_at(&mut editor, visibility, MouseKeys::empty());
	assert_eq!(hovered(&editor), (Some(LayerButton::Visibility), false), "the pointer is over the visibility button");
	press_at(&mut editor, visibility);
	release_at(&mut editor, visibility);
	assert!(!first_layer(&editor).visible, "the click hides the layer");
	assert_eq!(hovered(&editor), (Some(LayerButton::Visibility), true), "the clicked button shows the layer's state");
	point_at(&mut editor, visibility + DVec2::new(1., 1.), MouseKeys::empty());
	assert_eq!(hovered(&editor), (Some(LayerButton::Visibility), true), "the pointer is still over the clicked button");

	point_at(&mut editor, grip, MouseKeys::empty());
	assert_eq!(hovered(&editor), (Some(LayerButton::Grip), false), "the pointer is over the grip");
	point_at(&mut editor, visibility, MouseKeys::empty());
	assert_eq!(hovered(&editor), (Some(LayerButton::Visibility), false), "the pointer is back over the button");
	point_at(&mut editor, visibility + DVec2::new(0., 96.), MouseKeys::empty());
	assert_eq!(hovered(&editor), (None, false), "the pointer has left the layer");
}

#[tokio::test]
async fn hovering_a_layer_button_changes_only_that_layer_row() {
	let mut editor = engine_drawn_graph(2).await;
	let rows = |editor: &EditorTestUtils| scene(editor).layers().iter().cloned().collect::<Vec<_>>();
	let before = rows(&editor);
	let [hovered, other] = [&before[0], &before[1]];

	point_at(&mut editor, middle(hovered.visibility()), MouseKeys::empty());
	let after = rows(&editor);
	let row = |key| after.iter().find(|layer| layer.key == key);
	assert_eq!(row(hovered.key).and_then(|layer| layer.hovered_button), Some(LayerButton::Visibility));
	assert_eq!(row(other.key), Some(other), "the other layer row is kept");
	assert_eq!(changed_by_a_rebuild(&mut editor), [0; 6], "the rows are those a rebuild gives");
}

#[tokio::test]
async fn an_error_badge_is_hovered_while_the_pointer_is_over_it_or_the_box_around_its_tail() {
	let mut editor = engine_drawn_graph(0).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 3, 10).await;
	editor.dispatch(NodeGraphMessage::UpdateEvaluationErrors {
		errors: vec![NodeEvaluationError {
			node_path: vec![node],
			message: "Failed".to_string(),
		}],
	});
	let badge = |editor: &EditorTestUtils| scene(editor).errors().get(&node).cloned().unwrap();
	let [min, max] = badge(&editor).body(badge(&editor).text_size());
	assert!(!badge(&editor).hovered);

	point_at(&mut editor, middle([min, max]), MouseKeys::empty());
	assert!(badge(&editor).hovered, "the pointer is over the badge");
	let [tail, ..] = badge(&editor).tail();
	point_at(&mut editor, tail + DVec2::new(1., 7.), MouseKeys::empty());
	assert!(badge(&editor).hovered, "the pointer is beside the tip of the tail");
	point_at(&mut editor, max + DVec2::new(24., 0.), MouseKeys::empty());
	assert!(!badge(&editor).hovered, "the pointer has left the badge");
}

#[tokio::test]
async fn the_size_of_an_error_text_is_kept_for_hit_tests_until_no_badge_shows_it() {
	let mut editor = engine_drawn_graph(0).await;
	let node = editor.create_node_by_name_at(rectangle_definition(), 3, 10).await;
	let fail_and_point_at_the_badge = |editor: &mut EditorTestUtils, message: &str| {
		editor.dispatch(NodeGraphMessage::UpdateEvaluationErrors {
			errors: vec![NodeEvaluationError {
				node_path: vec![node],
				message: message.to_string(),
			}],
		});
		let tail = scene(editor).errors().get(&node).unwrap().tail()[0];
		point_at(editor, tail, MouseKeys::empty());
	};
	let measured = |editor: &EditorTestUtils| editor.active_document().node_graph_handler.graph_ui.error_text_sizes.keys().cloned().collect::<Vec<_>>();

	fail_and_point_at_the_badge(&mut editor, "Failed");
	assert_eq!(measured(&editor), ["Failed"], "measured when the pointer first meets the badge");
	fail_and_point_at_the_badge(&mut editor, "Missing input");
	assert_eq!(measured(&editor), ["Missing input"], "the text no badge shows any more is forgotten");
}

fn animation_frame_without_rendering(editor: &mut EditorTestUtils, timestamp: u64) {
	editor.dispatch(InputPreprocessorMessage::CurrentTime { timestamp });
	editor.dispatch(BroadcastMessage::TriggerEvent(EventMessage::AnimationFrame));
}

async fn a_rectangle_wired_into_a_fill() -> (EditorTestUtils, NodeId) {
	let mut editor = engine_drawn_graph(0).await;
	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let fill = editor.create_node_by_name_at(fill_definition(), 10, 4).await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(rectangle, 0),
			input_connector: InputConnector::node_at_index(fill, 0),
		})
		.await;
	(editor, fill)
}

#[tokio::test]
async fn a_dragged_node_glides_into_its_new_cell_with_its_wire_following_it_and_frames_stop_once_it_is_there() {
	let (mut editor, fill) = a_rectangle_wired_into_a_fill().await;
	let card = |editor: &EditorTestUtils| scene(editor).cards().get(&fill).cloned().unwrap();
	let wire_end = |editor: &EditorTestUtils| {
		let wire = wire(editor, WireKey::Input { node: fill, index: 0 }).unwrap();
		let end = wire.path.elements().last().and_then(|element| element.end_point()).unwrap();
		DVec2::new(end.x, end.y)
	};
	let input = |editor: &EditorTestUtils| editor.active_document().network_interface.input_position(&InputConnector::node_at_index(fill, 0), &[]).unwrap();
	let grabbed = middle(card(&editor).body());
	let one_cell = DVec2::new(GRID_SIZE, 0.);

	point_at(&mut editor, grabbed, MouseKeys::empty());
	press_at(&mut editor, grabbed);
	point_at(&mut editor, grabbed + one_cell, MouseKeys::LEFT);
	assert_eq!(card(&editor).cell, IVec2::new(11, 4), "the node is dragged a cell to the right");
	assert_eq!(card(&editor).glide, -one_cell, "it is drawn where it was");
	assert_eq!(wire_end(&editor), input(&editor) - one_cell, "the wire meets the node where it is drawn");
	release_at(&mut editor, grabbed + one_cell);

	let mut time = 1000;
	animation_frame_without_rendering(&mut editor, time);
	for _ in 0..3 {
		time += 16;
		let before = renders(&editor);
		animation_frame_without_rendering(&mut editor, time);
		let glide = card(&editor).glide;
		assert!(glide.x > -GRID_SIZE && glide.x < 0. && glide.y == 0., "the node glides toward its cell: {glide}");
		assert_eq!(wire_end(&editor), input(&editor) + glide, "the wire follows the gliding node");
		assert!(renders(&editor) > before, "the graph is drawn again for each frame of the glide");
	}
	for _ in 0..30 {
		time += 16;
		animation_frame_without_rendering(&mut editor, time);
	}
	assert_eq!(card(&editor).glide, DVec2::ZERO, "the node has arrived in its cell");
	assert_eq!(wire_end(&editor), input(&editor), "the wire meets the node in its cell");
	let before = renders(&editor);
	animation_frame_without_rendering(&mut editor, time + 16);
	assert_eq!(renders(&editor), before, "no frame is drawn once the glide has ended");
}

#[tokio::test]
async fn a_drag_step_inside_a_cell_sends_no_graph_ui_delta_and_no_render_request() {
	let (mut editor, fill) = a_rectangle_wired_into_a_fill().await;
	let grabbed = middle(scene(&editor).cards().get(&fill).unwrap().body());
	point_at(&mut editor, grabbed, MouseKeys::empty());
	press_at(&mut editor, grabbed);
	point_at(&mut editor, grabbed + DVec2::new(GRID_SIZE, 0.), MouseKeys::LEFT);
	let wired = WireKey::Input { node: fill, index: 0 };
	editor.portfolio_message_handler_mut().executor.update_graph_ui(|scene| assert!(scene.wires.remove(&wired)));
	let (scene_before, renders_before) = (scene(&editor).clone(), renders(&editor));

	point_at(&mut editor, grabbed + DVec2::new(30., 0.), MouseKeys::LEFT);
	let position = editor.active_document().network_interface.position(&fill, &[]);
	assert_eq!(position, Some(IVec2::new(11, 4)), "the node stays in the cell it was dragged into");
	assert_eq!(scene(&editor), &scene_before, "the wires are not routed again, which would bring back the removed one");
	assert_eq!(renders(&editor), renders_before, "the graph is not drawn again");
}

#[tokio::test]
async fn a_wire_dragged_out_of_an_output_lights_it_up_and_enlarges_the_input_it_snaps_to() {
	let mut editor = engine_drawn_graph(0).await;
	let rectangle = editor.create_node_by_name_at(rectangle_definition(), 0, 0).await;
	let fill = editor.create_node_by_name_at(fill_definition(), 10, 4).await;
	let network_interface = &editor.active_document().network_interface;
	let output = network_interface.output_position(&OutputConnector::node(rectangle, 0), &[]).unwrap();
	let input = network_interface.input_position(&InputConnector::node_at_index(fill, 0), &[]).unwrap();
	let ends = |editor: &EditorTestUtils| {
		let cards = scene(editor).cards();
		let output = cards.get(&rectangle).unwrap().primary_output.as_ref().unwrap().highlight;
		let input = cards.get(&fill).unwrap().primary_input.as_ref().unwrap().highlight;
		(output, input)
	};

	point_at(&mut editor, output, MouseKeys::empty());
	press_at(&mut editor, output);
	assert_eq!(ends(&editor), (Highlight::WireSource, Highlight::None), "the pressed output is lit as the source of a wire");
	point_at(&mut editor, middle([output, input]), MouseKeys::LEFT);
	assert_eq!(ends(&editor), (Highlight::WireSource, Highlight::None), "the wire is dragged out of the output");
	point_at(&mut editor, input, MouseKeys::LEFT);
	assert_eq!(ends(&editor), (Highlight::WireSource, Highlight::WireTarget), "the wire has snapped to the input");
	point_at(&mut editor, input + DVec2::new(-48., 48.), MouseKeys::LEFT);
	assert_eq!(ends(&editor), (Highlight::WireSource, Highlight::None), "the wire has left the input");

	editor.dispatch(pointer_down(true));
	assert_eq!(ends(&editor), (Highlight::None, Highlight::None), "the wire is aborted");
}

fn misses_in_open_network(editor: &EditorTestUtils) -> Vec<String> {
	let document = editor.active_document();
	let (network_interface, network_path) = (&document.network_interface, document.breadcrumb_network_path());
	let scene = scene(editor);
	let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
	let bounds = |target: &ClickTarget| target.bounding_box_with_transform(DAffine2::IDENTITY);
	let targets = |node: &NodeId| network_interface.with_node_click_targets(node, network_path, DocumentNodeClickTargets::clone);
	let exposed_inputs = |node: NodeId| {
		let exposed = move |&index: &usize| network_interface.frontend_input_from_connector(&InputConnector::node_at_index(node, index), network_path).is_some();
		(1..network_interface.number_of_inputs(&node, network_path)).filter(exposed)
	};
	let mut misses = Vec::new();
	let mut check = |hit: bool, miss: String| {
		if !hit {
			misses.push(miss);
		}
	};
	check(!scene.cards().is_empty() || !scene.layers().is_empty(), "no node is drawn".to_string());
	check(scene.network_connectors().iter().any(|row| row.kind == NetworkConnectorKind::Export), "no export is drawn".to_string());

	for card in scene.cards() {
		let name = format!("card {} \"{}\"", card.key, card.name);
		let Some(targets) = targets(&card.key) else {
			check(false, format!("{name} has no click targets"));
			continue;
		};
		let [min, max] = card.body();
		let target = bounds(&targets.node_click_target);
		let on_target = target.is_some_and(|[target_min, target_max]| near(min.x, target_min.x) && near(min.y, target_min.y) && near(max.x, target_max.x) && near(max.y, target_max.y));
		check(on_target, format!("{name}: body {:?}, click target {target:?}", [min, max]));
		let ports = &targets.port_click_targets;
		let inputs = card.primary_input.iter().map(|_| 0).chain(exposed_inputs(card.key));
		for ((top_left, _), index) in card.inputs().zip(inputs) {
			let hit = ports.clicked_input_port_from_point(top_left + CONNECTOR_SIZE / 2.);
			check(hit == Some(index), format!("{name}: input {index} drawn at {top_left} hits input {hit:?}"));
		}
		let outputs = card.primary_output.iter().map(|_| 0).chain(1..network_interface.number_of_outputs(&card.key, network_path));
		for ((top_left, _), index) in card.outputs().zip(outputs) {
			let hit = ports.clicked_output_port_from_point(top_left + CONNECTOR_SIZE / 2.);
			check(hit == Some(index), format!("{name}: output {index} drawn at {top_left} hits output {hit:?}"));
		}
	}

	for layer in scene.layers() {
		let name = format!("layer {} \"{}\"", layer.key, layer.name);
		let Some(targets) = targets(&layer.key) else {
			check(false, format!("{name} has no click targets"));
			continue;
		};
		let NodeTypeClickTargets::Layer(buttons) = &targets.node_type_metadata else {
			check(false, format!("{name} has the click targets of a node"));
			continue;
		};
		let Some([target_min, target_max]) = bounds(&targets.node_click_target) else {
			check(false, format!("{name} has an empty click target"));
			continue;
		};
		let [min, max] = layer.body();
		let on_target = near(min.y, target_min.y) && near(max.x, target_max.x) && near(max.y, target_max.y);
		check(on_target, format!("{name}: body {:?}, click target {:?}", [min, max], [target_min, target_max]));
		let undrawn_strip = if layer.chain_width > 0 { 0. } else { LAYER_LEFT_MARGIN };
		check(
			near(min.x - target_min.x, undrawn_strip),
			format!("{name}: drawn from x {}, clickable from x {}, not {undrawn_strip} px apart", min.x, target_min.x),
		);
		let inside = |target: &ClickTarget, part: [DVec2; 2]| target.intersect_point_no_stroke(middle(part));
		check(inside(&targets.node_click_target, layer.thumbnail()), format!("{name}: thumbnail {:?}", layer.thumbnail()));
		check(
			inside(&buttons.visibility_click_target, layer.visibility()),
			format!("{name}: visibility button {:?}", layer.visibility()),
		);
		match (layer.lock(), &buttons.lock_click_target) {
			(Some(lock), Some(target)) => check(inside(target, lock), format!("{name}: lock button {lock:?}")),
			(None, None) => {}
			(lock, target) => check(false, format!("{name}: lock button {lock:?}, click target {:?}", target.as_ref().map(bounds))),
		}
		check(inside(&buttons.grip_click_target, layer.grip()), format!("{name}: grip {:?}", layer.grip()));
		if !layer.name.is_empty() {
			let [top_left, bottom_right] = layer.name_box();
			let start = DVec2::new(top_left.x + 4., (top_left.y + bottom_right.y) / 2.);
			let hit = buttons.name_click_target.as_ref().is_some_and(|target| target.intersect_point_no_stroke(start));
			check(hit, format!("{name}: the start of the name at {start}"));
		}
		let ports = &targets.port_click_targets;
		if layer.primary_output.is_some() {
			let hit = ports.clicked_output_port_from_point(layer.stack_output() + STACK_CONNECTOR_SIZE / 2.);
			check(hit == Some(0), format!("{name}: stack output drawn at {} hits output {hit:?}", layer.stack_output()));
		}
		let hit = ports.clicked_input_port_from_point(layer.stack_input() + STACK_CONNECTOR_SIZE / 2.);
		check(hit == Some(0), format!("{name}: stack input drawn at {} hits input {hit:?}", layer.stack_input()));
		if layer.secondary_input.is_some() {
			let index = exposed_inputs(layer.key).next();
			let hit = ports.clicked_input_port_from_point(layer.secondary_input_connector() + CONNECTOR_SIZE / 2.);
			check(hit == index, format!("{name}: input {index:?} drawn at {} hits input {hit:?}", layer.secondary_input_connector()));
		}
	}

	for dot in scene.dots() {
		let name = format!("dot {}", dot.key);
		let Some(targets) = targets(&dot.key) else {
			check(false, format!("{name} has no click targets"));
			continue;
		};
		check(targets.node_click_target.intersect_point_no_stroke(dot.center()), format!("{name}: center {}", dot.center()));
		let arrow = dot.center() + DOT_OUTPUT_ARROW;
		let hit = targets.port_click_targets.clicked_output_port_from_point(arrow + CONNECTOR_SIZE / 2.);
		check(hit == Some(0), format!("{name}: output drawn at {arrow} hits output {hit:?}"));
	}

	let ports = network_interface.with_import_export_ports(network_path, Ports::clone).unwrap_or_default();
	let names = network_interface.import_export_name_bounds(network_path);
	for kind in [NetworkConnectorKind::Import, NetworkConnectorKind::Export] {
		let rows: Vec<&NetworkConnector> = scene.network_connectors().iter().filter(|row| row.kind == kind).collect();
		let is_import = kind == NetworkConnectorKind::Import;
		let added = |center: DVec2| network_interface.empty_import_export_connector_at(center, is_import, network_path).map(|(index, _)| index);
		let drawn = rows.iter().filter(|row| row.connector.is_some()).count();
		let placed = if is_import { ports.output_ports().count() } else { ports.input_ports().count() };
		check(drawn == placed, format!("{kind:?}: {drawn} connectors drawn for {placed} ports"));
		for row in rows {
			let (index, name) = (row.index, format!("{kind:?} {}", row.index));
			let center = row.connector_top_left() + CONNECTOR_SIZE / 2.;
			if row.connector.is_none() {
				let hit = added(center);
				check(hit == Some(index), format!("{name}: the connector for adding one drawn around {center} adds {hit:?}"));
				continue;
			}
			let hit = if is_import {
				ports.clicked_output_port_from_point(center)
			} else {
				ports.clicked_input_port_from_point(center)
			};
			check(hit == Some(index), format!("{name}: connector drawn around {center} hits {hit:?}"));
			if !row.editable {
				continue;
			}
			let key = if is_import { ImportOrExport::Import(index) } else { ImportOrExport::Export(index) };
			let Some(&(_, [target_min, target_max])) = names.iter().find(|(name, _)| *name == key) else {
				check(false, format!("{name}: its name has no click target"));
				continue;
			};
			let [top_left, _] = row.name_box(0.);
			let start = top_left + DVec2::new(if is_import { -4. } else { 4. }, GRID_SIZE / 2.);
			let on_target = target_min.cmple(start).all() && start.cmple(target_max).all();
			check(on_target, format!("{name}: the start of its name at {start}, click target {:?}", [target_min, target_max]));
			check(
				near(target_min.y - top_left.y, IMPORT_EXPORT_NAME_RAISE),
				format!("{name}: name drawn from y {}, clickable from y {}, not {IMPORT_EXPORT_NAME_RAISE} px apart", top_left.y, target_min.y),
			);
		}
	}
	misses
}

const ISOMETRIC_FOUNTAIN: &str = "isometric-fountain.graphite";

fn open_demo_artwork(editor: &mut EditorTestUtils, file_name: &str) {
	let data = std::fs::read(format!("../demo-artwork/{file_name}")).unwrap();
	editor.dispatch(IngestMessage::Ingest {
		content: DataTransferItem::from_file(file_name.into(), None, data).into(),
		action: IngestAction::Open,
	});
}

#[tokio::test]
async fn every_drawn_part_of_the_demo_documents_lies_on_its_click_target() {
	let mut editor = EditorTestUtils::create();
	let mut misses = Vec::new();
	for (document_name, _, file_name) in ARTWORK {
		open_demo_artwork(&mut editor, file_name);
		editor.dispatch(ViewportMessage::Update {
			x: 0.,
			y: 0.,
			width: 1200.,
			height: 800.,
			scale: 1.,
		});
		editor.dispatch(DocumentMessage::GraphViewOverlay { open: true });
		misses.extend(misses_in_open_network(&editor).into_iter().map(|miss| format!("{document_name}: {miss}")));

		let network_interface = &editor.active_document().network_interface;
		let document_network = network_interface.document_network();
		let Some(&network) = document_network.nodes.keys().filter(|node| document_network.nested_network(&[**node]).is_some()).min() else {
			continue;
		};
		editor.dispatch(DocumentMessage::EnterNestedNetwork { node_id: network });
		misses.extend(misses_in_open_network(&editor).into_iter().map(|miss| format!("{document_name}, inside node {network}: {miss}")));
	}
	assert!(misses.is_empty(), "{} drawn parts miss their click targets:\n{}", misses.len(), misses.join("\n"));
}

#[tokio::test]
async fn a_layer_without_a_chain_and_a_redirect_dot_lie_on_their_click_targets() {
	let mut editor = engine_drawn_graph(0).await;
	let layer = editor.create_node_by_name_at(merge_definition(), 0, 0).await;
	let hide_left_input = NodeGraphMessage::ExposeInput {
		input_connector: InputConnector::node_at_index(layer, 1),
		set_to_exposed: false,
		start_transaction: true,
	};
	editor.handle_message(hide_left_input).await;
	editor.create_node_by_name_at(redirect_dot_definition(), 20, 0).await;
	let scene = scene(&editor);
	let unchained = scene.layers().get(&layer).is_some_and(|layer| layer.chain_width == 0);
	assert!(unchained && !scene.dots().is_empty(), "a layer without a chain and a redirect dot");
	let misses = misses_in_open_network(&editor);
	assert!(misses.is_empty(), "{} drawn parts miss their click targets:\n{}", misses.len(), misses.join("\n"));
}

const DRAG_FRAMES: usize = 24;
const DRAG_PER_FRAME: f64 = 12.;
const SAMPLES: usize = 5;

fn median<T: Ord + Copy + Default>(mut values: Vec<T>) -> T {
	values.sort();
	values.get(values.len() / 2).copied().unwrap_or_default()
}

fn mean(times: &[Duration]) -> Duration {
	times.iter().sum::<Duration>() / times.len().max(1) as u32
}

fn ms(time: Duration) -> f64 {
	time.as_secs_f64() * 1000.
}

fn time_alone<T>(mut part: impl FnMut() -> T) -> Duration {
	let times = (0..SAMPLES).map(|_| {
		let start = Instant::now();
		std::hint::black_box(part());
		start.elapsed()
	});
	median(times.collect())
}

async fn settle(editor: &mut EditorTestUtils) {
	for _ in 0..16 {
		let before = renders(editor);
		editor.runtime.update();
		editor.runtime.run().await;
		editor.dispatch(Message::Wake);
		if renders(editor) == before {
			break;
		}
	}
}

fn changed<Row: Keyed + PartialEq>(before: &ReplicatedMap<Row>, after: &ReplicatedMap<Row>) -> usize {
	let removed = before.iter().filter(|row| after.get(&row.key()).is_none()).count();
	removed + after.iter().filter(|row| before.get(&row.key()) != Some(row)).count()
}

fn changed_rows(before: &GraphUiScene, after: &GraphUiScene) -> [usize; 6] {
	[
		changed(before.cards(), after.cards()),
		changed(before.layers(), after.layers()),
		changed(before.dots(), after.dots()),
		changed(before.wires(), after.wires()),
		changed(before.network_connectors(), after.network_connectors()),
		changed(before.errors(), after.errors()),
	]
}

fn changed_by_a_rebuild(editor: &mut EditorTestUtils) -> [usize; 6] {
	let before = scene(editor).clone();
	editor.dispatch(PortfolioMessage::UpdateGraphUi { part: GraphUiPart::All });
	changed_rows(&before, scene(editor))
}

fn sent(messages: &[FrontendMessage]) -> Vec<(String, usize)> {
	let size = |message: &FrontendMessage| serde_json::to_vec(message).map_or(0, |json| json.len());
	messages.iter().map(|message| (message.to_discriminant().local_name(), size(message))).collect()
}

#[derive(Default)]
struct DragTimes {
	inside: Vec<Duration>,
	crossing: Vec<Duration>,
	glide_frames: Vec<Duration>,
	renders: Vec<Duration>,
	responses: Vec<Duration>,
	rows_changed: Vec<[usize; 6]>,
	rows_changed_inside: usize,
	requests_inside: u64,
	sent_inside: Vec<(String, usize)>,
	sent_crossing: Vec<(String, usize)>,
}

async fn drag(editor: &mut EditorTestUtils, grab: DVec2, events_per_frame: usize, times: &mut DragTimes) -> Vec<Duration> {
	let hash = editor.active_document().network_interface.network_hash();
	let cell = |editor: &EditorTestUtils| editor.active_document().network_interface.drag_preview(&[]).0;
	let mut per_frame = Vec::new();
	let mut time = 1000;
	for frame in 0..DRAG_FRAMES {
		let mut editor_time = Duration::ZERO;
		for event in 1..=events_per_frame {
			let travelled = (frame * events_per_frame + event) as f64 * DRAG_PER_FRAME / events_per_frame as f64;
			let editor_mouse_state = pointer_state(editor, grab + DVec2::new(travelled, 0.), MouseKeys::LEFT);
			let cell_before = cell(editor);
			let scene_before = scene(editor).clone();
			let requests = renders(editor);

			let start = Instant::now();
			let messages = editor.editor.handle_message(InputPreprocessorMessage::PointerMove {
				editor_mouse_state,
				modifier_keys: ModifierKeys::empty(),
			});
			let dispatch = start.elapsed();
			editor_time += dispatch;

			assert_eq!(editor.active_document().network_interface.network_hash(), hash, "a drag step recompiles nothing");
			let rows_changed = changed_rows(&scene_before, scene(editor));
			if cell(editor) == cell_before {
				times.inside.push(dispatch);
				times.rows_changed_inside += rows_changed.iter().sum::<usize>();
				times.requests_inside += renders(editor) - requests;
				times.sent_inside = sent(&messages);
			} else {
				times.crossing.push(dispatch);
				times.rows_changed.push(rows_changed);
				times.sent_crossing = sent(&messages);
			}
		}

		time += 16;
		let start = Instant::now();
		editor.editor.handle_message(Message::Batched {
			messages: Box::new([
				InputPreprocessorMessage::CurrentTime { timestamp: time }.into(),
				AnimationMessage::IncrementFrameCounter.into(),
				BroadcastMessage::TriggerEvent(EventMessage::AnimationFrame).into(),
			]),
		});
		let glide_frame = start.elapsed();
		let start = Instant::now();
		editor.runtime.update();
		editor.runtime.run().await;
		times.renders.push(start.elapsed());
		let start = Instant::now();
		editor.dispatch(Message::Wake);
		let response = start.elapsed();

		times.glide_frames.push(glide_frame);
		times.responses.push(response);
		per_frame.push(editor_time + glide_frame + response);
	}
	per_frame
}

fn parts_alone(editor: &mut EditorTestUtils) -> Vec<(&'static str, Duration)> {
	let mut parts = {
		let facts = Facts::of(editor.active_document());
		vec![
			("Facts::cards", time_alone(|| facts.cards())),
			("layers", time_alone(|| facts.layers())),
			("dots", time_alone(|| facts.dots())),
			("wires", time_alone(|| facts.wires(GraphWireStyle::default()))),
			("network_connectors", time_alone(|| facts.network_connectors())),
			("errors", time_alone(|| facts.errors())),
		]
	};
	parts.push((
		"whole UpdateGraphUi All dispatch (a full rebuild)",
		time_alone(|| editor.editor.handle_message(PortfolioMessage::UpdateGraphUi { part: GraphUiPart::All })),
	));
	let moved: Vec<NodeId> = {
		let network_interface = &mut editor.active_document_mut().network_interface;
		let nodes: Vec<NodeId> = network_interface.document_network().nodes.keys().copied().collect();
		let before: HashMap<NodeId, IVec2> = network_interface.positions(nodes.iter().copied(), &[]).into_iter().collect();
		network_interface.shift_selected_nodes(IVec2::X, false, &[]);
		let after = network_interface.positions(nodes, &[]);
		network_interface.shift_selected_nodes(IVec2::NEG_X, false, &[]);
		after.into_iter().filter(|(node, position)| before.get(node) != Some(position)).map(|(node, _)| node).collect()
	};
	parts.push((
		"UpdateGraphUi Moved dispatch of the nodes a one unit shift moves (what a crossing sends)",
		time_alone(|| {
			editor.editor.handle_message(PortfolioMessage::UpdateGraphUi {
				part: GraphUiPart::Moved { nodes: moved.clone() },
			})
		}),
	));
	let network_interface = &mut editor.active_document_mut().network_interface;
	let mut shift = |offset| {
		network_interface.shift_selected_nodes(offset, false, &[]);
		network_interface.import_export_position(&[])
	};
	let shift_and_back = time_alone(|| [shift(IVec2::X), shift(IVec2::NEG_X)]);
	parts.push(("layout: one unit shift and the import and export placement", shift_and_back / 2));
	parts
}

fn open_the_isometric_fountain() -> EditorTestUtils {
	let mut editor = EditorTestUtils::create();
	open_demo_artwork(&mut editor, ISOMETRIC_FOUNTAIN);
	editor.dispatch(ViewportMessage::Update {
		x: 0.,
		y: 0.,
		width: 1920.,
		height: 1080.,
		scale: 1.,
	});
	editor.dispatch(DocumentMessage::GraphViewOverlay { open: true });
	editor
}

fn drag_subjects(editor: &EditorTestUtils) -> [(String, DVec2, Vec<NodeId>); 3] {
	let scene = scene(editor);
	let connected = |card: &Card| card.inputs().chain(card.outputs()).filter(|(_, connector)| connector.connected).count();
	let network_interface = &editor.active_document().network_interface;
	let card = scene.cards().iter().max_by_key(|card| connected(card)).expect("a card");
	let in_a_chain = if network_interface.is_chain(&card.key, &[]) { ", in a layer's chain" } else { "" };
	let layer = scene.layers().iter().find(|layer| layer.primary_output_connected_to_layer).expect("a layer in a stack");
	let mut anchors: Vec<&Layer> = scene.layers().iter().filter(|other| network_interface.is_absolute(&other.key, &[])).collect();
	anchors.sort_by_key(|other| ((other.cell - layer.cell).length_squared(), other.key));
	let three: Vec<NodeId> = anchors.iter().take(3).map(|anchor| anchor.key).collect();
	[
		(
			format!("a node \"{}\" with {} wired connectors{in_a_chain}", card.name, connected(card)),
			middle(card.body()),
			Vec::new(),
		),
		(format!("a layer in a stack \"{}\"", layer.name), middle(layer.thumbnail()), Vec::new()),
		(
			format!("three selected layers that each anchor a stack, grabbed at \"{}\"", anchors[0].name),
			middle(anchors[0].thumbnail()),
			three,
		),
	]
}

fn drag_comparing_the_scene_with_a_rebuild(editor: &mut EditorTestUtils, name: &str, grab: DVec2, selected: Vec<NodeId>) {
	let delta = DVec2::new(760., 540.) - pointer_state(editor, grab, MouseKeys::empty()).editor_position;
	editor.dispatch(NavigationMessage::CanvasPan { delta });
	if !selected.is_empty() {
		editor.dispatch(NodeGraphMessage::SelectedNodesSet { nodes: selected });
	}
	point_at(editor, grab, MouseKeys::empty());
	press_at(editor, grab);
	let mut time = 1000;
	for step in 1..=8 {
		point_at(editor, grab + DVec2::new(15., 7.) * step as f64, MouseKeys::LEFT);
		if step % 2 == 0 {
			time += 16;
			animation_frame_without_rendering(editor, time);
		}
		let rebuilt = changed_by_a_rebuild(editor);
		assert_eq!(rebuilt, [0; 6], "{name}, step {step}: the cards, layers, dots, wires, imports/exports and badges a rebuild changes");
	}
	editor.dispatch(pointer_down(true));
	release_at(editor, grab);
}

#[tokio::test]
async fn after_each_drag_step_the_scene_holds_the_rows_a_rebuild_would_give_it() {
	let mut editor = open_the_isometric_fountain();
	for (name, grab, selected) in drag_subjects(&editor) {
		drag_comparing_the_scene_with_a_rebuild(&mut editor, &name, grab, selected);
	}

	let (mut editor, fill) = a_rectangle_wired_into_a_fill().await;
	editor
		.handle_message(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::node(fill, 0),
			input_connector: InputConnector::Export(0),
		})
		.await;
	let grab = middle(scene(&editor).cards().get(&fill).unwrap().body());
	drag_comparing_the_scene_with_a_rebuild(&mut editor, "a node out of any chain", grab, Vec::new());
}

#[tokio::test]
#[ignore = "prints timings"]
async fn time_dragging_nodes_over_the_isometric_fountain() {
	let mut editor = open_the_isometric_fountain();
	settle(&mut editor).await;
	let scene = scene(&editor).clone();
	let reach = DVec2::new(DRAG_FRAMES as f64 * DRAG_PER_FRAME, 0.);
	let subjects = drag_subjects(&editor);

	let mut pans = Vec::new();
	for sample in 0..SAMPLES {
		editor.dispatch(NavigationMessage::CanvasPan {
			delta: DVec2::new(if sample % 2 == 0 { 1. } else { -1. }, 0.),
		});
		let start = Instant::now();
		editor.runtime.update();
		editor.runtime.run().await;
		pans.push(start.elapsed());
		editor.dispatch(Message::Wake);
	}
	let pan = median(pans);

	let profile = if cfg!(debug_assertions) { "dev" } else { "release" };
	let to_viewport = |editor: &EditorTestUtils| {
		editor
			.active_document()
			.network_interface
			.network_metadata(&[])
			.unwrap()
			.persistent_metadata
			.navigation_metadata
			.node_graph_to_viewport
	};
	println!(
		"{ISOMETRIC_FOUNTAIN} in a 1920x1080 viewport, graph at zoom {:.2}: {} cards, {} layers, {} dots, {} wires, {} error badges; {profile} profile; {DRAG_FRAMES} frames of a drag {DRAG_PER_FRAME} graph px per frame to the right, 1, 4 and 16 pointer moves per frame",
		to_viewport(&editor).matrix2.x_axis.x,
		scene.cards().len(),
		scene.layers().len(),
		scene.dots().len(),
		scene.wires().len(),
		scene.errors().len()
	);
	println!("runtime update + run of a frame that only pans the graph: {:.3} ms", ms(pan));
	for (name, grab, selected) in subjects {
		let delta = DVec2::new(760., 540.) - to_viewport(&editor).transform_point2(grab);
		editor.dispatch(NavigationMessage::CanvasPan { delta });
		settle(&mut editor).await;
		let mut times = DragTimes::default();
		let mut per_frame = Vec::new();
		let mut parts = Vec::new();
		for events_per_frame in [1, 4, 16] {
			if !selected.is_empty() {
				editor.dispatch(NodeGraphMessage::SelectedNodesSet { nodes: selected.clone() });
			}
			point_at(&mut editor, grab, MouseKeys::empty());
			press_at(&mut editor, grab);
			settle(&mut editor).await;

			let frames = drag(&mut editor, grab, events_per_frame, &mut times).await;
			per_frame.push((events_per_frame, mean(&frames)));
			if events_per_frame == 4 {
				parts = parts_alone(&mut editor);
			}

			editor.dispatch(pointer_down(true));
			release_at(&mut editor, grab + reach);
			settle(&mut editor).await;
		}

		let rows_changed: Vec<usize> = (0..6).map(|map| median(times.rows_changed.iter().map(|rows| rows[map]).collect())).collect();
		println!("{name}, panned to {} in the viewport:", to_viewport(&editor).transform_point2(grab).round());
		println!(
			"  move inside a cell: dispatch {:.3} ms (median of {}); {} render requests and {} changed rows in all of them",
			ms(median(times.inside.clone())),
			times.inside.len(),
			times.requests_inside,
			times.rows_changed_inside
		);
		println!(
			"  move crossing a cell: dispatch {:.3} ms (median of {}); rows changed (cards, layers, dots, wires, imports/exports, errors) {rows_changed:?}",
			ms(median(times.crossing.clone())),
			times.crossing.len()
		);
		println!(
			"  per frame: glide/animation frame dispatch {:.3} ms, runtime update + run {:.3} ms, response dispatch {:.3} ms (medians of {})",
			ms(median(times.glide_frames.clone())),
			ms(median(times.renders.clone())),
			ms(median(times.responses.clone())),
			times.renders.len()
		);
		let per_frame = per_frame.iter().map(|(events, time)| format!("{events} events {:.3} ms", ms(*time))).collect::<Vec<_>>().join(", ");
		println!("  editor time per frame (moves + animation frame + response, mean of {DRAG_FRAMES} frames): {per_frame}");
		let parts = parts.iter().map(|(part, time)| format!("{part} {:.3} ms", ms(*time))).collect::<Vec<_>>().join(", ");
		println!("  parts alone (a full rebuild per map, the Moved rebuild of a crossing, its layout): {parts}");
		let list = |sent: &[(String, usize)]| sent.iter().map(|(name, size)| format!("{name} ({size} B)")).collect::<Vec<_>>().join(", ");
		println!("  frontend messages of the last move inside a cell: [{}]", list(&times.sent_inside));
		println!("  frontend messages of the last crossing: [{}]", list(&times.sent_crossing));
	}
}
