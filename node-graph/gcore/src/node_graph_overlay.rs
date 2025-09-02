use graphene_core_shaders::{Ctx, color::Color};
use kurbo::{BezPath, Point};

use crate::{ExtractFootprint, table::Table, vector::Vector};

pub mod types;

#[node_macro::node(category(""))]
pub fn generate_nodes(_: impl Ctx, _node_graph_overlay_data: types::NodeGraphOverlayData) -> Table<Vector> {
	Table::new()
}

#[node_macro::node(category(""))]
pub fn transform_nodes(_ctx: impl Ctx + ExtractFootprint, nodes: Table<Vector>) -> Table<Vector> {
	nodes
}

#[node_macro::node(category(""))]
pub fn dot_grid_background(ctx: impl Ctx + ExtractFootprint, opacity: f64) -> Table<Vector> {
	let Some(footprint) = ctx.try_footprint() else {
		log::error!("Could not get footprint from context in dot_grid_background");
		return Table::new();
	};
	// From --color-2-mildblack: --color-2-mildblack-rgb: 34, 34, 34;
	let gray = (34. / 255.) as f32;
	let Some(bg_color) = Color::from_rgbaf32(gray, gray, gray, opacity as f32) else {
		log::error!("Could not create color in dot grid background");
		return Table::new();
	};

	let mut bez_path = BezPath::new();
	let p0 = Point::new(0., 0.); // bottom-left
	let p1 = Point::new(footprint.resolution.x as f64, 0.); // bottom-right
	let p2 = Point::new(footprint.resolution.x as f64, footprint.resolution.y as f64); // top-right
	let p3 = Point::new(0., footprint.resolution.y as f64); // top-left

	bez_path.move_to(p0);
	bez_path.line_to(p1);
	bez_path.line_to(p2);
	bez_path.line_to(p3);
	bez_path.close_path();

	let mut vector = Vector::from_bezpath(bez_path);
	vector.style.fill = crate::vector::style::Fill::Solid(bg_color);

	Table::new_from_element(vector)
}
