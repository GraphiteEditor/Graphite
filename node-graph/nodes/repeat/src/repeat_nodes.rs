use crate::gcore::Context;
use core::f64::consts::TAU;
use core_types::list::{Item, List};
use core_types::{ATTR_TRANSFORM, CloneVarArgs, Color, Ctx, ExtractAll, InjectVarArgs, Interrupt, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::{Artboard, Graphic, Vector};
use raster_types::{CPU, GPU, Raster};
use vector_types::Gradient;
use vector_types::vector::{no_paths, paths_from_list_row_attributes};

#[node_macro::node(category("Graphic"), icon("NodeRepeat"))]
fn repeat<T: Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> List<String>,
		Context -> List<bool>,
		Context -> List<f64>,
		Context -> List<i64>,
		Context -> List<DVec2>,
		Context -> List<DAffine2>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<Artboard>,
	)]
	content: impl Node<Context<'static>, Output = List<T>>,
	#[default(1)]
	#[hard(1..)]
	count: Item<i64>,
	reverse: Item<bool>,
) -> Result<List<T>, Interrupt> {
	// Someday this node can have the option to generate infinitely instead of a fixed count (basically `std::iter::repeat`).

	let (count, reverse) = (count.into_element(), reverse.into_element());
	let count = count as usize;

	let mut result_list = List::new();

	for index in 0..count {
		let index = if reverse { count - index - 1 } else { index };

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index);
		let generated_content = content.eval(new_ctx.into_context())?;

		for generated_row in generated_content.into_iter() {
			result_list.push(generated_row);
		}
	}

	Ok(result_list)
}

#[node_macro::node(category("Graphic"), icon("NodeRepeatArray"))]
fn repeat_array<T: Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> List<String>,
		Context -> List<bool>,
		Context -> List<f64>,
		Context -> List<i64>,
		Context -> List<DVec2>,
		Context -> List<DAffine2>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<Artboard>,
	)]
	content: impl Node<Context<'static>, Output = List<T>>,
	#[unit(" px")]
	#[default(100., 100.)]
	// TODO: When using a custom Inspector layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Inspector. Investigation is needed.
	direction: Item<DVec2>,
	#[unit("°")]
	#[range]
	#[soft(-360..360)]
	angle: Item<f64>,
	#[default(5)]
	#[hard(1..)]
	count: Item<i64>,
) -> Result<List<T>, Interrupt> {
	let (direction, angle, count) = (direction.into_element(), angle.into_element(), count.into_element());
	let angle = angle.to_radians();
	// A single copy has no steps between copies, so the denominator is kept at 1 to avoid `0. / 0.` producing a NaN transform
	let total = (count - 1).max(1) as f64;

	let mut result_list = List::new();

	for index in 0..count {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let transform = DAffine2::from_angle(angle) * DAffine2::from_translation(translation);

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index as usize);
		let generated_content = content.eval(new_ctx.into_context())?;

		for row_index in 0..generated_content.len() {
			let Some(mut row) = generated_content.clone_item(row_index) else { continue };

			let local_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
			let local_translation = DAffine2::from_translation(local_transform.translation);
			let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
			*row.attribute_mut_or_insert_default(ATTR_TRANSFORM) = local_translation * transform * local_matrix;

			result_list.push(row);
		}
	}

	Ok(result_list)
}

#[node_macro::node(category("Graphic"), icon("NodeRepeatRadial"))]
fn repeat_radial<T: Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> List<String>,
		Context -> List<bool>,
		Context -> List<f64>,
		Context -> List<i64>,
		Context -> List<DVec2>,
		Context -> List<DAffine2>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<Artboard>,
	)]
	content: impl Node<Context<'static>, Output = List<T>>,
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	start_angle: Item<f64>,
	#[unit(" px")]
	#[default(5)]
	radius: Item<f64>,
	#[default(5)]
	#[hard(1..)]
	count: Item<i64>,
) -> Result<List<T>, Interrupt> {
	let (start_angle, radius, count) = (start_angle.into_element(), radius.into_element(), count.into_element());

	let mut result_list = List::new();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + start_angle.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index as usize);
		let generated_content = content.eval(new_ctx.into_context())?;

		for row_index in 0..generated_content.len() {
			let Some(mut row) = generated_content.clone_item(row_index) else { continue };

			let local_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
			let local_translation = DAffine2::from_translation(local_transform.translation);
			let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
			*row.attribute_mut_or_insert_default(ATTR_TRANSFORM) = local_translation * transform * local_matrix;

			result_list.push(row);
		}
	}

	Ok(result_list)
}

#[node_macro::node(category("Graphic"), icon("NodeRepeatOnPoints"), name("Repeat on Points"))]
fn repeat_on_points<T: Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Sync + Ctx + InjectVarArgs,
	points: List<Vector>,
	#[implementations(
		Context -> List<String>,
		Context -> List<bool>,
		Context -> List<f64>,
		Context -> List<i64>,
		Context -> List<DVec2>,
		Context -> List<DAffine2>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<Artboard>,
	)]
	content: impl Node<Context<'static>, Output = List<T>>,
	reverse: Item<bool>,
) -> Result<List<T>, Interrupt> {
	let reverse = reverse.into_element();

	let mut result_list = List::new();

	for points_index in 0..points.len() {
		let Some(points_element) = points.element(points_index) else { continue };
		let transform: DAffine2 = points.attribute_cloned_or_default(ATTR_TRANSFORM, points_index);

		let mut iteration = |index, point| -> Result<(), Interrupt> {
			let transformed_point = transform.transform_point2(point);

			let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index).with_position(transformed_point);
			let generated_content = content.eval(new_ctx.into_context())?;

			for mut generated_row in generated_content.into_iter() {
				generated_row.attribute_mut_or_insert_default::<DAffine2>(ATTR_TRANSFORM).translation = transformed_point;
				result_list.push(generated_row);
			}
			Ok(())
		};

		let paths = paths_from_list_row_attributes(&points, points_index).unwrap_or_else(|| no_paths());
		let positions = points_element.anchor_positions(paths);
		let range = positions.iter().enumerate();
		if reverse {
			for (index, &point) in range.rev() {
				iteration(index, point)?;
			}
		} else {
			for (index, &point) in range {
				iteration(index, point)?;
			}
		}
	}

	Ok(result_list)
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::list::Item;
	use core_types::transform::Footprint;
	use core_types::{LazyInput, Node, Poll, Status};
	use glam::DVec2;
	use graphene_core::ReadPositionNode;
	use graphic_types::Vector;
	use kurbo::Shape;
	use kurbo::{BezPath, DEFAULT_ACCURACY, Rect};
	use vector_nodes::generator_nodes::RectangleNode;
	use vector_types::vector::Path;
	use vector_types::vector::misc::BoxCorners;

	fn vector_node_from_bezpath(bezpath: BezPath) -> List<Vector> {
		List::new_from_item(Vector::item_from_bezpath(&bezpath))
	}

	/// The paths riding the row, so geometry queries can resolve against them.
	fn row_paths(list: &List<Vector>, index: usize) -> &List<Path> {
		paths_from_list_row_attributes(list, index).unwrap_or_else(|| no_paths())
	}

	#[derive(Clone)]
	pub struct ValueNode<T: Clone>(T);

	impl<I, T: Clone> Node<I> for ValueNode<T> {
		type Output = T;
		fn eval(&self, _input: &I) -> Poll<T> {
			Poll::Final(self.0.clone())
		}
	}

	// Raises a generator's rank-0 `Item<Vector>` output to a singleton `List<Vector>` for the still-list-typed Repeat on Points content connector
	#[derive(Clone)]
	pub struct RaiseToListNode<N>(N);

	impl<I, N: Node<I, Output = Item<Vector>>> Node<I> for RaiseToListNode<N> {
		type Output = List<Vector>;
		fn eval(&self, input: &I) -> Poll<List<Vector>> {
			self.0.eval(input).map(Into::into)
		}
	}

	/// Test helper that extracts the Y component of an upstream node's `Item<DVec2>` output.
	#[derive(Clone)]
	struct ExtractYNode<Position>(Position);

	impl<I: Ctx, Position> Node<I> for ExtractYNode<Position>
	where
		Position: Node<I, Output = Item<DVec2>>,
	{
		type Output = Item<f64>;
		fn eval(&self, input: &I) -> Poll<Item<f64>> {
			self.0.eval(input).map(|position| Item::new_from_element(position.element().y))
		}
	}

	#[test]
	fn repeat_on_points_test() {
		let context = OwnedContextImpl::default().into_context();
		let rect = RectangleNode::new(
			ValueNode(()),
			ExtractYNode(ReadPositionNode::new(ValueNode(()), ValueNode(Item::new_from_element(0_i64)))),
			ValueNode(Item::new_from_element(2_f64)),
			ValueNode(Item::new_from_element(BoxCorners::default())),
			ValueNode(Item::new_from_element(false)),
			ValueNode(Item::new_from_element(false)),
		);

		let positions = [DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let mut polyline = BezPath::new();
		polyline.move_to((positions[0].x, positions[0].y));
		for position in &positions[1..] {
			polyline.line_to((position.x, position.y));
		}
		let points = vector_node_from_bezpath(polyline);
		let status = Status::new();
		let generated = super::repeat_on_points(context, points, LazyInput::new(&RaiseToListNode(rect), &status, 1), Item::new_from_element(false)).unwrap();
		assert_eq!(generated.len(), positions.len());
		for (position, index) in positions.into_iter().zip(0..generated.len()) {
			let bounds = generated
				.element(index)
				.unwrap()
				.bounding_box_with_transform(row_paths(&generated, index), generated.attribute_cloned_or_default(ATTR_TRANSFORM, index))
				.unwrap();
			assert!(position.abs_diff_eq((bounds[0] + bounds[1]) / 2., 1e-10));
			assert_eq!((bounds[1] - bounds[0]).x, position.y);
		}
	}

	#[test]
	fn repeat() {
		let direction = DVec2::X * 1.5;
		let count = 3;
		let context = OwnedContextImpl::default().into_context();
		let status = Status::new();
		let repeated = super::repeat_array(
			context,
			LazyInput::new(&ValueNode(vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY))), &status, 0),
			Item::new_from_element(direction),
			Item::new_from_element(0.),
			Item::new_from_element(count),
		)
		.unwrap();
		let vector_list = List::new_from_item(vector_nodes::combine_paths(Footprint::default(), List::new_from_element(Graphic::VectorList(repeated))));
		let vector = vector_list.element(0).unwrap();
		let paths = row_paths(&vector_list, 0);
		assert_eq!(vector.stroke_manipulator_groups(paths).count(), 3);
		for (index, (manipulator_groups, _)) in vector.stroke_manipulator_groups(paths).enumerate() {
			assert!((manipulator_groups[0].anchor - direction * index as f64 / (count - 1) as f64).length() < 1e-5);
		}
	}

	#[test]
	fn repeat_single_copy() {
		let context = OwnedContextImpl::default().into_context();
		let status = Status::new();
		let repeated = super::repeat_array(
			context,
			LazyInput::new(&ValueNode(vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY))), &status, 0),
			Item::new_from_element(DVec2::new(12., 10.)),
			Item::new_from_element(45.),
			Item::new_from_element(1),
		)
		.unwrap();
		let vector_list = List::new_from_item(vector_nodes::combine_paths(Footprint::default(), List::new_from_element(Graphic::VectorList(repeated))));
		let vector = vector_list.element(0).unwrap();
		let paths = row_paths(&vector_list, 0);
		assert_eq!(vector.stroke_manipulator_groups(paths).count(), 1);

		let (manipulator_groups, _) = vector.stroke_manipulator_groups(paths).next().unwrap();
		let anchor = manipulator_groups[0].anchor;
		assert!(anchor.length() < 1e-5, "Expected the single copy to be untransformed, found anchor {anchor}");
	}

	#[test]
	fn repeat_transform_position() {
		let direction = DVec2::new(12., 10.);
		let count = 8;
		let context = OwnedContextImpl::default().into_context();
		let status = Status::new();
		let repeated = super::repeat_array(
			context,
			LazyInput::new(&ValueNode(vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY))), &status, 0),
			Item::new_from_element(direction),
			Item::new_from_element(0.),
			Item::new_from_element(count),
		)
		.unwrap();
		let vector_list = List::new_from_item(vector_nodes::combine_paths(Footprint::default(), List::new_from_element(Graphic::VectorList(repeated))));
		let vector = vector_list.element(0).unwrap();
		let paths = row_paths(&vector_list, 0);
		assert_eq!(vector.stroke_manipulator_groups(paths).count(), 8);
		for (index, (manipulator_groups, _)) in vector.stroke_manipulator_groups(paths).enumerate() {
			assert!((manipulator_groups[0].anchor - direction * index as f64 / (count - 1) as f64).length() < 1e-5);
		}
	}

	#[test]
	fn repeat_radial() {
		let context = OwnedContextImpl::default().into_context();
		let status = Status::new();
		let repeated = super::repeat_radial(
			context,
			LazyInput::new(&ValueNode(vector_node_from_bezpath(Rect::new(-1., -1., 1., 1.).to_path(DEFAULT_ACCURACY))), &status, 0),
			Item::new_from_element(45.),
			Item::new_from_element(4.),
			Item::new_from_element(8),
		)
		.unwrap();
		let vector_list = List::new_from_item(vector_nodes::combine_paths(Footprint::default(), List::new_from_element(Graphic::VectorList(repeated))));
		let vector = vector_list.element(0).unwrap();
		let paths = row_paths(&vector_list, 0);
		assert_eq!(vector.stroke_manipulator_groups(paths).count(), 8);

		for (index, (manipulator_groups, _)) in vector.stroke_manipulator_groups(paths).enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;

			let center = (manipulator_groups[0].anchor + manipulator_groups[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_to(center).to_degrees();

			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5, "Expected {expected_angle} found {actual_angle}");
		}
	}
}
