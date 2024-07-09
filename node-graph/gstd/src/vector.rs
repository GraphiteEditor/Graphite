use crate::Node;

use bezier_rs::{ManipulatorGroup, Subpath};
use graphene_core::raster::ImageFrame;
pub use graphene_core::vector::*;
use graphene_core::{transform::Footprint, GraphicGroup};
use graphene_core::{vector::misc::BooleanOperation, GraphicElement};

use futures::Future;
use glam::{DAffine2, DVec2};
use wasm_bindgen::prelude::*;

pub struct BinaryBooleanOperationNode<LowerVectorData, BooleanOp> {
	lower_vector_data: LowerVectorData,
	boolean_operation: BooleanOp,
}

#[node_macro::node_fn(BinaryBooleanOperationNode)]
async fn binary_boolean_operation_node<Fut: Future<Output = VectorData>>(
	upper_vector_data: VectorData,
	lower_vector_data: impl Node<Footprint, Output = Fut>,
	boolean_operation: BooleanOperation,
) -> VectorData {
	let lower_vector_data = self.lower_vector_data.eval(Footprint::default()).await;
	let transform_of_lower_into_space_of_upper = upper_vector_data.transform.inverse() * lower_vector_data.transform;

	let upper_path_string = to_svg_string(&upper_vector_data, DAffine2::IDENTITY);
	let lower_path_string = to_svg_string(&lower_vector_data, transform_of_lower_into_space_of_upper);

	let mut use_lower_style = false;

	#[allow(unused_unsafe)]
	let result = unsafe {
		match boolean_operation {
			BooleanOperation::Union => boolean_union(upper_path_string, lower_path_string),
			BooleanOperation::SubtractFront => {
				use_lower_style = true;
				boolean_subtract(lower_path_string, upper_path_string)
			}
			BooleanOperation::SubtractBack => boolean_subtract(upper_path_string, lower_path_string),
			BooleanOperation::Intersect => boolean_intersect(upper_path_string, lower_path_string),
			BooleanOperation::Difference => boolean_difference(upper_path_string, lower_path_string),
			BooleanOperation::Divide => boolean_divide(upper_path_string, lower_path_string),
		}
	};

	let mut result = from_svg_string(&result);
	result.transform = upper_vector_data.transform;
	result.style = if use_lower_style { lower_vector_data.style } else { upper_vector_data.style };
	result.alpha_blending = if use_lower_style { lower_vector_data.alpha_blending } else { upper_vector_data.alpha_blending };

	result
}

pub struct BooleanOperationNode<BooleanOp> {
	boolean_operation: BooleanOp,
}

#[node_macro::node_fn(BooleanOperationNode)]
fn boolean_operation_node(graphic_group: GraphicGroup, boolean_operation: BooleanOperation) -> VectorData {
	fn vector_from_image<P: graphene_core::raster::Pixel>(image_frame: &ImageFrame<P>) -> VectorData {
		let corner1 = DVec2::ZERO;
		let corner2 = DVec2::new(image_frame.image.width as f64, image_frame.image.height as f64);
		let mut subpath = Subpath::new_rect(corner1, corner2);
		subpath.apply_transform(image_frame.transform);
		VectorData::from_subpath(subpath)
	}

	fn union_vector_data(graphic_element: &GraphicElement) -> VectorData {
		match graphic_element {
			GraphicElement::VectorData(vector_data) => *vector_data.clone(),
			// Union all vector data in the graphic group into a single vector
			GraphicElement::GraphicGroup(graphic_group) => {
				let vector_data = collect_vector_data(graphic_group);
				boolean_operation_on_vector_data(vector_data, BooleanOperation::Union)
			}
			GraphicElement::ImageFrame(image) => vector_from_image(image),
			GraphicElement::Text(_) => VectorData::empty(),
			// Union all vector data in the artboard into a single vector
			GraphicElement::Artboard(artboard) => {
				let artboard_subpath = Subpath::new_rect(artboard.location.as_dvec2(), artboard.location.as_dvec2() + artboard.dimensions.as_dvec2());

				let mut artboard_vector = VectorData::from_subpath(artboard_subpath);
				artboard_vector.style.set_fill(graphene_core::vector::style::Fill::Solid(artboard.background));

				let mut vector_data = vec![artboard_vector];
				vector_data.extend(collect_vector_data(&artboard.graphic_group).into_iter());

				boolean_operation_on_vector_data(vector_data, BooleanOperation::Union)
			}
		}
	}

	fn collect_vector_data(graphic_group: &GraphicGroup) -> Vec<VectorData> {
		// Ensure all non vector data in the graphic group is converted to vector data
		graphic_group.iter().map(|graphic_element| union_vector_data(graphic_element)).collect::<Vec<_>>()
	}

	fn boolean_operation_on_vector_data(mut vector_data: Vec<VectorData>, boolean_operation: BooleanOperation) -> VectorData {
		match boolean_operation {
			BooleanOperation::Union => {
				// Reverse vector data so that the result style is the style of the first vector data
				let mut vector_data = vector_data.into_iter().rev();
				let mut result = vector_data.next().unwrap_or(VectorData::empty());
				let mut second_vector_data = Some(vector_data.next().unwrap_or(VectorData::empty()));

				// Loop over all vector data and union it with the result
				while let Some(lower_vector_data) = second_vector_data {
					let transform_of_lower_into_space_of_upper = result.transform.inverse() * lower_vector_data.transform;

					let upper_path_string = to_svg_string(&result, DAffine2::IDENTITY);
					let lower_path_string = to_svg_string(&lower_vector_data, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let boolean_operation_string = unsafe { boolean_union(upper_path_string, lower_path_string) };
					let boolean_operation_result = from_svg_string(&boolean_operation_string);

					result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
					result.point_domain = boolean_operation_result.point_domain;
					result.segment_domain = boolean_operation_result.segment_domain;
					result.region_domain = boolean_operation_result.region_domain;
					second_vector_data = vector_data.next();
				}
				result
			}
			BooleanOperation::SubtractFront => {
				// Reverse vector data so that the first vector data is subtracted from the rest
				let mut vector_data = vector_data.into_iter().rev();
				let upper_vector_data = vector_data.next().unwrap_or(VectorData::empty());
				// Union the rest of the data into a single vector
				let lower_vector_data = boolean_operation_on_vector_data(vector_data.rev().collect::<Vec<_>>(), BooleanOperation::Union);

				let transform_of_lower_into_space_of_upper = upper_vector_data.transform.inverse() * lower_vector_data.transform;

				let upper_path_string = to_svg_string(&upper_vector_data, DAffine2::IDENTITY);
				let lower_path_string = to_svg_string(&lower_vector_data, transform_of_lower_into_space_of_upper);

				#[allow(unused_unsafe)]
				let boolean_operation_string = unsafe { boolean_subtract(lower_path_string, upper_path_string) };
				let mut boolean_operation_result = from_svg_string(&boolean_operation_string);

				boolean_operation_result.transform = upper_vector_data.transform;
				boolean_operation_result.style = lower_vector_data.style;
				boolean_operation_result.alpha_blending = lower_vector_data.alpha_blending;

				boolean_operation_result
			}
			BooleanOperation::SubtractBack => {
				// Rotate the vector data so that the last vector data is at the start, and subtract it from the rest
				vector_data.rotate_left(1);
				boolean_operation_on_vector_data(vector_data, BooleanOperation::SubtractFront)
			}
			BooleanOperation::Intersect => {
				let mut vector_data = vector_data.into_iter().rev();
				let mut result = vector_data.next().unwrap_or(VectorData::empty());
				let mut second_vector_data = Some(vector_data.next().unwrap_or(VectorData::empty()));

				// For each vector data, set the result to the intersection of that data and the result
				while let Some(lower_vector_data) = second_vector_data {
					let transform_of_lower_into_space_of_upper = result.transform.inverse() * lower_vector_data.transform;

					let upper_path_string = to_svg_string(&result, DAffine2::IDENTITY);
					let lower_path_string = to_svg_string(&lower_vector_data, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let boolean_operation_string = unsafe { boolean_intersect(upper_path_string, lower_path_string) };
					let boolean_operation_result = from_svg_string(&boolean_operation_string);

					result.colinear_manipulators = boolean_operation_result.colinear_manipulators;
					result.point_domain = boolean_operation_result.point_domain;
					result.segment_domain = boolean_operation_result.segment_domain;
					result.region_domain = boolean_operation_result.region_domain;
					second_vector_data = vector_data.next();
				}
				result
			}
			BooleanOperation::Difference => {
				let mut vector_data_iter = vector_data.clone().into_iter().rev();

				let mut any_intersection = VectorData::empty();
				let mut second_vector_data = Some(vector_data_iter.next().unwrap_or(VectorData::empty()));

				// Find where all vector data intersect at least once
				while let Some(lower_vector_data) = second_vector_data {
					let all_other_vector_data = boolean_operation_on_vector_data(vector_data.clone().into_iter().filter(|v| v != &lower_vector_data).collect::<Vec<_>>(), BooleanOperation::Union);

					let transform_of_lower_into_space_of_upper = all_other_vector_data.transform.inverse() * lower_vector_data.transform;

					let upper_path_string = to_svg_string(&all_other_vector_data, DAffine2::IDENTITY);
					let lower_path_string = to_svg_string(&lower_vector_data, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let boolean_intersection_string = unsafe { boolean_intersect(upper_path_string, lower_path_string) };
					let mut boolean_intersection_result = from_svg_string(&boolean_intersection_string);

					boolean_intersection_result.transform = all_other_vector_data.transform;
					boolean_intersection_result.style = all_other_vector_data.style.clone();
					boolean_intersection_result.alpha_blending = all_other_vector_data.alpha_blending;

					let transform_of_lower_into_space_of_upper = boolean_intersection_result.transform.inverse() * any_intersection.transform;

					let upper_path_string = to_svg_string(&boolean_intersection_result, DAffine2::IDENTITY);
					let lower_path_string = to_svg_string(&any_intersection, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let union_result = from_svg_string(&unsafe { boolean_union(upper_path_string, lower_path_string) });
					any_intersection = union_result;

					any_intersection.transform = boolean_intersection_result.transform;
					any_intersection.style = boolean_intersection_result.style.clone();
					any_intersection.alpha_blending = boolean_intersection_result.alpha_blending;

					second_vector_data = vector_data_iter.next();
				}
				// Subtract the area where they intersect at least once from the union of all vector data
				let union = boolean_operation_on_vector_data(vector_data.clone(), BooleanOperation::Union);
				boolean_operation_on_vector_data(vec![union, any_intersection], BooleanOperation::SubtractFront)
			}
			BooleanOperation::Divide => {
				let mut vector_data = vector_data.into_iter().rev();
				let mut result = vector_data.next().unwrap_or(VectorData::empty());
				let mut second_vector_data = Some(vector_data.next().unwrap_or(VectorData::empty()));

				// For each vector data, set the result to the division of that data and the result
				while let Some(lower_vector_data) = second_vector_data {
					let transform_of_lower_into_space_of_upper = result.transform.inverse() * lower_vector_data.transform;

					let upper_path_string = to_svg_string(&result, DAffine2::IDENTITY);
					let lower_path_string = to_svg_string(&lower_vector_data, transform_of_lower_into_space_of_upper);

					#[allow(unused_unsafe)]
					let boolean_operation_string = unsafe { boolean_divide(upper_path_string, lower_path_string) };
					let boolean_union_result = from_svg_string(&boolean_operation_string);

					result.colinear_manipulators = boolean_union_result.colinear_manipulators;
					result.point_domain = boolean_union_result.point_domain;
					result.segment_domain = boolean_union_result.segment_domain;
					result.region_domain = boolean_union_result.region_domain;
					second_vector_data = vector_data.next();
				}
				result
			}
		}
	}

	// The first index is the bottom of the stack
	boolean_operation_on_vector_data(collect_vector_data(&graphic_group), boolean_operation)
}

fn to_svg_string(vector: &VectorData, transform: DAffine2) -> String {
	let mut path = String::new();
	for (_, subpath) in vector.region_bezier_paths() {
		let _ = subpath.subpath_to_svg(&mut path, transform);
	}
	path
}

fn from_svg_string(svg_string: &str) -> VectorData {
	let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="{}"></path></svg>"#, svg_string);
	let Some(tree) = usvg::Tree::from_str(&svg, &Default::default()).ok() else {
		return VectorData::empty();
	};
	let Some(usvg::Node::Path(path)) = tree.root.children.first() else {
		return VectorData::empty();
	};

	VectorData::from_subpaths(convert_usvg_path(path), false)
}

pub fn convert_usvg_path(path: &usvg::Path) -> Vec<Subpath<PointId>> {
	let mut subpaths = Vec::new();
	let mut groups = Vec::new();

	let mut points = path.data.points().iter();
	let to_vec = |p: &usvg::tiny_skia_path::Point| DVec2::new(p.x as f64, p.y as f64);

	for verb in path.data.verbs() {
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

#[wasm_bindgen(module = "/../../frontend/src/utility-functions/computational-geometry.ts")]
extern "C" {
	#[wasm_bindgen(js_name = booleanUnion)]
	fn boolean_union(path1: String, path2: String) -> String;
	#[wasm_bindgen(js_name = booleanSubtract)]
	fn boolean_subtract(path1: String, path2: String) -> String;
	#[wasm_bindgen(js_name = booleanIntersect)]
	fn boolean_intersect(path1: String, path2: String) -> String;
	#[wasm_bindgen(js_name = booleanDifference)]
	fn boolean_difference(path1: String, path2: String) -> String;
	#[wasm_bindgen(js_name = booleanDivide)]
	fn boolean_divide(path1: String, path2: String) -> String;
}
