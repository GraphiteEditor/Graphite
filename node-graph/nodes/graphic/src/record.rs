//! The element-space walks the production flatten nodes share, plus the
//! level-nesting pilots (`nested_map`, `flatten_levels`) that have no
//! production counterpart yet. The tests here drive the production nodes in
//! `graphic.rs` with hand-wired layouts.

use core_types::attribute::{Opacity, OpacityFill, Transform};
use core_types::context::{DeriveCtx, ExtractIndex, IndexLink, InjectIndex};
use core_types::extent::{ExtentIn, LevelIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::lane::LaneSource;
use core_types::node::RecordLane;
use core_types::record::RunView;
use core_types::{ATTR_TRANSFORM, Color, Ctx};
use glam::DAffine2;
use graphic_types::Vector;
use graphic_types::graphic::{Graphic, RowStep, TryFromGraphic};
use raster_types::{CPU, Raster};
use vector_types::Gradient;

/// Whether the walk can descend into a group: the run holds `Graphic`
/// elements.
pub(crate) fn group_expands(group: &core_types::record::Group) -> bool {
	group.content.typed_lanes::<Graphic>().is_some()
}

pub(crate) fn group_leaf_count(group: &core_types::record::Group, fully_flatten: bool, depth: usize) -> usize {
	let lanes = group.content.typed_lanes::<Graphic>().expect("guarded by group_expands");
	(0..lanes.len()).map(|lane| leaf_count(lanes.element_ref(lane), fully_flatten, depth + 1)).sum()
}

pub(crate) fn group_locate<'e>(group: &core_types::record::Group<'e>, transform: DAffine2, fully_flatten: bool, depth: usize, remaining: &mut usize) -> Option<(Graphic<'e>, DAffine2)> {
	let item = &group.content;
	let lanes = item.typed_lanes::<Graphic>().expect("guarded by group_expands");
	let field = core_types::record::FieldOffset::<Transform>::of(item.layout(), 0);
	(0..lanes.len()).find_map(|lane| {
		let lane_transform = item.lanes().get(lane).attr_at(field);
		locate(lanes.element_ref(lane), transform * lane_transform, fully_flatten, depth + 1, remaining)
	})
}

/// Leaf rows a graphic expands to: its children's counts when the walk
/// descends (top rows always, deeper groups only in a full flatten), one for
/// itself otherwise.
pub(crate) fn leaf_count(graphic: &Graphic, fully_flatten: bool, depth: usize) -> usize {
	match graphic {
		Graphic::GraphicList(children) if fully_flatten || depth == 0 => (0..children.len())
			.map(|index| children.element(index).map_or(0, |child| leaf_count(child, fully_flatten, depth + 1)))
			.sum(),
		Graphic::Group(group) if (fully_flatten || depth == 0) && group_expands(group) => group_leaf_count(group, fully_flatten, depth),
		_ => 1,
	}
}

/// The `remaining`-th leaf of `graphic` in walk order, with the transforms
/// along its path composed onto `transform`.
pub(crate) fn locate<'e>(graphic: &Graphic<'e>, transform: DAffine2, fully_flatten: bool, depth: usize, remaining: &mut usize) -> Option<(Graphic<'e>, DAffine2)> {
	match graphic {
		Graphic::GraphicList(children) if fully_flatten || depth == 0 => (0..children.len()).find_map(|index| {
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

/// The ancestor composition a typed flatten's leaf inherits: transform,
/// opacity and fill opacity multiply down the path, as the legacy flatten did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Inherited {
	pub transform: DAffine2,
	pub opacity: f64,
	pub fill_opacity: f64,
}

impl Inherited {
	pub(crate) const IDENTITY: Self = Self {
		transform: DAffine2::IDENTITY,
		opacity: 1.,
		fill_opacity: 1.,
	};

	/// The composition a top-level row starts from: the row's own columns.
	pub(crate) fn of(lane: &RecordLane<'_>) -> Self {
		Self {
			transform: lane.attr::<Transform>(),
			opacity: lane.attr::<Opacity>(),
			fill_opacity: lane.attr::<OpacityFill>(),
		}
	}

	fn composed<S: LaneSource>(self, source: &S, lane: usize) -> Self {
		Self {
			transform: self.transform * source.attr::<Transform>(lane),
			opacity: self.opacity * source.attr::<Opacity>(lane),
			fill_opacity: self.fill_opacity * source.attr::<OpacityFill>(lane),
		}
	}
}

/// Visits every `T` leaf under `graphic`, at any depth, with the composition
/// along its path. A group run typed `T` contributes its lanes directly; runs
/// of other element types contribute nothing.
pub(crate) fn walk_typed_leaves<T: TryFromGraphic + dyn_any::StaticTypeSized>(graphic: &Graphic, inherited: Inherited, visit: &mut dyn FnMut(&T, Inherited) -> RowStep) -> RowStep {
	match graphic {
		Graphic::GraphicList(children) => {
			for index in 0..children.len() {
				let Some(child) = children.element(index) else { continue };
				if let RowStep::Stop = walk_typed_leaves(child, inherited.composed(children, index), visit) {
					return RowStep::Stop;
				}
			}
			RowStep::Continue
		}
		Graphic::Group(group) => {
			let item = &group.content;
			if let Some(run) = RunView::<Graphic>::new(item) {
				for lane in 0..item.len() {
					let Some(child) = run.element(lane) else { continue };
					if let RowStep::Stop = walk_typed_leaves(child, inherited.composed(&run, lane), visit) {
						return RowStep::Stop;
					}
				}
			} else if let Some(run) = RunView::<T>::new(item) {
				for lane in 0..item.len() {
					let Some(leaf) = run.element(lane) else { continue };
					if let RowStep::Stop = visit(leaf, inherited.composed(&run, lane)) {
						return RowStep::Stop;
					}
				}
			}
			RowStep::Continue
		}
		leaf => match T::leaf_of(leaf) {
			Some(leaf) => visit(leaf, inherited),
			None => RowStep::Continue,
		},
	}
}

/// The `T` leaves under `graphic`, at any depth.
pub(crate) fn typed_leaf_count<T: TryFromGraphic + dyn_any::StaticTypeSized>(graphic: &Graphic) -> usize {
	let mut count = 0;
	walk_typed_leaves::<T>(graphic, Inherited::IDENTITY, &mut |_, _| {
		count += 1;
		RowStep::Continue
	});
	count
}

/// One content row as the production vararg shape: a single-item legacy list
/// carrying the row's element only, so the list's dyn-hash is a complete
/// cache key over the observables.
pub(crate) fn vararg_row<Row: Clone + Send + Sync + 'static>(content: core_types::node::List<'_, Row>, row: usize) -> core_types::list::List<Row> {
	core_types::list::List::new_from_element(content.element_ref(row).clone())
}

/// Rank-model nested Map: one subgraph invocation per content row, the row
/// riding as a vararg; the subgraph's own level nests under the content level.
/// The levels report a lower bound; consumers drain to the past-end signal.
/// The production `Map` is this walk with the levels concatenated.
#[node_macro::node(category("Test"))]
fn nested_map<Row: Clone + Send + Sync + core_types::CacheHash + 'static, T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, Gradient, String)] content: IList<Row>,
	mapped: impl Node<Context<'_>, Output = IList<T>>,
) -> Result<IList<IList<T>>, Interrupt> {
	let mut remaining = ctx.index();
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
/// flat index already spans the input's depth, so the eval forwards it.
#[node_macro::node(category("Test"), extent(flatten_levels_extent))]
fn flatten_levels<T>(ctx: impl Ctx + DeriveCtx + ExtractIndex, content: impl Node<Context<'_>, Output = IList<IList<T>>>) -> Result<IList<T>, Interrupt> {
	let head = ctx.index_head();
	content.eval(&ctx.promoted(&head, ctx.index()))
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
	use crate::graphic::{
		ColorsToGradientNode, FlattenColorNode, FlattenGraphicNode, GradientToColorsNode, WrapGraphicNode, flatten_color_layout_meta, flatten_graphic_layout_meta, gradient_to_colors_layout_meta,
		wrap_graphic_layout_meta,
	};
	use core_types::arena::Arena;
	use core_types::attribute::Attribute as AttributeMarker;
	use core_types::context::{ContextImpl, ExtractArena};
	use core_types::list::{Item, List};
	use core_types::node::Node;
	use core_types::record::test_fixtures::*;
	use core_types::record::{self, FrameClaim, Layout, RecordSource, Served};
	use core_types::value::ValueSource;
	use graphene_core::list::{MapNode, map_entries};

	struct GraphicSource {
		layout: Layout,
		rows: Vec<(Graphic<'static>, DAffine2)>,
	}

	impl<C: ExtractIndex> Node<C> for GraphicSource {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let (graphic, transform) = &self.rows[input.innermost_index() as usize % self.rows.len()];
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(graphic.clone(), arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<Transform>(&mut frame, &self.layout, *transform);
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
	fn graphic_layout() -> Layout {
		Layout::default().with_writes(1, record::element_write_hashed::<Graphic>(), &[record::FieldWrite::of::<Transform>(0)])
	}

	fn text(label: &str) -> Graphic<'static> {
		Graphic::Text(label.to_string())
	}

	fn group(children: Vec<(Graphic<'static>, DAffine2)>) -> Graphic<'static> {
		let mut list = List::new();
		for (index, (child, transform)) in children.into_iter().enumerate() {
			list.push(Item::new_from_element(child));
			list.set_attribute(ATTR_TRANSFORM, index, transform);
		}
		Graphic::GraphicList(list)
	}

	fn text_of<'a>(graphic: &'a Graphic<'_>) -> &'a str {
		let Graphic::Text(text) = graphic else {
			panic!("expected a text leaf, got {graphic:?}");
		};
		text
	}

	fn translation(x: f64) -> DAffine2 {
		DAffine2::from_translation(glam::DVec2::new(x, 0.))
	}

	/// [a, G[b, H[c]]] with translations picked so each composed path is a
	/// distinct sum.
	fn fixture_rows() -> Vec<(Graphic<'static>, DAffine2)> {
		vec![
			(text("a"), translation(1.)),
			(
				group(vec![(text("b"), translation(20.)), (group(vec![(text("c"), translation(300.))]), translation(4000.))]),
				translation(0.5),
			),
		]
	}

	macro_rules! build {
		($layout:ident, $rows:expr, $fully:expr) => {
			install(
				FlattenGraphicNode::new(
					RecordSource::new(
						GraphicSource {
							layout: $layout.clone(),
							rows: $rows,
						},
						&$layout,
						&$layout,
					),
					ValueSource::new($fully),
				),
				flatten_graphic_layout_meta(),
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

	fn vararg_text<C: core_types::ExtractVarArgs>(input: &C) -> Option<String> {
		let arg = core_types::ExtractVarArgs::vararg(input, 0).ok()?;
		let list = arg.downcast_ref::<core_types::list::List<Graphic>>()?;
		let Graphic::Text(text) = list.element(0)? else { return None };
		Some(text.clone())
	}

	impl<C: ExtractIndex + core_types::ExtractVarArgs> Node<C> for PerRowSource {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let Some(label) = vararg_text(input) else {
				return GPoll::error("the subgraph fixture expects a text vararg");
			};
			let lane = input.innermost_index();
			let graphic = text(&format!("{label}{lane}"));
			let translated = DAffine2::from_translation(glam::DVec2::new(lane as f64, 0.));
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(graphic, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<Transform>(&mut frame, &self.layout, translated);
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, input: &C, _level: u8, _frames: &core_types::record::Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			match vararg_text(input) {
				Some(label) => GPoll::Final(Extent::Exactly(label.len())),
				None => GPoll::error("the subgraph fixture expects a text vararg"),
			}
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	fn ragged_rows() -> Vec<(Graphic<'static>, DAffine2)> {
		vec![(text("ab"), translation(10.)), (text("xyz"), translation(20.))]
	}

	fn routing_meta(source: u8, level_delta: i8) -> record::LayoutMeta {
		record::LayoutMeta {
			named_writes: Vec::new(),
			folded_names: Vec::new(),
			named_reads: Vec::new(),
			folded_read_names: Vec::new(),
			sources: vec![source],
			reads: vec![],
			element: record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta,
			folded: None,
			gathered: false,
		}
	}

	const RAGGED_FLAT: [(&str, f64); 5] = [("ab0", 0.), ("ab1", 1.), ("xyz0", 0.), ("xyz1", 1.), ("xyz2", 2.)];

	#[test]
	fn map_scans_ragged_rows() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let node = install(
			NestedMapNode::<_, _, Graphic>::new(
				RecordSource::new(
					GraphicSource {
						layout: layout.clone(),
						rows: ragged_rows(),
					},
					&layout,
					&layout,
				),
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
		assert_eq!(node.extent_at(&ctx, 1, &frames.reborrow()), GPoll::Final(Extent::AtLeast(0)));
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::AtLeast(0)));

		let head = ctx.index_head();
		for (lane, &(label, x)) in RAGGED_FLAT.iter().enumerate() {
			let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(text_of(&record.element::<Graphic>()), label, "lane {lane}");
			let transform: DAffine2 = record.attr::<Transform>();
			assert_eq!(transform.translation.x, x, "lane {lane}");
		}
	}

	#[test]
	fn flat_map_matches_flatten_of_map() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let flat = install(
			MapNode::<_, _, Graphic>::new(
				RecordSource::new(
					GraphicSource {
						layout: layout.clone(),
						rows: ragged_rows(),
					},
					&layout,
					&layout,
				),
				PerRowSource { layout: layout.clone() },
				&layout,
			),
			routing_meta(1, 0),
			&[Some(&layout), Some(&layout)],
		);
		let mapped = install(
			NestedMapNode::<_, _, Graphic>::new(
				RecordSource::new(
					GraphicSource {
						layout: layout.clone(),
						rows: ragged_rows(),
					},
					&layout,
					&layout,
				),
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
		assert_eq!(flat.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::AtLeast(0)));
		assert_eq!(composed.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::AtLeast(0)));

		let head = ctx.index_head();
		for (lane, &(label, x)) in RAGGED_FLAT.iter().enumerate() {
			let scoped = ctx.promoted(&head, lane as u64);
			let GPoll::Final(direct) = record::capture(&flat, &scoped, &frames) else {
				panic!("expected a final record from flat_map");
			};
			let direct_label = text_of(&direct.element::<Graphic>()).to_string();
			let direct_x: DAffine2 = direct.attr::<Transform>();
			let GPoll::Final(value) = record::capture(&composed, &scoped, &frames) else {
				panic!("expected a final record from flatten(map)");
			};
			assert_eq!(text_of(&value.element::<Graphic>()), direct_label, "lane {lane}");
			let composed_x: DAffine2 = value.attr::<Transform>();
			assert_eq!(composed_x, direct_x, "lane {lane}");
			assert_eq!((direct_label.as_str(), direct_x.translation.x), (label, x), "lane {lane}");
		}
	}

	#[test]
	fn flat_map_registers_one_row_per_content_type() {
		let entries = map_entries();
		assert_eq!(entries.len(), 6, "one registry row per content implementation");
		let content_types: Vec<core_types::Type> = entries.iter().map(|entry| entry.io.inputs[0].clone()).collect();
		assert_eq!(content_types[0], core_types::registry::record_source_type::<Graphic>());
		assert_eq!(content_types[1], core_types::registry::record_source_type::<Vector>());
		assert_eq!(content_types[5], core_types::registry::record_source_type::<String>());
		// The subject and the output stay erased across rows.
		assert_eq!(entries[0].io.inputs[1], entries[5].io.inputs[1]);
		assert_eq!(entries[0].io.return_value, entries[5].io.return_value);
	}

	#[test]
	fn flat_map_batch_matches_per_lane_eval() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let node = install(
			MapNode::<_, _, Graphic>::new(
				RecordSource::new(
					GraphicSource {
						layout: layout.clone(),
						rows: ragged_rows(),
					},
					&layout,
					&layout,
				),
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
		let core_types::node::BatchStatus::Filled(batch, ..) = node.eval_batch(&scoped, 0..5, Some(&mut scratch), &frames) else {
			panic!("expected a filled batch");
		};
		let batch = batch.into_shared();
		assert_eq!(batch.len(), 5);
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		for lane in 0..5 {
			let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			let single = text_of(&record.element::<Graphic>()).to_string();
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(batch.get(lane).rec()) }), single, "lane {lane}");
			let batched: DAffine2 = unsafe { batch.get(lane).rec().read(offset) };
			let direct: DAffine2 = record.attr::<Transform>();
			assert_eq!(batched, direct, "lane {lane}");
		}
	}

	#[test]
	fn flatten_expands_one_level() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let node = build!(layout, fixture_rows(), false);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 1);
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(3)));

		let head = ctx.index_head();
		// Lane 2 is the unexpanded subgroup H, riding as a leaf at G's depth.
		let expected: [(&str, f64); 2] = [("a", 1.), ("b", 20.5)];
		for (lane, &(label, x)) in expected.iter().enumerate() {
			let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(text_of(&record.element::<Graphic>()), label, "lane {lane}");
			let transform: DAffine2 = record.attr::<Transform>();
			assert_eq!(transform.translation.x, x, "lane {lane}");
		}

		let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, 2), &frames) else {
			panic!("expected a final record");
		};
		let Graphic::GraphicList(children) = record.element::<Graphic>() else {
			panic!("lane 2 keeps the subgroup element");
		};
		assert_eq!(children.len(), 1);
		assert_eq!(text_of(children.element(0).unwrap()), "c");
		assert_eq!(
			children.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, 0).translation.x,
			300.,
			"embedded transforms ride untouched"
		);
		let transform: DAffine2 = record.attr::<Transform>();
		assert_eq!(transform.translation.x, 4000.5);
	}

	#[test]
	fn a_wire_materializes_into_a_group_for_the_renderer() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source = core_types::value::LeveledValueSource::new(vec![text("a"), text("b")]);
		match graphic_types::boundary::materialize_group(&source, &ctx, &arena, &frames) {
			graphic_types::boundary::LevelGroup::Group(group, _) => {
				let list = graphic_types::graphic::group_to_legacy_list(&group);
				assert_eq!(list.len(), 2);
			}
			_ => panic!("expected a materialized group"),
		}
	}

	#[test]
	fn a_level_batch_converts_to_its_legacy_list() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source = core_types::value::LeveledValueSource::new(vec![1.5f64, 2.5]);
		let layout = Node::<ContextImpl>::layout(&source).clone();
		let record::LevelStatus::Batch(batch, _) = record::materialize_level(&source, &ctx, &arena, &frames) else {
			panic!("expected a batch");
		};
		let legacy = graphic_types::boundary::batch_to_legacy(&layout, batch, &arena).expect("f64 is in the legacy vocabulary");
		let list = legacy.downcast_ref::<List<f64>>().unwrap();
		assert_eq!(list.len(), 2);
		assert_eq!(list.element(0).copied(), Some(1.5));
		assert_eq!(list.element(1).copied(), Some(2.5));
	}

	#[test]
	fn wrap_collects_the_level_into_a_group() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let rows = vec![(text("a"), translation(1.)), (text("b"), translation(2.))];
		let node = install(
			WrapGraphicNode::<_, Graphic>::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows }, &layout, &layout), &layout),
			wrap_graphic_layout_meta(),
			&[Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 1);
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(1)), "the group is the level's single lane");

		let head = ctx.index_head();
		let GPoll::Final(value) = record::serve_input(&node, &ctx.promoted(&head, 0), &frames) else {
			panic!("expected a final record");
		};
		let Graphic::Group(group) = (unsafe { record::borrow_element::<Graphic>(out.rec(&value)) }) else {
			panic!("expected a group element");
		};
		assert!(group.row.is_none());
		let item = &group.content;
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
	fn a_group_element_deep_copies_to_its_owned_form_and_replays() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let rows = vec![(text("a"), translation(1.)), (text("b"), translation(2.))];
		let node = install(
			WrapGraphicNode::<_, Graphic>::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows }, &layout, &layout), &layout),
			wrap_graphic_layout_meta(),
			&[Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();

		let head = ctx.index_head();
		let GPoll::Final(value) = record::serve_input(&node, &ctx.promoted(&head, 0), &frames) else {
			panic!("expected a final record");
		};
		let copy = unsafe { (out.element.clone_out)(out.rec(&value).ptr()) };

		let replay_arena = Arena::new(1 << 16).unwrap();
		// Word storage: a parked element slot holds an 8-aligned reference.
		let mut slot = [0u64; 1];
		unsafe { (out.element.repark)(&*copy, slot.as_mut_ptr().cast(), &replay_arena) }.expect("the arena holds the replay");
		// SAFETY: the re-park wrote a parked `Graphic` element into `slot`.
		let Graphic::Group(group) = (unsafe { record::borrow_element::<Graphic>(record::Rec::new(slot.as_ptr().cast())) }) else {
			panic!("the replay restores the group element");
		};
		let item = &group.content;
		assert_eq!(item.len(), 2);
		let lanes = item.typed_lanes::<Graphic>().expect("the run holds the adopted graphic lanes");
		let offset = item.layout().offset_of(ATTR_TRANSFORM, 0).unwrap();
		for (lane, (label, x)) in [("a", 1.), ("b", 2.)].into_iter().enumerate() {
			assert_eq!(text_of(lanes.element_ref(lane)), label, "lane {lane}");
			let transform: DAffine2 = unsafe { item.lanes().get(lane).rec().read(offset) };
			assert_eq!(transform.translation.x, x, "lane {lane}");
		}
	}

	/// The stops of a gradient level unwrap to color lanes carrying their
	/// effective placement, which a `Colors to Gradient` reads back.
	#[test]
	fn gradient_stops_round_trip_through_color_lanes() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let mut gradient = Gradient::from(vec![Color::RED, Color::GREEN, Color::BLUE]);
		gradient.set_positions(&[0., 0.25, 1.]);
		gradient.set_midpoints(&[0.3, 0.5, 0.5]);
		let source = core_types::value::LeveledValueSource::new(vec![gradient.clone()]);
		let layout = Node::<ContextImpl>::layout(&source).clone();
		let colors = install(GradientToColorsNode::new(source), gradient_to_colors_layout_meta(), &[Some(&layout)]);
		let colors_layout = Node::<ContextImpl>::layout(&colors).clone();
		assert_eq!(colors.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(3)));

		let head = ctx.index_head();
		for (lane, (color, position, midpoint)) in [(Color::RED, 0., 0.3), (Color::GREEN, 0.25, 0.5), (Color::BLUE, 1., 0.5)].into_iter().enumerate() {
			let GPoll::Final(record) = record::capture(&colors, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(record.element::<Color>(), color, "lane {lane}");
			assert_eq!(record.attr::<core_types::attribute::Position>(), position, "lane {lane}");
			assert_eq!(record.attr::<core_types::attribute::Midpoint>(), midpoint, "lane {lane}");
		}

		let out = Layout::default().with_writes(0, record::element_write_hashed::<Gradient>(), &[]);
		let restored = install_flip(ColorsToGradientNode::new(colors, &colors_layout), &out);
		let GPoll::Final(record) = record::capture(&restored, &ctx, &frames) else {
			panic!("expected a final record");
		};
		let restored = record.element::<Gradient>();
		assert_eq!(restored.positions(false), gradient.positions(false));
		assert_eq!(restored.midpoints(), gradient.midpoints());
		assert_eq!(restored.iter().map(|stop| stop.color).collect::<Vec<_>>(), vec![Color::RED, Color::GREEN, Color::BLUE]);
	}

	#[test]
	fn colors_fold_into_evenly_spaced_stops() {
		let frames = core_types::record::test_frames(1 << 16);
		struct ColorSource {
			layout: Layout,
			colors: Vec<Color>,
		}

		impl<C: ExtractIndex> Node<C> for ColorSource {
			fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
			where
				C: ExtractArena<ArenaRef = &'e Arena>,
			{
				let color = self.colors[input.innermost_index() as usize];
				let mut frame = slot;
				let arena = ExtractArena::arena(input);
				if frame.element(color, arena).is_none() {
					return GPoll::arena_exhausted();
				}
				// SAFETY: the writes above complete the record of this layout.
				GPoll::Final(unsafe { frame.finish_served() })
			}

			fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &core_types::record::Frames<'x>) -> GPoll<Extent>
			where
				C: ExtractArena<ArenaRef = &'x Arena>,
			{
				GPoll::Final(Extent::Exactly(self.colors.len()))
			}

			fn layout(&self) -> &Layout {
				&self.layout
			}
		}

		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = Layout::default().with_writes(1, record::element_write_hashed::<Color>(), &[]);
		let out = Layout::default().with_writes(0, record::element_write_hashed::<Gradient>(), &[]);
		let build = |colors: Vec<Color>| {
			install_flip(
				ColorsToGradientNode::new(RecordSource::new(ColorSource { layout: layout.clone(), colors }, &layout, &layout), &layout),
				&out,
			)
		};
		let stops_of = |colors: Vec<Color>| {
			let node = build(colors);
			let GPoll::Final(record) = record::capture(&node, &ctx, &frames) else {
				panic!("expected a final record");
			};
			record.element::<Gradient>()
		};

		let three = stops_of(vec![Color::BLACK, Color::WHITE, Color::BLACK]);
		assert_eq!(three.iter().map(|stop| stop.position).collect::<Vec<_>>(), vec![0., 0.5, 1.]);
		assert_eq!(three.iter().map(|stop| stop.color).collect::<Vec<_>>(), vec![Color::BLACK, Color::WHITE, Color::BLACK]);

		// A lone color is a one-stop gradient and no colors a stopless one; neither is padded
		let single = stops_of(vec![Color::WHITE]);
		assert_eq!(single.iter().map(|stop| (stop.position, stop.color)).collect::<Vec<_>>(), vec![(0., Color::WHITE)]);

		let empty = stops_of(Vec::new());
		assert!(empty.iter().next().is_none());
	}

	#[test]
	fn a_group_converts_to_its_legacy_list() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let rows = vec![(text("a"), translation(1.)), (text("b"), translation(2.))];
		let node = install(
			WrapGraphicNode::<_, Graphic>::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows }, &layout, &layout), &layout),
			wrap_graphic_layout_meta(),
			&[Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		let head = ctx.index_head();
		let GPoll::Final(value) = record::serve_input(&node, &ctx.promoted(&head, 0), &frames) else {
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
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = graphic_layout();
		let rows = vec![(text("a"), translation(1.)), (text("b"), translation(2.))];
		let wrapped = install(
			WrapGraphicNode::<_, Graphic>::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows }, &layout, &layout), &layout),
			wrap_graphic_layout_meta(),
			&[Some(&layout)],
		);
		let wrap_out = Node::<ContextImpl>::layout(&wrapped).clone();
		let head = ctx.index_head();
		let group = {
			// SAFETY: the element is cloned out inside the scope, so no borrow
			// into the frame escapes it. The clone is shallow, so the `'static`
			// the `GraphicSource` rows infer launders a borrow of `arena`: it is
			// contained because `arena` outlives every use below and this test
			// never resets it, so the interior stays resident for the whole
			// generation the group is read in.
			let scope = frames.scope();
			let GPoll::Final(value) = record::serve_input(&wrapped, &ctx.promoted(&head, 0), &scope) else {
				panic!("expected a final record");
			};
			unsafe { record::borrow_element::<Graphic>(wrap_out.rec(&value)) }.clone()
		};

		// One row holding the wrapped group flattens back to the lanes, the
		// group's identity transform composed onto each child's.
		let node = build!(layout, vec![(group, DAffine2::IDENTITY)], false);
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(2)));

		let head = ctx.index_head();
		for (lane, &(label, x)) in [("a", 1.), ("b", 2.)].iter().enumerate() {
			let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(text_of(&record.element::<Graphic>()), label, "lane {lane}");
			let transform: DAffine2 = record.attr::<Transform>();
			assert_eq!(transform.translation.x, x, "lane {lane}");
		}
	}

	/// [Color a, G[Color b (opacity 0.5), Text], Text]: two color leaves, the
	/// nested one composing G's transform and its own opacity; the texts drop.
	#[test]
	fn flatten_color_keeps_only_color_leaves_and_composes_the_path() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let nested = {
			let Graphic::GraphicList(mut children) = group(vec![(Graphic::Color(Color::WHITE), translation(20.)), (text("x"), translation(300.))]) else {
				unreachable!("group builds a legacy graphic list");
			};
			children.set_attribute(core_types::ATTR_OPACITY, 0, 0.5);
			Graphic::GraphicList(children)
		};
		let rows = vec![(Graphic::Color(Color::BLACK), translation(1.)), (nested, translation(0.5)), (text("y"), translation(9.))];
		let layout = graphic_layout();
		let node = install(
			FlattenColorNode::new(RecordSource::new(GraphicSource { layout: layout.clone(), rows }, &layout, &layout)),
			flatten_color_layout_meta(),
			&[Some(&layout)],
		);
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(2)));

		let head = ctx.index_head();
		let expected = [(Color::BLACK, 1., 1.), (Color::WHITE, 20.5, 0.5)];
		for (lane, &(color, x, opacity)) in expected.iter().enumerate() {
			let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(record.element::<Color>(), color, "lane {lane}");
			let transform: DAffine2 = record.attr::<Transform>();
			assert_eq!(transform.translation.x, x, "lane {lane}");
			assert_eq!(record.attr::<Opacity>(), opacity, "lane {lane}");
		}
	}

	#[test]
	fn flatten_fully_composes_the_path() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let mut rows = fixture_rows();
		rows.push((group(vec![]), translation(9.)));
		let layout = graphic_layout();
		let node = build!(layout, rows, true);
		assert_eq!(node.extent_at(&ctx, 0, &frames.reborrow()), GPoll::Final(Extent::Exactly(3)), "the empty group contributes no leaves");

		let head = ctx.index_head();
		let expected: [(&str, f64); 3] = [("a", 1.), ("b", 20.5), ("c", 4300.5)];
		for (lane, &(label, x)) in expected.iter().enumerate() {
			let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(text_of(&record.element::<Graphic>()), label, "lane {lane}");
			let transform: DAffine2 = record.attr::<Transform>();
			assert_eq!(transform.translation.x, x, "lane {lane}");
		}
	}

	#[test]
	fn flatten_batch_matches_per_lane_eval() {
		let frames = core_types::record::test_frames(1 << 16);
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
		let core_types::node::BatchStatus::Filled(batch, ..) = node.eval_batch(&scoped, 0..3, Some(&mut scratch), &frames) else {
			panic!("expected a filled batch");
		};
		let batch = batch.into_shared();
		assert_eq!(batch.len(), 3);
		let offset = out.offset_of(<Transform as AttributeMarker>::NAME, 0).unwrap();
		for lane in 0..3 {
			let GPoll::Final(record) = record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			let single = text_of(&record.element::<Graphic>()).to_string();
			assert_eq!(text_of(unsafe { record::borrow_element::<Graphic>(batch.get(lane).rec()) }), single, "lane {lane}");
			let batched: DAffine2 = unsafe { batch.get(lane).rec().read(offset) };
			let direct: DAffine2 = record.attr::<Transform>();
			assert_eq!(batched, direct, "lane {lane}");
		}
	}
}
