use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Handle word layout: 24 generation bits above 40 offset bits, so a 1 TiB arena is
/// addressable and generations run out after ~3 days of 60fps resets.
const OFFSET_BITS: u32 = 40;
const OFFSET_MASK: u64 = (1 << OFFSET_BITS) - 1;
const GENERATION_MASK: u64 = (1 << (64 - OFFSET_BITS)) - 1;

/// Out of the encodable range, so no handle, including `NULL`, matches it.
const PARKED_GENERATION: u64 = GENERATION_MASK + 1;

pub struct Arena {
	generation: AtomicU64,
	offset: AtomicUsize,
	buf: Box<[UnsafeCell<MaybeUninit<u8>>]>,
	drops: Mutex<Vec<DropEntry>>,
}

impl std::fmt::Debug for Arena {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Arena").field("generation", &self.generation).field("size", &self.buf.len()).finish()
	}
}

struct DropEntry {
	offset: usize,
	drop_fn: unsafe fn(*mut u8),
}

// SAFETY: disjoint regions are handed out by an atomic bump; a region is written
// only by its allocating caller before publication, and cross-thread hand-off is
// ordered by the Release/Acquire pair on the published handle word. Payloads are
// `Send + Sync` by the bound on every allocating method.
unsafe impl Sync for Arena {}
unsafe impl Send for Arena {}

impl std::panic::UnwindSafe for Arena {}
impl std::panic::RefUnwindSafe for Arena {}

/// Shared by all arenas, so a foreign handle misses like a stale one.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

static LIVE_ARENAS: AtomicUsize = AtomicUsize::new(0);

/// `None` past [`GENERATION_MASK`], where a reissued generation would let an ancient
/// handle upgrade against a current arena. Recovered by `reset_generation_counter`.
fn next_generation() -> Option<u64> {
	let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
	(generation <= GENERATION_MASK).then_some(generation)
}

/// Rewinds the shared generation counter so previously issued values are reused.
/// Returns `false` without rewinding when an [`Arena`] is live at the time of the
/// check, which is a debugging aid rather than a guarantee.
///
/// # Safety
///
/// No [`ArenaWeak`] minted before this call may be upgraded afterwards. Dropping
/// every [`Arena`] is not sufficient, since nodes also hold handles in
/// [`ArenaCell`]s; those nodes must be dropped too. No arena may be constructed
/// concurrently either, since the live check and the rewind are separate steps.
pub unsafe fn reset_generation_counter() -> bool {
	if LIVE_ARENAS.load(Ordering::Acquire) != 0 {
		return false;
	}
	NEXT_GENERATION.store(1, Ordering::Release);
	true
}

impl Arena {
	pub fn new(capacity: usize) -> Option<Self> {
		let generation = next_generation()?;
		let buf = (0..capacity).map(|_| UnsafeCell::new(MaybeUninit::uninit())).collect();
		LIVE_ARENAS.fetch_add(1, Ordering::Release);
		Some(Self {
			generation: AtomicU64::new(generation),
			offset: AtomicUsize::new(0),
			buf,
			drops: Mutex::new(Vec::new()),
		})
	}

	/// An arena that refuses every allocation and resolves no handle, so a caller that
	/// cannot fail can degrade instead of propagating exhaustion.
	pub fn parked() -> Self {
		LIVE_ARENAS.fetch_add(1, Ordering::Release);
		Self {
			generation: AtomicU64::new(PARKED_GENERATION),
			offset: AtomicUsize::new(0),
			buf: Box::new([]),
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

	pub fn alloc<T: Send + Sync>(&self, value: T) -> Option<(&T, ArenaWeak<T>)> {
		let offset = self.reserve(size_of::<T>(), align_of::<T>())?;
		// Built before the write so an unencodable offset drops `value` here
		// rather than stranding it in the arena without drop glue.
		let weak = ArenaWeak::new(self.generation(), offset)?;
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
		Some((unsafe { &*ptr }, weak))
	}

	pub fn alloc_slice_copy<T: Copy + Send + Sync>(&self, src: &[T]) -> Option<&[T]> {
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
	pub fn alloc_scratch<T: Send + Sync>(&self, len: usize) -> Option<&mut [MaybeUninit<T>]> {
		let size = size_of::<T>().checked_mul(len)?;
		let offset = self.reserve(size, align_of::<T>())?;
		let ptr = unsafe { self.base().add(offset) }.cast::<MaybeUninit<T>>();
		// SAFETY: exclusive region; lifetime tied to `&self`, and `reset` takes
		// `&mut self`, so the slice cannot outlive the generation.
		Some(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
	}

	/// `false` once generations are exhausted, parking the arena on [`PARKED_GENERATION`]
	/// where every handle misses and further allocation is refused.
	pub fn reset(&mut self) -> bool {
		let base = self.base();
		let entries = std::mem::take(self.drops.get_mut().unwrap());
		self.generation.store(PARKED_GENERATION, Ordering::Release);
		for entry in entries.into_iter().rev() {
			// SAFETY: registered at alloc time; insert-only means the region was
			// never overwritten within this generation.
			unsafe { (entry.drop_fn)(base.add(entry.offset)) }
		}
		*self.offset.get_mut() = 0;
		let Some(generation) = next_generation() else { return false };
		self.generation.store(generation, Ordering::Release);
		true
	}
}

impl Drop for Arena {
	fn drop(&mut self) {
		self.reset();
		LIVE_ARENAS.fetch_sub(1, Ordering::Release);
	}
}

pub struct ArenaWeak<T> {
	word: u64,
	_marker: PhantomData<fn() -> T>,
}

impl<T> Clone for ArenaWeak<T> {
	fn clone(&self) -> Self {
		*self
	}
}
impl<T> Copy for ArenaWeak<T> {}

impl<T> ArenaWeak<T> {
	pub const NULL: Self = ArenaWeak { word: 0, _marker: PhantomData };

	/// `None` once either field leaves its encodable range, so an oversized or parked
	/// arena refuses to hand out a handle rather than truncating it to a live address.
	fn new(generation: u64, offset: usize) -> Option<Self> {
		let offset = u64::try_from(offset).ok().filter(|offset| *offset <= OFFSET_MASK)?;
		(generation <= GENERATION_MASK).then_some(Self {
			word: (generation << OFFSET_BITS) | offset,
			_marker: PhantomData,
		})
	}

	pub fn upgrade(self, arena: &Arena) -> Option<&T> {
		let generation = self.word >> OFFSET_BITS;
		if generation != arena.generation() {
			return None;
		}
		let offset = (self.word & OFFSET_MASK) as usize;
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

impl<T> Clone for ArenaCell<T> {
	fn clone(&self) -> Self {
		Self {
			word: AtomicU64::new(self.word.load(Ordering::Acquire)),
			_marker: PhantomData,
		}
	}
}

impl<T> std::fmt::Debug for ArenaCell<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ArenaCell").field("word", &self.word.load(Ordering::Relaxed)).finish()
	}
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
	use std::sync::PoisonError;
	use std::sync::atomic::AtomicU32;

	#[test]
	fn alloc_upgrade_reset_miss() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let mut arena = Arena::new(1024).unwrap();
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
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let mut arena = Arena::new(64 + align_of::<u32>() - 1).unwrap();
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
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		fn assert_ref_unwind_safe<T: std::panic::RefUnwindSafe>() {}
		assert_ref_unwind_safe::<Arena>();

		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe(#[allow(dead_code)] String);
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		let mut arena = Arena::new(1024).unwrap();
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
	fn a_panicking_destructor_leaves_no_resolvable_handle() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		struct Bomb;
		impl Drop for Bomb {
			fn drop(&mut self) {
				panic!("payload destructor");
			}
		}
		let mut arena = Arena::new(1024).unwrap();
		let cell = ArenaCell::new();
		let (_, weak) = arena.alloc(Bomb).unwrap();
		cell.store(weak);

		let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| arena.reset()));
		assert!(unwound.is_err(), "the panic must propagate out of reset");
		assert!(cell.load(&arena).is_none(), "a half-dropped generation must resolve no handle");
	}

	/// Held by every test that perturbs [`NEXT_GENERATION`], so a swapped-out counter
	/// is never observed by a concurrently constructing test.
	static COUNTER_GUARD: Mutex<()> = Mutex::new(());

	#[test]
	fn an_exhausted_reset_parks_the_arena_and_refuses_handles() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let mut arena = Arena::new(1024).unwrap();
		let (_, weak) = arena.alloc(41u32).unwrap();

		let restore = NEXT_GENERATION.swap(GENERATION_MASK + 1, Ordering::Relaxed);
		assert!(!arena.reset(), "an exhausted counter must report failure");
		NEXT_GENERATION.store(restore, Ordering::Relaxed);

		assert_eq!(weak.upgrade(&arena), None, "a parked arena resolves no handle");
		assert_eq!(ArenaWeak::<u32>::NULL.upgrade(&arena), None, "not even the null handle");
		assert!(arena.alloc(0u32).is_none(), "a parked arena refuses allocation");
	}

	#[test]
	fn the_generation_counter_rewinds_only_without_live_arenas() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let arena = Arena::new(64).unwrap();
		assert!(!unsafe { reset_generation_counter() }, "a live arena must block the rewind");
		drop(arena);
	}

	#[test]
	fn handles_do_not_upgrade_against_a_foreign_arena() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let first = Arena::new(1024).unwrap();
		let second = Arena::new(1024).unwrap();
		let (_, weak) = first.alloc(41u32).unwrap();
		assert_eq!(weak.upgrade(&first), Some(&41));
		assert_eq!(weak.upgrade(&second), None, "a handle must not resolve against another arena");
		assert_eq!(ArenaWeak::<u32>::NULL.upgrade(&second), None, "the null handle never upgrades");
	}

	#[test]
	fn reset_drops_dependents_before_their_dependencies() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		static ORDER: Mutex<Vec<u32>> = Mutex::new(Vec::new());
		struct Probe(u32);
		impl Drop for Probe {
			fn drop(&mut self) {
				ORDER.lock().unwrap().push(self.0);
			}
		}
		let mut arena = Arena::new(1024).unwrap();
		for id in 0..3 {
			arena.alloc(Probe(id)).unwrap();
		}
		arena.reset();
		assert_eq!(*ORDER.lock().unwrap(), vec![2, 1, 0], "later allocations may borrow earlier ones, so they drop first");
	}

	#[test]
	fn drop_glue_runs_on_reset() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe(#[allow(dead_code)] String);
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		let mut arena = Arena::new(1024).unwrap();
		arena.alloc(Probe("owns heap".into())).unwrap();
		arena.alloc(Probe("me too".into())).unwrap();
		assert_eq!(DROPS.load(Ordering::Relaxed), 0);
		arena.reset();
		assert_eq!(DROPS.load(Ordering::Relaxed), 2);
	}
}
