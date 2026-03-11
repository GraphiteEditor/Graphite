use core_types::table::{Table, TableRow, TableRowRef};
use core_types::{Color, Ctx};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::subpath::{ManipulatorGroup, Subpath};
use graphic_types::vector_types::vector::PointId;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphic_types::vector_types::vector::style::Fill;
use graphic_types::{Graphic, Vector};
use linesweeper::topology::Topology;
use linesweeper::{BinaryOp, FillRule, binary_op};
use smallvec::SmallVec;
use vector_types::kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, PathSeg, Point, QuadBez};
pub use vector_types::vector::misc::BooleanOperation;

// TODO: Fix boolean ops to work by removing .transform() and .one_instance_*() calls,
// TODO: since before we used a Vec of single-row tables and now we use a single table
// TODO: with multiple rows while still assuming a single row for the boolean operations.

/// Combines the geometric forms of one or more closed paths into a new vector path that results from cutting or joining the paths by the chosen method.
#[node_macro::node(category(""))]
async fn boolean_operation<I: graphic_types::IntoGraphicTable + 'n + Send + Clone>(
	_: impl Ctx,
	/// The table of vector paths to perform the boolean operation on. Nested tables are automatically flattened.
	#[implementations(Table<Graphic>, Table<Vector>)]
	content: I,
	/// Which boolean operation to perform on the paths.
	///
	/// Union combines all paths while cutting out overlapping areas (even the interiors of a single path).
	/// Subtraction cuts overlapping areas out from the last (Subtract Front) or first (Subtract Back) path.
	/// Intersection cuts away all but the overlapping areas shared by every path.
	/// Difference cuts away the overlapping areas shared by every path, leaving only the non-overlapping areas.
	operation: BooleanOperation,
) -> Table<Vector> {
	let content = content.into_graphic_table();

	// The first index is the bottom of the stack
	let mut result_vector_table = boolean_operation_on_vector_table(flatten_vector(&content).iter(), operation);

	// Replace the transformation matrix with a mutation of the vector points themselves
	if let Some(result_vector) = result_vector_table.iter_mut().next() {
		let transform = *result_vector.transform;
		*result_vector.transform = DAffine2::IDENTITY;

		Vector::transform(result_vector.element, transform);
		result_vector.element.style.set_stroke_transform(DAffine2::IDENTITY);
		result_vector.element.upstream_data = Some(content.clone());

		// Clean up the boolean operation result by merging duplicated points
		result_vector.element.merge_by_distance_spatial(*result_vector.transform, 0.0001);
	}

	result_vector_table
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WindingNumber {
	elems: SmallVec<[i16; 8]>,
}

impl linesweeper::topology::WindingNumber for WindingNumber {
	type Tag = (usize, usize);

	fn single((tag, out_of): (usize, usize), positive: bool) -> Self {
		let mut elems = SmallVec::with_capacity(out_of);
		elems.resize(out_of, 0);
		elems[tag] = if positive { 1 } else { -1 };
		Self { elems }
	}
}

impl std::ops::AddAssign for WindingNumber {
	fn add_assign(&mut self, rhs: Self) {
		if rhs.elems.is_empty() {
			return;
		}
		if self.elems.is_empty() {
			self.elems = rhs.elems;
		} else {
			for (me, them) in self.elems.iter_mut().zip(&rhs.elems) {
				*me += *them;
			}
		}
	}
}

impl std::ops::Add for WindingNumber {
	type Output = WindingNumber;

	fn add(mut self, rhs: Self) -> Self::Output {
		self += rhs;
		self
	}
}

impl WindingNumber {
	fn is_inside(&self, op: BooleanOperation) -> bool {
		let is_in = |w: &i16| *w != 0;
		let is_out = |w: &i16| *w == 0;
		match op {
			BooleanOperation::Union => self.elems.iter().any(is_in),
			BooleanOperation::SubtractFront => self.elems.first().is_some_and(is_in) && self.elems.iter().skip(1).all(is_out),
			BooleanOperation::SubtractBack => self.elems.last().is_some_and(is_in) && self.elems.iter().rev().skip(1).all(is_out),
			BooleanOperation::Intersect => !self.elems.is_empty() && self.elems.iter().all(is_in),
			BooleanOperation::Difference => self.elems.iter().any(is_in) && !self.elems.iter().all(is_in),
		}
	}
}

fn boolean_operation_on_vector_table<'a>(vector: impl DoubleEndedIterator<Item = TableRowRef<'a, Vector>> + Clone, boolean_operation: BooleanOperation) -> Table<Vector> {
	const EPSILON: f64 = 1e-5;
	let mut table = Table::new();
	let mut paths = Vec::new();
	let mut row = TableRow::<Vector>::default();

	let copy_from = if matches!(boolean_operation, BooleanOperation::SubtractFront) {
		vector.clone().next()
	} else {
		vector.clone().next_back()
	};
	if let Some(copy_from) = copy_from {
		row.alpha_blending = *copy_from.alpha_blending;
		row.source_node_id = *copy_from.source_node_id;
		row.element.style = copy_from.element.style.clone();
		row.element.upstream_data = copy_from.element.upstream_data.clone();
	}

	for element in vector {
		paths.push(to_bez_path(element.element, *element.transform));
	}

	let top = match Topology::<WindingNumber>::from_paths(paths.iter().enumerate().map(|(idx, path)| (path, (idx, paths.len()))), EPSILON) {
		Ok(top) => top,
		Err(e) => {
			log::error!("Boolean operation failed while building topology: {e}");
			table.push(row);
			return table;
		}
	};
	let contours = top.contours(|winding| winding.is_inside(boolean_operation));

	for subpath in from_bez_paths(contours.contours().map(|c| &c.path)) {
		row.element.append_subpath(subpath, false);
	}

	table.push(row);
	table
}

fn flatten_vector(graphic_table: &Table<Graphic>) -> Table<Vector> {
	graphic_table
		.iter()
		.flat_map(|element| {
			match element.element.clone() {
				Graphic::Vector(vector) => {
					// Apply the parent graphic's transform to each element of the vector table
					vector
						.into_iter()
						.map(|mut sub_vector| {
							sub_vector.transform = *element.transform * sub_vector.transform;

							sub_vector
						})
						.collect::<Vec<_>>()
				}
				Graphic::RasterCPU(image) => {
					let make_row = |transform| {
						// Convert the image frame into a rectangular subpath with the image's transform
						let mut subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						// Create a vector table row from the rectangular subpath, with a default black fill
						let mut element = Vector::from_subpath(subpath);
						element.style.set_fill(Fill::Solid(Color::BLACK));

						TableRow { element, ..Default::default() }
					};

					// Apply the parent graphic's transform to each raster element
					image.iter().map(|row| make_row(*element.transform * *row.transform)).collect::<Vec<_>>()
				}
				Graphic::RasterGPU(image) => {
					let make_row = |transform| {
						// Convert the image frame into a rectangular subpath with the image's transform
						let mut subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						// Create a vector table row from the rectangular subpath, with a default black fill
						let mut element = Vector::from_subpath(subpath);
						element.style.set_fill(Fill::Solid(Color::BLACK));

						TableRow { element, ..Default::default() }
					};

					// Apply the parent graphic's transform to each raster element
					image.iter().map(|row| make_row(*element.transform * *row.transform)).collect::<Vec<_>>()
				}
				Graphic::Graphic(mut graphic) => {
					// Apply the parent graphic's transform to each element of inner table
					for sub_element in graphic.iter_mut() {
						*sub_element.transform = *element.transform * *sub_element.transform;
					}

					// Recursively flatten the inner table into the output vector table
					let unioned = boolean_operation_on_vector_table(flatten_vector(&graphic).iter(), BooleanOperation::Union);

					unioned.into_iter().collect::<Vec<_>>()
				}
				Graphic::Color(color) => color
					.into_iter()
					.map(|row| {
						let mut element = Vector::default();
						element.style.set_fill(Fill::Solid(row.element));
						element.style.set_stroke_transform(DAffine2::IDENTITY);

						TableRow {
							element,
							transform: row.transform,
							alpha_blending: row.alpha_blending,
							source_node_id: row.source_node_id,
						}
					})
					.collect::<Vec<_>>(),
				Graphic::Gradient(gradient) => gradient
					.into_iter()
					.map(|row| {
						let mut element = Vector::default();
						element.style.set_fill(Fill::Gradient(graphic_types::vector_types::gradient::Gradient {
							stops: row.element,
							..Default::default()
						}));
						element.style.set_stroke_transform(DAffine2::IDENTITY);

						TableRow {
							element,
							transform: row.transform,
							alpha_blending: row.alpha_blending,
							source_node_id: row.source_node_id,
						}
					})
					.collect::<Vec<_>>(),
			}
		})
		.collect()
}

// This quantization should potentially be removed since it's not conceptually necessary,
// but without it, the oak leaf in the Changing Seasons demo artwork is funky because
// quantization is needed for the top and bottom points to line up vertically.
fn quantize_segment(seg: PathSeg) -> PathSeg {
	const QUANTIZE_EPS: f64 = 1e-8;
	fn q(p: Point) -> Point {
		Point::new((p.x / QUANTIZE_EPS).round() * QUANTIZE_EPS, (p.y / QUANTIZE_EPS).round() * QUANTIZE_EPS)
	}

	match seg {
		PathSeg::Line(s) => PathSeg::Line(Line::new(q(s.p0), q(s.p1))),
		PathSeg::Quad(s) => PathSeg::Quad(QuadBez::new(q(s.p0), q(s.p1), q(s.p2))),
		PathSeg::Cubic(s) => PathSeg::Cubic(CubicBez::new(q(s.p0), q(s.p1), q(s.p2), q(s.p3))),
	}
}

fn to_bez_path(vector: &Vector, transform: DAffine2) -> BezPath {
	let mut path = BezPath::new();
	for subpath in vector.stroke_bezier_paths() {
		push_subpath(&mut path, &subpath, transform);
	}
	path
}

fn push_subpath(path: &mut BezPath, subpath: &Subpath<PointId>, transform: DAffine2) {
	let transform = Affine::new(transform.to_cols_array());
	let mut first = true;

	for seg in subpath.iter_closed() {
		let quantized = quantize_segment(transform * seg);
		if first {
			first = false;
			path.move_to(quantized.start());
		}
		path.push(quantized.as_path_el());
	}
	path.close_path();
}

fn from_bez_paths<'a>(paths: impl Iterator<Item = &'a BezPath>) -> Vec<Subpath<PointId>> {
	let mut all_subpaths = Vec::new();

	for path in paths {
		let cubics: Vec<CubicBez> = path.segments().map(|segment| segment.to_cubic()).collect();
		let mut manipulators_list = Vec::new();
		let mut current_start = None;

		for (index, cubic) in cubics.iter().enumerate() {
			let d = |p: Point| DVec2::new(p.x, p.y);
			let [start, handle1, handle2, end] = [d(cubic.p0), d(cubic.p1), d(cubic.p2), d(cubic.p3)];

			if current_start.is_none() {
				// Use the correct in-handle (None) and out-handle for the start point
				manipulators_list.push(ManipulatorGroup::new(start, None, Some(handle1)));
			} else {
				// Update the out-handle of the previous point
				if let Some(last) = manipulators_list.last_mut() {
					last.out_handle = Some(handle1);
				}
			}

			// Add the end point with the correct in-handle and out-handle (None)
			manipulators_list.push(ManipulatorGroup::new(end, Some(handle2), None));

			current_start = Some(end);

			// Check if this is the last segment
			if index == cubics.len() - 1 {
				all_subpaths.push(Subpath::new(manipulators_list, true));
				manipulators_list = Vec::new(); // Reset manipulators for the next path
			}
		}
	}

	all_subpaths
}

pub fn boolean_intersect(a: &BezPath, b: &BezPath) -> Vec<BezPath> {
	match binary_op(a, b, FillRule::NonZero, BinaryOp::Intersection) {
		Ok(contours) => contours.contours().map(|c| c.path.clone()).collect(),
		Err(e) => {
			log::error!("Boolean Operation failed (a: {} segments, b: {} segments): {e}", a.segments().count(), b.segments().count());
			Vec::new()
		}
	}
}
