use graphene_core_shaders::color::Color;
use kurbo::{BezPath, Point};

use crate::{
	node_graph_overlay::{consts::*, ui_context::UIContext},
	table::{Table, TableRow},
	vector::{
		Vector,
		style::{Fill, Stroke, StrokeCap},
	},
};

pub fn generate_background(ui_context: UIContext, opacity: f64) -> Table<Vector> {
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
	vector.style.fill = Fill::Solid(bg_color);

	let mut bg_table = Table::new_from_element(vector);

	let mut grid_spacing = ui_context.transform.scale * GRID_SIZE;
	while grid_spacing > 0. && grid_spacing < GRID_COLLAPSE_SPACING {
		grid_spacing *= 2.;
	}
	let grid_dot_radius = 1. + (ui_context.transform.scale - 0.5 + 0.001).floor() / 2.;
	let grid_offset_left = (ui_context.transform.x % grid_spacing + grid_spacing) % grid_spacing;
	let grid_offset_top = (ui_context.transform.y % grid_spacing + grid_spacing) % grid_spacing;

	// make sure we cover full screen (+1 avoids missing last col/row)
	let number_of_rows = (ui_context.resolution.y as f64 / grid_spacing).ceil() as u32 + 1;
	// for col in 0..number_of_cols {
	for row in 0..number_of_rows {
		let circle_color = Color::from_rgba8_no_srgb(COLOR_7_MIDDLEGRAY).unwrap();
		let line_y = (row as f64 - 1.) * grid_spacing + grid_offset_top;
		let mut line = BezPath::new();
		line.move_to(Point::new(grid_offset_left, line_y));
		line.line_to(Point::new(grid_offset_left + ui_context.resolution.x as f64, line_y));
		let mut line_vector = Vector::from_bezpath(line);
		let dash_gap = grid_spacing - 0.00001;
		let stroke_cap = StrokeCap::Round;
		line_vector.style.stroke = Some(
			Stroke::new(Some(circle_color), grid_dot_radius * 2.)
				.with_dash_lengths(vec![0.00001, dash_gap])
				.with_stroke_cap(stroke_cap),
		);
		bg_table.push(TableRow::new_from_element(line_vector));
	}
	// }
	bg_table
}

// fn circle_bezpath(center: Point, radius: f64) -> BezPath {
// 	// "magic constant" for approximating a circle with 4 cubic Beziers
// 	let k = 0.5522847498307936;

// 	let cx = center.x;
// 	let cy = center.y;
// 	let r = radius;
// 	let c = k * r;

// 	let mut path = BezPath::new();

// 	// start at rightmost point
// 	path.move_to((cx + r, cy));

// 	// top-right quadrant
// 	path.curve_to((cx + r, cy + c), (cx + c, cy + r), (cx, cy + r));

// 	// top-left quadrant
// 	path.curve_to((cx - c, cy + r), (cx - r, cy + c), (cx - r, cy));

// 	// bottom-left quadrant
// 	path.curve_to((cx - r, cy - c), (cx - c, cy - r), (cx, cy - r));

// 	// bottom-right quadrant
// 	path.curve_to((cx + c, cy - r), (cx + r, cy - c), (cx + r, cy));

// 	path.close_path();
// 	path
// }
