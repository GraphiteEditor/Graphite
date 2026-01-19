use core_types::table::{Table, TableRow, TableRowRef};
use core_types::{Color, Ctx};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::subpath::{ManipulatorGroup, Subpath};
use graphic_types::vector_types::vector::PointId;
use graphic_types::vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphic_types::vector_types::vector::style::Fill;
use graphic_types::{Graphic, Vector};
use linesweeper::topology::Topology;
pub use path_bool as path_bool_lib;
use path_bool::{FillRule, PathBooleanOperation};
use smallvec::SmallVec;
use vector_types::kurbo::{Affine, BezPath, CubicBez, ParamCurve, Point};

// Import specta so derive macros can find it
use core_types::specta;

// TODO: Fix boolean ops to work by removing .transform() and .one_instance_*() calls,
// TODO: since before we used a Vec of single-row tables and now we use a single table
// TODO: with multiple rows while still assuming a single row for the boolean operations.

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum BooleanOperation {
	#[default]
	#[icon("BooleanUnion")]
	Union,
	#[icon("BooleanSubtractFront")]
	SubtractFront,
	#[icon("BooleanSubtractBack")]
	SubtractBack,
	#[icon("BooleanIntersect")]
	Intersect,
	#[icon("BooleanDifference")]
	Difference,
}

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
	type Tag = usize;

	fn single(tag: usize, positive: bool) -> Self {
		let mut elems = SmallVec::with_capacity(tag);
		let sign = if positive { 1 } else { -1 };
		for _ in 0..tag {
			elems.push(0);
		}
		elems.push(sign);
		Self { elems }
	}
}

impl std::ops::AddAssign for WindingNumber {
	fn add_assign(&mut self, rhs: Self) {
		if self.elems.len() < rhs.elems.len() {
			self.elems.resize(rhs.elems.len(), 0);
		}

		for (me, them) in self.elems.iter_mut().zip(&rhs.elems) {
			*me += *them;
		}

		// Removing trailing zeros normalizes the representation so that the derived
		// PartialEq works. (Alternatively, we could write our own PartialEq.)
		let trailing_zeros = self.elems.iter().rev().take_while(|w| **w == 0).count();
		self.elems.truncate(self.elems.len() - trailing_zeros);
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
		match op {
			BooleanOperation::Union => self.elems.iter().any(|w| *w != 0),
			BooleanOperation::SubtractFront => self.elems.first().is_some_and(|w| *w != 0) && self.elems.iter().skip(1).all(|w| *w == 0),
			BooleanOperation::SubtractBack => self.elems.last().is_some_and(|w| *w != 0) && self.elems.iter().rev().skip(1).all(|w| *w == 0),
			BooleanOperation::Intersect => self.elems.iter().all(|w| *w != 0),
			BooleanOperation::Difference => self.elems.iter().any(|w| *w != 0) && !self.elems.iter().all(|w| *w != 0),
		}
	}
}

fn boolean_operation_on_vector_table<'a>(vector: impl DoubleEndedIterator<Item = TableRowRef<'a, Vector>>, boolean_operation: BooleanOperation) -> Table<Vector> {
	const EPSILON: f64 = 1e-5;
	let mut table = Table::new();
	let mut alpha_blending = None;
	let mut source_node_id = None;
	let mut paths = Vec::new();
	for v in vector {
		if alpha_blending.is_none() {
			alpha_blending = Some(*v.alpha_blending);
			source_node_id = Some(*v.source_node_id);
		}

		paths.push(to_bez_path(v.element, *v.transform));
	}

	log::warn!("boolean op {boolean_operation:?} on paths:");
	for p in &paths {
		log::warn!("{}", p.to_svg());
	}

	// unwrap: Topology::from_paths only errors on a non-closed path, and our paths are closed by construction.
	let top = Topology::<WindingNumber>::from_paths(paths.iter().enumerate().map(|(idx, path)| (path, idx)), EPSILON).unwrap();
	let contours = top.contours(|w| w.is_inside(boolean_operation));

	log::warn!("boolean op output paths:");
	for c in contours.contours() {
		log::warn!("{}", c.path.to_svg());
	}
	table.push(TableRow {
		element: from_bez_paths(contours.contours().map(|c| &c.path)),
		transform: DAffine2::IDENTITY,
		alpha_blending: alpha_blending.unwrap_or_default(),
		source_node_id: source_node_id.unwrap_or_default(),
	});
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

	for seg in subpath.iter() {
		if first {
			first = false;
			path.move_to(transform * seg.start());
		}
		path.push(transform * seg.as_path_el());
	}
}

fn from_bez_paths<'a>(paths: impl Iterator<Item = &'a BezPath>) -> Vector {
	let mut all_subpaths = Vec::new();

	for path in paths {
		let cubics: Vec<CubicBez> = path.segments().map(|segment| segment.to_cubic()).collect();
		let mut manipulators_list = Vec::new();
		let mut current_start = None;

		for (index, cubic) in cubics.iter().enumerate() {
			let d = |p: Point| DVec2::new(p.x, p.y);
			let [start, handle1, handle2, end] = [d(cubic.p0), d(cubic.p1), d(cubic.p2), d(cubic.p3)];

			if current_start.is_none() {
				// Start a new subpath
				if !manipulators_list.is_empty() {
					all_subpaths.push(Subpath::new(std::mem::take(&mut manipulators_list), true));
				}
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

	Vector::from_subpaths(all_subpaths, false)
}

type Path = Vec<path_bool::PathSegment>;

fn path_bool(a: Path, b: Path, op: PathBooleanOperation) -> Vec<Path> {
	match path_bool::path_boolean(&a, FillRule::NonZero, &b, FillRule::NonZero, op) {
		Ok(results) => results,
		Err(e) => {
			let a_path = path_bool::path_to_path_data(&a, 0.001);
			let b_path = path_bool::path_to_path_data(&b, 0.001);
			log::error!("Boolean error {e:?} encountered while processing {a_path}\n {op:?}\n {b_path}");
			Vec::new()
		}
	}
}

pub fn boolean_intersect(a: Path, b: Path) -> Vec<Path> {
	path_bool(a, b, PathBooleanOperation::Intersection)
}
