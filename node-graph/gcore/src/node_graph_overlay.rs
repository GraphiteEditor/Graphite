use graphene_core_shaders::Ctx;

use crate::{
	Graphic,
	node_graph_overlay::{
		background::generate_background,
		nodes_and_wires::{draw_layers, draw_nodes, draw_wires},
		types::NodeGraphOverlayData,
		ui_context::{UIContext, UIRuntimeResponse},
	},
	table::Table,
	transform::ApplyTransform,
};

pub mod background;
pub mod consts;
pub mod nodes_and_wires;
pub mod types;
pub mod ui_context;

#[node_macro::node(skip_impl)]
pub fn generate_nodes(_: impl Ctx, mut node_graph_overlay_data: NodeGraphOverlayData) -> Table<Graphic> {
	let mut nodes_and_wires = Table::new();
	let layers = draw_layers(&node_graph_overlay_data.nodes_to_render);
	nodes_and_wires.extend(layers);

	let wires = draw_wires(&mut node_graph_overlay_data.nodes_to_render);
	nodes_and_wires.extend(wires);

	let nodes = draw_nodes(&node_graph_overlay_data.nodes_to_render);
	nodes_and_wires.extend(nodes);

	nodes_and_wires
}

#[node_macro::node(skip_impl)]
pub fn transform_nodes(ui_context: UIContext, mut nodes: Table<Graphic>) -> Table<Graphic> {
	let matrix = ui_context.transform.to_daffine2();
	nodes.left_apply_transform(&matrix);
	nodes
}

#[node_macro::node(skip_impl)]
pub fn dot_grid_background(ui_context: UIContext, opacity: f64) -> Table<Graphic> {
	Table::new_from_element(Graphic::Vector(generate_background(ui_context, opacity)))
}

#[node_macro::node(skip_impl)]
pub fn node_graph_ui_extend(_: impl Ctx, new: Table<Graphic>, mut base: Table<Graphic>) -> Table<Graphic> {
	base.extend(new);
	base
}

#[node_macro::node(skip_impl)]
pub fn send_render(ui_context: UIContext, render: String) -> () {
	let _ = ui_context.response_sender.send(UIRuntimeResponse::OverlaySVG(render));
}
