//! Pilot record nodes over the production graphic types: element-space
//! expanders whose ragged nesting lives inside `Graphic` values, ahead of the
//! flip. Wiring is by hand until the compiler pass constructs layouts.

use core_types::attribute::{Attr, Transform};
use core_types::context::{ExtractIndex, InjectIndex};
use core_types::extent::{LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::{ATTR_TRANSFORM, Ctx};
use glam::DAffine2;
use graphic_types::graphic::Graphic;

/// Leaf rows a graphic expands to: its children's counts when the walk
/// descends (top rows always, deeper groups only in a full flatten), one for
/// itself otherwise.
fn leaf_count(graphic: &Graphic, fully_flatten: bool, depth: usize) -> usize {
	match graphic {
		Graphic::Graphic(children) if fully_flatten || depth == 0 => (0..children.len())
			.map(|index| children.element(index).map_or(0, |child| leaf_count(child, fully_flatten, depth + 1)))
			.sum(),
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
		Layout::default().with_writes(1, record::element_write::<Graphic>(), &[record::FieldWrite::of::<Transform>(0)])
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
		let core_types::node::BatchStatus::Filled(batch, _) = node.eval_batch(&scoped, 0..3, Some(&mut scratch)) else {
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
