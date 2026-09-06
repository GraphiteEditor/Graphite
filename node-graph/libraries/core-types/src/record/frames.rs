//! The evaluation's record frame storage.

use super::access::RecordValue;
use super::layout::Layout;
use super::serve::{FrameClaim, SlotRun};

/// The evaluation's record frame space: a grow-only buffer the executor owns
/// and lends by `&mut`, sized by the wiring-derived frame need, so exhaustion
/// is an accounting failure the debug assertion catches rather than a hot-path
/// branch. Frame bytes carry no drop glue, so growth and reuse are plain
/// buffer operations.
#[derive(Debug, Default)]
pub struct FrameArena {
	buf: Vec<u64>,
}

impl FrameArena {
	pub fn new() -> Self {
		Self { buf: Vec::new() }
	}

	/// Grows the buffer to hold `bytes`, the root's wiring-derived frame need.
	/// Grow-only, so repeated evaluations reuse one allocation.
	pub fn reserve(&mut self, bytes: usize) {
		let words = bytes.div_ceil(8).max(1);
		if self.buf.len() < words {
			self.buf.resize(words, 0);
		}
	}

	/// The whole buffer as free space, for one evaluation.
	pub fn frames(&mut self) -> Frames<'_> {
		let base = self.buf.as_mut_ptr().cast::<u8>();
		Frames {
			base: std::cell::Cell::new(base),
			words: std::cell::Cell::new(self.buf.len()),
			bounds: (base as usize, self.buf.len() * 8),
			_lifetime: std::marker::PhantomData,
		}
	}
}

/// The free frame space at one point of an evaluation. [`Self::claim`] splits
/// a node's own frame off the front and the claim carries the remainder, so a
/// node's inputs claim beyond its frame, one after another, and the space they
/// used is free again once the claim dies: the release is the claim's
/// lifetime, not a rewind contract. The cursor is shared through `&self` so
/// the lazy inputs a kernel holds claim beyond each other rather than over each
/// other. Covariant in `'e`, so a claim minted at the evaluation shortens onto
/// a derived context's arena lifetime.
pub struct Frames<'e> {
	base: std::cell::Cell<*mut u8>,
	words: std::cell::Cell<usize>,
	/// The whole buffer as (address, bytes), which stays fixed while claims
	/// advance the cursor, so a promote can range-check a reference against it.
	bounds: (usize, usize),
	_lifetime: std::marker::PhantomData<&'e ()>,
}

impl std::fmt::Debug for Frames<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Frames").field("free_words", &self.words.get()).finish()
	}
}

impl<'e> Frames<'e> {
	/// An independent cursor over the same free space: claims made through it
	/// are free again when it dies, so a repeated evaluation reuses one region.
	/// Two live cursors hand out the same region, so only one may be claimed
	/// from at a time.
	pub fn reborrow(&self) -> Frames<'e> {
		Frames {
			base: std::cell::Cell::new(self.base.get()),
			words: std::cell::Cell::new(self.words.get()),
			bounds: self.bounds,
			_lifetime: std::marker::PhantomData,
		}
	}

	/// The whole frame buffer as (address, bytes), the range a promote treats
	/// as evaluation-lived.
	pub fn bounds(&self) -> (usize, usize) {
		self.bounds
	}

	/// Runs claims against this space and gives it back on the guard's drop, so
	/// a loop that evaluates a subtree per iteration reuses the same region.
	pub fn scope(&self) -> FrameScope<'_, 'e> {
		FrameScope {
			frames: self,
			base: self.base.get(),
			words: self.words.get(),
		}
	}

	/// The words still free, the observable the frame accounting asserts on.
	pub fn free_words(&self) -> usize {
		self.words.get()
	}

	/// Claims `layout`'s frame at the front of the free space; the claim
	/// carries the remainder, and an inline layout's record builds in the
	/// claim itself.
	pub fn claim<'l>(&self, layout: &'l Layout) -> FrameClaim<'e, 'l> {
		let frame = match layout.frame_bytes() {
			0 => None,
			bytes => Some(self.split(bytes)),
		};
		FrameClaim {
			layout,
			inline: RecordValue::zeroed(),
			frame,
			free: self.reborrow(),
		}
	}

	/// Splits `bytes` (rounded to word alignment) off the front.
	fn split(&self, bytes: usize) -> *mut u8 {
		let words = bytes.div_ceil(8);
		debug_assert!(words <= self.words.get(), "record frame space exhausted: the root buffer must cover the graph's frame need");
		let frame = self.base.get();
		// SAFETY: the buffer covers the wiring-derived need, so the advanced
		// cursor stays within it.
		self.base.set(unsafe { frame.add(words * 8) });
		self.words.set(self.words.get() - words);
		frame
	}

	/// A run of same-layout slots over caller scratch: lanes serve in place,
	/// each backed by its own region of the slab, and the collected proofs
	/// certify the filled prefix. `None` where the scratch cannot hold `len`
	/// lanes.
	pub fn run<'a>(&self, scratch: &'a mut [std::mem::MaybeUninit<u64>], len: usize, layout: &'a Layout) -> Option<SlotRun<'a>> {
		SlotRun::new(scratch, len, layout)
	}
}

/// See [`Frames::scope`]. A forgotten guard leaks its region until the frame
/// space itself dies, rather than releasing it.
pub struct FrameScope<'s, 'e> {
	frames: &'s Frames<'e>,
	base: *mut u8,
	words: usize,
}

impl<'e> std::ops::Deref for FrameScope<'_, 'e> {
	type Target = Frames<'e>;

	fn deref(&self) -> &Frames<'e> {
		self.frames
	}
}

impl Drop for FrameScope<'_, '_> {
	fn drop(&mut self) {
		self.frames.base.set(self.base);
		self.frames.words.set(self.words);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::record::layout::element_write;
	use crate::record::test_support::f64_field;

	#[test]
	fn a_scope_releases_and_reuses_its_frames() {
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(1 << 10);
		let frames = frame_arena.frames();
		let free = frames.free_words();
		let mut addresses = Vec::new();
		for _ in 0..3 {
			let scope = frames.scope();
			let mut claim = scope.claim(&layout);
			addresses.push(claim.dst() as usize);
			drop(scope);
			assert_eq!(frames.free_words(), free, "the scope returns its claims");
		}
		assert!(addresses.windows(2).all(|pair| pair[0] == pair[1]), "each claim reuses the same region");
	}
}
