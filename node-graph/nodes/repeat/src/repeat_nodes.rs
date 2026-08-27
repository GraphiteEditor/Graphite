use core::f64::consts::TAU;
use core_types::attribute::{Attr, Transform as TransformAttr};
use core_types::context::IndexLink;
use core_types::extent::{ExtentIn, LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::registry::types::{Angle, PixelSize};
use core_types::{Ctx, DeriveCtx, ExtractIndex, InjectIndex};
use glam::{DAffine2, DVec2};
use graphic_types::Vector;

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

/// Each copy evaluates the content within the copy's index pushed in, rotated
/// around the center by the copy's share of the turn.
#[node_macro::node(category("Repeat"), extent(repeat_radial_extent))]
fn repeat_radial<T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex,
	content: impl Node<Context<'_>, Output = (T, Attr<TransformAttr>)>,
	start_angle: Angle,
	#[unit(" px")]
	#[default(5)]
	radius: f64,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> Result<IList<(T, Attr<TransformAttr>)>, Interrupt> {
	let inner = content.inner_extent(ctx)?;
	let (copy, rest) = ctx.split_innermost(inner);
	if copy >= count as u64 {
		return Err(GraphError::past_end().into());
	}
	let mut frame = IndexLink { index: 0, outer: None };
	let (element, local) = content.eval(&ctx.push_level(&mut frame, copy, rest))?;

	let angle = DAffine2::from_angle((TAU / count as f64) * copy as f64 + start_angle.to_radians());
	let translation = DAffine2::from_translation(radius * DVec2::Y);
	let step = angle * translation;
	let local_translation = DAffine2::from_translation(local.translation);
	let local_matrix = DAffine2::from_mat2(local.matrix2);
	Ok((element, Attr(local_translation * step * local_matrix)))
}

/// The pushed level's extent is the copy count; inner levels forward to the
/// content, whose extent is taken uniform across copies (queried at copy 0).
fn repeat_radial_extent(content: ExtentIn<'_>, _start_angle: ValueIn<'_, Angle>, _radius: ValueIn<'_, f64>, count: ValueIn<'_, u32>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => content.at(level),
	}
}

/// The pushed level flattens every point of every points row, mirroring the
/// legacy iteration order (rows in order, a row's points reversed when
/// `reverse` is set); each copy evaluates the content with its point's
/// transformed position pushed, then lands the content row's transform on
/// that position.
#[node_macro::node(category("Repeat"), name("Repeat on Points"), extent(repeat_on_points_extent))]
fn repeat_on_points<T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	content: impl Node<Context<'_>, Output = (T, Attr<TransformAttr>)>,
	points: IList<Vector>,
	reverse: bool,
) -> Result<IList<(T, Attr<TransformAttr>)>, Interrupt> {
	let inner = content.inner_extent(ctx)?;
	let (copy, rest) = ctx.split_innermost(inner);

	let mut remaining = copy as usize;
	for row_index in 0..points.len() {
		let vector = points.element_ref(row_index);
		let positions = vector.point_domain.positions();
		if remaining >= positions.len() {
			remaining -= positions.len();
			continue;
		}
		let index = match reverse {
			true => positions.len() - 1 - remaining,
			false => remaining,
		};
		let transform: DAffine2 = points.lane(row_index).attr::<TransformAttr>();
		let transformed_point = transform.transform_point2(positions[index]);

		let scoped = ctx.push_position(transformed_point);
		let mut frame = IndexLink { index: 0, outer: None };
		let (element, local) = content.eval(&scoped.ctx().push_level(&mut frame, copy, rest))?;
		let mut composed = *local;
		composed.translation = transformed_point;
		return Ok((element, Attr(composed)));
	}
	Err(GraphError::past_end().into())
}

/// The pushed level's extent is the flattened point count across the points
/// rows; inner levels forward to the content, uniform across copies.
fn repeat_on_points_extent(content: ExtentIn<'_>, points: ListIn<'_, Vector>, _reverse: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => points
			.get()
			.map(|points| Extent::Exactly((0..points.len()).map(|row| points.element_ref(row).point_domain.positions().len()).sum())),
		false => content.at(level),
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::SourceId;
	use core_types::arena::Arena;
	use core_types::context::{ContextImpl, EvalScope};
	use core_types::node::Node;
	use core_types::record::{FieldWrite, FrameBuilder, Layout, RecordSource, RecordValue, capture, element_write, stack};
	use vector_types::subpath::Subpath;

	struct ValueNode<T>(T);

	impl<T: Clone, Input> Node<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	struct TransformSource {
		layout: Layout,
		element: f64,
		transform: DAffine2,
	}

	impl<'e> Node<ContextImpl<'e>> for TransformSource {
		type Output = RecordValue<'e>;

		fn eval(&self, input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			use core_types::context::ExtractArena;
			let mut frame = FrameBuilder::new(&self.layout, input.arena());
			frame.element(self.element);
			frame.attr::<TransformAttr>(self.transform);
			let Some(value) = frame.finish() else { return GPoll::arena_exhausted() };
			GPoll::Final(value)
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		stack::reserve(1 << 12);
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	struct VectorRows {
		layout: Layout,
		rows: Vec<(Vector, DAffine2)>,
	}

	impl<'e> Node<ContextImpl<'e>> for VectorRows {
		type Output = RecordValue<'e>;

		fn eval(&self, input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			use core_types::context::{ExtractArena, ExtractIndices};
			let (vector, transform) = &self.rows[input.innermost_index() as usize % self.rows.len()];
			let mut frame = FrameBuilder::new(&self.layout, input.arena());
			frame.element(vector.clone());
			frame.attr::<TransformAttr>(*transform);
			let Some(value) = frame.finish() else { return GPoll::arena_exhausted() };
			GPoll::Final(value)
		}

		fn extent_at(&self, _input: &ContextImpl<'e>, _level: u8) -> GPoll<Extent> {
			GPoll::Final(Extent::Exactly(self.rows.len()))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	fn vector_rows_layout() -> Layout {
		Layout::default().with_writes(1, element_write::<Vector>(), &[FieldWrite::of::<TransformAttr>(0)])
	}

	struct PositionProbe {
		layout: Layout,
	}

	impl<'e> Node<ContextImpl<'e>> for PositionProbe {
		type Output = RecordValue<'e>;

		fn eval(&self, input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			use core_types::context::{ExtractArena, ExtractPosition};
			let position = input.try_position().and_then(|mut positions| positions.next()).unwrap_or(DVec2::ZERO);
			let mut frame = FrameBuilder::new(&self.layout, input.arena());
			frame.element(position.x);
			frame.attr::<TransformAttr>(DAffine2::IDENTITY);
			let Some(value) = frame.finish() else { return GPoll::arena_exhausted() };
			GPoll::Final(value)
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	fn transform_layout() -> Layout {
		Layout::default().with_writes(0, element_write::<f64>(), &[FieldWrite::of::<TransformAttr>(0)])
	}

	#[test]
	fn repeat_array_composes_the_step_onto_each_copys_transform() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = transform_layout();
		let content = TransformSource {
			layout: layout.clone(),
			element: 7.,
			transform: DAffine2::from_translation(DVec2::new(5., 5.)),
		};

		let mut node = RepeatArrayNode::new(
			RecordSource::new(content, &layout, &layout),
			ValueNode(DVec2::new(10., 0.)),
			ValueNode(0.0f64),
			ValueNode(3u32),
			&layout,
		);
		Node::<ContextImpl>::set_layout(&mut node, repeat_array_layout_meta().resolve(&[Some(&layout)]));
		let leveled = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(leveled.depth, 1, "the IList return pushed one rank level above the content");
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::Exactly(3)));

		let head = ctx.index_head();
		for copy in 0..3u64 {
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(record) = capture(&node, &lane) else {
				panic!("expected a final record");
			};
			assert_eq!(record.element::<f64>(), 7.);
			// Zero angle, direction (10, 0), count 3: copy `j` steps j * (5, 0)
			// past the row's own (5, 5) translation.
			let composed: DAffine2 = record.attr::<TransformAttr>();
			assert_eq!(composed, DAffine2::from_translation(DVec2::new(5. + copy as f64 * 5., 5.)));
		}
	}

	#[test]
	fn repeat_radial_rotates_each_copy_around_the_center() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = transform_layout();
		let local = DAffine2::from_translation(DVec2::new(1., 0.));
		let content = TransformSource {
			layout: layout.clone(),
			element: 7.,
			transform: local,
		};

		let mut node = RepeatRadialNode::new(RecordSource::new(content, &layout, &layout), ValueNode(90.0f64), ValueNode(2.0f64), ValueNode(4u32), &layout);
		Node::<ContextImpl>::set_layout(&mut node, repeat_radial_layout_meta().resolve(&[Some(&layout)]));
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::Exactly(4)));

		let head = ctx.index_head();
		for copy in 0..4u64 {
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(record) = capture(&node, &lane) else {
				panic!("expected a final record");
			};
			assert_eq!(record.element::<f64>(), 7.);
			// The kernel's own formula, so the float operations match exactly.
			let step = DAffine2::from_angle((TAU / 4.) * copy as f64 + 90.0f64.to_radians()) * DAffine2::from_translation(2. * DVec2::Y);
			let expected = DAffine2::from_translation(local.translation) * step * DAffine2::from_mat2(local.matrix2);
			let composed: DAffine2 = record.attr::<TransformAttr>();
			assert_eq!(composed, expected);
		}
	}

	#[test]
	fn repeat_on_points_lands_each_copy_on_its_transformed_point() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let row0: Vec<DVec2> = vec![DVec2::new(40., 20.), DVec2::ONE];
		let row1: Vec<DVec2> = vec![DVec2::new(-42., 9.), DVec2::new(10., 345.), DVec2::new(3., 4.)];
		let row0_transform = DAffine2::from_translation(DVec2::new(100., 0.));
		let points = VectorRows {
			layout: vector_rows_layout(),
			rows: vec![
				(Vector::from_subpath(Subpath::from_anchors(row0.clone(), false)), row0_transform),
				(Vector::from_subpath(Subpath::from_anchors(row1.clone(), false)), DAffine2::IDENTITY),
			],
		};
		let content_layout = transform_layout();
		let content = PositionProbe { layout: content_layout.clone() };

		let mut node = RepeatOnPointsNode::new(RecordSource::new(content, &content_layout, &content_layout), points, ValueNode(false), &content_layout);
		Node::<ContextImpl>::set_layout(&mut node, repeat_on_points_layout_meta().resolve(&[Some(&content_layout)]));
		let leveled = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(leveled.depth, 1);
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::Exactly(5)), "the pushed level flattens both rows' points");

		let expected: Vec<DVec2> = row0.iter().map(|&point| row0_transform.transform_point2(point)).chain(row1.iter().copied()).collect();

		let head = ctx.index_head();
		for (flat, &point) in expected.iter().enumerate() {
			let lane = ctx.promoted(&head, flat as u64);
			let GPoll::Final(record) = capture(&node, &lane) else {
				panic!("expected a final record");
			};
			// The content saw the pushed position, and the output transform lands on it.
			assert_eq!(record.element::<f64>(), point.x);
			let composed: DAffine2 = record.attr::<TransformAttr>();
			assert_eq!(composed.translation, point);
		}
	}

	#[test]
	fn repeat_on_points_reverse_flips_each_rows_points() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let positions: Vec<DVec2> = vec![DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let points = VectorRows {
			layout: vector_rows_layout(),
			rows: vec![(Vector::from_subpath(Subpath::from_anchors(positions.clone(), false)), DAffine2::IDENTITY)],
		};
		let content_layout = transform_layout();
		let content = PositionProbe { layout: content_layout.clone() };

		let mut node = RepeatOnPointsNode::new(RecordSource::new(content, &content_layout, &content_layout), points, ValueNode(true), &content_layout);
		Node::<ContextImpl>::set_layout(&mut node, repeat_on_points_layout_meta().resolve(&[Some(&content_layout)]));

		let mut expected = positions.clone();
		expected.reverse();
		let head = ctx.index_head();
		for (flat, &point) in expected.iter().enumerate() {
			let lane = ctx.promoted(&head, flat as u64);
			let GPoll::Final(record) = capture(&node, &lane) else {
				panic!("expected a final record");
			};
			let composed: DAffine2 = record.attr::<TransformAttr>();
			assert_eq!(composed.translation, point);
		}
	}
}
