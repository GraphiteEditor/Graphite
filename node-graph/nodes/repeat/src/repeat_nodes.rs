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
#[node_macro::node(category("Repeat"), extent(repeat_extent), batch(repeat_batch))]
pub fn repeat<T>(
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
	use core_types::context::{ContextImpl, EvalScope, ExtractArena};
	use core_types::node::Node;
	use core_types::record::{FieldWrite, FrameClaim, Layout, RecordSource, Served, capture, element_write};
	use core_types::value::ValueSource;

	/// An open polyline through the anchors, standing in for the deleted `Subpath::from_anchors`.
	fn polyline(anchors: &[DVec2]) -> Vector {
		let mut bezpath = vector_types::kurbo::BezPath::new();
		let Some((&first, rest)) = anchors.split_first() else { return Vector::default() };
		bezpath.move_to(vector_types::vector::misc::dvec2_to_point(first));
		for &anchor in rest {
			bezpath.line_to(vector_types::vector::misc::dvec2_to_point(anchor));
		}
		Vector::from_bezpath(bezpath)
	}

	struct TransformSource {
		layout: Layout,
		element: f64,
		transform: DAffine2,
	}

	impl<C: ExtractIndex> Node<C> for TransformSource {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(self.element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<TransformAttr>(&mut frame, &self.layout, self.transform);
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// Writes a field at the layout's resolved offset, the wiring-proven pairing
	/// a generated node performs.
	fn write_field_at<T: Copy + 'static>(frame: &mut FrameClaim<'_, '_>, layout: &Layout, name: &str, level: u8, value: T) {
		let field = layout
			.fields
			.iter()
			.find(|field| field.name == name && field.level == level)
			.expect("the layout carries the written field");
		assert_eq!(field.type_id, std::any::TypeId::of::<T>(), "the field was declared at this value type");
		// SAFETY: the offset is this layout's own, at the field's declared type.
		unsafe { frame.attr_at(field.offset, value) };
	}

	/// [`write_field_at`] for a census marker at level 0.
	fn write_attr_at<A: core_types::attribute::Attribute>(frame: &mut FrameClaim<'_, '_>, layout: &Layout, value: A::Value<'static>)
	where
		A::Value<'static>: Copy + 'static,
	{
		write_field_at(frame, layout, A::NAME, 0, value);
	}
	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	struct VectorRows {
		layout: Layout,
		rows: Vec<(Vector, DAffine2)>,
	}

	impl<C: ExtractIndex> Node<C> for VectorRows {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let (vector, transform) = &self.rows[input.innermost_index() as usize % self.rows.len()];
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(vector.clone(), arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<TransformAttr>(&mut frame, &self.layout, *transform);
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &core_types::record::Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
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

	impl<C: ExtractIndex + core_types::context::ExtractPosition> Node<C> for PositionProbe {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let position = input.try_position().and_then(|mut positions| positions.next()).unwrap_or(DVec2::ZERO);
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(position.x, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<TransformAttr>(&mut frame, &self.layout, DAffine2::IDENTITY);
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
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
		let frames = core_types::record::test_frames(1 << 16);
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
			ValueSource::new(DVec2::new(10., 0.)),
			ValueSource::new(0.0f64),
			ValueSource::new(3u32),
			&layout,
		);
		Node::<ContextImpl>::set_layout(&mut node, repeat_array_layout_meta().resolve(&[Some(&layout)]));
		let leveled = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(leveled.depth, 1, "the IList return pushed one rank level above the content");
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(3)));

		let head = ctx.index_head();
		for copy in 0..3u64 {
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(record) = capture(&node, &lane, &frames) else {
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
		let frames = core_types::record::test_frames(1 << 16);
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

		let mut node = RepeatRadialNode::new(
			RecordSource::new(content, &layout, &layout),
			ValueSource::new(90.0f64),
			ValueSource::new(2.0f64),
			ValueSource::new(4u32),
			&layout,
		);
		Node::<ContextImpl>::set_layout(&mut node, repeat_radial_layout_meta().resolve(&[Some(&layout)]));
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(4)));

		let head = ctx.index_head();
		for copy in 0..4u64 {
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(record) = capture(&node, &lane, &frames) else {
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
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let row0: Vec<DVec2> = vec![DVec2::new(40., 20.), DVec2::ONE];
		let row1: Vec<DVec2> = vec![DVec2::new(-42., 9.), DVec2::new(10., 345.), DVec2::new(3., 4.)];
		let row0_transform = DAffine2::from_translation(DVec2::new(100., 0.));
		let points = VectorRows {
			layout: vector_rows_layout(),
			rows: vec![(polyline(&row0), row0_transform), (polyline(&row1), DAffine2::IDENTITY)],
		};
		let content_layout = transform_layout();
		let content = PositionProbe { layout: content_layout.clone() };

		let mut node = RepeatOnPointsNode::new(RecordSource::new(content, &content_layout, &content_layout), points, ValueSource::new(false), &content_layout);
		Node::<ContextImpl>::set_layout(&mut node, repeat_on_points_layout_meta().resolve(&[Some(&content_layout)]));
		let leveled = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(leveled.depth, 1);
		assert_eq!(
			node.extent_at(&ctx, 0, &frames.reborrow()),
			GPoll::Final(Extent::Exactly(5)),
			"the pushed level flattens both rows' points"
		);

		let expected: Vec<DVec2> = row0.iter().map(|&point| row0_transform.transform_point2(point)).chain(row1.iter().copied()).collect();

		let head = ctx.index_head();
		for (flat, &point) in expected.iter().enumerate() {
			let lane = ctx.promoted(&head, flat as u64);
			let GPoll::Final(record) = capture(&node, &lane, &frames) else {
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
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let positions: Vec<DVec2> = vec![DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let points = VectorRows {
			layout: vector_rows_layout(),
			rows: vec![(polyline(&positions), DAffine2::IDENTITY)],
		};
		let content_layout = transform_layout();
		let content = PositionProbe { layout: content_layout.clone() };

		let mut node = RepeatOnPointsNode::new(RecordSource::new(content, &content_layout, &content_layout), points, ValueSource::new(true), &content_layout);
		Node::<ContextImpl>::set_layout(&mut node, repeat_on_points_layout_meta().resolve(&[Some(&content_layout)]));

		let mut expected = positions.clone();
		expected.reverse();
		let head = ctx.index_head();
		for (flat, &point) in expected.iter().enumerate() {
			let lane = ctx.promoted(&head, flat as u64);
			let GPoll::Final(record) = capture(&node, &lane, &frames) else {
				panic!("expected a final record");
			};
			let composed: DAffine2 = record.attr::<TransformAttr>();
			assert_eq!(composed.translation, point);
		}
	}
}

/// The batch over the repeated level: the copies refine the map, one level
/// of `count` copies over the content's inner lanes, and the content answers
/// the whole flat range in one call.
fn repeat_batch<'batch, 'serve, 'r, Input, Content, Count, Reverse>(
	node: &'batch _repeat_mod::RepeatNode<Content, Count, Reverse>,
	dispatch: core_types::dispatch::Dispatch<'serve>,
	scratch: Option<&'r mut [std::mem::MaybeUninit<u64>]>,
	frames: &core_types::record::Frames<'serve>,
) -> core_types::node::BatchStatus<'r>
where
	'batch: 'r,
	'serve: 'r,
	Input: Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy + core_types::dispatch::AsDispatch<'serve> + core_types::context::ExtractArena<ArenaRef = &'serve core_types::arena::Arena>,
	Content: for<'derived> core_types::record::DerivedRecordInput<'derived, core_types::context::Derived<'derived, Input>>,
	Count: core_types::node::Node<Input>,
	Reverse: core_types::node::Node<Input>,
	_repeat_mod::RepeatNode<Content, Count, Reverse>: core_types::node::Node<Input>,
{
	use core_types::node::BatchStatus;

	let exhausted = || {
		BatchStatus::Error(GraphError {
			kind: core_types::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	let range = dispatch.range();
	let Some(base) = Input::at_lane(&dispatch, range.start) else {
		return exhausted();
	};
	let cell = core_types::node::StatusCell::new();
	let count = match cell.eval_input(1, &node.count, &base, frames) {
		// SAFETY: input 1 is the count, read at the layout resolved for it.
		Ok(value) => (unsafe { core_types::record::read_element::<u32>(node.__in_1.rec(&value)) }) as u64,
		Err(interrupt) => return interrupt.into(),
	};
	let reverse = match cell.eval_input(2, &node.reverse, &base, frames) {
		// SAFETY: input 2 is the reverse flag.
		Ok(value) => unsafe { core_types::record::read_element::<bool>(node.__in_2.rec(&value)) },
		Err(interrupt) => return interrupt.into(),
	};
	let layout = core_types::node::Node::<Input>::layout(node);
	let inner_levels = layout.depth.saturating_sub(1);
	let inner = match core_types::record::inner_extent_of(&node.content, &base, 0, inner_levels, 0, frames) {
		Ok(inner) => inner,
		Err(interrupt) => return interrupt.into(),
	};
	// Copies are rectangular unless the content reads the copy index; a copy
	// whose inner extent differs leaves the lanes to serve their own copies.
	let reads_copy = node.__lane_invariant & 1 == 0 && !(node.__single_lane & 1 != 0 && inner == 1) && core_types::record::ragged_checks_enabled();
	for copy in 1..count.min(if reads_copy { u64::MAX } else { 1 }) {
		match core_types::record::inner_extent_of(&node.content, &base, copy, inner_levels, 0, frames) {
			Ok(extent) if extent == inner => {}
			Ok(_) => {
				#[cfg(debug_assertions)]
				core_types::record::note_kernel_batch("repeat", "ragged", 0);
				return BatchStatus::Unbatched;
			}
			Err(interrupt) => return interrupt.into(),
		}
	}
	// The level ends at the last copy of the last enclosing lane: a range
	// reaching past it comes back short with the exact flat total, so a
	// lower-bound consumer stops guessing. Open enclosing levels leave it unbounded.
	let total = count * inner;
	let map = dispatch.map();
	let enclosing = (1..map.depth())
		.map(|level| map.extent(level))
		.try_fold(1u64, |product, extent| (extent != u64::MAX).then(|| product.saturating_mul(extent)));
	let flat_total = enclosing.map(|enclosing| total.saturating_mul(enclosing));
	let end = range.end.min(flat_total.unwrap_or(u64::MAX));
	if flat_total.is_some_and(|flat_total| range.start >= flat_total) {
		let Some(scratch) = scratch else {
			return BatchStatus::NeedBuffer;
		};
		return BatchStatus::Filled(
			core_types::node::RecordBatchMut::filled(scratch, 0, layout),
			core_types::gpoll::Finality::AllFinal,
			Extent::Exactly(flat_total.unwrap_or_default() as usize),
		);
	}
	let Some(refined) = dispatch.sized(total).over(range.start..end).refined(inner.max(1), reverse) else {
		return BatchStatus::Error(GraphError::new("repeat could not refine its lane map"));
	};
	let range = range.start..end;
	#[cfg(debug_assertions)]
	core_types::record::note_kernel_batch("repeat", "refined forward", range.end.saturating_sub(range.start) as usize);
	let lanes = |scratch: &'r mut [std::mem::MaybeUninit<u64>]| {
		use core_types::gpoll::Finality;
		let Ok(len) = usize::try_from(range.end.saturating_sub(range.start)) else {
			return BatchStatus::InvalidRange;
		};
		let Some(mut run) = frames.run(scratch, len, layout) else {
			return BatchStatus::InvalidRange;
		};
		let mut finality = Finality::AllFinal;
		let mut hint = Extent::AtLeast(range.end as usize);
		for lane in 0..len {
			let Some(ctx) = <core_types::context::Derived<'_, Input> as core_types::dispatch::AsDispatch<'_>>::at_lane(&refined, range.start + lane as u64) else {
				return exhausted();
			};
			let lane_frames = frames.scope();
			let slot = run.slot(lane, &lane_frames);
			let served = match node.content.serve_derived(&ctx, slot) {
				GPoll::Final(served) => served,
				GPoll::Partial(served) => {
					finality = Finality::Partial;
					served
				}
				GPoll::Pending => return BatchStatus::Pending,
				GPoll::Fallback(boxed) => return BatchStatus::Error(boxed.1),
				GPoll::Error(error) if error.kind == core_types::gpoll::ErrorKind::PastEnd => {
					#[cfg(debug_assertions)]
					core_types::gpoll::trace_past_end("repeat", lane, range.start);
					hint = Extent::Exactly(range.start as usize + lane);
					break;
				}
				GPoll::Error(error) => return BatchStatus::Error(*error),
			};
			run.served(lane, &served);
		}
		BatchStatus::Filled(run.finish(), finality, hint)
	};
	match core_types::record::forward_dispatch(&node.content, &refined, scratch, frames, layout, lanes) {
		BatchStatus::Filled(batch, finality, hint) if flat_total == Some(end) => {
			let _ = hint;
			BatchStatus::Filled(batch, finality, Extent::Exactly(end as usize))
		}
		status => status,
	}
}
