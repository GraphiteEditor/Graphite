use crate::grid_topology::Lattice;
use core_types::list::{Item, List};
use core_types::{CacheHash, Ctx};
use dyn_any::DynAny;
use glam::DVec2;
use graphic_types::Vector;
use vector_types::vector::algorithms::shapes;
use vector_types::vector::misc::SpiralType;
use vector_types::vector::misc::{ArcType, BoxCorners, GridTopology, GridType};
use vector_types::vector::{Path, SetVectorPaths};

/// Generates a circle shape with a chosen radius.
#[node_macro::node(category("Vector: Geometry"), icon("NodeCircle"))]
fn circle(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50.)]
	radius: Item<f64>,
) -> Item<Vector> {
	let radius = radius.element().abs();
	Vector::item_from_bezpath(&shapes::ellipse_bezpath(DVec2::splat(-radius), DVec2::splat(radius)))
}

/// Generates an arc shape forming a portion of a circle which may be open, closed, or a pie slice.
#[node_macro::node(category("Vector: Geometry"), icon("NodeArc"))]
fn arc(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50.)]
	radius: Item<f64>,
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	start_angle: Item<f64>,
	#[unit("°")]
	#[default(270.)]
	#[range]
	#[soft(0..360)]
	sweep_angle: Item<f64>,
	arc_type: Item<ArcType>,
) -> Item<Vector> {
	let (radius, start_angle, sweep_angle, arc_type) = (*radius.element(), *start_angle.element(), *sweep_angle.element(), arc_type.into_element());
	Vector::item_from_bezpath(&shapes::arc_bezpath(
		radius,
		start_angle / 360. * std::f64::consts::TAU,
		sweep_angle / 360. * std::f64::consts::TAU,
		arc_type,
	))
}

/// Generates a spiral shape that winds from an inner to an outer radius.
#[node_macro::node(category("Vector: Geometry"), icon("NodeSpiral"), inspector("spiral_inspector"))]
fn spiral(
	_: impl Ctx,
	_primary: (),
	spiral_type: Item<SpiralType>,
	#[default(5.)] turns: Item<f64>,
	#[unit("°")]
	#[default(0.)]
	start_angle: Item<f64>,
	#[unit(" px")]
	#[default(0.)]
	inner_radius: Item<f64>,
	#[unit(" px")]
	#[default(25)]
	outer_radius: Item<f64>,
	#[unit("°")]
	#[default(90.)]
	angular_resolution: Item<f64>,
) -> Item<Vector> {
	let (turns, start_angle, inner_radius, outer_radius, angular_resolution) = (
		*turns.element(),
		*start_angle.element(),
		*inner_radius.element(),
		*outer_radius.element(),
		*angular_resolution.element(),
	);
	Vector::item_from_bezpath(&shapes::spiral_bezpath(
		inner_radius,
		outer_radius,
		turns,
		start_angle.to_radians(),
		angular_resolution.to_radians(),
		spiral_type.into_element(),
	))
}

/// Generates an ellipse shape (an oval or stretched circle) with the chosen radii.
#[node_macro::node(category("Vector: Geometry"), icon("NodeEllipse"))]
fn ellipse(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50)]
	radius_x: Item<f64>,
	#[unit(" px")]
	#[default(25)]
	radius_y: Item<f64>,
) -> Item<Vector> {
	let radius = DVec2::new(*radius_x.element(), *radius_y.element());
	let corner1 = -radius;
	let corner2 = radius;

	let (mut ellipse, mut paths) = Vector::from_bezpath(&shapes::ellipse_bezpath(corner1, corner2));

	// Each junction between consecutive segments keeps its tangent pair smooth
	for segment in ellipse.segment_ids(&paths) {
		let Some(anchor) = ellipse.segment_end_from_id(&paths, segment) else { continue };
		ellipse.set_smooth(&mut paths, anchor, true);
	}

	Item::new_from_element(ellipse).with_paths(paths)
}

/// Generates a rectangle shape with the chosen width and height. It may also have rounded corners if desired.
#[node_macro::node(category("Vector: Geometry"), icon("NodeRectangle"), inspector("rectangle_inspector"))]
fn rectangle(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(100)]
	width: Item<f64>,
	#[unit(" px")]
	#[default(100)]
	height: Item<f64>,
	corner_radius: Item<BoxCorners>,
	#[default(true)] clamped: Item<bool>,
	_individual_corner_radii: Item<bool>,
) -> Item<Vector> {
	let size = DVec2::new(*width.element(), *height.element());
	let radii = corner_radius.element().to_corner_values();

	// Scale down overlapping adjacent radii to fit, following the CSS spec: <https://drafts.csswg.org/css-backgrounds/#corner-overlap>
	let radii = if *clamped.element() {
		let radii = radii.map(|radius| radius.max(0.));

		let mut scale_factor: f64 = 1.;
		for i in 0..4 {
			let side_length = if i % 2 == 0 { size.x } else { size.y };
			let adjacent_corner_radius_sum = radii[i] + radii[(i + 1) % 4];
			if side_length < adjacent_corner_radius_sum {
				scale_factor = scale_factor.min((side_length / adjacent_corner_radius_sum).max(0.));
			}
		}

		radii.map(|radius| radius * scale_factor)
	} else {
		radii
	};

	Vector::item_from_bezpath(&shapes::rounded_rectangle_bezpath(size / -2., size / 2., radii))
}

/// Builds a set of four corner values, such as a rectangle's corner radii, from a list of one, two, three, or four values.
#[node_macro::node(category("Vector: Geometry"), icon("NodeBoxCorners"))]
fn box_corners(
	_: impl Ctx,
	/// The corner values, filling the four corners clockwise from the top-left. Give one value for all corners, two for opposite pairs, three for top-left, the two sides, then bottom-right, or four for each corner.
	values: List<f64>,
) -> Item<BoxCorners> {
	let values: Vec<f64> = values.iter_element_values().copied().collect();
	Item::new_from_element(BoxCorners::from(values))
}

/// Generates an regular polygon shape like a triangle, square, pentagon, hexagon, heptagon, octagon, or any higher n-gon.
#[node_macro::node(category("Vector: Geometry"), icon("NodeRegularPolygon"))]
fn regular_polygon(
	_: impl Ctx,
	_primary: (),
	#[default(6)]
	#[hard(3..)]
	sides: Item<i64>,
	#[unit(" px")]
	#[default(50)]
	radius: Item<f64>,
) -> Item<Vector> {
	let points = *sides.element() as u64;
	Vector::item_from_bezpath(&shapes::regular_polygon_bezpath(DVec2::ZERO, points, *radius.element()))
}

/// Generates an n-pointed star shape with inner and outer points at chosen radii from the center.
#[node_macro::node(category("Vector: Geometry"), icon("NodeStar"))]
fn star(
	_: impl Ctx,
	_primary: (),
	#[default(5)]
	#[hard(2..)]
	sides: Item<i64>,
	#[unit(" px")]
	#[default(50)]
	radius_1: Item<f64>,
	#[unit(" px")]
	#[default(25)]
	radius_2: Item<f64>,
) -> Item<Vector> {
	let points = *sides.element() as u64;
	Vector::item_from_bezpath(&shapes::star_polygon_bezpath(DVec2::ZERO, points, *radius_1.element(), *radius_2.element()))
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum QRCodeErrorCorrectionLevel {
	/// Allows recovery from up to 7% data loss.
	#[default]
	Low,
	/// Allows recovery from up to 15% data loss.
	Medium,
	/// Allows recovery from up to 25% data loss.
	Quartile,
	/// Allows recovery from up to 30% data loss.
	High,
}

/// Generates a QR code from the input text.
#[node_macro::node(category("Vector: Geometry"), icon("NodeQrCode"), name("QR Code"))]
fn qr_code(
	_: impl Ctx,
	_primary: (),
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("https://graphite.art")]
	text: Item<String>,
	#[widget(ParsedWidgetOverride::Hidden)] has_size: Item<bool>,
	#[unit(" px")]
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	size: Item<f64>,
	error_correction: Item<QRCodeErrorCorrectionLevel>,
	individual_squares: Item<bool>,
) -> Item<Vector> {
	let (text, error_correction) = (text.into_element(), error_correction.into_element());
	let (has_size, size, individual_squares) = (*has_size.element(), *size.element(), *individual_squares.element());

	let ecc = match error_correction {
		QRCodeErrorCorrectionLevel::Low => qrcodegen::QrCodeEcc::Low,
		QRCodeErrorCorrectionLevel::Medium => qrcodegen::QrCodeEcc::Medium,
		QRCodeErrorCorrectionLevel::Quartile => qrcodegen::QrCodeEcc::Quartile,
		QRCodeErrorCorrectionLevel::High => qrcodegen::QrCodeEcc::High,
	};

	let Ok(qr_code) = qrcodegen::QrCode::encode_text(&text, ecc) else {
		return Item::new_from_element(Vector::default()).with_paths(List::<Path>::new());
	};

	let (mut vector, paths) = match individual_squares {
		true => {
			let (mut vector, mut paths) = (Vector::default(), List::new());

			let dimension = qr_code.size();
			let cells = (0..dimension).flat_map(|y| (0..dimension).map(move |x| (x, y)));
			let squares = cells.filter(|&(x, y)| qr_code.get_module(x, y)).map(|(x, y)| {
				let corner1 = DVec2::new(x as f64, y as f64);
				shapes::rectangle_bezpath(corner1, corner1 + DVec2::splat(1.))
			});
			vector.append_bezpaths(&mut paths, squares);

			(vector, paths)
		}
		false => crate::merge_qr_squares::merge_qr_squares(&qr_code),
	};

	if has_size {
		vector.transform(glam::DAffine2::from_scale(DVec2::splat(size / qr_code.size() as f64)));
	}

	Item::new_from_element(vector).with_paths(paths)
}

/// Generates an arrow from the origin to the chosen coordinate.
#[node_macro::node(category("Vector: Geometry"), icon("NodeArrow"))]
fn arrow(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(100., 0.)]
	arrow_to: Item<DVec2>,
	#[unit(" px")]
	#[hard(0..)]
	#[default(10)]
	shaft_width: Item<f64>,
	#[unit(" px")]
	#[hard(0..)]
	#[default(30)]
	head_width: Item<f64>,
	#[unit(" px")]
	#[hard(0..)]
	#[default(20)]
	head_length: Item<f64>,
) -> Item<Vector> {
	let (arrow_to, shaft_width, head_width, head_length) = (*arrow_to.element(), *shaft_width.element(), *head_width.element(), *head_length.element());
	Vector::item_from_bezpath(&shapes::arrow_bezpath(DVec2::ZERO, arrow_to, shaft_width, head_width, head_length))
}

#[node_macro::node(category("Vector: Geometry"), icon("NodeLine"))]
fn line(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(100., 100.)]
	line_to: Item<DVec2>,
) -> Item<Vector> {
	Vector::item_from_bezpath(&shapes::line_bezpath(DVec2::ZERO, *line_to.element()))
}

trait GridSpacing {
	fn as_dvec2(&self) -> DVec2;
}
impl GridSpacing for f64 {
	fn as_dvec2(&self) -> DVec2 {
		DVec2::splat(*self)
	}
}
impl GridSpacing for DVec2 {
	fn as_dvec2(&self) -> DVec2 {
		*self
	}
}

/// Generates a rectangular or isometric grid of points with the chosen number of columns and rows, joined into paths by the chosen topology.
#[node_macro::node(category("Vector: Geometry"), icon("NodeGrid"), inspector("grid_inspector"))]
fn grid<T: GridSpacing>(
	_: impl Ctx,
	_primary: (),
	lattice: Item<GridType>,
	/// How the points join into paths.
	topology: Item<GridTopology>,
	/// Removes edges so each point keeps three of them, turning rectangular grids into a brick wall and isometric grids into a honeycomb.
	staggered: Item<bool>,
	#[default(10)]
	#[hard(0..)]
	columns: Item<i64>,
	#[default(10)]
	#[hard(0..)]
	rows: Item<i64>,
	#[unit(" px")]
	#[hard(0..)]
	#[default(10)]
	#[implementations(f64, DVec2)]
	spacing: Item<T>,
	#[unit("°")]
	#[default(30., 30.)]
	angles: Item<DVec2>,
) -> Item<Vector> {
	let (grid_type, topology, staggered, columns, rows, angles) = (
		lattice.into_element(),
		topology.into_element(),
		*staggered.element(),
		*columns.element(),
		*rows.element(),
		*angles.element(),
	);
	let (columns, rows) = (columns as u32, rows as u32);

	let (x_spacing, y_spacing) = spacing.element().as_dvec2().into();
	let (angle_a, angle_b) = angles.into();

	// Isometric grid spacing based on the two skew angles. Unused for rectangular grids.
	let tan_a = angle_a.to_radians().tan();
	let tan_b = angle_b.to_radians().tan();
	let isometric_spacing = DVec2::new(y_spacing / (tan_a + tan_b), y_spacing);

	// The position of the grid point at column `x`, row `y`.
	let position = |x: u32, y: i64| -> DVec2 {
		if !grid_type.is_isometric() {
			return DVec2::new(x_spacing * x as f64, y_spacing * y as f64);
		}

		// Odd columns are offset vertically so the cells skew into the isometric shape.
		let a_angles_eaten = x.div_ceil(2) as f64;
		let b_angles_eaten = (x / 2) as f64;
		let offset_y_fraction = b_angles_eaten * tan_b - a_angles_eaten * tan_a;
		DVec2::new(isometric_spacing.x * x as f64, isometric_spacing.y * y as f64 + offset_y_fraction * isometric_spacing.x)
	};

	Lattice::new(grid_type, columns, rows, staggered && topology.supports_staggered(), position)
		.layout(topology)
		.into_item()
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::ParamCurve;
	use vector_types::vector::misc::point_to_dvec2;
	use vector_types::vector::shared_paths_from_item_attributes;

	fn item<T>(value: T) -> Item<T> {
		Item::new_from_element(value)
	}

	#[test]
	fn isometric_grid_test() {
		// Doesn't crash with weird angles
		grid(
			(),
			(),
			item(GridType::IsometricTiling),
			item(GridTopology::Edges),
			item(false),
			item(5_i64),
			item(5_i64),
			item(0.),
			item((0., 0.).into()),
		);
		grid(
			(),
			(),
			item(GridType::IsometricTiling),
			item(GridTopology::Edges),
			item(false),
			item(5_i64),
			item(5_i64),
			item(90.),
			item((90., 90.).into()),
		);

		// Works properly
		let grid = grid(
			(),
			(),
			item(GridType::IsometricTiling),
			item(GridTopology::Edges),
			item(false),
			item(5_i64),
			item(5_i64),
			item(10.),
			item((30., 30.).into()),
		);
		let paths = shared_paths_from_item_attributes(&grid);
		assert_eq!(grid.element().anchor_points(&paths).count(), 5 * 5);
		assert_eq!(grid.element().segment_iter(&paths).count(), 4 * 5 + 4 * 9);
		for (_, segment, _, _) in grid.element().segment_iter(&paths) {
			assert!(matches!(segment, kurbo::PathSeg::Line(_)));
			let span = point_to_dvec2(segment.start()) - point_to_dvec2(segment.end());
			assert!((span.length() - 10.).abs() < 1e-5, "Length of {} should be 10", span.length());
		}
	}

	#[test]
	fn skew_isometric_grid_test() {
		let grid = grid(
			(),
			(),
			item(GridType::IsometricTiling),
			item(GridTopology::Edges),
			item(false),
			item(5_i64),
			item(5_i64),
			item(10.),
			item((40., 30.).into()),
		);
		let paths = shared_paths_from_item_attributes(&grid);
		assert_eq!(grid.element().anchor_points(&paths).count(), 5 * 5);
		assert_eq!(grid.element().segment_iter(&paths).count(), 4 * 5 + 4 * 9);
		for (_, segment, _, _) in grid.element().segment_iter(&paths) {
			assert!(matches!(segment, kurbo::PathSeg::Line(_)));
			let vector = point_to_dvec2(segment.start()) - point_to_dvec2(segment.end());
			let angle = (vector.angle_to(DVec2::X).to_degrees() + 180.) % 180.;
			assert!([90., 150., 40.].into_iter().any(|target| (target - angle).abs() < 1e-10), "unexpected angle of {angle}")
		}
	}

	#[test]
	fn grid_edges_are_each_their_own_path() {
		let grid = grid(
			(),
			(),
			item(GridType::Rectangular),
			item(GridTopology::Edges),
			item(false),
			item(3_i64),
			item(3_i64),
			item(10.),
			item((30., 30.).into()),
		);
		let paths = shared_paths_from_item_attributes(&grid);
		let vector = grid.element();

		assert_eq!(vector.segment_ids(&paths).len(), 2 * 3 + 2 * 3);
		assert_eq!(paths.len(), 2 * 3 + 2 * 3);
		assert_eq!(vector.anchor_points(&paths).count(), 3 * 3);
	}

	#[test]
	fn grid_lines_run_along_each_row_and_column() {
		let grid = grid(
			(),
			(),
			item(GridType::Rectangular),
			item(GridTopology::Lines),
			item(false),
			item(3_i64),
			item(4_i64),
			item(10.),
			item((30., 30.).into()),
		);
		let paths = shared_paths_from_item_attributes(&grid);
		let vector = grid.element();

		assert_eq!(paths.len(), 4 + 3);
		assert_eq!(vector.segment_ids(&paths).len(), 4 * 2 + 3 * 3);
		assert_eq!(vector.anchor_points(&paths).count(), 3 * 4);
	}

	#[test]
	fn grid_disconnected_cells_test() {
		// A 3x3 rectangular grid has a 2x2 arrangement of cells, each its own closed quad subpath.
		let grid = grid(
			(),
			(),
			item(GridType::Rectangular),
			item(GridTopology::Cells),
			item(false),
			item(3_i64),
			item(3_i64),
			item(10.),
			item((30., 30.).into()),
		);
		let paths = shared_paths_from_item_attributes(&grid);
		let vector = grid.element();
		assert_eq!(vector.stroke_manipulator_groups(&paths).filter(|(_, closed)| *closed).count(), 4);
		assert_eq!(vector.anchor_points(&paths).count(), 4 * 4);
		assert_eq!(vector.segment_ids(&paths).len(), 4 * 4);

		// Each cell winds counter-clockwise (positive signed area), matching the shape generators.
		for (group, closed) in vector.stroke_manipulator_groups(&paths) {
			assert!(closed);
			let anchors: Vec<DVec2> = group.iter().map(|g| g.anchor).collect();
			let signed_area: f64 = (0..anchors.len()).map(|i| anchors[i].perp_dot(anchors[(i + 1) % anchors.len()])).sum::<f64>() / 2.;
			assert!(signed_area > 0., "grid cell should wind counter-clockwise");
		}
	}

	#[test]
	fn qr_code_test() {
		let qr = qr_code(
			(),
			(),
			item("https://graphite.art".to_string()),
			item(false),
			item(1.),
			item(QRCodeErrorCorrectionLevel::Low),
			item(true),
		);
		let paths = shared_paths_from_item_attributes(&qr);
		assert!(qr.element().anchor_points(&paths).count() > 0);
		assert!(!qr.element().segment_ids(&paths).is_empty());
	}
}
