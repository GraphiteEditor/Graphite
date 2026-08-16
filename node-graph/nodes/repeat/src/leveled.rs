//! The rank-model repeat family: lazy leveled creators that evaluate their
//! content once per copy and compose the per-copy transform onto each row.
//! Test-category until the flip swaps them in for the eager variants.

use core_types::attribute::{Attr, Transform};
use core_types::context::ExtractIndex;
use core_types::extent::{ExtentIn, LevelIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, Interrupt};
use core_types::registry::types::{Angle, PixelSize};
use core_types::{Ctx, DeriveCtx};
use glam::DAffine2;

/// The rank-model Repeat Array: each copy evaluates the lazy content at its
/// own index and composes the linear step onto the row's transform.
#[node_macro::node(category("Test"), extent(repeat_array_extent))]
fn repeat_array<T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex,
	content: impl Node<Context<'_>, Output = (T, Attr<Transform>)>,
	#[default(100., 100.)] direction: PixelSize,
	angle: Angle,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> Result<IList<(T, Attr<Transform>)>, Interrupt> {
	let spilled = ctx.index_head();
	let copy = ctx.innermost_index() % count as u64;
	let (element, local) = content.eval(&ctx.promoted(&spilled, copy))?;

	// A single copy has no steps between copies, so the denominator stays 1.
	let total = (count - 1).max(1) as f64;
	let step = DAffine2::from_angle(copy as f64 * angle.to_radians() / total) * DAffine2::from_translation(copy as f64 * direction / total);
	let local_translation = DAffine2::from_translation(local.translation);
	let local_matrix = DAffine2::from_mat2(local.matrix2);
	Ok(emit(element, Attr(local_translation * step * local_matrix)))
}

/// The pushed level's extent is the copy count; inner levels forward to the
/// content, whose extent is taken uniform across copies (queried at copy 0).
fn repeat_array_extent(content: ExtentIn<'_>, _direction: ValueIn<'_, PixelSize>, _angle: ValueIn<'_, Angle>, count: ValueIn<'_, u32>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => content.at(level),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::arena::Arena;
	use core_types::attribute::Attribute as AttributeMarker;
	use core_types::context::{ContextImpl, EvalScope};
	use core_types::node::Node;
	use core_types::record::{FieldWrite, Layout, Rec, RecordSource, RecordValue, element_write, stack};
	use glam::DVec2;

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

		fn eval(&self, _input: &ContextImpl<'e>) -> GPoll<RecordValue<'e>> {
			let dst = stack::push(self.layout.frame_bytes());
			unsafe {
				dst.cast::<f64>().write(self.element);
				dst.add(self.layout.offset_of(Transform::NAME, 0).unwrap()).cast::<DAffine2>().write(self.transform);
			}
			stack::pop(dst);
			GPoll::Final(RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) }))
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		stack::reserve(1 << 12);
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn transform_layout() -> Layout {
		Layout::default().with_writes(0, element_write::<f64>(), &[FieldWrite::of::<Transform>(0)])
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
			let mark = stack::sp();
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(value) = node.eval(&lane) else {
				panic!("expected a final record");
			};
			let rec = leveled.rec(&value);
			assert_eq!(unsafe { rec.element::<f64>() }, 7.);
			// Zero angle, direction (10, 0), count 3: copy `j` steps j * (5, 0)
			// past the row's own (5, 5) translation.
			let composed: DAffine2 = unsafe { rec.read(leveled.offset_of(Transform::NAME, 0).unwrap()) };
			assert_eq!(composed, DAffine2::from_translation(DVec2::new(5. + copy as f64 * 5., 5.)));
			// SAFETY: the element and transform were read out above, so no borrow into this lane's frames remains.
			unsafe { stack::rewind(mark) };
		}
	}
}
