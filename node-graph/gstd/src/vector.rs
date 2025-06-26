use bezier_rs::{ManipulatorGroup, Subpath};
use glam::{DAffine2, DVec2};
use graphene_core::instances::{Instance, InstanceRef};
use graphene_core::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use graphene_core::vector::misc::BooleanOperation;
use graphene_core::vector::style::Fill;
pub use graphene_core::vector::*;
use graphene_core::{Color, Ctx, GraphicElement, GraphicGroupTable};
pub use path_bool as path_bool_lib;
use path_bool::{FillRule, PathBooleanOperation};
use std::ops::Mul;

// TODO: Fix boolean ops to work by removing .transform() and .one_instnace_*() calls,
// TODO: since before we used a Vec of single-row tables and now we use a single table
// TODO: with multiple rows while still assuming a single row for the boolean operations.

/// Combines the geometric forms of one or more closed paths into a new vector path that results from cutting or joining the paths by the chosen method.
#[node_macro::node(category(""))]
async fn boolean_operation<I: Into<GraphicGroupTable> + 'n + Send + Clone>(
	_: impl Ctx,
	/// The group of paths to perform the boolean operation on. Nested groups are automatically flattened.
	#[implementations(GraphicGroupTable, VectorDataTable)]
	group_of_paths: I,
	/// Which boolean operation to perform on the paths.
	///
	/// Union combines all paths while cutting out overlapping areas (even the interiors of a single path).
	/// Subtraction cuts overlapping areas out from the last (Subtract Front) or first (Subtract Back) path.
	/// Intersection cuts away all but the overlapping areas shared by every path.
	/// Difference cuts away the overlapping areas shared by every path, leaving only the non-overlapping areas.
	operation: BooleanOperation,
) -> VectorDataTable {
	let group_of_paths = group_of_paths.into();

	// The first index is the bottom of the stack
	let mut result_vector_data_table = boolean_operation_on_vector_data_table(flatten_vector_data(&group_of_paths).instance_ref_iter(), operation);

	// Replace the transformation matrix with a mutation of the vector points themselves
	if let Some(result_vector_data) = result_vector_data_table.instance_mut_iter().next() {
		let transform = *result_vector_data.transform;
		*result_vector_data.transform = DAffine2::IDENTITY;

		VectorData::transform(result_vector_data.instance, transform);
		result_vector_data.instance.style.set_stroke_transform(DAffine2::IDENTITY);
		result_vector_data.instance.upstream_graphic_group = Some(group_of_paths.clone());

		// Clean up the boolean operation result by merging duplicated points
		result_vector_data.instance.merge_by_distance_spatial(*result_vector_data.transform, 0.0001);
	}

	result_vector_data_table
}

fn boolean_operation_on_vector_data_table<'a>(vector_data: impl DoubleEndedIterator<Item = InstanceRef<'a, VectorData>> + Clone, boolean_operation: BooleanOperation) -> VectorDataTable {
	match boolean_operation {
		BooleanOperation::Union => union(vector_data),
		BooleanOperation::SubtractFront => subtract(vector_data),
		BooleanOperation::SubtractBack => subtract(vector_data.rev()),
		BooleanOperation::Intersect => intersect(vector_data),
		BooleanOperation::Difference => difference(vector_data),
	}
}

fn union<'a>(vector_data: impl DoubleEndedIterator<Item = InstanceRef<'a, VectorData>>) -> VectorDataTable {
	// Reverse vector data so that the result style is the style of the first vector data
	let mut vector_data_reversed = vector_data.rev();

	let mut result_vector_data_table = VectorDataTable::default();
	result_vector_data_table.push(vector_data_reversed.next().map(|x| x.to_instance_cloned()).unwrap_or_default());
	let mut first_instance = result_vector_data_table.instance_mut_iter().next().expect("Expected the one instance we just pushed");

	// Loop over all vector data and union it with the result
	let default = Instance::default();
	let mut second_vector_data = Some(vector_data_reversed.next().unwrap_or(default.to_instance_ref()));
	while let Some(lower_vector_data) = second_vector_data {
		let transform_of_lower_into_space_of_upper = first_instance.transform.inverse() * *lower_vector_data.transform;

		let result = &mut first_instance.instance;

		let upper_path_string = to_path(result, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.instance, transform_of_lower_into_space_of_upper);

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

fn subtract<'a>(vector_data: impl Iterator<Item = InstanceRef<'a, VectorData>>) -> VectorDataTable {
	let mut vector_data = vector_data.into_iter();

	let mut result_vector_data_table = VectorDataTable::default();
	result_vector_data_table.push(vector_data.next().map(|x| x.to_instance_cloned()).unwrap_or_default());
	let mut first_instance = result_vector_data_table.instance_mut_iter().next().expect("Expected the one instance we just pushed");

	let mut next_vector_data = vector_data.next();

	while let Some(lower_vector_data) = next_vector_data {
		let transform_of_lower_into_space_of_upper = first_instance.transform.inverse() * *lower_vector_data.transform;

		let result = &mut first_instance.instance;

		let upper_path_string = to_path(result, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.instance, transform_of_lower_into_space_of_upper);

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

fn intersect<'a>(vector_data: impl DoubleEndedIterator<Item = InstanceRef<'a, VectorData>>) -> VectorDataTable {
	let mut vector_data = vector_data.rev();

	let mut result_vector_data_table = VectorDataTable::default();
	result_vector_data_table.push(vector_data.next().map(|x| x.to_instance_cloned()).unwrap_or_default());
	let mut first_instance = result_vector_data_table.instance_mut_iter().next().expect("Expected the one instance we just pushed");

	let default = Instance::default();
	let mut second_vector_data = Some(vector_data.next().unwrap_or(default.to_instance_ref()));

	// For each vector data, set the result to the intersection of that data and the result
	while let Some(lower_vector_data) = second_vector_data {
		let transform_of_lower_into_space_of_upper = first_instance.transform.inverse() * *lower_vector_data.transform;

		let result = &mut first_instance.instance;

		let upper_path_string = to_path(result, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.instance, transform_of_lower_into_space_of_upper);

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

fn difference<'a>(vector_data: impl DoubleEndedIterator<Item = InstanceRef<'a, VectorData>> + Clone) -> VectorDataTable {
	let mut vector_data_iter = vector_data.clone().rev();
	let mut any_intersection = Instance::default();
	let default = Instance::default();
	let mut second_vector_data = Some(vector_data_iter.next().unwrap_or(default.to_instance_ref()));

	// Find where all vector data intersect at least once
	while let Some(lower_vector_data) = second_vector_data {
		let filtered_vector_data = vector_data.clone().filter(|v| *v != lower_vector_data).collect::<Vec<_>>().into_iter();
		let unioned = boolean_operation_on_vector_data_table(filtered_vector_data, BooleanOperation::Union);
		let first_instance = unioned.instance_ref_iter().next().expect("Expected at least one instance after the boolean union");

		let transform_of_lower_into_space_of_upper = first_instance.transform.inverse() * *lower_vector_data.transform;

		let upper_path_string = to_path(first_instance.instance, DAffine2::IDENTITY);
		let lower_path_string = to_path(lower_vector_data.instance, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let boolean_intersection_string = unsafe { boolean_intersect(upper_path_string, lower_path_string) };
		let mut instance = from_path(&boolean_intersection_string);
		instance.style = first_instance.instance.style.clone();
		let boolean_intersection_result = Instance {
			instance,
			transform: *first_instance.transform,
			alpha_blending: *first_instance.alpha_blending,
			source_node_id: *first_instance.source_node_id,
		};

		let transform_of_lower_into_space_of_upper = boolean_intersection_result.transform.inverse() * any_intersection.transform;

		let upper_path_string = to_path(&boolean_intersection_result.instance, DAffine2::IDENTITY);
		let lower_path_string = to_path(&any_intersection.instance, transform_of_lower_into_space_of_upper);

		#[allow(unused_unsafe)]
		let union_result = from_path(&unsafe { boolean_union(upper_path_string, lower_path_string) });
		any_intersection.instance = union_result;

		any_intersection.transform = boolean_intersection_result.transform;
		any_intersection.instance.style = boolean_intersection_result.instance.style.clone();
		any_intersection.alpha_blending = boolean_intersection_result.alpha_blending;

		second_vector_data = vector_data_iter.next();
	}

	// Subtract the area where they intersect at least once from the union of all vector data
	let union = boolean_operation_on_vector_data_table(vector_data, BooleanOperation::Union);
	boolean_operation_on_vector_data_table(union.instance_ref_iter().chain(std::iter::once(any_intersection.to_instance_ref())), BooleanOperation::SubtractFront)
}

fn flatten_vector_data(graphic_group_table: &GraphicGroupTable) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for element in graphic_group_table.instance_ref_iter() {
		match element.instance.clone() {
			GraphicElement::VectorData(vector_data) => {
				// Apply the parent group's transform to each element of vector data
				for mut sub_vector_data in vector_data.instance_iter() {
					sub_vector_data.transform = *element.transform * sub_vector_data.transform;

					result_table.push(sub_vector_data);
				}
			}
			GraphicElement::RasterDataCPU(image) => {
				let make_instance = |transform| {
					// Convert the image frame into a rectangular subpath with the image's transform
					let mut subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
					subpath.apply_transform(transform);

					// Create a vector data table row from the rectangular subpath, with a default black fill
					let mut instance = VectorData::from_subpath(subpath);
					instance.style.set_fill(Fill::Solid(Color::BLACK));

					Instance { instance, ..Default::default() }
				};

				// Apply the parent group's transform to each element of raster data
				for instance in image.instance_ref_iter() {
					result_table.push(make_instance(*element.transform * *instance.transform));
				}
			}
			GraphicElement::RasterDataGPU(image) => {
				let make_instance = |transform| {
					// Convert the image frame into a rectangular subpath with the image's transform
					let mut subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
					subpath.apply_transform(transform);

					// Create a vector data table row from the rectangular subpath, with a default black fill
					let mut instance = VectorData::from_subpath(subpath);
					instance.style.set_fill(Fill::Solid(Color::BLACK));

					Instance { instance, ..Default::default() }
				};

				// Apply the parent group's transform to each element of raster data
				for instance in image.instance_ref_iter() {
					result_table.push(make_instance(*element.transform * *instance.transform));
				}
			}
			GraphicElement::GraphicGroup(mut graphic_group) => {
				// Apply the parent group's transform to each element of inner group
				for sub_element in graphic_group.instance_mut_iter() {
					*sub_element.transform = *element.transform * *sub_element.transform;
				}

				// Recursively flatten the inner group into vector data
				let unioned = boolean_operation_on_vector_data_table(flatten_vector_data(&graphic_group).instance_ref_iter(), BooleanOperation::Union);

				for element in unioned.instance_iter() {
					result_table.push(element);
				}
			}
		}
	}

	result_table
}

fn to_path(vector: &VectorData, transform: DAffine2) -> Vec<path_bool::PathSegment> {
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

fn from_path(path_data: &[Path]) -> VectorData {
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

	VectorData::from_subpaths(all_subpaths, false)
}

pub fn convert_usvg_path(path: &usvg::Path) -> Vec<Subpath<PointId>> {
	let mut subpaths = Vec::new();
	let mut groups = Vec::new();

	let mut points = path.data().points().iter();
	let to_vec = |p: &usvg::tiny_skia_path::Point| DVec2::new(p.x as f64, p.y as f64);

	for verb in path.data().verbs() {
		match verb {
			usvg::tiny_skia_path::PathVerb::Move => {
				subpaths.push(Subpath::new(std::mem::take(&mut groups), false));
				let Some(start) = points.next().map(to_vec) else { continue };
				groups.push(ManipulatorGroup::new(start, Some(start), Some(start)));
			}
			usvg::tiny_skia_path::PathVerb::Line => {
				let Some(end) = points.next().map(to_vec) else { continue };
				groups.push(ManipulatorGroup::new(end, Some(end), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Quad => {
				let Some(handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = groups.last_mut() {
					last.out_handle = Some(last.anchor + (2. / 3.) * (handle - last.anchor));
				}
				groups.push(ManipulatorGroup::new(end, Some(end + (2. / 3.) * (handle - end)), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Cubic => {
				let Some(first_handle) = points.next().map(to_vec) else { continue };
				let Some(second_handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = groups.last_mut() {
					last.out_handle = Some(first_handle);
				}
				groups.push(ManipulatorGroup::new(end, Some(second_handle), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Close => {
				subpaths.push(Subpath::new(std::mem::take(&mut groups), true));
			}
		}
	}
	subpaths.push(Subpath::new(groups, false));
	subpaths
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
