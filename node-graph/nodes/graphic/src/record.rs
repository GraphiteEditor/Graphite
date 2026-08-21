//! Pilot record nodes over the production graphic types: element-space
//! expanders whose ragged nesting lives inside `Graphic` values, ahead of the
//! flip. Wiring is by hand until the compiler pass constructs layouts.

use core_types::attribute::{Attr, Transform};
use core_types::context::{DeriveCtx, ExtractIndex, IndexLink, InjectIndex};
use core_types::extent::{ExtentIn, LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::{ATTR_TRANSFORM, Color, Ctx};
use glam::DAffine2;
use graphic_types::graphic::Graphic;
use graphic_types::Vector;
use raster_types::{CPU, Raster};
use vector_types::GradientStops;

/// Whether the walk can descend into a group: every run holds `Graphic`
/// elements.
fn group_expands(group: &core_types::record::Group) -> bool {
	match &group.content {
		core_types::record::GroupContent::Run(item) => item.typed_lanes::<Graphic>().is_some(),
		core_types::record::GroupContent::Stack(children) => children.iter().all(group_expands),
	}
}

fn group_leaf_count(group: &core_types::record::Group, fully_flatten: bool, depth: usize) -> usize {
	match &group.content {
		core_types::record::GroupContent::Run(item) => {
			let lanes = item.typed_lanes::<Graphic>().expect("guarded by group_expands");
			(0..lanes.len()).map(|lane| leaf_count(lanes.element_ref(lane), fully_flatten, depth + 1)).sum()
		}
		core_types::record::GroupContent::Stack(children) => children.iter().map(|child| group_leaf_count(child, fully_flatten, depth)).sum(),
	}
}

fn group_locate(group: &core_types::record::Group, transform: DAffine2, fully_flatten: bool, depth: usize, remaining: &mut usize) -> Option<(Graphic, DAffine2)> {
	match &group.content {
		core_types::record::GroupContent::Run(item) => {
			let lanes = item.typed_lanes::<Graphic>().expect("guarded by group_expands");
			let offset = item.layout().offset_of(ATTR_TRANSFORM, 0);
			(0..lanes.len()).find_map(|lane| {
				// SAFETY: the offset comes from the item's own layout.
				let lane_transform = offset.map(|offset| unsafe { item.lanes().get(lane).rec().read::<DAffine2>(offset) }).unwrap_or(DAffine2::IDENTITY);
				locate(lanes.element_ref(lane), transform * lane_transform, fully_flatten, depth + 1, remaining)
			})
		}
		core_types::record::GroupContent::Stack(children) => children
			.iter()
			.find_map(|child| group_locate(child, transform * graphic_types::graphic::group_row_transform(child), fully_flatten, depth, remaining)),
	}
}

/// Leaf rows a graphic expands to: its children's counts when the walk
/// descends (top rows always, deeper groups only in a full flatten), one for
/// itself otherwise.
fn leaf_count(graphic: &Graphic, fully_flatten: bool, depth: usize) -> usize {
	match graphic {
		Graphic::Graphic(children) if fully_flatten || depth == 0 => (0..children.len())
			.map(|index| children.element(index).map_or(0, |child| leaf_count(child, fully_flatten, depth + 1)))
			.sum(),
		Graphic::Group(group) if (fully_flatten || depth == 0) && group_expands(group) => group_leaf_count(group, fully_flatten, depth),
		_ => 1,
	}
}

/// The `remaining`-th leaf of `graphic` in walk order, with the transforms
/// along its path composed onto `transform`.
fn locate(graphic: &Graphic, transform: DAffine2, fully_flatten: bool, depth: usize, remaining: &mut usize) -> Option<(Graphic, DAffine2)> {
	match graphic {
		Graphic::Graphic(children) if fully_flatten || depth == 0 => (0..children.len()).find_map(|index| {
			let child = children.element(index)?;
			let child_transform: DAffine2 = children.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			locate(child, transform * child_transform, fully_flatten, depth + 1, remaining)
		}),
		Graphic::Group(group) if (fully_flatten || depth == 0) && group_expands(group) => group_locate(group, transform, fully_flatten, depth, remaining),
		_ if *remaining == 0 => Some((graphic.clone(), transform)),
		_ => {
			*remaining -= 1;
			None
		}
	}
}

/// Rank-model Flatten: one flat level holding the content's leaves, each with
/// the transforms along its path composed; a group beyond the walk's depth
/// rides as a leaf with its embedded transforms untouched.
#[node_macro::node(category("Test"), extent(flatten_extent))]
fn flatten(ctx: impl Ctx + ExtractIndex + InjectIndex + Copy, content: IList<Graphic>, fully_flatten: bool) -> Result<IList<(Graphic, Attr<Transform>)>, Interrupt> {
	let mut remaining = ctx.innermost_index() as usize;
	for row in 0..content.len() {
		let graphic = content.element_ref(row);
		let count = leaf_count(graphic, fully_flatten, 0);
		if remaining >= count {
			remaining -= count;
			continue;
		}
		let transform: DAffine2 = content.lane(row).attr::<Transform>();
		if let Some((leaf, composed)) = locate(graphic, transform, fully_flatten, 0, &mut remaining) {
			return Ok((leaf, Attr(composed)));
		}
	}
	Err(GraphError::new("flatten addressed past its leaf count").into())
}

/// The level holds one row per leaf of the walk.
fn flatten_extent(content: ListIn<'_, Graphic>, fully_flatten: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => fully_flatten
			.get()
			.zip(content.get())
			.map(|(fully_flatten, content)| Extent::Exactly((0..content.len()).map(|row| leaf_count(content.element_ref(row), fully_flatten, 0)).sum())),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Rank-model Wrap: the content level as one group element on a one-lane
/// level, the inverse of flatten's one-level descent.
#[node_macro::node(category("Test"), extent(wrap_extent))]
fn wrap(_: impl Ctx + ExtractIndex + InjectIndex + Copy, content: IList<Graphic>) -> Result<IList<Graphic>, Interrupt> {
	// SAFETY: a materialized input's frames are arena-resident.
	let item = unsafe { core_types::record::GroupItem::from_resident(content.batch()) };
	Ok(Graphic::Group(core_types::record::Group {
		row: None,
		content: core_types::record::GroupContent::Run(item),
	}))
}

/// The collected group is the level's single lane.
fn wrap_extent(_content: ListIn<'_, Graphic>, _level: LevelIn) -> GPoll<Extent> {
	GPoll::Final(Extent::Exactly(1))
}

/// One content row as the production vararg shape: a single-item legacy list
/// carrying the row's element only, so the list's dyn-hash is a complete
/// cache key over the observables.
fn vararg_row<Row: Clone + Send + Sync + 'static>(content: core_types::node::List<'_, Row>, row: usize) -> core_types::list::List<Row> {
	core_types::list::List::new_from_element(content.element_ref(row).clone())
}

/// Rank-model Map: one subgraph invocation per content row, the row riding as
/// a vararg; the subgraph's own level nests under the content level. The
/// levels report a lower bound; consumers drain to the past-end signal.
#[node_macro::node(category("Test"))]
fn map<Row: Clone + Send + Sync + core_types::CacheHash + 'static, T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)] content: IList<Row>,
	mapped: impl Node<Context<'_>, Output = IList<T>>,
) -> Result<IList<IList<T>>, Interrupt> {
	let mut remaining = ctx.innermost_index();
	for row in 0..content.len() {
		let item = vararg_row(content, row);
		let scoped = ctx.push_vararg(&item);
		let lanes = mapped.inner_extent_at(&scoped.ctx(), row as u64)?;
		if remaining >= lanes {
			remaining -= lanes;
			continue;
		}
		let mut frame = IndexLink { index: 0, outer: None };
		return mapped.eval(&scoped.ctx().push_level(&mut frame, row as u64, remaining));
	}
	Err(GraphError::past_end().into())
}

/// Rank-model flat-map (the production Map): map's walk with the subgraph's
/// lanes concatenated into one flat level. The level reports a lower bound;
/// consumers drain to the past-end signal.
#[node_macro::node(category("Test"))]
fn flat_map<Row: Clone + Send + Sync + core_types::CacheHash + 'static, T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)] content: IList<Row>,
	mapped: impl Node<Context<'_>, Output = IList<T>>,
) -> Result<IList<T>, Interrupt> {
	let mut remaining = ctx.innermost_index();
	for row in 0..content.len() {
		let item = vararg_row(content, row);
		let scoped = ctx.push_vararg(&item);
		let lanes = mapped.inner_extent_at(&scoped.ctx(), row as u64)?;
		if remaining >= lanes {
			remaining -= lanes;
			continue;
		}
		let mut frame = IndexLink { index: 0, outer: None };
		return mapped.eval(&scoped.ctx().push_level(&mut frame, row as u64, remaining));
	}
	Err(GraphError::past_end().into())
}

/// Rank-model level collapse: two nested levels become one flat level. The
/// flat index already spans the edge's depth, so the eval forwards it.
#[node_macro::node(category("Test"), extent(flatten_levels_extent))]
fn flatten_levels<T>(ctx: impl Ctx + DeriveCtx + ExtractIndex, content: impl Node<Context<'_>, Output = IList<IList<T>>>) -> Result<IList<T>, Interrupt> {
	let head = ctx.index_head();
	content.eval(&ctx.promoted(&head, ctx.innermost_index()))
}

/// The collapsed level's extent is the sum of the inner extents across the
/// outer copies; the product composite cannot express a ragged total. A
/// lower-bound level keeps the sum a lower bound.
fn flatten_levels_extent(content: ExtentIn<'_>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => {
			let outer = match content.at_copy(0, LevelIn { level: 1, depth: 2 }) {
				GPoll::Final(Extent::Exactly(outer)) => outer,
				GPoll::Final(Extent::AtLeast(bound)) => return GPoll::Final(Extent::AtLeast(bound)),
				GPoll::Final(Extent::Free) => return GPoll::error("flatten over an unbounded outer level"),
				other => return other,
			};
			let mut total = 0;
			for copy in 0..outer {
				match content.at_copy(copy as u64, LevelIn { level: 0, depth: 2 }) {
					GPoll::Final(Extent::Exactly(count)) => total += count,
					GPoll::Final(Extent::AtLeast(count)) => return GPoll::Final(Extent::AtLeast(total + count)),
					GPoll::Final(Extent::Free) => return GPoll::error("flatten over an unbounded inner level"),
					other => return other,
				}
			}
			GPoll::Final(Extent::Exactly(total))
		}
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::arena::Arena;
	use core_types::attribute::Attribute as AttributeMarker;
	use core_types::context::{ContextImpl, EvalScope, ExtractArena};
	use core_types::list::{Item, List};
	use core_types::node::Node;
	use core_types::record::{self, Layout, Rec, RecordSource, RecordValue, stack};

	struct ValueNode<T>(T);

	impl<T: Clone, Input> Node<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	struct GraphicSource {
		layout: Layout,
		rows: Vec<(Graphic, DAffine2)>,
	}

	impl<'e> Node<ContextImpl<'e>> for GraphicSource {
		type Output = RecordValue<'e>;

		fn eval(&self, input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			let (graphic, transform) = &self.rows[input.innermost_index() as usize % self.rows.len()];
			let dst = stack::push(self.layout.frame_bytes());
			if unsafe { record::write_element(dst, graphic.clone(), input.arena()) }.is_none() {
				return GPoll::arena_exhausted();
			}
			unsafe {
				dst.add(self.layout.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap()).cast::<DAffine2>().write(*transform);
			}
			stack::pop(dst);
			GPoll::Final(RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) }))
		}

		fn extent_at(&self, _input: &ContextImpl<'e>, _level: u8) -> GPoll<Extent> {
			GPoll::Final(Extent::Exactly(self.rows.len()))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		stack::reserve(1 << 16);
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn install<N: Node<ContextImpl<'static>>>(mut node: N, meta: record::LayoutMeta, inputs: &[Option<&Layout>]) -> N {
		<N as Node<ContextImpl<'static>>>::set_layout(&mut node, meta.resolve(inputs));
		node
	}

	fn graphic_layout() -> Layout {
		Layout::default().with_writes(1, record::element_write_hashed::<Graphic>(), &[record::FieldWrite::of::<Transform>(0)])
	}

	fn text(label: &str) -> Graphic {
		Graphic::Text(List::new_from_element(label.to_string()))
	}

	fn group(children: Vec<(Graphic, DAffine2)>) -> Graphic {
		let mut list = List::new();
		for (index, (child, transform)) in children.into_iter().enumerate() {
			list.push(Item::new_from_element(child));
			list.set_attribute(ATTR_TRANSFORM, index, transform);
		}
		Graphic::Graphic(list)
	}

	fn text_of(graphic: &Graphic) -> &str {
		let Graphic::Text(list) = graphic else {
			panic!("expected a text leaf, got {graphic:?}");
		};
		list.element(0).expect("a text leaf holds its string")
	}

	fn translation(x: f64) -> DAffine2 {
		DAffine2::from_translation(glam::DVec2::new(x, 0.))
	}

	/// [a, G[b, H[c]]] with translations picked so each composed path is a
	/// distinct sum.
	fn fixture_rows() -> Vec<(Graphic, DAffine2)> {
		vec![
			(text("a"), translation(1.)),
			(group(vec![(text("b"), translation(20.)), (group(vec![(text("c"), translation(300.))]), translation(4000.))]), translation(0.5)),
		]
	}

	macro_rules! build {
		($layout:ident, $rows:expr, $fully:expr) => {
			install(
				FlattenNode::new(RecordSource::new(GraphicSource { layout: $layout.clone(), rows: $rows }, &$layout, &$layout), ValueNode($fully)),
				flatten_layout_meta(),
				&[Some(&$layout)],
			)
		};
	}

	/// A subgraph source deriving its rows from the vararg: a `Text` row of
	/// string `s` expands to `s.len()` lanes labeled `s{k}`, each translated
	/// by `k`. The vararg is attr-less, so the content rows' transforms must
	/// not reach these lanes.
	struct PerRowSource {
		layout: Layout,
	}

	fn vararg_text(input: &ContextImpl<'_>) -> Option<String> {
		let arg = core_types::ExtractVarArgs::vararg(input, 0).ok()?;
		let list = arg.downcast_ref::<core_types::list::List<Graphic>>()?;
		let Graphic::Text(text) = list.element(0)? else { return None };
		Some(text.element(0)?.clone())
	}

	impl<'e> Node<ContextImpl<'e>> for PerRowSource {
		type Output = RecordValue<'e>;

		fn eval(&self, input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			let Some(label) = vararg_text(input) else {
				return GPoll::error("the subgraph fixture expects a text vararg");
			};
			let lane = input.innermost_index();
			let graphic = text(&format!("{label}{lane}"));
			let translated = DAffine2::from_translation(glam::DVec2::new(lane as f64, 0.));
			let dst = stack::push(self.layout.frame_bytes());
			if unsafe { record::write_element(dst, graphic, input.arena()) }.is_none() {
				return GPoll::arena_exhausted();
			}
			unsafe {
				dst.add(self.layout.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap()).cast::<DAffine2>().write(translated);
			}
			stack::pop(dst);
			GPoll::Final(RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) }))
		}

		fn extent_at(&self, input: &ContextImpl<'e>, _level: u8) -> GPoll<Extent> {
			match vararg_text(input) {
				Some(label) => GPoll::Final(Extent::Exactly(label.len())),
				None => GPoll::error("the subgraph fixture expects a text vararg"),
			}
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	fn ragged_rows() -> Vec<(Graphic, DAffine2)> {
		vec![(text("ab"), translation(10.)), (text("xyz"), translation(20.))]
	}

	fn routing_meta(source: u8, level_delta: i8) -> record::LayoutMeta {
		record::LayoutMeta {
			sources: vec![source],
			reads: vec![],
			element: record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta,
			folded: None,
		}
	}

	const RAGGED_FLAT: [(&str, f64); 5] = [("ab0", 0.), ("ab1", 1.), ("xyz0", 0.), ("xyz1", 1.), ("xyz2", 2.)];

	#[test]
	fn map_scans_ragged_rows() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let node = install(
			MapNode::<_, _, Graphic>::new(
				RecordSource::new(GraphicSource { layout: layout.clone(), rows: ragged_rows() }, &layout, &layout),
				PerRowSource { layout: layout.clone() },
				&layout,
			),
			routing_meta(1, 1),
			&[Some(&layout), Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 2);
		// The extent-fn-less levels report a lower bound; addressing below
		// proves the lanes are all reachable regardless.
		assert_eq!(node.extent_at(&ctx, 1), GPoll::Final(Extent::AtLeast(0)));
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::AtLeast(0)));

		let head = ctx.index_head();
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		for (lane, &(label, x)) in RAGGED_FLAT.iter().enumerate() {
			let mark = stack::sp();
			let GPoll::Final(value) = node.eval(&ctx.promoted(&head, lane as u64)) else {
				panic!("expected a final record");
			};
			let rec = out.rec(&value);
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(rec) }), label, "lane {lane}");
			let transform: DAffine2 = unsafe { rec.read(offset) };
			assert_eq!(transform.translation.x, x, "lane {lane}");
			unsafe { stack::rewind(mark) };
		}
	}

	#[test]
	fn flat_map_matches_flatten_of_map() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let flat = install(
			FlatMapNode::<_, _, Graphic>::new(
				RecordSource::new(GraphicSource { layout: layout.clone(), rows: ragged_rows() }, &layout, &layout),
				PerRowSource { layout: layout.clone() },
				&layout,
			),
			routing_meta(1, 0),
			&[Some(&layout), Some(&layout)],
		);
		let mapped = install(
			MapNode::<_, _, Graphic>::new(
				RecordSource::new(GraphicSource { layout: layout.clone(), rows: ragged_rows() }, &layout, &layout),
				PerRowSource { layout: layout.clone() },
				&layout,
			),
			routing_meta(1, 1),
			&[Some(&layout), Some(&layout)],
		);
		let map_out = Node::<ContextImpl>::layout(&mapped).clone();
		let composed = install(FlattenLevelsNode::new(mapped, &map_out), routing_meta(0, -1), &[Some(&map_out)]);

		let flat_out = Node::<ContextImpl>::layout(&flat).clone();
		let composed_out = Node::<ContextImpl>::layout(&composed).clone();
		assert_eq!(flat_out.depth, 1);
		assert_eq!(composed_out.depth, 1);
		// Both spellings report the same lower bound; the lane loop below is
		// the law.
		assert_eq!(flat.extent_at(&ctx, 0), GPoll::Final(Extent::AtLeast(0)));
		assert_eq!(composed.extent_at(&ctx, 0), GPoll::Final(Extent::AtLeast(0)));

		let head = ctx.index_head();
		let offset = flat_out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		for (lane, &(label, x)) in RAGGED_FLAT.iter().enumerate() {
			let mark = stack::sp();
			let scoped = ctx.promoted(&head, lane as u64);
			let GPoll::Final(direct) = flat.eval(&scoped) else {
				panic!("expected a final record from flat_map");
			};
			let direct_label = text_of(unsafe { record::borrow_element::<Graphic>(flat_out.rec(&direct)) }).to_string();
			let direct_x: DAffine2 = unsafe { flat_out.rec(&direct).read(offset) };
			let GPoll::Final(value) = composed.eval(&scoped) else {
				panic!("expected a final record from flatten(map)");
			};
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(composed_out.rec(&value)) }), direct_label, "lane {lane}");
			let composed_x: DAffine2 = unsafe { composed_out.rec(&value).read(offset) };
			assert_eq!(composed_x, direct_x, "lane {lane}");
			assert_eq!((direct_label.as_str(), direct_x.translation.x), (label, x), "lane {lane}");
			unsafe { stack::rewind(mark) };
		}
	}

	#[test]
	fn flat_map_registers_one_row_per_content_type() {
		let entries = _flat_map_mod::flat_map_entries();
		assert_eq!(entries.len(), 6, "one registry row per content implementation");
		let content_types: Vec<core_types::Type> = entries.iter().map(|entry| entry.io.inputs[0].clone()).collect();
		assert_eq!(content_types[0], core_types::registry::record_edge_type::<Graphic>());
		assert_eq!(content_types[1], core_types::registry::record_edge_type::<Vector>());
		assert_eq!(content_types[5], core_types::registry::record_edge_type::<String>());
		// The subject and the output stay erased across rows.
		assert_eq!(entries[0].io.inputs[1], entries[5].io.inputs[1]);
		assert_eq!(entries[0].io.return_value, entries[5].io.return_value);
	}

	#[test]
	fn flat_map_batch_matches_per_lane_eval() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let node = install(
			FlatMapNode::<_, _, Graphic>::new(
				RecordSource::new(GraphicSource { layout: layout.clone(), rows: ragged_rows() }, &layout, &layout),
				PerRowSource { layout: layout.clone() },
				&layout,
			),
			routing_meta(1, 0),
			&[Some(&layout), Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		let head = ctx.index_head();
		let scoped = ctx.promoted(&head, 0);

		let mut scratch = vec![std::mem::MaybeUninit::<u64>::uninit(); 5 * out.lane_stride() / 8];
		let core_types::node::BatchStatus::Filled(batch, ..) = node.eval_batch(&scoped, 0..5, Some(&mut scratch)) else {
			panic!("expected a filled batch");
		};
		let batch = batch.into_shared();
		assert_eq!(batch.len(), 5);
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		for lane in 0..5 {
			let mark = stack::sp();
			let GPoll::Final(value) = node.eval(&ctx.promoted(&head, lane as u64)) else {
				panic!("expected a final record");
			};
			let rec = out.rec(&value);
			let single = text_of(unsafe { record::borrow_element::<Graphic>(rec) }).to_string();
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(batch.get(lane).rec()) }), single, "lane {lane}");
			let batched: DAffine2 = unsafe { batch.get(lane).rec().read(offset) };
			let direct: DAffine2 = unsafe { rec.read(offset) };
			assert_eq!(batched, direct, "lane {lane}");
			unsafe { stack::rewind(mark) };
		}
	}

	#[test]
	fn flatten_expands_one_level() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let node = build!(layout, fixture_rows(), false);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 1);
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::Exactly(3)));

		let head = ctx.index_head();
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		// Lane 2 is the unexpanded subgroup H, riding as a leaf at G's depth.
		let expected: [(&str, f64); 2] = [("a", 1.), ("b", 20.5)];
		for (lane, &(label, x)) in expected.iter().enumerate() {
			let mark = stack::sp();
			let GPoll::Final(value) = node.eval(&ctx.promoted(&head, lane as u64)) else {
				panic!("expected a final record");
			};
			let rec = out.rec(&value);
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(rec) }), label, "lane {lane}");
			let transform: DAffine2 = unsafe { rec.read(offset) };
			assert_eq!(transform.translation.x, x, "lane {lane}");
			unsafe { stack::rewind(mark) };
		}

		let mark = stack::sp();
		let GPoll::Final(value) = node.eval(&ctx.promoted(&head, 2)) else {
			panic!("expected a final record");
		};
		let rec = out.rec(&value);
		let Graphic::Graphic(children) = (unsafe { record::borrow_element::<Graphic>(rec) }) else {
			panic!("lane 2 keeps the subgroup element");
		};
		assert_eq!(children.len(), 1);
		assert_eq!(text_of(children.element(0).unwrap()), "c");
		assert_eq!(children.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, 0).translation.x, 300., "embedded transforms ride untouched");
		let transform: DAffine2 = unsafe { rec.read(offset) };
		assert_eq!(transform.translation.x, 4000.5);
		unsafe { stack::rewind(mark) };
	}

	#[test]
	fn wrap_collects_the_level_into_a_group() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let rows = vec![(text("a"), translation(1.)), (text("b"), translation(2.))];
		let node = install(
			WrapNode::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows, }, &layout, &layout), &layout),
			wrap_layout_meta(),
			&[Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 1);
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::Exactly(1)), "the group is the level's single lane");

		let head = ctx.index_head();
		let GPoll::Final(value) = node.eval(&ctx.promoted(&head, 0)) else {
			panic!("expected a final record");
		};
		let Graphic::Group(group) = (unsafe { record::borrow_element::<Graphic>(out.rec(&value)) }) else {
			panic!("expected a group element");
		};
		assert!(group.row.is_none());
		let record::GroupContent::Run(item) = &group.content else {
			panic!("expected a single run");
		};
		assert_eq!(item.len(), 2);
		let lanes = item.typed_lanes::<Graphic>().expect("the run holds the adopted graphic lanes");
		let offset = item.layout().offset_of(ATTR_TRANSFORM, 0).unwrap();
		for (lane, (label, x)) in [("a", 1.), ("b", 2.)].into_iter().enumerate() {
			assert_eq!(text_of(lanes.element_ref(lane)), label, "lane {lane}");
			let transform: DAffine2 = unsafe { item.lanes().get(lane).rec().read(offset) };
			assert_eq!(transform.translation.x, x, "lane {lane}");
		}
	}

	#[test]
	fn a_group_converts_to_its_legacy_list() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let rows = vec![(text("a"), translation(1.)), (text("b"), translation(2.))];
		let node = install(
			WrapNode::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows, }, &layout, &layout), &layout),
			wrap_layout_meta(),
			&[Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		let head = ctx.index_head();
		let GPoll::Final(value) = node.eval(&ctx.promoted(&head, 0)) else {
			panic!("expected a final record");
		};
		let Graphic::Group(group) = (unsafe { record::borrow_element::<Graphic>(out.rec(&value)) }) else {
			panic!("expected a group element");
		};

		let legacy = graphic_types::graphic::group_to_legacy_list(group);
		assert_eq!(legacy.len(), 2);
		for (index, (label, x)) in [("a", 1.), ("b", 2.)].into_iter().enumerate() {
			assert_eq!(text_of(legacy.element(index).unwrap()), label, "item {index}");
			assert_eq!(legacy.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index).translation.x, x, "item {index}");
		}
	}

	#[test]
	fn flatten_reverses_wrap() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let rows = vec![(text("a"), translation(1.)), (text("b"), translation(2.))];
		let wrapped = install(
			WrapNode::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows, }, &layout, &layout), &layout),
			wrap_layout_meta(),
			&[Some(&layout)],
		);
		let wrap_out = Node::<ContextImpl>::layout(&wrapped).clone();
		let head = ctx.index_head();
		let group = {
			let mark = stack::sp();
			let GPoll::Final(value) = wrapped.eval(&ctx.promoted(&head, 0)) else {
				panic!("expected a final record");
			};
			let group = unsafe { record::borrow_element::<Graphic>(wrap_out.rec(&value)) }.clone();
			// SAFETY: the element was cloned out above, so no borrow into the frame remains.
			unsafe { stack::rewind(mark) };
			group
		};

		// One row holding the wrapped group flattens back to the lanes, the
		// group's identity transform composed onto each child's.
		let node = build!(layout, vec![(group, DAffine2::IDENTITY)], false);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::Exactly(2)));

		let head = ctx.index_head();
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		for (lane, &(label, x)) in [("a", 1.), ("b", 2.)].iter().enumerate() {
			let mark = stack::sp();
			let GPoll::Final(value) = node.eval(&ctx.promoted(&head, lane as u64)) else {
				panic!("expected a final record");
			};
			let rec = out.rec(&value);
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(rec) }), label, "lane {lane}");
			let transform: DAffine2 = unsafe { rec.read(offset) };
			assert_eq!(transform.translation.x, x, "lane {lane}");
			unsafe { stack::rewind(mark) };
		}
	}

	#[test]
	fn flatten_fully_composes_the_path() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let mut rows = fixture_rows();
		rows.push((group(vec![]), translation(9.)));
		let layout = graphic_layout();
		let node = build!(layout, rows, true);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(node.extent_at(&ctx, 0), GPoll::Final(Extent::Exactly(3)), "the empty group contributes no leaves");

		let head = ctx.index_head();
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		let expected: [(&str, f64); 3] = [("a", 1.), ("b", 20.5), ("c", 4300.5)];
		for (lane, &(label, x)) in expected.iter().enumerate() {
			let mark = stack::sp();
			let GPoll::Final(value) = node.eval(&ctx.promoted(&head, lane as u64)) else {
				panic!("expected a final record");
			};
			let rec = out.rec(&value);
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(rec) }), label, "lane {lane}");
			let transform: DAffine2 = unsafe { rec.read(offset) };
			assert_eq!(transform.translation.x, x, "lane {lane}");
			unsafe { stack::rewind(mark) };
		}
	}

	#[test]
	fn flatten_batch_matches_per_lane_eval() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let node = build!(layout, fixture_rows(), true);
		let out = Node::<ContextImpl>::layout(&node).clone();
		let head = ctx.index_head();
		let scoped = ctx.promoted(&head, 0);

		let mut scratch = vec![std::mem::MaybeUninit::<u64>::uninit(); 3 * out.lane_stride() / 8];
		let core_types::node::BatchStatus::Filled(batch, ..) = node.eval_batch(&scoped, 0..3, Some(&mut scratch)) else {
			panic!("expected a filled batch");
		};
		let batch = batch.into_shared();
		assert_eq!(batch.len(), 3);
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		for lane in 0..3 {
			let mark = stack::sp();
			let GPoll::Final(value) = node.eval(&ctx.promoted(&head, lane as u64)) else {
				panic!("expected a final record");
			};
			let rec = out.rec(&value);
			let single = text_of(unsafe { record::borrow_element::<Graphic>(rec) }).to_string();
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(batch.get(lane).rec()) }), single, "lane {lane}");
			let batched: DAffine2 = unsafe { batch.get(lane).rec().read(offset) };
			let direct: DAffine2 = unsafe { rec.read(offset) };
			assert_eq!(batched, direct, "lane {lane}");
			unsafe { stack::rewind(mark) };
		}
	}
}
