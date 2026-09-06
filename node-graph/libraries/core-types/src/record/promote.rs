//! Transient-to-persistent promotion of records and their parked payloads.

use super::layout::Layout;
use super::owned::{deepen_field_value, replay_field_value};
use super::serve::MaterializedSpan;

/// The regions a promote dispatches on. A payload already living in the
/// persistent region outlives every entry promoted into it and is shared; a
/// payload in the transient arena or the frame buffer dies at the next reset
/// and is cloned.
///
/// The dispatch is decidable only where the reference addresses the payload
/// itself, which holds for parked elements and for a group interior's frames.
/// A reference-valued attribute instead names storage whose owner the address
/// alone does not identify, so those are never provenance-shared; they clone,
/// or move where [`Arena::move_park`](crate::arena::Arena::move_park) confirms
/// the reference is the transient arena's own park.
///
/// THE SHARING LAW: sharing a payload between persistent entries is sound
/// because persistent invalidation is epochal, so every entry dies at one
/// flush and no entry can outlive a payload another still names. Per-entry
/// eviction would have to refcount the shared payloads before it could
/// reclaim one entry's storage.
#[derive(Clone, Copy)]
pub struct Promotion<'a> {
	transient: &'a crate::arena::Arena,
	frames: (usize, usize),
	persistent: &'a crate::arena::Arena,
}

impl<'a> Promotion<'a> {
	/// `frames` is the whole frame buffer as (address, bytes), from
	/// [`Frames::bounds`].
	pub fn new(transient: &'a crate::arena::Arena, frames: (usize, usize), persistent: &'a crate::arena::Arena) -> Self {
		Promotion { transient, frames, persistent }
	}

	pub fn persistent(&self) -> &'a crate::arena::Arena {
		self.persistent
	}

	/// Whether the reference dies with the evaluation, which is the promote's
	/// clone-or-share question. A null or byte-carried slot reads as neither
	/// region's and shares.
	pub fn evaluation_lived(&self, ptr: *const u8) -> bool {
		self.transient.contains(ptr) || (ptr as usize).wrapping_sub(self.frames.0) < self.frames.1
	}

	/// Moves a transient payload's header into the persistent region instead of
	/// cloning the heap it owns: the heap travels with the drop obligation and
	/// is freed at the persistent flush, never at the transient reset. `None`
	/// where the header is not the transient arena's own park keyed to `T`'s
	/// static type or the region refused it, which leaves the caller its clone
	/// path.
	///
	/// The forwarding is the evaluation's, so a payload two records share moves
	/// once and both reach the one persistent header. The source header stays
	/// readable to the evaluation's remaining sharers, which are the only reads
	/// the move keeps sound: no read of it may outlive the persistent flush.
	///
	/// # Safety
	/// `parked` must address a live `T`, and `T` must own all of its content, a
	/// persistent header being allowed to reference no transient storage, which
	/// is also what lets it be republished at `T::Static`. The park's key
	/// settles the type, so the caller owes no identity argument.
	pub unsafe fn move_park<T: dyn_any::StaticTypeSized>(&self, parked: *const u8, retained: usize) -> Option<*const T::Static>
	where
		T::Static: Send + Sync,
	{
		// SAFETY: the caller's contract, forwarded to the parking arena.
		unsafe { self.transient.move_park::<T>(parked, self.persistent, retained) }
	}
}

/// Rewrites one promoted record's parked references in place: the lane bytes
/// are already the persistent region's, and each reference whose payload dies
/// with the evaluation is replaced by a clone parked there.
///
/// A parked element rides the arena slot its payload was written into, so the
/// region holding it decides share against clone. A parked field's reference
/// names storage the promote cannot attribute from the address alone, so it is
/// never provenance-shared: it clones, or takes the route a registered
/// [`FieldPromote`] states under the two-level sharing law.
///
/// # Safety
/// `dst` must be a persistent image of a live record of `layout`.
pub(in crate::record) unsafe fn promote_record(layout: &Layout, dst: *mut u8, promotion: &Promotion<'_>) -> Option<()> {
	if layout.element.parked {
		// SAFETY: a parked element slot holds one reference at offset 0.
		let parked = unsafe { dst.cast::<*const u8>().read() };
		if !promotion.persistent.contains(parked) {
			match element_promote_glue(layout.element.type_id) {
				// SAFETY: the slot images a parked element of this type.
				Some(promote) => unsafe { promote(dst.cast_const(), dst, promotion) }?,
				// SAFETY: as above; the header is the payload's own.
				None => match unsafe { (layout.element.park_move)(parked, promotion) } {
					// SAFETY: a parked element slot holds one reference at offset 0.
					Some(moved) => unsafe { dst.cast::<*const u8>().write(moved) },
					None => {
						// SAFETY: as above, and the clone owns its content.
						let owned = unsafe { (layout.element.clone_out)(dst.cast_const()) };
						unsafe { (layout.element.repark)(&*owned, dst, promotion.persistent) }?;
					}
				},
			}
		}
	}
	for field in &layout.fields {
		let Some(repark) = field.repark else { continue };
		// SAFETY: a parked field slot holds one reference at its offset.
		let slot = unsafe { dst.add(field.offset) };
		if let Some(promote) = field_promote_glue(field.type_id) {
			// SAFETY: the slot images a parked field of this descriptor, and the
			// promoted reference is written back into that same slot.
			unsafe { promote(slot.cast_const(), slot, promotion) }?;
			continue;
		}
		// SAFETY: the slot images a parked field of this descriptor.
		let value = deepen_field_value(unsafe { (field.read_erased)(slot.cast_const()) });
		let resident = replay_field_value(&*value, promotion.persistent)?;
		// SAFETY: the replay produced this field's own value type.
		unsafe { repark(resident.as_deref().unwrap_or(&*value), slot, promotion.persistent) }?;
	}
	Some(())
}

/// Re-walks a promoted record and asserts no reference into the evaluation's
/// storage survived, which is the postcondition every later hit and every
/// shared interior relies on. The element's payload must sit in the persistent
/// region itself; a field's payload is heap its owner holds, so the weaker
/// range check is all that is decidable there.
///
/// # Safety
/// `ptr` must be a live record of `layout`.
pub unsafe fn assert_promoted(layout: &Layout, ptr: *const u8, promotion: &Promotion<'_>) {
	if layout.element.parked {
		// SAFETY: a parked element slot holds one reference at offset 0.
		let parked = unsafe { ptr.cast::<*const u8>().read() };
		assert!(promotion.persistent.contains(parked), "a promoted element kept a reference outside the persistent region");
	}
	for field in &layout.fields {
		if field.repark.is_none() {
			continue;
		}
		// SAFETY: a parked field slot holds one reference at its offset.
		let parked = unsafe { ptr.add(field.offset).cast::<*const u8>().read() };
		assert!(!promotion.evaluation_lived(parked), "a promoted field kept a reference into the evaluation");
	}
}

/// The promote override for element types whose payload holds arena-resident
/// interiors: the generic path clones through an owned intermediate, while a
/// registered promote shares the interiors already living in the persistent
/// region and copies only the rest.
type ElementPromote = unsafe fn(*const u8, *mut u8, &Promotion<'_>) -> Option<()>;

static ELEMENT_PROMOTES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, ElementPromote>>> = std::sync::LazyLock::new(Default::default);

/// Registers the promote for elements of `T`. Called at startup from the crate
/// that owns the type. The promote must leave no reference the promotion calls
/// evaluation-lived.
pub fn register_element_promote<T: dyn_any::StaticTypeSized>(promote: ElementPromote) {
	ELEMENT_PROMOTES.lock().unwrap().insert(std::any::TypeId::of::<T::Static>(), promote);
}

fn element_promote_glue(type_id: std::any::TypeId) -> Option<ElementPromote> {
	ELEMENT_PROMOTES.lock().unwrap().get(&type_id).copied()
}

/// The deep field glue's third half, opt-in beside [`register_deep_field_value`]'s
/// pair: a registered promote routes the field payload transient-to-persistent
/// at the slot itself, so the promote never builds the owned form the other two
/// halves round-trip through. Keyed by the field's stored type rather than its
/// owned one, since the route runs before any erased read. The owned halves keep
/// serving every other seam, [`OwnedRecord::copy_out`] and [`GroupItem::copy_out`]
/// included.
///
/// THE TWO-LEVEL SHARING LAW, which a registrant must hold to: the field's own
/// header is not provenance-shared, since a stored reference names storage whose
/// owner the promote cannot attribute from the address alone; it clones, or moves
/// where the payload owns all of its content and the transient arena confirms the
/// reference is its own park. One level inside the payload, an interior that is
/// arena-resident has decidable provenance and takes the Cow dispatch: an interior
/// the persistent region already holds is shared pointer for pointer, and one that
/// dies with the evaluation is copied or moved.
type FieldPromote = unsafe fn(*const u8, *mut u8, &Promotion<'_>) -> Option<()>;

static FIELD_PROMOTES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, FieldPromote>>> = std::sync::LazyLock::new(Default::default);

/// Registers the promote for fields stored as `T`. Called at startup from the
/// crate that owns the type. The promote must leave no reference the promotion
/// calls evaluation-lived.
pub fn register_field_promote<T: 'static>(promote: FieldPromote) {
	FIELD_PROMOTES.lock().unwrap().insert(std::any::TypeId::of::<T>(), promote);
}

fn field_promote_glue(type_id: std::any::TypeId) -> Option<FieldPromote> {
	FIELD_PROMOTES.lock().unwrap().get(&type_id).copied()
}

/// The park glue's heap estimate for values of a type, keyed as the deep glue
/// is. Consulted where a payload parks, so a region's retained heap is known
/// without walking it.
type RetainedMeasure = fn(&(dyn std::any::Any + Send + Sync)) -> usize;

static RETAINED_MEASURES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, RetainedMeasure>>> = std::sync::LazyLock::new(Default::default);

/// Registers the retained-heap estimate for values of `T`. Called at startup
/// from the crate that owns the type. The estimate is a hint: an unregistered
/// type contributes 0, so a region's counter is a lower bound.
pub fn register_retained_heap<T: dyn_any::StaticTypeSized>(measure: RetainedMeasure) {
	RETAINED_MEASURES.lock().unwrap().insert(std::any::TypeId::of::<T::Static>(), measure);
}

pub(in crate::record) fn retained_measure(type_id: std::any::TypeId) -> Option<RetainedMeasure> {
	RETAINED_MEASURES.lock().unwrap().get(&type_id).copied()
}

impl MaterializedSpan {
	/// Copies the batch into the persistent region as a copy-on-write over
	/// provenance: the frame bytes memcpy, and each parked reference is cloned
	/// only where it dies with the evaluation, so a layout carrying no parked
	/// slot reduces to the memcpy and a level whose payloads an upstream memo
	/// already published costs nothing beyond it. `None` where the region
	/// could not hold the copy, which leaves the caller with nothing to cache.
	///
	/// # Safety
	/// The batch's lanes must be live records of its layout.
	pub unsafe fn to_persistent(batch: &crate::node::RecordBatch<'_>, promotion: &Promotion<'_>) -> Option<MaterializedSpan> {
		let layout = batch.layout();
		let stride = layout.lane_stride();
		let len = batch.len();
		if len == 0 {
			return Some(MaterializedSpan {
				base: crate::arena::ArenaWeak::NULL,
				len: 0,
			});
		}
		let persistent = promotion.persistent();
		let slab = persistent.alloc_scratch::<u64>((len * stride).div_ceil(8))?;
		let base: *mut u8 = slab.as_mut_ptr().cast();
		for lane in 0..len {
			// SAFETY: the caller's contract on the lane, into the lane's own
			// region of the freshly reserved slab.
			let dst = unsafe { base.add(lane * stride) };
			unsafe { std::ptr::copy_nonoverlapping(batch.get(lane).rec().ptr(), dst, layout.size) };
			// SAFETY: the copy images a record of this layout.
			unsafe { promote_record(layout, dst, promotion) }?;
		}
		#[cfg(debug_assertions)]
		for lane in 0..len {
			// SAFETY: every lane was imaged and promoted above.
			unsafe { assert_promoted(layout, base.add(lane * stride).cast_const(), promotion) };
		}
		Some(MaterializedSpan {
			base: persistent.handle_at(base.cast_const())?,
			len,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::record::access::{borrow_element, write_element, write_element_sized};
	use crate::record::layout::element_write;
	use crate::record::test_support::f64_field;

	#[test]
	fn a_pod_level_promotes_as_a_bare_memcpy() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(1, element_write::<f64>(), &[f64_field("opacity")]);
		let stride = layout.lane_stride();

		let mut buffer = vec![0u64; (4 * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		for lane in 0..4usize {
			unsafe { base.add(lane * stride).cast::<f64>().write(lane as f64) };
		}
		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 4, &layout) };

		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let slab = (4 * stride).div_ceil(8) * 8;
		assert_eq!(persistent.occupancy(), slab, "a layout with no parked slot allocates the slab and nothing else");
		assert_eq!(persistent.retained_heap(), 0, "no payload parked, so nothing is retained");
		let published = span.batch(&persistent, &layout).unwrap();
		for lane in 0..4usize {
			assert_eq!(unsafe { published.get(lane).rec().element::<f64>() }, lane as f64, "lane {lane}");
		}
	}

	#[test]
	fn a_promoted_element_lands_in_the_persistent_region() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		unsafe { write_element(base, String::from("parked in the evaluation"), &transient) }.unwrap();
		let source = unsafe { base.cast::<*const u8>().read() };
		assert!(transient.contains(source));

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 1, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let promoted = unsafe { published.get(0).rec().ptr().cast::<*const u8>().read() };
		assert!(persistent.contains(promoted), "the promote re-parked the payload into the persistent region");
		assert_ne!(source, promoted, "an evaluation-lived payload gets its own persistent header");
		assert_eq!(unsafe { borrow_element::<String>(published.get(0).rec()) }, "parked in the evaluation");
	}

	/// The measure production registers for `String`, without which a promote
	/// credits the region 0 and the counters cannot be observed to transfer.
	fn measure_strings() {
		register_retained_heap::<String>(|value| value.downcast_ref::<String>().map_or(0, String::len));
	}

	#[test]
	fn a_promoted_payload_moves_its_heap_rather_than_cloning_it() {
		measure_strings();
		let mut transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		let owned = String::from("the obligation travels with the header");
		let (heap, length) = (owned.as_ptr(), owned.len());
		unsafe { write_element_sized(base, owned, &transient, length) }.unwrap();
		assert_eq!(transient.retained_heap(), length);

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 1, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let served = unsafe { borrow_element::<String>(published.get(0).rec()) };
		assert_eq!(served.as_ptr(), heap, "the promote moved the header, so the served view names the pre-promote heap");
		assert_eq!(transient.retained_heap(), 0, "the transient counter gave the hint up");
		assert_eq!(persistent.retained_heap(), length, "and the persistent counter took it");

		transient.reset();
		let published = span.batch(&persistent, &layout).unwrap();
		assert_eq!(
			unsafe { borrow_element::<String>(published.get(0).rec()) },
			"the obligation travels with the header",
			"the moved payload survives the transient reset"
		);
	}

	#[test]
	fn a_payload_two_records_share_promotes_to_one_persistent_header() {
		measure_strings();
		let transient = crate::arena::Arena::new(4096).unwrap();
		let mut persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);
		let stride = layout.lane_stride();

		let mut buffer = vec![0u64; (2 * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		let length = "shared across lanes".len();
		unsafe { write_element_sized(base, String::from("shared across lanes"), &transient, length) }.unwrap();
		// A carried field byte-copies the reference, so both lanes name the one park.
		let shared = unsafe { base.cast::<*const u8>().read() };
		unsafe { base.add(stride).cast::<*const u8>().write(shared) };

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 2, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let first = unsafe { published.get(0).rec().ptr().cast::<*const u8>().read() };
		let second = unsafe { published.get(1).rec().ptr().cast::<*const u8>().read() };
		assert_eq!(first, second, "a payload two records share moves once");
		assert_eq!(persistent.retained_heap(), length, "and its hint transfers once");

		persistent.reset();
		assert_eq!(persistent.retained_heap(), 0, "the flush frees the one header exactly once");
	}

	#[test]
	fn a_persistent_element_is_shared_by_the_promote() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		// The payload an upstream memo already published: the promote must
		// name it rather than copy it.
		unsafe { write_element(base, String::from("published upstream"), &persistent) }.unwrap();
		let upstream = unsafe { base.cast::<*const u8>().read() };
		let occupied = persistent.occupancy();

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 1, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let promoted = unsafe { published.get(0).rec().ptr().cast::<*const u8>().read() };
		assert_eq!(upstream, promoted, "an already persistent payload is shared, pointer for pointer");
		assert_eq!(persistent.occupancy() - occupied, layout.frame_bytes(), "only the lane slab was allocated");
	}

	#[test]
	#[should_panic(expected = "a promoted element kept a reference outside the persistent region")]
	fn the_rewalk_catches_a_reference_the_promote_left_behind() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		unsafe { write_element(base, String::from("never promoted"), &transient) }.unwrap();

		let promotion = Promotion::new(&transient, bounds, &persistent);
		unsafe { assert_promoted(&layout, base.cast_const(), &promotion) };
	}
}
