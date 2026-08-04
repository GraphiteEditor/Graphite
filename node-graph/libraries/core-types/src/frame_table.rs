use crate::gpoll::Finality;
use std::cell::UnsafeCell;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

const SLOT_EMPTY: u8 = 0;
const SLOT_FINAL: u8 = 1;
const SLOT_PARTIAL: u8 = 2;

pub struct FrameTable<T, const CAP: usize> {
	slots: [FrameSlot<T>; CAP],
}

struct FrameSlot<T> {
	key: AtomicU64,
	state: AtomicU8,
	value: UnsafeCell<MaybeUninit<T>>,
}

pub enum Lookup<'t, T> {
	Hit(Finality, &'t T),
	Vacant(VacantSlot<'t, T>),
	Full,
}

pub struct VacantSlot<'t, T> {
	slot: &'t FrameSlot<T>,
}

impl<T, const CAP: usize> Default for FrameTable<T, CAP> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T, const CAP: usize> FrameTable<T, CAP> {
	pub fn new() -> Self {
		Self {
			slots: std::array::from_fn(|_| FrameSlot {
				key: AtomicU64::new(0),
				state: AtomicU8::new(SLOT_EMPTY),
				value: UnsafeCell::new(MaybeUninit::uninit()),
			}),
		}
	}

	pub fn lookup(&self, hash: u64) -> Lookup<'_, T> {
		// Only hash 0 is remapped, so distinct hashes stay distinct keys.
		let key = if hash == 0 { 1 } else { hash };
		for probe in 0..CAP {
			let slot = &self.slots[(key as usize).wrapping_add(probe) % CAP];
			let stored = slot.key.load(Ordering::Acquire);
			if stored == key {
				return match slot.state.load(Ordering::Acquire) {
					// SAFETY: a published state was stored with Release after the
					// value write; the Acquire load above ordered that write.
					SLOT_FINAL => Lookup::Hit(Finality::AllFinal, unsafe { (*slot.value.get()).assume_init_ref() }),
					SLOT_PARTIAL => Lookup::Hit(Finality::Partial, unsafe { (*slot.value.get()).assume_init_ref() }),
					_ => Lookup::Full,
				};
			}
			if stored == 0 && slot.key.compare_exchange(0, key, Ordering::AcqRel, Ordering::Acquire).is_ok() {
				return Lookup::Vacant(VacantSlot { slot });
			}
		}
		Lookup::Full
	}
}

impl<T, const CAP: usize> Drop for FrameTable<T, CAP> {
	fn drop(&mut self) {
		for slot in &self.slots {
			if slot.state.load(Ordering::Acquire) != SLOT_EMPTY {
				// SAFETY: a non-empty state is only ever stored after the value
				// write in `publish`.
				unsafe { (*slot.value.get()).assume_init_drop() }
			}
		}
	}
}

impl<'t, T> VacantSlot<'t, T> {
	pub fn publish(self, value: T, finality: Finality) -> &'t T {
		let slot = ManuallyDrop::new(self).slot;
		// SAFETY: the CAS in `lookup` reserved this slot exclusively for us and
		// its state is still SLOT_EMPTY, so nobody reads the value yet.
		let lent = unsafe { &*(*slot.value.get()).write(value) };
		let state = match finality {
			Finality::AllFinal => SLOT_FINAL,
			Finality::Partial => SLOT_PARTIAL,
		};
		slot.state.store(state, Ordering::Release);
		lent
	}

	pub fn release(self) {
		drop(self);
	}
}

/// Frees the reservation, so an early return or panic between `lookup` and
/// `publish` cannot retire the slot for the rest of the table's life.
impl<T> Drop for VacantSlot<'_, T> {
	fn drop(&mut self) {
		self.slot.key.store(0, Ordering::Release);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::AtomicU32;

	#[test]
	fn publish_then_hit_with_finality() {
		let table = FrameTable::<u32, 8>::new();
		let Lookup::Vacant(slot) = table.lookup(7) else {
			panic!("fresh table must be vacant");
		};
		assert_eq!(*slot.publish(41, Finality::Partial), 41);
		let Lookup::Hit(finality, value) = table.lookup(7) else {
			panic!("published key must hit");
		};
		assert_eq!((finality, *value), (Finality::Partial, 41));
	}

	#[test]
	fn released_slot_is_vacant_again() {
		let table = FrameTable::<u32, 8>::new();
		let Lookup::Vacant(slot) = table.lookup(7) else { unreachable!() };
		slot.release();
		assert!(matches!(table.lookup(7), Lookup::Vacant(_)));
	}

	#[test]
	fn a_dropped_reservation_frees_the_slot() {
		let table = FrameTable::<u32, 8>::new();
		let Lookup::Vacant(slot) = table.lookup(7) else { unreachable!() };
		drop(slot);
		assert!(matches!(table.lookup(7), Lookup::Vacant(_)), "an abandoned reservation must not retire the slot");
	}

	#[test]
	fn neighboring_hashes_do_not_share_an_entry() {
		let table = FrameTable::<u32, 8>::new();
		let Lookup::Vacant(slot) = table.lookup(6) else { unreachable!() };
		slot.publish(600, Finality::AllFinal);
		assert!(matches!(table.lookup(7), Lookup::Vacant(_)), "an even hash must not answer for its odd neighbor");
	}

	#[test]
	fn the_zero_hash_round_trips() {
		let table = FrameTable::<u32, 8>::new();
		let Lookup::Vacant(slot) = table.lookup(0) else { unreachable!() };
		slot.publish(11, Finality::AllFinal);
		let Lookup::Hit(_, value) = table.lookup(0) else {
			panic!("the remapped sentinel hash must still hit");
		};
		assert_eq!(*value, 11);
	}

	#[test]
	fn distinct_keys_probe_past_collisions_until_full() {
		let table = FrameTable::<u32, 4>::new();
		for hash in [2, 4, 6, 8] {
			let Lookup::Vacant(slot) = table.lookup(hash) else {
				panic!("hash {hash} should find a vacant slot");
			};
			slot.publish(hash as u32, Finality::AllFinal);
		}
		assert!(matches!(table.lookup(100), Lookup::Full));
	}

	#[test]
	fn drop_runs_glue_for_published_values_only() {
		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe;
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		let table = FrameTable::<Probe, 8>::new();
		let Lookup::Vacant(slot) = table.lookup(1) else { unreachable!() };
		slot.publish(Probe, Finality::AllFinal);
		let Lookup::Vacant(reserved_but_unpublished) = table.lookup(2) else { unreachable!() };
		reserved_but_unpublished.release();
		drop(table);
		assert_eq!(DROPS.load(Ordering::Relaxed), 1);
	}
}
