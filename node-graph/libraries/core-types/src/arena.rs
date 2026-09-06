use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

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
	/// Set by a refused reservation and cleared by [`Arena::reset`], so a region
	/// no evaluation resets can be seen to need one.
	exhausted: AtomicBool,
	/// Heap the parked payloads keep alive, which occupancy does not measure:
	/// a park costs one pointer in the arena and owns its content outside it.
	retained_heap: AtomicUsize,
	/// Where [`Arena::move_park`] sent each moved park, as its offset here to
	/// the header's address in the receiving arena and the moved type's key, so
	/// a payload two records share is moved once and a mistyped sharer is
	/// refused. Cleared by [`Arena::reset`], so a forwarding holds for one
	/// generation.
	forwarded: Mutex<HashMap<usize, (usize, TypeId)>>,
}

impl std::fmt::Debug for Arena {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Arena").field("generation", &self.generation).field("size", &self.buf.len()).finish()
	}
}

struct DropEntry {
	offset: usize,
	/// The parked payload's static-type key, `None` where the park was
	/// allocated without one and so never moves.
	type_of: Option<TypeId>,
	drop_fn: unsafe fn(*mut u8),
	/// The park glue's estimate of the heap this payload owns, 0 where the
	/// glue cannot measure it, so the counter is a lower bound.
	retained: usize,
}

/// The glue a tombstoned entry carries: its payload was moved to another arena,
/// which now owns the obligation.
unsafe fn inert(_: *mut u8) {}

/// The entry parking `offset`, `None` where the arena holds no obligation for
/// it. Entries are pushed in reserve order, which is offset order, so the
/// search is exact; the scan covers a push order that concurrency interleaved.
fn entry_at(entries: &[DropEntry], offset: usize) -> Option<usize> {
	let probe = entries.partition_point(|entry| entry.offset < offset);
	match entries.get(probe).is_some_and(|entry| entry.offset == offset) {
		true => Some(probe),
		false => entries.iter().rposition(|entry| entry.offset == offset),
	}
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
			exhausted: AtomicBool::new(false),
			retained_heap: AtomicUsize::new(0),
			forwarded: Mutex::new(HashMap::new()),
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
			exhausted: AtomicBool::new(false),
			retained_heap: AtomicUsize::new(0),
			forwarded: Mutex::new(HashMap::new()),
		}
	}

	/// Whether a reservation has been refused since the last [`Arena::reset`].
	pub fn exhausted(&self) -> bool {
		self.exhausted.load(Ordering::Relaxed)
	}

	pub fn generation(&self) -> u64 {
		self.generation.load(Ordering::Acquire)
	}

	/// Bytes handed out since the last [`Arena::reset`], including alignment
	/// padding, so a caller can flush at a boundary before a refusal.
	pub fn occupancy(&self) -> usize {
		self.offset.load(Ordering::Relaxed)
	}

	pub fn capacity(&self) -> usize {
		self.buf.len()
	}

	/// The heap the parked payloads own, summed from the park glue's hints.
	/// A lower bound: glue that cannot measure its payload contributes 0.
	pub fn retained_heap(&self) -> usize {
		self.retained_heap.load(Ordering::Relaxed)
	}

	/// Whether `ptr` addresses this arena's backbone, which is the provenance
	/// question a promote asks of every parked reference.
	pub fn contains(&self, ptr: *const u8) -> bool {
		let base = self.base() as usize;
		(ptr as usize).wrapping_sub(base) < self.buf.len()
	}

	fn base(&self) -> *mut u8 {
		self.buf.as_ptr() as *mut u8
	}

	fn reserve(&self, size: usize, align: usize) -> Option<usize> {
		debug_assert!(align.is_power_of_two());
		let base = self.base() as usize;
		let mut start = 0;
		let reserved = self
			.offset
			.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
				// Alignment is computed on the absolute address; the backbone
				// allocation itself has no alignment guarantee.
				let addr = (base.checked_add(current)?.checked_add(align - 1)?) & !(align - 1);
				start = addr - base;
				let end = start.checked_add(size)?;
				(end <= self.buf.len()).then_some(end)
			})
			.ok();
		#[cfg(debug_assertions)]
		{
			static ARENA_TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
			if *ARENA_TRACE.get_or_init(|| std::env::var_os("GRAPHENE_ARENA_DEBUG").is_some()) {
				static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
				let count = COUNT.fetch_add(1, Ordering::Relaxed);
				if reserved.is_none() {
					eprintln!("arena> EXHAUSTED after {count} allocations, wanted {size} bytes\n{}", std::backtrace::Backtrace::force_capture());
				} else if count.is_multiple_of(20000) {
					eprintln!("arena> {count} allocations, offset {}, this {size} bytes", self.offset.load(Ordering::Relaxed));
				}
			}
		}
		if reserved.is_none() {
			self.exhausted.store(true, Ordering::Relaxed);
			return None;
		}
		Some(start)
	}

	pub fn alloc<T: Send + Sync>(&self, value: T) -> Option<(&T, ArenaWeak<T>)> {
		self.alloc_sized(value, 0)
	}

	/// [`Arena::alloc`] with the park glue's estimate of the heap `value` owns,
	/// which the region's own occupancy cannot see.
	pub fn alloc_sized<T: Send + Sync>(&self, value: T, retained: usize) -> Option<(&T, ArenaWeak<T>)> {
		self.alloc_stamped(value, retained, None)
	}

	/// [`Arena::alloc_sized`] stamping the park's static-type key, which is what
	/// [`Arena::move_park`] matches on, so only a park allocated here can move.
	pub fn alloc_sized_keyed<T: Send + Sync + dyn_any::StaticTypeSized>(&self, value: T, retained: usize) -> Option<(&T, ArenaWeak<T>)> {
		self.alloc_stamped(value, retained, Some(TypeId::of::<T::Static>()))
	}

	/// [`Arena::alloc_sized_keyed`] for park glue already holding the element's
	/// static form, which projects the key from the element type instead of
	/// from the value's own. Crate-private: a wrong key mistypes a move.
	pub(crate) fn alloc_sized_as<T: Send + Sync>(&self, value: T, retained: usize, type_of: TypeId) -> Option<(&T, ArenaWeak<T>)> {
		self.alloc_stamped(value, retained, Some(type_of))
	}

	fn alloc_stamped<T: Send + Sync>(&self, value: T, retained: usize, type_of: Option<TypeId>) -> Option<(&T, ArenaWeak<T>)> {
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
			self.drops.lock().unwrap().push(DropEntry { offset, type_of, drop_fn: glue::<T>, retained });
			self.retained_heap.fetch_add(retained, Ordering::Relaxed);
		}
		// SAFETY: initialized above; insert-only, so no `&mut` to it can exist.
		Some((unsafe { &*ptr }, weak))
	}

	/// Moves the payload at `src` out of this arena and into `dst`: the header's
	/// own bytes are copied, its drop obligation is pushed onto `dst` with the
	/// `retained` hint, and the entry here is tombstoned in place, its glue
	/// swapped for [`inert`] so the reset costs nothing per entry to honour it.
	/// The heap the payload owns is neither copied nor freed; ownership travels
	/// with the obligation and is discharged by `dst`'s flush.
	///
	/// THE MOVE'S OWNERSHIP CONTRACT: the source header keeps its bytes and
	/// stays readable to this generation's remaining sharers, but it owns
	/// nothing, so no read of it may outlive `dst`'s flush. A payload moved
	/// once is not moved again: its recorded destination is returned, so a
	/// fan-out sharer's second move reaches the one header and the obligation
	/// is never duplicated.
	///
	/// `dst` is credited `retained`, which is what a clone of the payload would
	/// have credited it, and this arena is debited what its own park recorded,
	/// so neither counter reads worse than it did before the move.
	///
	/// The move republishes the payload at `T::Static`, which is the key both
	/// [`Arena::alloc_sized_keyed`] and this stamp, so a park and its move
	/// project the type exactly once each and agree by construction.
	///
	/// `None` where `src` is not this arena's park keyed to `T::Static`, where
	/// the type is zero-sized (whose parks share an offset and so cannot be
	/// told apart), or where `dst` refused the header.
	///
	/// # Safety
	/// `src` must address a live `T`, and `T` must own all of its content: the
	/// moved header may reference no storage of this arena, which is also what
	/// lets the payload be republished at `T::Static`. The key settles the
	/// type, so the caller owes no size or identity argument beyond those two.
	pub unsafe fn move_park<T: dyn_any::StaticTypeSized>(&self, src: *const u8, dst: &Arena, retained: usize) -> Option<*const T::Static>
	where
		T::Static: Send + Sync,
	{
		(size_of::<T::Static>() != 0).then_some(())?;
		let offset = (src as usize).checked_sub(self.base() as usize)?;
		(offset < self.buf.len()).then_some(())?;
		let type_of = TypeId::of::<T::Static>();
		let mut forwarded = self.forwarded.lock().unwrap();
		if let Some(&(moved, moved_type)) = forwarded.get(&offset) {
			(moved_type == type_of).then_some(())?;
			return Some(moved as *const T::Static);
		}
		let mut entries = self.drops.lock().unwrap();
		let entry = entry_at(&entries, offset)?;
		(entries[entry].type_of == Some(type_of)).then_some(())?;
		let slot = dst.alloc_scratch::<T::Static>(1)?;
		let target = slot.as_mut_ptr().cast::<T::Static>();
		// SAFETY: the caller's contract on `src`, into a freshly reserved,
		// aligned, unaliased slot of the same type.
		unsafe { std::ptr::copy_nonoverlapping(src.cast::<T::Static>(), target, 1) };
		let parked = std::mem::replace(&mut entries[entry].retained, 0);
		entries[entry].drop_fn = inert;
		drop(entries);
		unsafe fn glue<T>(p: *mut u8) {
			unsafe { p.cast::<T>().drop_in_place() }
		}
		dst.drops.lock().unwrap().push(DropEntry {
			offset: target as usize - dst.base() as usize,
			type_of: Some(type_of),
			drop_fn: glue::<T::Static>,
			retained,
		});
		dst.retained_heap.fetch_add(retained, Ordering::Relaxed);
		let _ = self.retained_heap.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| Some(current.saturating_sub(parked)));
		forwarded.insert(offset, (target as usize, type_of));
		Some(target.cast_const())
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

	/// The generation-checked handle for a region this arena holds, `None` for
	/// a pointer from anywhere else. The handle keeps the region's provenance,
	/// so a cache stores one where it would otherwise launder an address.
	pub fn handle_at(&self, ptr: *const u8) -> Option<ArenaWeak<u8>> {
		let offset = (ptr as usize).checked_sub(self.base() as usize)?;
		(offset < self.buf.len()).then_some(())?;
		ArenaWeak::new(self.generation(), offset)
	}

	/// `false` once generations are exhausted, parking the arena on [`PARKED_GENERATION`]
	/// where every handle misses and further allocation is refused.
	///
	/// A park [`Arena::move_park`] handed to another arena carries [`inert`]
	/// glue and a zeroed hint, so it costs the loop nothing beyond the call it
	/// would have made anyway.
	pub fn reset(&mut self) -> bool {
		let base = self.base();
		let entries = std::mem::take(self.drops.get_mut().unwrap());
		self.forwarded.get_mut().unwrap().clear();
		self.generation.store(PARKED_GENERATION, Ordering::Release);
		for entry in entries.into_iter().rev() {
			// SAFETY: registered at alloc time; insert-only means the region was
			// never overwritten within this generation.
			unsafe { (entry.drop_fn)(base.add(entry.offset)) }
			// Decremented as each payload's heap is freed, so an unwinding reset
			// leaves the counter matching what is still parked.
			let retained = self.retained_heap.get_mut();
			*retained = retained.saturating_sub(entry.retained);
		}
		*self.offset.get_mut() = 0;
		*self.exhausted.get_mut() = false;
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
	fn sized_parks_account_for_their_retained_heap() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let mut arena = Arena::new(1024).unwrap();
		assert_eq!(arena.retained_heap(), 0);

		let owned = String::from("retained by the park");
		let length = owned.len();
		arena.alloc_sized(owned, length).unwrap();
		assert_eq!(arena.retained_heap(), length, "the park's hint reaches the counter");

		arena.alloc(String::from("unmeasured")).unwrap();
		assert_eq!(arena.retained_heap(), length, "an unmeasured park contributes nothing");

		arena.reset();
		assert_eq!(arena.retained_heap(), 0, "the flush frees every parked payload");
	}

	#[test]
	fn a_moved_park_carries_its_heap_and_its_obligation() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let mut transient = Arena::new(1024).unwrap();
		let mut persistent = Arena::new(1024).unwrap();

		let owned = String::from("moved, never cloned");
		let length = owned.len();
		let heap = owned.as_ptr();
		let (parked, _) = transient.alloc_sized_keyed(owned, length).unwrap();
		let src = std::ptr::from_ref(parked).cast::<u8>();

		let moved = unsafe { transient.move_park::<String>(src, &persistent, length) }.unwrap();
		assert_eq!(unsafe { &*moved }.as_ptr(), heap, "the move copies the header and leaves the heap where it was");
		assert!(persistent.contains(moved.cast()), "the header lands in the receiving arena");
		assert_eq!(transient.retained_heap(), 0, "the parking arena gives the hint up");
		assert_eq!(persistent.retained_heap(), length, "and the receiving arena takes it");

		transient.reset();
		assert_eq!(unsafe { &*moved }, "moved, never cloned", "the parking arena's reset leaves a moved payload alone");
		persistent.reset();
		assert_eq!(persistent.retained_heap(), 0, "the receiving flush frees it");
	}

	#[test]
	fn a_park_two_records_share_moves_once_and_frees_once() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe(#[allow(dead_code)] String);
		unsafe impl dyn_any::StaticType for Probe {
			type Static = Probe;
		}
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		DROPS.store(0, Ordering::Relaxed);
		let mut transient = Arena::new(1024).unwrap();
		let mut persistent = Arena::new(1024).unwrap();

		let (parked, _) = transient.alloc_sized_keyed(Probe(String::from("shared by two records")), 21).unwrap();
		let src = std::ptr::from_ref(parked).cast::<u8>();
		let first = unsafe { transient.move_park::<Probe>(src, &persistent, 21) }.unwrap();
		let second = unsafe { transient.move_park::<Probe>(src, &persistent, 21) }.unwrap();
		assert_eq!(first, second, "the second move forwards to the header the first wrote");
		assert_eq!(persistent.retained_heap(), 21, "the hint transfers once, not once per sharer");

		transient.reset();
		assert_eq!(DROPS.load(Ordering::Relaxed), 0, "a tombstoned entry is skipped by its arena's reset");
		persistent.reset();
		assert_eq!(DROPS.load(Ordering::Relaxed), 1, "the flush that owns the obligation frees it exactly once");
	}

	#[test]
	fn a_move_refuses_a_mistyped_park_and_its_forwarding() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		struct Owner(#[allow(dead_code)] String);
		unsafe impl dyn_any::StaticType for Owner {
			type Static = Owner;
		}
		/// Owner's layout exactly, so only the type key tells the two apart.
		struct Twin(#[allow(dead_code)] String);
		unsafe impl dyn_any::StaticType for Twin {
			type Static = Twin;
		}
		let mut transient = Arena::new(1024).unwrap();
		let mut persistent = Arena::new(1024).unwrap();

		let (parked, _) = transient.alloc_sized_keyed(Owner(String::from("a keyed park")), 0).unwrap();
		let src = std::ptr::from_ref(parked).cast::<u8>();
		assert!(unsafe { transient.move_park::<Twin>(src, &persistent, 0) }.is_none(), "a park of another type of the same size is refused");
		unsafe { transient.move_park::<Owner>(src, &persistent, 0) }.unwrap();
		assert!(unsafe { transient.move_park::<Twin>(src, &persistent, 0) }.is_none(), "the forwarding refuses the same mistype");

		let (unkeyed, _) = transient.alloc(Owner(String::from("an unkeyed park"))).unwrap();
		let src = std::ptr::from_ref(unkeyed).cast::<u8>();
		assert!(unsafe { transient.move_park::<Owner>(src, &persistent, 0) }.is_none(), "a park allocated without a key never moves");

		transient.reset();
		persistent.reset();
	}

	#[test]
	fn the_forwarding_map_lives_one_generation() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe(#[allow(dead_code)] String);
		unsafe impl dyn_any::StaticType for Probe {
			type Static = Probe;
		}
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		DROPS.store(0, Ordering::Relaxed);
		let mut transient = Arena::new(1024).unwrap();
		let mut persistent = Arena::new(1024).unwrap();

		let (parked, _) = transient.alloc_sized_keyed(Probe(String::from("first generation")), 16).unwrap();
		let src = std::ptr::from_ref(parked).cast::<u8>();
		unsafe { transient.move_park::<Probe>(src, &persistent, 16) }.unwrap();
		assert_eq!(transient.forwarded.lock().unwrap().len(), 1, "the move tombstoned the entry it left");

		transient.reset();
		assert!(transient.forwarded.lock().unwrap().is_empty(), "the tombstone set is empty after the reset");

		transient.alloc_sized(Probe(String::from("second generation")), 16).unwrap();
		transient.reset();
		assert_eq!(DROPS.load(Ordering::Relaxed), 1, "a fresh park at a tombstoned offset drops normally");
		persistent.reset();
		assert_eq!(DROPS.load(Ordering::Relaxed), 2);
	}

	#[test]
	fn occupancy_tracks_the_bump_and_clears_on_reset() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let mut arena = Arena::new(1024).unwrap();
		assert_eq!(arena.occupancy(), 0);
		assert_eq!(arena.capacity(), 1024);
		arena.alloc(0u64).unwrap();
		assert_eq!(arena.occupancy(), 8);
		arena.reset();
		assert_eq!(arena.occupancy(), 0);
	}

	#[test]
	fn containment_answers_only_for_this_arena() {
		let _guard = COUNTER_GUARD.lock().unwrap_or_else(PoisonError::into_inner);
		let first = Arena::new(1024).unwrap();
		let second = Arena::new(1024).unwrap();
		let (value, _) = first.alloc(41u32).unwrap();
		let ptr = std::ptr::from_ref(value).cast::<u8>();
		assert!(first.contains(ptr));
		assert!(!second.contains(ptr));
		assert!(!first.contains(std::ptr::null()));
		let heap = Box::new(7u32);
		assert!(!first.contains(std::ptr::from_ref(&*heap).cast::<u8>()), "heap the arena does not back is outside it");
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
