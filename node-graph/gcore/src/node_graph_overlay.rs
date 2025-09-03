use graphene_core_shaders::{Ctx, color::Color};
use kurbo::{BezPath, Point};

use crate::{
	node_graph_overlay::{
		nodes_and_wires::{draw_layers, draw_nodes},
		types::NodeGraphOverlayData,
		ui_context::{UIContext, UIRuntimeResponse},
	},
	table::Table,
	transform::ApplyTransform,
	vector::Vector,
};

pub mod consts;
pub mod nodes_and_wires;
pub mod types;
pub mod ui_context;

#[node_macro::node(skip_impl)]
pub fn generate_nodes(_: impl Ctx, node_graph_overlay_data: NodeGraphOverlayData) -> Table<Vector> {
	let mut nodes_and_wires = Table::new();
	let layers = draw_layers(&node_graph_overlay_data.nodes_to_render);
	nodes_and_wires.extend(layers);

	let nodes = draw_nodes(&node_graph_overlay_data.nodes_to_render);
	nodes_and_wires.extend(nodes);

	nodes_and_wires
}

#[node_macro::node(skip_impl)]
pub fn transform_nodes(ui_context: UIContext, mut nodes: Table<Vector>) -> Table<Vector> {
	let matrix = ui_context.transform.to_daffine2();
	nodes.apply_transform(&matrix);
	nodes
}

#[node_macro::node(skip_impl)]
pub fn dot_grid_background(ui_context: UIContext, opacity: f64) -> Table<Vector> {
	// From --color-2-mildblack: --color-2-mildblack-rgb: 34, 34, 34;
	let gray = (34. / 255.) as f32;
	let Some(bg_color) = Color::from_rgbaf32(gray, gray, gray, (opacity / 100.) as f32) else {
		log::error!("Could not create color in dot grid background");
		return Table::new();
	};

	let mut bez_path = BezPath::new();
	let p0 = Point::new(0., 0.); // bottom-left
	let p1 = Point::new(ui_context.resolution.x as f64, 0.); // bottom-right
	let p2 = Point::new(ui_context.resolution.x as f64, ui_context.resolution.y as f64); // top-right
	let p3 = Point::new(0., ui_context.resolution.y as f64); // top-left

	bez_path.move_to(p0);
	bez_path.line_to(p1);
	bez_path.line_to(p2);
	bez_path.line_to(p3);
	bez_path.close_path();

	let mut vector = Vector::from_bezpath(bez_path);
	vector.style.fill = crate::vector::style::Fill::Solid(bg_color);

	Table::new_from_element(vector)
}

#[node_macro::node(skip_impl)]
pub fn node_graph_ui_extend(_: impl Ctx, new: Table<Vector>, mut base: Table<Vector>) -> Table<Vector> {
	base.extend(new);
	base
}

#[node_macro::node(skip_impl)]
pub fn send_render(ui_context: UIContext, render: String) -> () {
	let _ = ui_context.response_sender.send(UIRuntimeResponse::OverlaySVG(render));
}
