//! The serving protocol: a node's own frame claim and the proof it closes with.

use super::access::{Rec, RecordValue, apply_plan, lift_poll_into, write_element, write_field};
use super::frames::Frames;
use super::layout::{Layout, element_dims, element_parked};
use super::route::SourcePlan;
use crate::gpoll::GPoll;
use crate::node::Node;

/// A claimed run of same-layout slots over the caller's scratch: [`Self::slot`]
/// backs an ordinary claim's own region by the lane's region of the slab, so a
/// lane serves in place with no staging copy, and [`Self::served`] records the
/// proof. The filled prefix is the only readable part, which is what makes
/// [`Self::finish`] safe.
pub struct SlotRun<'a> {
	scratch: &'a mut [std::mem::MaybeUninit<u64>],
	layout: &'a Layout,
	len: usize,
	filled: usize,
}

impl<'a> SlotRun<'a> {
	pub(in crate::record) fn new(scratch: &'a mut [std::mem::MaybeUninit<u64>], len: usize, layout: &'a Layout) -> Option<SlotRun<'a>> {
		(scratch.len() * 8 >= len * layout.lane_stride()).then_some(SlotRun { scratch, layout, len, filled: 0 })
	}

	pub fn layout(&self) -> &'a Layout {
		self.layout
	}

	/// Lane `lane`'s claim: its own frame is the lane's region of the slab and
	/// its free space is `frames`, so the lane's inputs claim beyond it.
	pub fn slot<'e>(&mut self, lane: usize, frames: &Frames<'e>) -> FrameClaim<'e, 'a> {
		assert!(lane < self.len, "lane {lane} out of bounds for a run of {}", self.len);
		let stride = self.layout.lane_stride();
		// SAFETY: in-bounds by the assert against the capacity check `new` made.
		let frame = unsafe { self.scratch.as_mut_ptr().cast::<u8>().add(lane * stride) };

		FrameClaim {
			layout: self.layout,
			inline: RecordValue::zeroed(),
			frame: (self.layout.frame_bytes() != 0).then_some(frame),
			free: frames.reborrow(),
		}
	}

	/// Records lane `lane` as served; the proof came from that lane's slot. An
	/// inline record rides the value's own storage, so it takes the one copy
	/// the run makes.
	pub fn served(&mut self, lane: usize, proof: &Served<'_>) {
		if self.layout.size == 0 {
			let stride = self.layout.lane_stride();
			// SAFETY: in-bounds by the capacity check `new` made, and the lane
			// takes the whole inline record.
			unsafe { std::ptr::copy_nonoverlapping(self.layout.rec(proof.record()).ptr(), self.scratch.as_mut_ptr().cast::<u8>().add(lane * stride), stride) };
		}
		self.filled = self.filled.max(lane + 1);
	}

	/// The served lanes as the caller's exclusive batch.
	pub fn finish(self) -> crate::node::RecordBatchMut<'a> {
		crate::node::RecordBatchMut::new(self.scratch, self.filled, self.layout)
	}
}

/// A node's own output frame, minted by its caller from the caller's frame
/// space: the one closing surface for every exit. Writes land through it,
/// [`Self::lift`] and [`Self::finish`] serve the record, and it carries the
/// free space beyond the frame, so a node's inputs claim past it and their
/// space is free again when the claim dies, on value, error, and pending exits
/// alike with no per-exit ritual.
pub struct FrameClaim<'e, 'l> {
	pub(in crate::record) layout: &'l Layout,
	pub(in crate::record) inline: RecordValue<'static>,
	pub(in crate::record) frame: Option<*mut u8>,
	pub(in crate::record) free: Frames<'e>,
}

impl<'e, 'l> FrameClaim<'e, 'l> {
	/// The free space beyond this claim's frame, which the node's own inputs
	/// claim from.
	pub fn frames(&mut self) -> &Frames<'e> {
		&mut self.free
	}

	pub(in crate::record) fn dst(&mut self) -> *mut u8 {
		match self.frame {
			Some(frame) => frame,
			None => (&raw mut self.inline).cast(),
		}
	}

	/// Asserts the served element matches the wired layout, so a node whose
	/// layout never resolved (or resolved at another type) panics here
	/// instead of writing past its frame.
	fn check_element<T: dyn_any::StaticTypeSized>(&self) {
		let (size, _) = element_dims::<T>();
		assert!(
			self.layout.element.size == size && self.layout.element.parked == element_parked::<T>() && self.layout.element.type_id == std::any::TypeId::of::<T::Static>(),
			"the served element `{}` ({size} bytes) must match the wired layout ({} bytes)",
			std::any::type_name::<T>(),
			self.layout.element.size,
		);
	}

	/// Carries the plan's fields from a source record into the frame.
	///
	/// # Safety
	/// `src` must be a live record of the plan's source layout, and the plan
	/// must be the wiring-resolved plan of this frame's layout.
	pub unsafe fn carry(&mut self, src: Rec<'_>, plan: &[(usize, usize, usize)]) {
		unsafe { apply_plan(src, self.dst(), plan) };
	}

	/// Writes a field at its wiring-resolved offset.
	///
	/// # Safety
	/// `offset` must be this layout's resolved offset for a field of `T`.
	pub unsafe fn attr_at<T>(&mut self, offset: usize, value: T) {
		unsafe { write_field(self.dst(), offset, value) };
	}

	/// Writes the element; `None` reports arena exhaustion for a parked
	/// element. Panics where the element does not match the wired layout.
	pub fn element<T: Send + Sync + dyn_any::StaticTypeSized>(&mut self, value: T, arena: &crate::arena::Arena) -> Option<()> {
		self.check_element::<T>();
		// SAFETY: the frame is this layout's fresh claim and the element
		// check pinned `T` to the layout's element slot.
		unsafe { write_element(self.dst(), value, arena) }
	}

	/// Lifts a kernel's poll into the frame and closes it: the element
	/// writes on value polls, every poll keeps the frame claimed, and arena
	/// exhaustion of a parked element reports as an error poll. Panics where
	/// the element does not match the wired layout.
	pub fn lift<T: Send + Sync + dyn_any::StaticTypeSized>(mut self, poll: GPoll<T>, arena: &'e crate::arena::Arena) -> GPoll<RecordValue<'e>> {
		self.check_element::<T>();
		let frame_bytes = self.layout.frame_bytes();
		let dst = self.dst();
		// SAFETY: the frame is this layout's fresh claim, the element check
		// pinned `T`, and the drop keeps the frame contract on every poll.
		unsafe { lift_poll_into(poll, dst, frame_bytes, arena) }
	}

	/// Copies a complete record of this layout into the frame, for serving
	/// cached or published bytes.
	///
	/// # Safety
	/// `src` must point at a live record of this layout whose parked
	/// references outlive the serving evaluation.
	pub unsafe fn fill_copy(&mut self, src: *const u8) {
		unsafe { std::ptr::copy_nonoverlapping(src, self.dst(), self.layout.size) };
	}

	/// The served record. The frame stays claimed for the consumer; the drop
	/// releases only what was claimed above it.
	///
	/// # Safety
	/// The frame must hold a complete record of the layout, written through
	/// the carry, element, and field writes.
	pub unsafe fn finish(mut self) -> RecordValue<'e> {
		match self.frame {
			Some(frame) => RecordValue::spilled(unsafe { Rec::new(frame.cast_const()) }),
			// SAFETY: the inline record is the value's own bytes.
			None => unsafe { (&raw mut self.inline).cast::<RecordValue<'e>>().read() },
		}
	}

	/// [`Self::lift`] with the proof-bearing return for [`Node::serve`].
	pub fn lift_served<T: Send + Sync + dyn_any::StaticTypeSized>(self, poll: GPoll<T>, arena: &'e crate::arena::Arena) -> GPoll<Served<'e>> {
		self.lift(poll, arena).map(|value| Served { value })
	}

	/// [`Self::finish`] with the proof-bearing return for [`Node::serve`].
	///
	/// # Safety
	/// As [`Self::finish`].
	pub unsafe fn finish_served(self) -> Served<'e> {
		Served { value: unsafe { self.finish() } }
	}

	/// Fills the frame from a record a forwarded wire already served, and
	/// closes it: the source's frame sits above this claim and dies with its
	/// drop, so the served record is this claim's own.
	///
	/// # Safety
	/// `value` must be a live record of this frame's layout.
	pub unsafe fn forward(mut self, value: &RecordValue<'_>) -> Served<'e> {
		let src = self.layout.rec(value).ptr();
		unsafe {
			self.fill_copy(src);
			self.finish_served()
		}
	}

	/// Translates a source record into the frame through a wiring-resolved plan.
	///
	/// # Safety
	/// `src` must be a live record of `plan`'s source layout, and `plan` must
	/// translate into this frame's layout.
	pub unsafe fn translate(&mut self, src: Rec<'_>, plan: &SourcePlan) {
		unsafe { plan.translate(src, self.dst()) };
	}
}

/// A materialized run of lanes as the arena region its frames live in: the
/// handle keeps the provenance the region was allocated with and carries the
/// generation, so resolving it re-checks liveness where an address would have
/// been trusted. The layout stays with the holder, which proved it at wiring.
#[derive(Clone, Copy)]
pub struct MaterializedSpan {
	pub(in crate::record) base: crate::arena::ArenaWeak<u8>,
	pub(in crate::record) len: usize,
}

impl std::fmt::Debug for MaterializedSpan {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MaterializedSpan").field("len", &self.len).finish()
	}
}

impl MaterializedSpan {
	/// `None` where the batch's frames are not this arena's, which is the
	/// caller's cue to re-materialize rather than cache.
	pub fn of(batch: &crate::node::RecordBatch<'_>, arena: &crate::arena::Arena) -> Option<MaterializedSpan> {
		match batch.len() {
			0 => Some(MaterializedSpan {
				base: crate::arena::ArenaWeak::NULL,
				len: 0,
			}),
			len => Some(MaterializedSpan {
				base: arena.handle_at(batch.get(0).rec().ptr())?,
				len,
			}),
		}
	}

	/// The span's lanes at `layout`, or `None` once the generation moved on.
	pub fn batch<'a>(&self, arena: &'a crate::arena::Arena, layout: &'a Layout) -> Option<crate::node::RecordBatch<'a>> {
		let base: *const u8 = match self.len {
			0 => std::ptr::NonNull::<u8>::dangling().as_ptr(),
			_ => self.base.upgrade(arena)?,
		};
		// SAFETY: the handle resolved in generation, so the region still holds
		// the lanes it was published with, packed at the layout's stride; an
		// empty span reads no lane.
		Some(unsafe { crate::node::RecordBatch::new(base, self.len, layout) })
	}

	/// Lane `lane`'s record, or `None` past the span or once the generation
	/// moved on.
	pub fn lane(&self, arena: &crate::arena::Arena, lane: usize, layout: &Layout) -> Option<*const u8> {
		(lane < self.len).then_some(())?;
		let base: *const u8 = self.base.upgrade(arena)?;
		// SAFETY: in-bounds by the length check, at the layout the span was
		// published under.
		Some(unsafe { base.add(lane * layout.lane_stride()) })
	}
}

/// The proof a record was served through a frame claim: mintable only by the
/// claim's closing methods, so holding one means the record is of the
/// claimed layout.
pub struct Served<'e> {
	value: RecordValue<'e>,
}

impl<'e> Served<'e> {
	pub fn value(self) -> RecordValue<'e> {
		self.value
	}

	/// The served record in place, for producers that read it before passing
	/// the proof on.
	pub fn record(&self) -> &RecordValue<'e> {
		&self.value
	}
}

/// Claims `node`'s own frame from `frames` and serves through it: the
/// caller-side half of [`Node::serve`], for drivers that want the record
/// rather than the proof.
pub fn serve_input<'e, C, N>(node: &N, input: &C, frames: &Frames<'e>) -> GPoll<RecordValue<'e>>
where
	N: Node<C> + ?Sized,
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
{
	let slot = frames.claim(node.layout());
	node.serve(input, slot).map(Served::value)
}

/// A claim shortens onto a derived context's arena lifetime.
#[cfg(test)]
fn claim_shortens<'long: 'short, 'short, 'l>(claim: FrameClaim<'long, 'l>) -> FrameClaim<'short, 'l> {
	claim
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::record::frames::FrameArena;
	use crate::record::layout::element_write;

	#[test]
	#[should_panic(expected = "must match the wired layout")]
	fn a_mistyped_element_is_rejected_at_the_write() {
		let arena = crate::arena::Arena::new(256).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[]);
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(1 << 10);
		let frames = frame_arena.frames();
		frames.claim(&layout).element(1u32, &arena);
	}

	#[test]
	fn a_claim_shortens_onto_a_derived_lifetime() {
		fn shorten<'long: 'short, 'short, 'l>(claim: FrameClaim<'long, 'l>) -> FrameClaim<'short, 'l> {
			claim_shortens(claim)
		}
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[]);
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(64);
		let frames = frame_arena.frames();
		let claim = shorten(frames.claim(&layout));
		assert_eq!(claim.layout, &layout);
	}
}
