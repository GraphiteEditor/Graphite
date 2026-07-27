use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub struct Arena {
	generation: AtomicU64,
	offset: AtomicUsize,
	buf: Box<[UnsafeCell<MaybeUninit<u8>>]>,
	drops: Mutex<Vec<DropEntry>>,
}

struct DropEntry {
	offset: usize,
	drop_fn: unsafe fn(*mut u8),
}

// SAFETY: disjoint regions are handed out by an atomic bump; a region is written
// only by its allocating caller before publication, and cross-thread hand-off is
// ordered by the Release/Acquire pair on the published handle word.
unsafe impl Sync for Arena {}
unsafe impl Send for Arena {}

impl std::panic::UnwindSafe for Arena {}
impl std::panic::RefUnwindSafe for Arena {}

impl Arena {
	pub fn new(capacity: usize) -> Self {
		let buf = (0..capacity).map(|_| UnsafeCell::new(MaybeUninit::uninit())).collect();
		Self {
			// Starts at 1 so the null handle word (0) can never upgrade.
			generation: AtomicU64::new(1),
			offset: AtomicUsize::new(0),
			buf,
			drops: Mutex::new(Vec::new()),
		}
	}

	pub fn generation(&self) -> u64 {
		self.generation.load(Ordering::Acquire)
	}

	fn base(&self) -> *mut u8 {
		self.buf.as_ptr() as *mut u8
	}

	fn reserve(&self, size: usize, align: usize) -> Option<usize> {
		debug_assert!(align.is_power_of_two());
		let base = self.base() as usize;
		let mut start = 0;
		self.offset
			.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
				// Alignment is computed on the absolute address; the backbone
				// allocation itself has no alignment guarantee.
				let addr = (base.checked_add(current)?.checked_add(align - 1)?) & !(align - 1);
				start = addr - base;
				let end = start.checked_add(size)?;
				(end <= self.buf.len()).then_some(end)
			})
			.ok()?;
		Some(start)
	}

	pub fn alloc<T>(&self, value: T) -> Option<(&T, ArenaWeak<T>)> {
		let offset = self.reserve(size_of::<T>(), align_of::<T>())?;
		let ptr = unsafe { self.base().add(offset) }.cast::<T>();
		// SAFETY: freshly reserved, aligned, in-bounds, unaliased.
		unsafe { ptr.write(value) };
		if std::mem::needs_drop::<T>() {
			unsafe fn glue<T>(p: *mut u8) {
				unsafe { p.cast::<T>().drop_in_place() }
			}
			self.drops.lock().unwrap().push(DropEntry { offset, drop_fn: glue::<T> });
		}
		// SAFETY: initialized above; insert-only, so no `&mut` to it can exist.
		Some((unsafe { &*ptr }, ArenaWeak::new(self.generation(), offset)))
	}

	pub fn alloc_slice_copy<T: Copy>(&self, src: &[T]) -> Option<&[T]> {
		let buf = self.alloc_scratch::<T>(src.len())?;
		for (slot, &value) in buf.iter_mut().zip(src) {
			slot.write(value);
		}
		// SAFETY: every lane written above from `src`.
		Some(unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<T>(), src.len()) })
	}

	// Not drop-tracked: callers must either consume every written lane or
	// restrict themselves to `Copy` payloads (leak, not UB, otherwise).
	#[allow(clippy::mut_from_ref)]
	pub fn alloc_scratch<T>(&self, len: usize) -> Option<&mut [MaybeUninit<T>]> {
		let size = size_of::<T>().checked_mul(len)?;
		let offset = self.reserve(size, align_of::<T>())?;
		let ptr = unsafe { self.base().add(offset) }.cast::<MaybeUninit<T>>();
		// SAFETY: exclusive region; lifetime tied to `&self`, and `reset` takes
		// `&mut self`, so the slice cannot outlive the generation.
		Some(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
	}

	pub fn reset(&mut self) {
		let base = self.base();
		for entry in self.drops.get_mut().unwrap().drain(..) {
			// SAFETY: registered at alloc time; insert-only means the region was
			// never overwritten within this generation.
			unsafe { (entry.drop_fn)(base.add(entry.offset)) }
		}
		*self.offset.get_mut() = 0;
		self.generation.fetch_add(1, Ordering::Release);
	}
}

impl Drop for Arena {
	fn drop(&mut self) {
		self.reset();
	}
}

pub struct ArenaWeak<T> {
	word: u64,
	_marker: PhantomData<*const T>,
}

impl<T> Clone for ArenaWeak<T> {
	fn clone(&self) -> Self {
		*self
	}
}
impl<T> Copy for ArenaWeak<T> {}

impl<T> ArenaWeak<T> {
	pub const NULL: Self = ArenaWeak { word: 0, _marker: PhantomData };

	fn new(generation: u64, offset: usize) -> Self {
		debug_assert!(offset < u32::MAX as usize);
		Self {
			word: ((generation & 0xFFFF_FFFF) << 32) | offset as u64,
			_marker: PhantomData,
		}
	}

	pub fn upgrade(self, arena: &Arena) -> Option<&T> {
		let generation = self.word >> 32;
		if generation != arena.generation() & 0xFFFF_FFFF {
			return None;
		}
		let offset = (self.word & 0xFFFF_FFFF) as usize;
		// SAFETY: same generation means the entry was fully written before its
		// word was published (Release) and cannot move or be overwritten within
		// a generation (insert-only); the Acquire load that produced this word
		// ordered the payload writes.
		Some(unsafe { &*arena.base().add(offset).cast::<T>() })
	}
}

pub struct ArenaCell<T> {
	word: AtomicU64,
	_marker: PhantomData<fn() -> T>,
}

impl<T> Default for ArenaCell<T> {
	fn default() -> Self {
		Self {
			word: AtomicU64::new(0),
			_marker: PhantomData,
		}
	}
}

impl<T> ArenaCell<T> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn load<'e>(&self, arena: &'e Arena) -> Option<&'e T> {
		let weak = ArenaWeak::<T> {
			word: self.word.load(Ordering::Acquire),
			_marker: PhantomData,
		};
		weak.upgrade(arena)
	}

	pub fn store(&self, weak: ArenaWeak<T>) {
		self.word.store(weak.word, Ordering::Release);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::AtomicU32;

	#[test]
	fn alloc_upgrade_reset_miss() {
		let mut arena = Arena::new(1024);
		let cell = ArenaCell::new();
		let (value, weak) = arena.alloc(41u32).unwrap();
		assert_eq!(*value, 41);
		cell.store(weak);
		assert_eq!(cell.load(&arena), Some(&41));
		arena.reset();
		assert_eq!(cell.load(&arena), None, "stale handle must miss");
	}

	#[test]
	fn capacity_survives_reset() {
		let mut arena = Arena::new(64 + align_of::<u32>() - 1);
		for _ in 0..10 {
			for _ in 0..16 {
				assert!(arena.alloc(0u32).is_some());
			}
			assert!(arena.alloc(0u32).is_none(), "exhausted within generation");
			arena.reset();
		}
	}

	#[test]
	fn panics_leave_the_arena_coherent() {
		fn assert_ref_unwind_safe<T: std::panic::RefUnwindSafe>() {}
		assert_ref_unwind_safe::<Arena>();

		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe(#[allow(dead_code)] String);
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		let mut arena = Arena::new(1024);
		let cell = ArenaCell::new();
		let result = std::panic::catch_unwind(|| {
			let (_, weak) = arena.alloc(Probe("pre-panic".into())).unwrap();
			cell.store(weak);
			panic!("mid-eval");
		});
		assert!(result.is_err());
		assert!(cell.load(&arena).is_some(), "the generation is still live after the caught panic");
		arena.reset();
		assert_eq!(DROPS.load(Ordering::Relaxed), 1, "reset reclaims pre-panic allocations");
		assert!(cell.load(&arena).is_none(), "the bump kills stale handles");
		assert!(arena.alloc(0u32).is_some(), "the arena stays usable");
	}

	#[test]
	fn drop_glue_runs_on_reset() {
		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe(#[allow(dead_code)] String);
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		let mut arena = Arena::new(1024);
		arena.alloc(Probe("owns heap".into())).unwrap();
		arena.alloc(Probe("me too".into())).unwrap();
		assert_eq!(DROPS.load(Ordering::Relaxed), 0);
		arena.reset();
		assert_eq!(DROPS.load(Ordering::Relaxed), 2);
	}
}
