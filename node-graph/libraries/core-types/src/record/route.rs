//! Producer-side routing: a source's translation into the union layout.

use super::access::{Rec, apply_plan};
use super::frames::Frames;
use super::layout::{Layout, copy_plan};
use super::serve::{FrameClaim, Served, serve_input};
use crate::attribute;
use crate::gpoll::GPoll;
use crate::node::Node;

/// The default bytes for a union field the source does not carry: the census
/// default for declared names, zeroes otherwise.
fn default_fill_bytes(name: &str, size: usize) -> Box<[u8]> {
	let mut bytes = vec![0u8; size].into_boxed_slice();
	if let Some(info) = attribute::info(name)
		&& info.size == size
	{
		(info.write_default_bytes)(&mut bytes);
	}
	bytes
}

/// A routing source's wiring-resolved translation: field moves into the
/// union layout plus census default fill for union fields the source lacks.
/// Absent when the source's layout already equals the union, in which case
/// the record pointer forwards untouched.
#[derive(Debug)]
pub struct SourcePlan {
	moves: Vec<(usize, usize, usize)>,
	fills: Vec<(usize, Box<[u8]>)>,
	source: Layout,
}

impl SourcePlan {
	pub fn new(source: &Layout, union: &Layout) -> Option<SourcePlan> {
		if source == union {
			return None;
		}
		let moves = copy_plan(source, union, true, &[]);
		let fills = union
			.fields
			.iter()
			.filter(|field| source.offset_of(field.name, field.level).is_none())
			.map(|field| (field.offset, default_fill_bytes(field.name, field.size)))
			.collect();
		Some(SourcePlan { moves, fills, source: source.clone() })
	}

	/// # Safety
	/// `src` must be a record of this plan's source layout and `dst` a
	/// buffer of the plan's union layout. The returned view borrows `dst`, so
	/// `'d` must not outlive it.
	pub unsafe fn translate<'d>(&self, src: Rec<'_>, dst: *mut u8) -> Rec<'d> {
		unsafe {
			apply_plan(src, dst, &self.moves);
			for (offset, bytes) in &self.fills {
				std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.add(*offset), bytes.len());
			}
			Rec::new(dst)
		}
	}
}

/// A routing input's claimed source plus its wiring-resolved [`SourcePlan`].
/// Evaluating it yields the source's record translated to the union layout
/// (or forwarded untouched when the layouts already agree), so the kernel
/// holds and returns record values without ever seeing the representation.
/// A translation lands in the claim the caller minted, so the value survives
/// sibling evaluations and its region is free again with that claim, which
/// bounds claims at one per source evaluation the kernel performs.
pub struct RecordSource<N> {
	edge: N,
	plan: Option<SourcePlan>,
	union: Layout,
}

impl<N> RecordSource<N> {
	pub fn new(edge: N, source: &Layout, union: &Layout) -> Self {
		Self {
			edge,
			plan: SourcePlan::new(source, union),
			union: union.clone(),
		}
	}
}

impl<C, N> Node<C> for RecordSource<N>
where
	N: Node<C>,
{
	fn serve<'e, 'l>(&self, input: &C, mut slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
	where
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		// The source's frame is claimed beyond this one and dies with the
		// claim; the translated union record stays.
		let Some(plan) = &self.plan else {
			return self.edge.serve(input, slot);
		};
		match serve_input(&self.edge, input, &mut slot.frames().reborrow()) {
			GPoll::Final(value) => {
				// SAFETY: the value came from this source, so it carries the
				// plan's source layout.
				unsafe { slot.translate(plan.source.rec(&value), plan) };
				// SAFETY: the translation completes the union record.
				GPoll::Final(unsafe { slot.finish_served() })
			}
			GPoll::Partial(value) => {
				// SAFETY: as for the final arm.
				unsafe { slot.translate(plan.source.rec(&value), plan) };
				// SAFETY: as for the final arm.
				GPoll::Partial(unsafe { slot.finish_served() })
			}
			GPoll::Fallback(boxed) => {
				let (value, error) = *boxed;
				// SAFETY: as for the final arm.
				unsafe { slot.translate(plan.source.rec(&value), plan) };
				// SAFETY: as for the final arm.
				GPoll::Fallback(Box::new((unsafe { slot.finish_served() }, error)))
			}
			GPoll::Pending => GPoll::Pending,
			GPoll::Error(error) => GPoll::Error(error),
		}
	}

	fn extent_at<'x>(&self, input: &C, level: u8, frames: &Frames<'x>) -> GPoll<crate::gpoll::Extent>
	where
		C: crate::context::ExtractArena<ArenaRef = &'x crate::arena::Arena>,
	{
		self.edge.extent_at(input, level, frames)
	}

	fn layout(&self) -> &Layout {
		&self.union
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::record::layout::element_write;
	use crate::record::test_support::f64_field;

	#[test]
	fn translation_moves_fields_and_fills_census_defaults() {
		let source = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("length")]);
		let union = Layout::union(&[&source, &Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")])]);

		let plan = SourcePlan::new(&source, &union).unwrap();
		let record = [5f64, 7f64];
		let mut buffer = vec![0u64; union.size.div_ceil(8)];
		let translated = unsafe { plan.translate(Rec::new(record.as_ptr().cast()), buffer.as_mut_ptr().cast()) };
		assert_eq!(unsafe { translated.element::<f64>() }, 5.);
		assert_eq!(unsafe { translated.read::<f64>(union.offset_of("length", 0).unwrap()) }, 7.);
		assert_eq!(unsafe { translated.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 1.);
	}

	#[test]
	fn identity_layouts_forward() {
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		assert!(SourcePlan::new(&layout, &layout.clone()).is_none());
	}
}
