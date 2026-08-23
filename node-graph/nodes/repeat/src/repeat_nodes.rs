use crate::gcore::Context;
use core::f64::consts::TAU;
use core_types::context::IndexLink;
use core_types::extent::{ExtentIn, LevelIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::list::List;
use core_types::registry::types::{Angle, PixelSize};
use core_types::attribute::{Attr, Transform as TransformAttr};
use core_types::{ATTR_TRANSFORM, Color, Ctx, DeriveCtx, ExtractIndex, InjectVarArgs};
use glam::{DAffine2, DVec2};
use graphic_types::{Graphic, Vector};
use raster_types::{CPU, Raster};
use vector_types::GradientStops;

/// Each copy evaluates the content within the copy's index pushed in,
/// producing a level of `count` copies.
// Someday this node can have the option to generate infinitely instead of a fixed count (basically `std::iter::repeat`).
#[node_macro::node(category("Repeat"), extent(repeat_extent))]
fn repeat<T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex,
	content: impl Node<Context<'_>, Output = T>,
	#[default(1)]
	#[hard(1..)]
	count: u32,
	reverse: bool,
) -> Result<IList<T>, Interrupt> {
	let inner = content.inner_extent(ctx)?;
	let (copy, rest) = ctx.split_innermost(inner);
	if copy >= count as u64 {
		return Err(GraphError::past_end().into());
	}
	let copy = match reverse {
		true => count as u64 - 1 - copy,
		false => copy,
	};
	let mut frame = IndexLink { index: 0, outer: None };
	content.eval(&ctx.push_level(&mut frame, copy, rest))
}

/// The pushed level's extent is the copy count; inner levels forward to the
/// content, whose extent is taken uniform across copies (queried at copy 0).
fn repeat_extent(content: ExtentIn<'_>, count: ValueIn<'_, u32>, _reverse: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => content.at(level),
	}
}

/// Each copy evaluates the content within the copy's index pushed in, the
/// copy's step transform composed between the lane transform's translation
/// and matrix parts.
#[node_macro::node(category("Repeat"), extent(repeat_array_extent))]
pub fn repeat_array<T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex,
	content: impl Node<Context<'_>, Output = (T, Attr<TransformAttr>)>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: PixelSize,
	angle: Angle,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> Result<IList<(T, Attr<TransformAttr>)>, Interrupt> {
	let angle = angle.to_radians();
	// A single copy has no steps between copies, so the denominator is kept at 1 to avoid `0. / 0.` producing a NaN transform
	let total = (count - 1).max(1) as f64;

	let inner = content.inner_extent(ctx)?;
	let (copy, rest) = ctx.split_innermost(inner);
	if copy >= count as u64 {
		return Err(GraphError::past_end().into());
	}
	let step_angle = copy as f64 * angle / total;
	let translation = copy as f64 * direction / total;
	let transform = DAffine2::from_angle(step_angle) * DAffine2::from_translation(translation);

	let mut frame = IndexLink { index: 0, outer: None };
	let (element, local_transform) = content.eval(&ctx.push_level(&mut frame, copy, rest))?;
	let local_translation = DAffine2::from_translation(local_transform.translation);
	let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
	Ok((element, Attr(local_translation * transform * local_matrix)))
}

/// The pushed level's extent is the copy count; inner levels forward to the
/// content, whose extent is taken uniform across copies (queried at copy 0).
fn repeat_array_extent(content: ExtentIn<'_>, _direction: ValueIn<'_, DVec2>, _angle: ValueIn<'_, f64>, count: ValueIn<'_, u32>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => content.at(level),
	}
}

#[node_macro::node(category("Repeat"))]
fn repeat_radial<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl Ctx + DeriveCtx,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<Context<'_>, Output = List<T>>,
	start_angle: Angle,
	#[unit(" px")]
	#[default(5)]
	radius: f64,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> Result<List<T>, Interrupt> {
	let spilled = ctx.index_head();
	let mut result_list = List::new();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + start_angle.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		let mark = core_types::record::stack::sp();
		let generated_content = content.eval(&ctx.promoted(&spilled, index as u64))?;

		for row_index in 0..generated_content.len() {
			let Some(mut row) = generated_content.clone_item(row_index) else { continue };

			let local_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
			let local_translation = DAffine2::from_translation(local_transform.translation);
			let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
			*row.attribute_mut_or_insert_default(ATTR_TRANSFORM) = local_translation * transform * local_matrix;

			result_list.push(row);
		}
		// SAFETY: rows are cloned into result_list and generated_content is owned, so no record borrow into this iteration's frames remains.
		unsafe { core_types::record::stack::rewind(mark) };
	}

	Ok(result_list)
}

#[node_macro::node(category("Repeat"), name("Repeat on Points"))]
fn repeat_on_points<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl Ctx + DeriveCtx + InjectVarArgs,
	points: List<Vector>,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<Context<'_>, Output = List<T>>,
	reverse: bool,
) -> Result<List<T>, Interrupt> {
	let spilled = ctx.index_head();
	let mut result_list = List::new();

	for points_index in 0..points.len() {
		let Some(points_element) = points.element(points_index) else { continue };
		let transform: DAffine2 = points.attribute_cloned_or_default(ATTR_TRANSFORM, points_index);

		let positions = points_element.point_domain.positions();
		let range: Box<dyn Iterator<Item = (usize, &DVec2)>> = match reverse {
			true => Box::new(positions.iter().enumerate().rev()),
			false => Box::new(positions.iter().enumerate()),
		};

		for (index, &point) in range {
			let transformed_point = transform.transform_point2(point);

			let scoped = ctx.push_position(transformed_point);
			let mark = core_types::record::stack::sp();
			let generated_content = content.eval(&scoped.ctx().promoted(&spilled, index as u64))?;

			for mut generated_row in generated_content.into_iter() {
				generated_row.attribute_mut_or_insert_default::<DAffine2>(ATTR_TRANSFORM).translation = transformed_point;
				result_list.push(generated_row);
			}
			// SAFETY: generated_content is an owned list fully moved into result_list, so no record borrow into this iteration's frames remains.
			unsafe { core_types::record::stack::rewind(mark) };
		}
	}

	Ok(result_list)
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::arena::Arena;
	use core_types::context::{ContextImpl, EvalScope, ExtractPosition};
	use core_types::gpoll::GPoll;
	use core_types::list::Item;
	use core_types::node::{Node, StatusCell};
	use core_types::record::{ElementLazyInput, RecordLift};
	use vector_types::subpath::Subpath;

	const TEST_POSITION: &str = "test-position";

	struct ValueNode<T>(T);

	impl<T: Clone, Input> Node<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	/// Returns one default `Vector` recording the innermost context position under `TEST_POSITION`.
	struct PositionProbe;

	impl<Input: ExtractPosition> Node<Input> for PositionProbe {
		type Output = List<Vector>;

		fn eval(&self, input: &Input) -> GPoll<List<Vector>> {
			let position = input.try_position().and_then(|mut positions| positions.next()).expect("repeat_on_points must push a position level");
			let mut list = List::new();
			list.push(Item::new_from_element(Vector::default()).with_attribute(TEST_POSITION, DAffine2::from_translation(position)));
			GPoll::Final(list)
		}
	}

	fn single_default_vector() -> List<Vector> {
		List::new_from_element(Vector::default())
	}

	fn row_translations(list: &List<Vector>, key: &str) -> Vec<DVec2> {
		(0..list.len()).map(|index| list.attribute_cloned_or_default::<DAffine2>(key, index).translation).collect()
	}

	macro_rules! test_ctx {
		($ctx:ident, $cell:ident) => {
			core_types::record::stack::reserve(1 << 16);
			let arena = Arena::new(4096).unwrap();
			let generations = [];
			let scope = EvalScope::new(None, None, None, &generations, &arena);
			let $ctx = ContextImpl::root(&scope);
			let $cell = StatusCell::default();
		};
	}

	#[test]
	fn repeat_radial_rotates_copies_around_the_center() {
		test_ctx!(ctx, cell);
		let (radius, count) = (5., 4);

		let lift = RecordLift::<List<Vector>, _>::new(ValueNode(single_default_vector()));
		let layout = Node::<ContextImpl>::layout(&lift).clone();
		let repeated = super::repeat_radial(&ctx, ElementLazyInput::<List<Vector>, _>::new(&lift, &cell, 0, &layout), 0., radius, count).unwrap();

		assert_eq!(repeated.len(), count as usize);
		for index in 0..count as usize {
			let transform: DAffine2 = repeated.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let expected = DAffine2::from_angle((TAU / count as f64) * index as f64) * DAffine2::from_translation(radius * DVec2::Y);
			assert!(transform.abs_diff_eq(expected, 1e-10), "copy {index}: {transform:?} != {expected:?}");
		}
	}

	#[test]
	fn repeat_on_points_pushes_each_point_as_the_position() {
		test_ctx!(ctx, cell);
		let positions = [DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let points = List::new_from_element(Vector::from_subpath(Subpath::from_anchors(positions, false)));

		let lift = RecordLift::<List<Vector>, _>::new(PositionProbe);
		let layout = Node::<ContextImpl>::layout(&lift).clone();

		let generated = super::repeat_on_points(&ctx, points.clone(), ElementLazyInput::new(&lift, &cell, 0, &layout), false).unwrap();
		assert_eq!(row_translations(&generated, ATTR_TRANSFORM), positions.to_vec());
		assert_eq!(row_translations(&generated, TEST_POSITION), positions.to_vec());

		let reversed = super::repeat_on_points(&ctx, points, ElementLazyInput::new(&lift, &cell, 0, &layout), true).unwrap();
		let mut expected = positions.to_vec();
		expected.reverse();
		assert_eq!(row_translations(&reversed, ATTR_TRANSFORM), expected);
	}
}
