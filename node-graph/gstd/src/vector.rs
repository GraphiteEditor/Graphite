use crate::Node;

use bezier_rs::{ManipulatorGroup, Subpath};
use graphene_core::raster::{Bitmap, ImageFrame};
pub use graphene_core::vector::*;
use graphene_core::{transform::Footprint, GraphicGroup};
use graphene_core::{vector, Artboard};
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
			GraphicElement::GraphicGroup(graphic_group) => {
				let mut vector_data = collect_vector_data(graphic_group);
				boolean_operation_on_vector_data(vector_data, BooleanOperation::Union)
			}
			GraphicElement::ImageFrame(image) => vector_from_image(image),
			GraphicElement::Text(_) => VectorData::empty(),
			GraphicElement::Artboard(artboard) => {
				let mut vector_data = collect_vector_data(&artboard.graphic_group);
				boolean_operation_on_vector_data(vector_data, BooleanOperation::Union)
			}
		}
	};

	fn collect_vector_data(graphic_group: &GraphicGroup) -> impl Iterator<Item = VectorData> + '_ {
		graphic_group.iter().rev().map(|graphic_element| union_vector_data(graphic_element))
	}

	fn boolean_operation_on_vector_data(mut vector_data: impl Iterator<Item = VectorData>, boolean_operation: BooleanOperation) -> VectorData {
		let mut result = VectorData::empty();
		let first_vector_data = vector_data.next().unwrap_or(VectorData::empty());
		let mut second_vector_data = Some(vector_data.next().unwrap_or(VectorData::empty()));

		while let Some(lower_vector_data) = second_vector_data {
			let transform_of_lower_into_space_of_upper = first_vector_data.transform.inverse() * lower_vector_data.transform;

			let upper_path_string = to_svg_string(&first_vector_data, DAffine2::IDENTITY);
			let lower_path_string = to_svg_string(&lower_vector_data, transform_of_lower_into_space_of_upper);

			let mut use_lower_style = false;

			#[allow(unused_unsafe)]
			let boolean_operation_string = unsafe {
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
			let mut boolean_operation_result = from_svg_string(&boolean_operation_string);

			result = from_svg_string(&unsafe { boolean_union(to_svg_string(&result, DAffine2::IDENTITY), to_svg_string(&boolean_operation_result, DAffine2::IDENTITY)) });
			result.transform = first_vector_data.transform;

			result.transform = first_vector_data.transform;
			result.style = if use_lower_style { lower_vector_data.style } else { first_vector_data.style.clone() };
			result.alpha_blending = if use_lower_style { lower_vector_data.alpha_blending } else { first_vector_data.alpha_blending };

			second_vector_data = vector_data.next();
		}
		result
	}

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
