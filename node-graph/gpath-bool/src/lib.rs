use bezier_rs::{ManipulatorGroup, Subpath};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graphene_core::table::{Table, TableRow, TableRowRef};
use graphene_core::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphene_core::vector::style::Fill;
use graphene_core::vector::{PointId, Vector};
use graphene_core::{Color, Ctx, Graphic};
pub use path_bool as path_bool_lib;
use path_bool::{FillRule, PathBooleanOperation};
use std::ops::Mul;

// TODO: Fix boolean ops to work by removing .transform() and .one_instnace_*() calls,
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
async fn boolean_operation<I: Into<Table<Graphic>> + 'n + Send + Clone>(
	_: impl Ctx,
	/// The group of paths to perform the boolean operation on. Nested groups are automatically flattened.
	#[implementations(Table<Graphic>, Table<Vector>)]
	group_of_paths: I,
	/// Which boolean operation to perform on the paths.
	///
	/// Union combines all paths while cutting out overlapping areas (even the interiors of a single path).
	/// Subtraction cuts overlapping areas out from the last (Subtract Front) or first (Subtract Back) path.
	/// Intersection cuts away all but the overlapping areas shared by every path.
	/// Difference cuts away the overlapping areas shared by every path, leaving only the non-overlapping areas.
	operation: BooleanOperation,
) -> Table<Vector> {
	let group_of_paths = group_of_paths.into();

	// The first index is the bottom of the stack
	let mut result_vector_data_table = boolean_operation_on_vector_data_table(flatten_vector_data(&group_of_paths).iter_ref(), operation);

	// Replace the transformation matrix with a mutation of the vector points themselves
	if let Some(result_vector_data) = result_vector_data_table.iter_mut().next() {
		let transform = *result_vector_data.transform;
		*result_vector_data.transform = DAffine2::IDENTITY;

		Vector::transform(result_vector_data.element, transform);
		result_vector_data.element.style.set_stroke_transform(DAffine2::IDENTITY);
		result_vector_data.element.upstream_graphic_group = Some(group_of_paths.clone());

		// Clean up the boolean operation result by merging duplicated points
		result_vector_data.element.merge_by_distance_spatial(*result_vector_data.transform, 0.0001);
	}

	result_vector_data_table
}

fn boolean_operation_on_vector_data_table<'a>(vector_data: impl DoubleEndedIterator<Item = TableRowRef<'a, Vector>> + Clone, boolean_operation: BooleanOperation) -> Table<Vector> {
	match boolean_operation {
		BooleanOperation::Union => union(vector_data),
		BooleanOperation::SubtractFront => subtract(vector_data),
		BooleanOperation::SubtractBack => subtract(vector_data.rev()),
		BooleanOperation::Intersect => intersect(vector_data),
		BooleanOperation::Difference => difference(vector_data),
	}
}

fn union<'a>(vector_data: impl DoubleEndedIterator<Item = TableRowRef<'a, Vector>>) -> Table<Vector> {
	// Reverse vector data so that the result style is the style of the first vector data
	let mut vector_data_reversed = vector_data.rev();

	let mut result_vector_data_table = Table::new_from_row(vector_data_reversed.next().map(|x| x.into_cloned()).unwrap_or_default());
	let mut first_row = result_vector_data_table.iter_mut().next().expect("Expected the one row we just pushed");

	// Loop over all vector data and union it with the result
	let default = TableRow::default();
	let mut second_vector_data = Some(vector_data_reversed.next().unwrap_or(default.as_ref()));
	while let Some(lower_vector_data) = second_vector_data {
		let transform_of_lower_into_space_of_upper = first_row.transform.inverse() * *lower_vector_data.transform;

		let result = &mut first_row.element;

		let upper_path_string = to_path(result, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.element, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let boolean_operation_string = unsafe { boolean_union(upper_path_string, lower_path_string) };
		let boolean_operation_result = from_path(&boolean_operation_string);

		result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
		result.point_domain = boolean_operation_result.point_domain;
		result.segment_domain = boolean_operation_result.segment_domain;
		result.region_domain = boolean_operation_result.region_domain;

		second_vector_data = vector_data_reversed.next();
	}

	result_vector_data_table
}

fn subtract<'a>(vector_data: impl Iterator<Item = TableRowRef<'a, Vector>>) -> Table<Vector> {
	let mut vector_data = vector_data.into_iter();

	let mut result_vector_data_table = Table::new_from_row(vector_data.next().map(|x| x.into_cloned()).unwrap_or_default());
	let mut first_row = result_vector_data_table.iter_mut().next().expect("Expected the one row we just pushed");

	let mut next_vector_data = vector_data.next();

	while let Some(lower_vector_data) = next_vector_data {
		let transform_of_lower_into_space_of_upper = first_row.transform.inverse() * *lower_vector_data.transform;

		let result = &mut first_row.element;

		let upper_path_string = to_path(result, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.element, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let boolean_operation_string = unsafe { boolean_subtract(upper_path_string, lower_path_string) };
		let boolean_operation_result = from_path(&boolean_operation_string);

		result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
		result.point_domain = boolean_operation_result.point_domain;
		result.segment_domain = boolean_operation_result.segment_domain;
		result.region_domain = boolean_operation_result.region_domain;

		next_vector_data = vector_data.next();
	}

	result_vector_data_table
}

fn intersect<'a>(vector_data: impl DoubleEndedIterator<Item = TableRowRef<'a, Vector>>) -> Table<Vector> {
	let mut vector_data = vector_data.rev();

	let mut result_vector_data_table = Table::new_from_row(vector_data.next().map(|x| x.into_cloned()).unwrap_or_default());
	let mut first_row = result_vector_data_table.iter_mut().next().expect("Expected the one row we just pushed");

	let default = TableRow::default();
	let mut second_vector_data = Some(vector_data.next().unwrap_or(default.as_ref()));

	// For each vector data, set the result to the intersection of that data and the result
	while let Some(lower_vector_data) = second_vector_data {
		let transform_of_lower_into_space_of_upper = first_row.transform.inverse() * *lower_vector_data.transform;

		let result = &mut first_row.element;

		let upper_path_string = to_path(result, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.element, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let boolean_operation_string = unsafe { boolean_intersect(upper_path_string, lower_path_string) };
		let boolean_operation_result = from_path(&boolean_operation_string);

		result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
		result.point_domain = boolean_operation_result.point_domain;
		result.segment_domain = boolean_operation_result.segment_domain;
		result.region_domain = boolean_operation_result.region_domain;
		second_vector_data = vector_data.next();
	}

	result_vector_data_table
}

fn difference<'a>(vector_data: impl DoubleEndedIterator<Item = TableRowRef<'a, Vector>> + Clone) -> Table<Vector> {
	let mut vector_data_iter = vector_data.clone().rev();
	let mut any_intersection = TableRow::default();
	let default = TableRow::default();
	let mut second_vector_data = Some(vector_data_iter.next().unwrap_or(default.as_ref()));

	// Find where all vector data intersect at least once
	while let Some(lower_vector_data) = second_vector_data {
		let filtered_vector_data = vector_data.clone().filter(|v| *v != lower_vector_data).collect::<Vec<_>>().into_iter();
		let unioned = boolean_operation_on_vector_data_table(filtered_vector_data, BooleanOperation::Union);
		let first_row = unioned.iter_ref().next().expect("Expected at least one row after the boolean union");

		let transform_of_lower_into_space_of_upper = first_row.transform.inverse() * *lower_vector_data.transform;

		let upper_path_string = to_path(first_row.element, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.element, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let boolean_intersection_string = unsafe { boolean_intersect(upper_path_string, lower_path_string) };
		let mut element = from_path(&boolean_intersection_string);
		element.style = first_row.element.style.clone();
		let boolean_intersection_result = TableRow {
			element,
			transform: *first_row.transform,
			alpha_blending: *first_row.alpha_blending,
			source_node_id: *first_row.source_node_id,
		};

		let transform_of_lower_into_space_of_upper = boolean_intersection_result.transform.inverse() * any_intersection.transform;

		let upper_path_string = to_path(&boolean_intersection_result.element, DAffine2::IDENTITY);
		let lower_path_string = to_path(&any_intersection.element, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let union_result = from_path(&unsafe { boolean_union(upper_path_string, lower_path_string) });
		any_intersection.element = union_result;

		any_intersection.transform = boolean_intersection_result.transform;
		any_intersection.element.style = boolean_intersection_result.element.style.clone();
		any_intersection.alpha_blending = boolean_intersection_result.alpha_blending;

		second_vector_data = vector_data_iter.next();
	}

	// Subtract the area where they intersect at least once from the union of all vector data
	let union = boolean_operation_on_vector_data_table(vector_data, BooleanOperation::Union);
	boolean_operation_on_vector_data_table(union.iter_ref().chain(std::iter::once(any_intersection.as_ref())), BooleanOperation::SubtractFront)
}

fn flatten_vector_data(graphic_group_table: &Table<Graphic>) -> Table<Vector> {
	graphic_group_table
		.iter_ref()
		.flat_map(|element| {
			match element.element.clone() {
				Graphic::Vector(vector_data) => {
					// Apply the parent group's transform to each element of vector data
					vector_data
						.iter()
						.map(|mut sub_vector_data| {
							sub_vector_data.transform = *element.transform * sub_vector_data.transform;

							sub_vector_data
						})
						.collect::<Vec<_>>()
				}
				Graphic::RasterDataCPU(image) => {
					let make_row = |transform| {
						// Convert the image frame into a rectangular subpath with the image's transform
						let mut subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						// Create a vector data table row from the rectangular subpath, with a default black fill
						let mut element = Vector::from_subpath(subpath);
						element.style.set_fill(Fill::Solid(Color::BLACK));

						TableRow { element, ..Default::default() }
					};

					// Apply the parent group's transform to each element of raster data
					image.iter_ref().map(|row| make_row(*element.transform * *row.transform)).collect::<Vec<_>>()
				}
				Graphic::RasterDataGPU(image) => {
					let make_row = |transform| {
						// Convert the image frame into a rectangular subpath with the image's transform
						let mut subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
						subpath.apply_transform(transform);

						// Create a vector data table row from the rectangular subpath, with a default black fill
						let mut element = Vector::from_subpath(subpath);
						element.style.set_fill(Fill::Solid(Color::BLACK));

						TableRow { element, ..Default::default() }
					};

					// Apply the parent group's transform to each element of raster data
					image.iter_ref().map(|row| make_row(*element.transform * *row.transform)).collect::<Vec<_>>()
				}
				Graphic::GraphicGroup(mut graphic_group) => {
					// Apply the parent group's transform to each element of inner group
					for sub_element in graphic_group.iter_mut() {
						*sub_element.transform = *element.transform * *sub_element.transform;
					}

					// Recursively flatten the inner group into vector data
					let unioned = boolean_operation_on_vector_data_table(flatten_vector_data(&graphic_group).iter_ref(), BooleanOperation::Union);

					unioned.iter().collect::<Vec<_>>()
				}
			}
		})
		.collect()
}

fn to_path(vector: &Vector, transform: DAffine2) -> Vec<path_bool::PathSegment> {
	let mut path = Vec::new();
	for subpath in vector.stroke_bezier_paths() {
		to_path_segments(&mut path, &subpath, transform);
	}
	path
}

fn to_path_segments(path: &mut Vec<path_bool::PathSegment>, subpath: &Subpath<PointId>, transform: DAffine2) {
	use path_bool::PathSegment;
	let mut global_start = None;
	let mut global_end = DVec2::ZERO;
	for bezier in subpath.iter() {
		const EPS: f64 = 1e-8;
		let transformed = bezier.apply_transformation(|pos| transform.transform_point2(pos).mul(EPS.recip()).round().mul(EPS));
		let start = transformed.start;
		let end = transformed.end;
		if global_start.is_none() {
			global_start = Some(start);
		}
		global_end = end;
		let segment = match transformed.handles {
			bezier_rs::BezierHandles::Linear => PathSegment::Line(start, end),
			bezier_rs::BezierHandles::Quadratic { handle } => PathSegment::Quadratic(start, handle, end),
			bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => PathSegment::Cubic(start, handle_start, handle_end, end),
		};
		path.push(segment);
	}
	if let Some(start) = global_start {
		path.push(PathSegment::Line(global_end, start));
	}
}

fn from_path(path_data: &[Path]) -> Vector {
	const EPSILON: f64 = 1e-5;

	fn is_close(a: DVec2, b: DVec2) -> bool {
		(a - b).length_squared() < EPSILON * EPSILON
	}

	let mut all_subpaths = Vec::new();

	for path in path_data.iter().filter(|path| !path.is_empty()) {
		let cubics: Vec<[DVec2; 4]> = path.iter().map(|segment| segment.to_cubic()).collect();
		let mut groups = Vec::new();
		let mut current_start = None;

		for (index, cubic) in cubics.iter().enumerate() {
			let [start, handle1, handle2, end] = *cubic;

			if current_start.is_none() || !is_close(start, current_start.unwrap()) {
				// Start a new subpath
				if !groups.is_empty() {
					all_subpaths.push(Subpath::new(std::mem::take(&mut groups), true));
				}
				// Use the correct in-handle (None) and out-handle for the start point
				groups.push(ManipulatorGroup::new(start, None, Some(handle1)));
			} else {
				// Update the out-handle of the previous point
				if let Some(last) = groups.last_mut() {
					last.out_handle = Some(handle1);
				}
			}

			// Add the end point with the correct in-handle and out-handle (None)
			groups.push(ManipulatorGroup::new(end, Some(handle2), None));

			current_start = Some(end);

			// Check if this is the last segment
			if index == cubics.len() - 1 {
				all_subpaths.push(Subpath::new(groups, true));
				groups = Vec::new(); // Reset groups for the next path
			}
		}
	}

	Vector::from_subpaths(all_subpaths, false)
}

type Path = Vec<path_bool::PathSegment>;

fn boolean_union(a: Path, b: Path) -> Vec<Path> {
	path_bool(a, b, PathBooleanOperation::Union)
}

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

fn boolean_subtract(a: Path, b: Path) -> Vec<Path> {
	path_bool(a, b, PathBooleanOperation::Difference)
}

pub fn boolean_intersect(a: Path, b: Path) -> Vec<Path> {
	path_bool(a, b, PathBooleanOperation::Intersection)
}
