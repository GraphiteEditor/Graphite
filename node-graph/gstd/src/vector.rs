use crate::transform::Footprint;

use bezier_rs::{ManipulatorGroup, Subpath};
use graphene_core::transform::Transform;
use graphene_core::vector::misc::BooleanOperation;
pub use graphene_core::vector::*;
use graphene_core::{Color, GraphicElement, GraphicGroup};

use glam::{DAffine2, DVec2};
use path_bool::PathBooleanOperation;
use std::ops::{Div, Mul};

#[node_macro::node(category(""))]
async fn boolean_operation<F: 'n + Copy + Send>(
	#[implementations((), Footprint)] footprint: F,
	#[implementations(() -> GraphicGroup, Footprint -> GraphicGroup)] group_of_paths: impl Node<F, Output = GraphicGroup>,
	operation: BooleanOperation,
) -> VectorData {
	let group_of_paths = group_of_paths.eval(footprint).await;

	fn vector_from_image<T: Transform>(image_frame: T) -> VectorData {
		let corner1 = DVec2::ZERO;
		let corner2 = DVec2::new(1., 1.);
		let mut subpath = Subpath::new_rect(corner1, corner2);
		subpath.apply_transform(image_frame.transform());
		let mut vector_data = VectorData::from_subpath(subpath);
		vector_data
			.style
			.set_fill(graphene_core::vector::style::Fill::Solid(Color::from_rgb_str("777777").unwrap().to_gamma_srgb()));
		vector_data
	}

	fn union_vector_data(graphic_element: &GraphicElement) -> VectorData {
		match graphic_element {
			GraphicElement::VectorData(vector_data) => *vector_data.clone(),
			// Union all vector data in the graphic group into a single vector
			GraphicElement::GraphicGroup(graphic_group) => {
				let vector_data = collect_vector_data(graphic_group);
				boolean_operation_on_vector_data(&vector_data, BooleanOperation::Union)
			}
			GraphicElement::Raster(image) => vector_from_image(image),
		}
	}

	fn collect_vector_data(graphic_group: &GraphicGroup) -> Vec<VectorData> {
		// Ensure all non vector data in the graphic group is converted to vector data
		let vector_data = graphic_group.iter().map(|(element, _)| union_vector_data(element));
		// Apply the transform from the parent graphic group
		let transformed_vector_data = vector_data.map(|mut vector_data| {
			vector_data.transform = graphic_group.transform * vector_data.transform;
			vector_data
		});
		transformed_vector_data.collect::<Vec<_>>()
	}

	fn subtract<'a>(vector_data: impl Iterator<Item = &'a VectorData>) -> VectorData {
		let mut vector_data = vector_data.into_iter();
		let mut result = vector_data.next().cloned().unwrap_or_default();
		let mut next_vector_data = vector_data.next();

		while let Some(lower_vector_data) = next_vector_data {
			let transform_of_lower_into_space_of_upper = result.transform.inverse() * lower_vector_data.transform;

			let upper_path_string = to_path(&result, DAffine2::IDENTITY);
			let lower_path_string = to_path(lower_vector_data, transform_of_lower_into_space_of_upper);

			#[allow(unused_unsafe)]
			let boolean_operation_string = unsafe { boolean_subtract(upper_path_string, lower_path_string) };
			let boolean_operation_result = from_path(&boolean_operation_string);

			result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
			result.point_domain = boolean_operation_result.point_domain;
			result.segment_domain = boolean_operation_result.segment_domain;
			result.region_domain = boolean_operation_result.region_domain;

			next_vector_data = vector_data.next();
		}
		result
	}

	fn boolean_operation_on_vector_data(vector_data: &[VectorData], boolean_operation: BooleanOperation) -> VectorData {
		match boolean_operation {
			BooleanOperation::Union => {
				// Reverse vector data so that the result style is the style of the first vector data
				let mut vector_data = vector_data.iter().rev();
				let mut result = vector_data.next().cloned().unwrap_or_default();
				let mut second_vector_data = Some(vector_data.next().unwrap_or(const { &VectorData::empty() }));

				// Loop over all vector data and union it with the result
				while let Some(lower_vector_data) = second_vector_data {
					let transform_of_lower_into_space_of_upper = result.transform.inverse() * lower_vector_data.transform;

					let upper_path_string = to_path(&result, DAffine2::IDENTITY);
					let lower_path_string = to_path(lower_vector_data, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let boolean_operation_string = unsafe { boolean_union(upper_path_string, lower_path_string) };
					let boolean_operation_result = from_path(&boolean_operation_string);

					result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
					result.point_domain = boolean_operation_result.point_domain;
					result.segment_domain = boolean_operation_result.segment_domain;
					result.region_domain = boolean_operation_result.region_domain;
					second_vector_data = vector_data.next();
				}
				result
			}
			BooleanOperation::SubtractFront => subtract(vector_data.iter()),
			BooleanOperation::SubtractBack => subtract(vector_data.iter().rev()),
			BooleanOperation::Intersect => {
				let mut vector_data = vector_data.iter().rev();
				let mut result = vector_data.next().cloned().unwrap_or_default();
				let mut second_vector_data = Some(vector_data.next().unwrap_or(const { &VectorData::empty() }));

				// For each vector data, set the result to the intersection of that data and the result
				while let Some(lower_vector_data) = second_vector_data {
					let transform_of_lower_into_space_of_upper = result.transform.inverse() * lower_vector_data.transform;

					let upper_path_string = to_path(&result, DAffine2::IDENTITY);
					let lower_path_string = to_path(lower_vector_data, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let boolean_operation_string = unsafe { boolean_intersect(upper_path_string, lower_path_string) };
					let boolean_operation_result = from_path(&boolean_operation_string);

					result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
					result.point_domain = boolean_operation_result.point_domain;
					result.segment_domain = boolean_operation_result.segment_domain;
					result.region_domain = boolean_operation_result.region_domain;
					second_vector_data = vector_data.next();
				}
				result
			}
			BooleanOperation::Difference => {
				let mut vector_data_iter = vector_data.iter().rev();
				let mut any_intersection = VectorData::empty();
				let mut second_vector_data = Some(vector_data_iter.next().unwrap_or(const { &VectorData::empty() }));

				// Find where all vector data intersect at least once
				while let Some(lower_vector_data) = second_vector_data {
					let all_other_vector_data = boolean_operation_on_vector_data(&vector_data.iter().filter(|v| v != &lower_vector_data).cloned().collect::<Vec<_>>(), BooleanOperation::Union);

					let transform_of_lower_into_space_of_upper = all_other_vector_data.transform.inverse() * lower_vector_data.transform;

					let upper_path_string = to_path(&all_other_vector_data, DAffine2::IDENTITY);
					let lower_path_string = to_path(lower_vector_data, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let boolean_intersection_string = unsafe { boolean_intersect(upper_path_string, lower_path_string) };
					let mut boolean_intersection_result = from_path(&boolean_intersection_string);

					boolean_intersection_result.transform = all_other_vector_data.transform;
					boolean_intersection_result.style = all_other_vector_data.style.clone();
					boolean_intersection_result.alpha_blending = all_other_vector_data.alpha_blending;

					let transform_of_lower_into_space_of_upper = boolean_intersection_result.transform.inverse() * any_intersection.transform;

					let upper_path_string = to_path(&boolean_intersection_result, DAffine2::IDENTITY);
					let lower_path_string = to_path(&any_intersection, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let union_result = from_path(&unsafe { boolean_union(upper_path_string, lower_path_string) });
					any_intersection = union_result;

					any_intersection.transform = boolean_intersection_result.transform;
					any_intersection.style = boolean_intersection_result.style.clone();
					any_intersection.alpha_blending = boolean_intersection_result.alpha_blending;

					second_vector_data = vector_data_iter.next();
				}
				// Subtract the area where they intersect at least once from the union of all vector data
				let union = boolean_operation_on_vector_data(vector_data, BooleanOperation::Union);
				boolean_operation_on_vector_data(&[union, any_intersection], BooleanOperation::SubtractFront)
			}
		}
	}

	// The first index is the bottom of the stack
	let mut boolean_operation_result = boolean_operation_on_vector_data(&collect_vector_data(&group_of_paths), operation);

	let transform = boolean_operation_result.transform;
	VectorData::transform(&mut boolean_operation_result, transform);
	boolean_operation_result.style.set_stroke_transform(DAffine2::IDENTITY);
	boolean_operation_result.transform = DAffine2::IDENTITY;
	boolean_operation_result.upstream_graphic_group = Some(group_of_paths);

	boolean_operation_result
}

fn to_path(vector: &VectorData, transform: DAffine2) -> Vec<path_bool::PathSegment> {
	let mut path = Vec::new();
	for subpath in vector.stroke_bezier_paths() {
		to_path_segments(&mut path, &subpath, transform);
	}
	path
}

fn to_path_segments(path: &mut Vec<path_bool::PathSegment>, subpath: &bezier_rs::Subpath<PointId>, transform: DAffine2) {
	use path_bool::PathSegment;
	for bezier in subpath.iter() {
		const EPS: f64 = 1e-8;
		let transformed = bezier.apply_transformation(|pos| transform.transform_point2(pos).mul(EPS.recip()).round().div(EPS.recip()));
		let start = transformed.start;
		let end = transformed.end;
		let segment = match transformed.handles {
			bezier_rs::BezierHandles::Linear => PathSegment::Line(start, end),
			bezier_rs::BezierHandles::Quadratic { handle } => PathSegment::Quadratic(start, handle, end),
			bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => PathSegment::Cubic(start, handle_start, handle_end, end),
		};
		path.push(segment);
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
	use path_bool::FillRule;
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
fn boolean_intersect(a: Path, b: Path) -> Vec<Path> {
	path_bool(a, b, PathBooleanOperation::Intersection)
}
