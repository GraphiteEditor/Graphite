use crate::gcore::Context;
use core::f64::consts::TAU;
use core_types::list::List;
use core_types::registry::types::{Angle, PixelSize};
use core_types::{ATTR_TRANSFORM, CloneVarArgs, Color, Ctx, ExtractAll, InjectVarArgs, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::{Graphic, Vector};
use raster_types::{CPU, Raster};
use vector_types::GradientStops;

#[node_macro::node(category("Repeat"))]
async fn repeat<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<'n, Context<'static>, Output = List<T>>,
	#[default(1)]
	#[hard(1..)]
	count: u32,
	reverse: bool,
) -> List<T> {
	// Someday this node can have the option to generate infinitely instead of a fixed count (basically `std::iter::repeat`).

	let count = count as usize;

	let mut result_list = List::new();

	for index in 0..count {
		let index = if reverse { count - index - 1 } else { index };

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index);
		let generated_content = content.eval(new_ctx.into_context()).await;

		for generated_row in generated_content.into_iter() {
			result_list.push(generated_row);
		}
	}

	result_list
}

#[node_macro::node(category("Repeat"))]
pub async fn repeat_array<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<'n, Context<'static>, Output = List<T>>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: PixelSize,
	angle: Angle,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> List<T> {
	let angle = angle.to_radians();
	// A single copy has no steps between copies, so the denominator is kept at 1 to avoid `0. / 0.` producing a NaN transform
	let total = (count - 1).max(1) as f64;

	let mut result_list = List::new();

	for index in 0..count {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let transform = DAffine2::from_angle(angle) * DAffine2::from_translation(translation);

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index as usize);
		let generated_content = content.eval(new_ctx.into_context()).await;

		for row_index in 0..generated_content.len() {
			let Some(mut row) = generated_content.clone_item(row_index) else { continue };

			let local_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
			let local_translation = DAffine2::from_translation(local_transform.translation);
			let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
			*row.attribute_mut_or_insert_default(ATTR_TRANSFORM) = local_translation * transform * local_matrix;

			result_list.push(row);
		}
	}

	result_list
}

#[node_macro::node(category("Repeat"))]
async fn repeat_radial<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<'n, Context<'static>, Output = List<T>>,
	start_angle: Angle,
	#[unit(" px")]
	#[default(5)]
	radius: f64,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> List<T> {
	let mut result_list = List::new();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + start_angle.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index as usize);
		let generated_content = content.eval(new_ctx.into_context()).await;

		for row_index in 0..generated_content.len() {
			let Some(mut row) = generated_content.clone_item(row_index) else { continue };

			let local_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
			let local_translation = DAffine2::from_translation(local_transform.translation);
			let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
			*row.attribute_mut_or_insert_default(ATTR_TRANSFORM) = local_translation * transform * local_matrix;

			result_list.push(row);
		}
	}

	result_list
}

#[node_macro::node(category("Repeat"), name("Repeat on Points"))]
async fn repeat_on_points<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Sync + Ctx + InjectVarArgs,
	points: List<Vector>,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<'n, Context<'static>, Output = List<T>>,
	reverse: bool,
) -> List<T> {
	let mut result_list = List::new();

	for points_index in 0..points.len() {
		let Some(points_element) = points.element(points_index) else { continue };
		let transform: DAffine2 = points.attribute_cloned_or_default(ATTR_TRANSFORM, points_index);

		let mut iteration = async |index, point| {
			let transformed_point = transform.transform_point2(point);

			let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index).with_position(transformed_point);
			let generated_content = content.eval(new_ctx.into_context()).await;

			for mut generated_row in generated_content.into_iter() {
				generated_row.attribute_mut_or_insert_default::<DAffine2>(ATTR_TRANSFORM).translation = transformed_point;
				result_list.push(generated_row);
			}
		};

		let range = points_element.point_domain.positions().iter().enumerate();
		if reverse {
			for (index, &point) in range.rev() {
				iteration(index, point).await;
			}
		} else {
			for (index, &point) in range {
				iteration(index, point).await;
			}
		}
	}

	result_list
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Ctx;
	use core_types::Node;
	use core_types::list::Item;
	use core_types::transform::Footprint;
	use glam::DVec2;
	use graphene_core::ReadPositionNode;
	use graphene_core::extract_xy::{ExtractXyNode, XY};
	use graphic_types::Vector;
	use kurbo::Shape;
	use kurbo::{BezPath, DEFAULT_ACCURACY, Rect};
	use std::future::Future;
	use std::pin::Pin;
	use vector_nodes::generator_nodes::RectangleNode;
	use vector_types::subpath::Subpath;

	fn vector_node_from_bezpath(bezpath: BezPath) -> List<Vector> {
		List::new_from_element(Vector::from_bezpath(bezpath))
	}

	#[derive(Clone)]
	pub struct FutureWrapperNode<T: Clone>(T);

	impl<'i, I: Ctx, T: 'i + Clone + Send> Node<'i, I> for FutureWrapperNode<T> {
		type Output = Pin<Box<dyn Future<Output = T> + 'i + Send>>;
		fn eval(&'i self, _input: I) -> Self::Output {
			let value = self.0.clone();
			Box::pin(async move { value })
		}
	}

	// Raises a generator's rank-0 `Item<Vector>` output to a singleton `List<Vector>` for the still-list-typed Repeat on Points content connector
	#[derive(Clone)]
	pub struct RaiseToListNode<N>(N);

	impl<'i, I: 'i, N> Node<'i, I> for RaiseToListNode<N>
	where
		N: Node<'i, I, Output = Pin<Box<dyn Future<Output = Item<Vector>> + 'i + Send>>>,
	{
		type Output = Pin<Box<dyn Future<Output = List<Vector>> + 'i + Send>>;
		fn eval(&'i self, input: I) -> Self::Output {
			let future = self.0.eval(input);
			Box::pin(async move { future.await.into() })
		}
	}

	// Lowers a rank-0 `Item<T>` to its bare `T` element for a connector that predates ranked wires
	#[derive(Clone)]
	pub struct UnwrapElementNode<N>(N);

	impl<'i, I: 'i, T: 'i + Send, N> Node<'i, I> for UnwrapElementNode<N>
	where
		N: Node<'i, I, Output = Pin<Box<dyn Future<Output = Item<T>> + 'i + Send>>>,
	{
		type Output = Pin<Box<dyn Future<Output = T> + 'i + Send>>;
		fn eval(&'i self, input: I) -> Self::Output {
			let future = self.0.eval(input);
			Box::pin(async move { future.await.into_element() })
		}
	}

	#[tokio::test]
	async fn repeat_on_points_test() {
		let context = OwnedContextImpl::default().into_context();
		let rect = RectangleNode::new(
			FutureWrapperNode(()),
			UnwrapElementNode(ExtractXyNode::new(
				ReadPositionNode::new(FutureWrapperNode(()), FutureWrapperNode(0)),
				FutureWrapperNode(Item::new_from_element(XY::Y)),
			)),
			FutureWrapperNode(2_f64),
			FutureWrapperNode(false),
			FutureWrapperNode(0_f64),
			FutureWrapperNode(false),
		);

		let positions = [DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let points = List::new_from_element(Vector::from_subpath(Subpath::from_anchors(positions, false)));
		let generated = super::repeat_on_points(context, points, &RaiseToListNode(rect), false).await;
		assert_eq!(generated.len(), positions.len());
		for (position, index) in positions.into_iter().zip(0..generated.len()) {
			let bounds = generated
				.element(index)
				.unwrap()
				.bounding_box_with_transform(generated.attribute_cloned_or_default(ATTR_TRANSFORM, index))
				.unwrap();
			assert!(position.abs_diff_eq((bounds[0] + bounds[1]) / 2., 1e-10));
			assert_eq!((bounds[1] - bounds[0]).x, position.y);
		}
	}

	#[tokio::test]
	async fn repeat() {
		let direction = DVec2::X * 1.5;
		let count = 3;
		let context = OwnedContextImpl::default().into_context();
		let repeated = super::repeat_array(
			context,
			&FutureWrapperNode(vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY))),
			direction,
			0.,
			count,
		)
		.await;
		let vector_list = vector_nodes::flatten_path(Footprint::default(), repeated).await;
		let vector = vector_list.element(0).unwrap();
		assert_eq!(vector.region_manipulator_groups().count(), 3);
		for (index, (_, manipulator_groups)) in vector.region_manipulator_groups().enumerate() {
			assert!((manipulator_groups[0].anchor - direction * index as f64 / (count - 1) as f64).length() < 1e-5);
		}
	}

	#[tokio::test]
	async fn repeat_single_copy() {
		let context = OwnedContextImpl::default().into_context();
		let repeated = super::repeat_array(
			context,
			&FutureWrapperNode(vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY))),
			DVec2::new(12., 10.),
			45.,
			1,
		)
		.await;
		let vector_list = vector_nodes::flatten_path(Footprint::default(), repeated).await;
		let vector = vector_list.element(0).unwrap();
		assert_eq!(vector.region_manipulator_groups().count(), 1);

		let (_, manipulator_groups) = vector.region_manipulator_groups().next().unwrap();
		let anchor = manipulator_groups[0].anchor;
		assert!(anchor.length() < 1e-5, "Expected the single copy to be untransformed, found anchor {anchor}");
	}

	#[tokio::test]
	async fn repeat_transform_position() {
		let direction = DVec2::new(12., 10.);
		let count = 8;
		let context = OwnedContextImpl::default().into_context();
		let repeated = super::repeat_array(
			context,
			&FutureWrapperNode(vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY))),
			direction,
			0.,
			count,
		)
		.await;
		let vector_list = vector_nodes::flatten_path(Footprint::default(), repeated).await;
		let vector = vector_list.element(0).unwrap();
		assert_eq!(vector.region_manipulator_groups().count(), 8);
		for (index, (_, manipulator_groups)) in vector.region_manipulator_groups().enumerate() {
			assert!((manipulator_groups[0].anchor - direction * index as f64 / (count - 1) as f64).length() < 1e-5);
		}
	}

	#[tokio::test]
	async fn repeat_radial() {
		let context = OwnedContextImpl::default().into_context();
		let repeated = super::repeat_radial(context, &FutureWrapperNode(vector_node_from_bezpath(Rect::new(-1., -1., 1., 1.).to_path(DEFAULT_ACCURACY))), 45., 4., 8).await;
		let vector_list = vector_nodes::flatten_path(Footprint::default(), repeated).await;
		let vector = vector_list.element(0).unwrap();
		assert_eq!(vector.region_manipulator_groups().count(), 8);

		for (index, (_, manipulator_groups)) in vector.region_manipulator_groups().enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;

			let center = (manipulator_groups[0].anchor + manipulator_groups[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_to(center).to_degrees();

			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5, "Expected {expected_angle} found {actual_angle}");
		}
	}
}
