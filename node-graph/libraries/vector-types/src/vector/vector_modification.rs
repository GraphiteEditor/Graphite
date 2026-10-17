use super::*;
use crate::vector::flatten::WorkingVector;
use crate::vector::misc::{BezierHandles, HandleId, HandleType};
use crate::vector::path::{CurveType, Path, curve_type_at};
use core_types::list::{ATTR_ID, List};
use core_types::uuid::generate_uuid;
use dyn_any::DynAny;
use glam::DVec2;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::BuildHasher;
use std::hash::Hash;

/// Represents a procedural change to the points of a [`Vector`].
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct PointModification {
	add: Vec<PointId>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashset"))]
	remove: HashSet<PointId>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap"))]
	delta: HashMap<PointId, DVec2>,
}

impl PointModification {
	/// Apply this modification to the working form's points.
	fn apply(&self, working: &mut WorkingVector) {
		// Remove points, remapping segment endpoints and leaving out-of-range markers on segments
		// that referenced a removed point, for the segment pass to delete
		let mut id_map = Vec::with_capacity(working.point_ids.len());
		let mut new_index = 0;
		for id in &working.point_ids {
			if !self.remove.contains(id) {
				id_map.push(new_index);
				new_index += 1;
			} else {
				id_map.push(usize::MAX);
			}
		}
		let update_index = |index: &mut usize| {
			if *index < id_map.len() {
				*index = id_map[*index];
			}
		};
		working.segment_start.iter_mut().for_each(update_index);
		working.segment_end.iter_mut().for_each(update_index);

		let mut keep = working.point_ids.iter().map(|id| !self.remove.contains(id));
		working.point_positions.retain(|_| keep.next().unwrap_or_default());
		working.point_ids.retain(|id| !self.remove.contains(id));

		// Gathering the moves first keeps the segment pass below off the common path where nothing moves
		let mut moved = Vec::new();
		let mut any_moved = false;
		if !self.delta.is_empty() {
			moved.resize(working.point_ids.len(), None);
			for (index, id) in working.point_ids.iter().enumerate() {
				let Some(&delta) = self.delta.get(id) else { continue };
				if !delta.is_finite() {
					warn!("Invalid delta when applying a point modification");
					continue;
				}

				working.point_positions[index] += delta;
				moved[index] = Some(delta);
				any_moved = true;
			}
		}

		// Handles travel with their anchor, carried in one pass over the segment ends
		if any_moved {
			let moved_by = |index: usize| moved.get(index).copied().flatten();
			for index in 0..working.segment_ids.len() {
				if let Some(delta) = moved_by(working.segment_start[index]) {
					working.segment_handles[index].move_start(delta);
				}
				if let Some(delta) = moved_by(working.segment_end[index]) {
					working.segment_handles[index].move_end(delta);
				}
			}
		}

		let mut named: FxHashSet<PointId> = if self.add.is_empty() { FxHashSet::default() } else { working.point_ids.iter().copied().collect() };
		for &add_id in &self.add {
			let Some(&position) = self.delta.get(&add_id) else { continue };
			if !position.is_finite() {
				warn!("Invalid position when applying a point modification");
				continue;
			}

			if !named.insert(add_id) {
				warn!("Tried to push a duplicate point to the working form");
				continue;
			}

			working.point_ids.push(add_id);
			working.point_positions.push(position);
		}
	}

	/// Moves each point named by a delta directly in the pool, carrying the handles that follow it. `handles` marks the pool's handles,
	/// none of which may be shared between segments, or a shared one would move once for each.
	fn move_in_place(&self, vector: &mut Vector, paths: &List<Path>, handles: &[bool]) {
		if self.delta.is_empty() {
			return;
		}

		let ids = vector.0.iter_attribute_values::<PointId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();

		// Deltas move only anchors and free points, as the working form leaves handles out of its points
		let mut moved = vec![None; vector.point_count()];
		let mut any_moved = false;
		for (index, _) in handles.iter().enumerate().filter(|&(_, &handle)| !handle) {
			let Some(&delta) = self.delta.get(&ids.get(index).copied().unwrap_or_default()) else { continue };
			if !delta.is_finite() {
				warn!("Invalid delta when applying a point modification");
				continue;
			}

			moved[index] = Some(delta);
			any_moved = true;
		}

		if !any_moved {
			return;
		}

		for (position, delta) in vector.0.iter_element_values_mut().zip(&moved) {
			if let Some(delta) = delta {
				*position += *delta;
			}
		}

		// A segment's first handle follows its start anchor, and a cubic's second handle follows its end anchor
		for (path_index, path) in paths.iter_element_values().enumerate() {
			let curve_type = curve_type_at(paths, path_index);
			if curve_type == CurveType::Linear {
				continue;
			}

			let stride = curve_type.stride();
			let moved_by = |reference: usize| path.reference(reference).and_then(|point| moved.get(point as usize).copied().flatten());

			for span in 0..path.segment_count(curve_type) {
				let start = span * stride;
				let mut carry = |handle_reference: usize, delta: Option<DVec2>| {
					if let (Some(delta), Some(handle)) = (delta, path.reference(handle_reference))
						&& let Some(position) = vector.0.element_mut(handle as usize)
					{
						*position += delta;
					}
				};

				carry(start + 1, moved_by(start));
				if curve_type == CurveType::Cubic {
					carry(start + 2, moved_by(start + stride));
				}
			}
		}
	}

	/// Create a new modification that will convert an empty [`Vector`] into the target working form.
	fn create_from_working(working: &WorkingVector) -> Self {
		Self {
			add: working.point_ids.clone(),
			remove: HashSet::new(),
			delta: working.point_ids.iter().copied().zip(working.point_positions.iter().copied()).collect(),
		}
	}

	fn push(&mut self, id: PointId, position: DVec2) {
		self.add.push(id);
		self.delta.insert(id, position);
	}

	fn remove(&mut self, id: PointId) {
		self.remove.insert(id);
		self.add.retain(|&add| add != id);
		self.delta.remove(&id);
	}
}

/// Represents a procedural change to the segments of a [`Vector`].
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct SegmentModification {
	add: Vec<SegmentId>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashset"))]
	remove: HashSet<SegmentId>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap"))]
	start_point: HashMap<SegmentId, PointId>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap"))]
	end_point: HashMap<SegmentId, PointId>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap"))]
	handle_primary: HashMap<SegmentId, Option<DVec2>>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap"))]
	handle_end: HashMap<SegmentId, Option<DVec2>>,
}

impl SegmentModification {
	/// The kept segment after removed ones begins a path when it carried on from the last of them or one of them began a path,
	/// unless an added segment arrives where it sets out and so carries the path on into it. Runs in O(segments + adds).
	fn mark_path_beginnings_after_removals(&self, working: &mut WorkingVector, keep: &[bool]) {
		if !keep.contains(&false) {
			return;
		}

		let arrivals: FxHashSet<PointId> = self.add.iter().filter_map(|id| self.end_point.get(id).copied()).collect();
		let segment_count = keep.len();
		working.segment_begins_path.resize(segment_count, false);

		let (mut removed_began_path, mut removed_end) = (false, None);
		for index in 0..segment_count {
			if !keep[index] {
				removed_began_path |= std::mem::take(&mut working.segment_begins_path[index]);
				removed_end = Some(working.segment_end[index]);
				continue;
			}
			let Some(end) = removed_end.take() else { continue };

			let start = working.segment_start[index];
			let carried_on = start == end;
			let arrived_at = working.point_ids.get(start).is_some_and(|point| arrivals.contains(point));
			if (std::mem::take(&mut removed_began_path) || carried_on) && !arrived_at {
				working.segment_begins_path[index] = true;
			}
		}
	}

	/// Apply this modification to the working form's segments, where the added ones named by `path_breaks` each begin a path.
	fn apply(&self, working: &mut WorkingVector, path_breaks: &HashSet<SegmentId>) {
		// Remove the deleted segments, plus any segment left referencing a removed point
		let point_count = working.point_ids.len();
		let keep: Vec<bool> = (0..working.segment_ids.len())
			.map(|index| !self.remove.contains(&working.segment_ids[index]) && working.segment_start[index] < point_count && working.segment_end[index] < point_count)
			.collect();
		self.mark_path_beginnings_after_removals(working, &keep);
		let mut keep_iter = keep.iter();
		working.segment_start.retain(|_| *keep_iter.next().unwrap_or(&false));
		let mut keep_iter = keep.iter();
		working.segment_end.retain(|_| *keep_iter.next().unwrap_or(&false));
		let mut keep_iter = keep.iter();
		working.segment_handles.retain(|_| *keep_iter.next().unwrap_or(&false));
		let mut keep_iter = keep.iter();
		working.segment_ids.retain(|_| *keep_iter.next().unwrap_or(&false));
		let mut keep_iter = keep.iter();
		working.segment_begins_path.retain(|_| *keep_iter.next().unwrap_or(&false));

		// The points no longer change here, so one index serves every endpoint lookup below.
		// Nothing looks an endpoint up in the common pure-handle case, so it skips the build.
		let mut point_index: FxHashMap<PointId, usize> = FxHashMap::default();
		if !self.start_point.is_empty() || !self.end_point.is_empty() || !self.add.is_empty() {
			point_index.reserve(working.point_ids.len());
			for (index, &id) in working.point_ids.iter().enumerate() {
				point_index.entry(id).or_insert(index);
			}
		}

		for (index, id) in working.segment_ids.iter().enumerate() {
			if let Some(new) = self.start_point.get(id) {
				match point_index.get(new) {
					Some(&point) => working.segment_start[index] = point,
					None => warn!("Invalid start ID when applying a segment modification"),
				}
			}
			if let Some(new) = self.end_point.get(id) {
				match point_index.get(new) {
					Some(&point) => working.segment_end[index] = point,
					None => warn!("Invalid end ID when applying a segment modification"),
				}
			}
		}

		for (index, id) in working.segment_ids.iter().enumerate() {
			let Some(&start) = working.point_positions.get(working.segment_start[index]) else { continue };
			let Some(&end) = working.point_positions.get(working.segment_end[index]) else { continue };

			// Compute the actual start and end position based on the offset from the anchor
			let new_start = self.handle_primary.get(id).copied().map(|handle| handle.map(|handle| handle + start));
			let new_end = self.handle_end.get(id).copied().map(|handle| handle.map(|handle| handle + end));

			if !new_start.unwrap_or_default().is_none_or(|start| start.is_finite()) || !new_end.unwrap_or_default().is_none_or(|end| end.is_finite()) {
				warn!("Invalid handles when applying a segment modification");
				continue;
			}

			let handles = &mut working.segment_handles[index];
			match (new_start, new_end) {
				// The new handles are fully specified by the modification
				(Some(Some(handle_start)), Some(Some(handle_end))) => *handles = BezierHandles::Cubic { handle_start, handle_end },
				(Some(Some(handle)), Some(None)) | (Some(None), Some(Some(handle))) => *handles = BezierHandles::Quadratic { handle },
				(Some(None), Some(None)) => *handles = BezierHandles::Linear,
				// Remove the end handle
				(None, Some(None)) => {
					if let BezierHandles::Cubic { handle_start, .. } = *handles {
						*handles = BezierHandles::Quadratic { handle: handle_start }
					}
				}
				// Change the end handle
				(None, Some(Some(handle_end))) => match *handles {
					BezierHandles::Linear => *handles = BezierHandles::Quadratic { handle: handle_end },
					BezierHandles::Quadratic { handle: handle_start } => *handles = BezierHandles::Cubic { handle_start, handle_end },
					BezierHandles::Cubic { handle_start, .. } => *handles = BezierHandles::Cubic { handle_start, handle_end },
				},
				// Remove the start handle
				(Some(None), None) => *handles = BezierHandles::Linear,
				// Change the start handle
				(Some(Some(handle_start)), None) => match *handles {
					BezierHandles::Linear => *handles = BezierHandles::Quadratic { handle: handle_start },
					BezierHandles::Quadratic { .. } => *handles = BezierHandles::Quadratic { handle: handle_start },
					BezierHandles::Cubic { handle_end, .. } => *handles = BezierHandles::Cubic { handle_start, handle_end },
				},
				// No change
				(None, None) => {}
			};
		}

		working.segment_begins_path.resize(working.segment_ids.len(), false);

		let mut named: FxHashSet<SegmentId> = if self.add.is_empty() {
			FxHashSet::default()
		} else {
			working.segment_ids.iter().copied().collect()
		};
		for &add_id in &self.add {
			let Some(&start) = self.start_point.get(&add_id) else { continue };
			let Some(&end) = self.end_point.get(&add_id) else { continue };
			let Some(&handle_start) = self.handle_primary.get(&add_id) else { continue };
			let Some(&handle_end) = self.handle_end.get(&add_id) else { continue };

			if !named.insert(add_id) {
				warn!("Tried to push a duplicate segment to the working form");
				continue;
			}

			let Some(&start_index) = point_index.get(&start) else {
				warn!("Invalid start ID: {start:#?}");
				continue;
			};
			let Some(&end_index) = point_index.get(&end) else {
				warn!("Invalid end ID: {end:#?}");
				continue;
			};

			let start_position = working.point_positions[start_index];
			let end_position = working.point_positions[end_index];
			let handles = match (handle_start, handle_end) {
				(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic {
					handle_start: handle_start + start_position,
					handle_end: handle_end + end_position,
				},
				(Some(handle), None) | (None, Some(handle)) => BezierHandles::Quadratic { handle: handle + start_position },
				(None, None) => BezierHandles::Linear,
			};

			if !handles.is_finite() {
				warn!("Invalid handles");
				continue;
			}

			working.segment_ids.push(add_id);
			working.segment_start.push(start_index);
			working.segment_end.push(end_index);
			working.segment_handles.push(handles);
			working.segment_begins_path.push(path_breaks.contains(&add_id));
		}

		assert!(working.segment_start.iter().all(|&index| index < working.point_ids.len()), "index should be in range");
		assert!(working.segment_end.iter().all(|&index| index < working.point_ids.len()), "index should be in range");
	}

	/// The curve type a segment ends up with once its handle edits apply, following the same rules as [`Self::apply`].
	fn edited_curve_type(current: CurveType, primary: Option<Option<DVec2>>, end: Option<Option<DVec2>>) -> CurveType {
		match (primary.map(|handle| handle.is_some()), end.map(|handle| handle.is_some())) {
			(Some(true), Some(true)) => CurveType::Cubic,
			(Some(true), Some(false)) | (Some(false), Some(true)) => CurveType::Quadratic,
			(Some(false), Some(false)) | (Some(false), None) => CurveType::Linear,
			(None, Some(false)) if current == CurveType::Cubic => CurveType::Quadratic,
			(None, Some(true)) if current == CurveType::Linear => CurveType::Quadratic,
			(None, Some(true)) => CurveType::Cubic,
			(Some(true), None) if current == CurveType::Linear => CurveType::Quadratic,
			_ => current,
		}
	}

	/// Calls the visitor with each span a handle edit names, along with its curve type, references, starting reference, and edits.
	fn for_each_edited_span(&self, paths: &List<Path>, mut visit: impl FnMut(CurveType, &Path, usize, Option<Option<DVec2>>, Option<Option<DVec2>>)) {
		for (path_index, path) in paths.iter_element_values().enumerate() {
			let curve_type = curve_type_at(paths, path_index);
			let stride = curve_type.stride();
			let ids = path.0.iter_attribute_values::<SegmentId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();

			for span in 0..path.segment_count(curve_type) {
				let id = ids.get(span * stride).copied().unwrap_or_default();
				let (primary, end) = (self.handle_primary.get(&id).copied(), self.handle_end.get(&id).copied());
				if primary.is_some() || end.is_some() {
					visit(curve_type, path, span * stride, primary, end);
				}
			}
		}
	}

	/// Whether every handle edit leaves its segment's curve type as it is, so the edits can land in the pool without reweaving.
	fn handle_edits_keep_curve_types(&self, paths: &List<Path>) -> bool {
		if self.handle_primary.is_empty() && self.handle_end.is_empty() {
			return true;
		}

		let mut keeps = true;
		self.for_each_edited_span(paths, |curve_type, _, _, primary, end| {
			keeps &= Self::edited_curve_type(curve_type, primary, end) == curve_type;
		});
		keeps
	}

	/// Sets each edited handle directly in the pool, relative to its anchor's current position. Only valid once
	/// [`Self::handle_edits_keep_curve_types`] has confirmed no curve type changes and [`Vector::point_roles`] that no handle is shared.
	fn set_handles_in_place(&self, vector: &mut Vector, paths: &List<Path>) {
		if self.handle_primary.is_empty() && self.handle_end.is_empty() {
			return;
		}

		let mut writes = Vec::new();
		self.for_each_edited_span(paths, |curve_type, path, start, primary, end| {
			let position_at = |reference: usize| path.reference(reference).and_then(|point| vector.position(point as usize));
			let (Some(start_position), Some(end_position)) = (position_at(start), position_at(start + curve_type.stride())) else {
				return;
			};

			let new_start = primary.map(|handle| handle.map(|handle| handle + start_position));
			let new_end = end.map(|handle| handle.map(|handle| handle + end_position));
			if !new_start.unwrap_or_default().is_none_or(|start| start.is_finite()) || !new_end.unwrap_or_default().is_none_or(|end| end.is_finite()) {
				warn!("Invalid handles when applying a segment modification");
				return;
			}

			let mut write = |reference: usize, position: DVec2| {
				if let Some(point) = path.reference(reference) {
					writes.push((point, position));
				}
			};
			match (curve_type, new_start.flatten(), new_end.flatten()) {
				// A quadratic's lone handle takes whichever side's position the edit sets
				(CurveType::Quadratic, Some(handle), _) | (CurveType::Quadratic, None, Some(handle)) => write(start + 1, handle),
				(CurveType::Cubic, handle_start, handle_end) => {
					if let Some(handle_start) = handle_start {
						write(start + 1, handle_start);
					}
					if let Some(handle_end) = handle_end {
						write(start + 2, handle_end);
					}
				}
				_ => {}
			}
		});

		for (point, position) in writes {
			if let Some(slot) = vector.0.element_mut(point as usize) {
				*slot = position;
			}
		}
	}

	/// Create a new modification that will convert an empty [`Vector`] into the target working form.
	fn create_from_working(working: &WorkingVector) -> Self {
		let point_id = |(&segment, &index): (&SegmentId, &usize)| (segment, working.point_ids[index]);
		Self {
			add: working.segment_ids.clone(),
			remove: HashSet::new(),
			start_point: working.segment_ids.iter().zip(&working.segment_start).map(point_id).collect(),
			end_point: working.segment_ids.iter().zip(&working.segment_end).map(point_id).collect(),
			handle_primary: working
				.segment_ids
				.iter()
				.enumerate()
				.map(|(index, &id)| (id, working.segment_handles[index].start().map(|handle| handle - working.point_positions[working.segment_start[index]])))
				.collect(),
			handle_end: working
				.segment_ids
				.iter()
				.enumerate()
				.map(|(index, &id)| (id, working.segment_handles[index].end().map(|handle| handle - working.point_positions[working.segment_end[index]])))
				.collect(),
		}
	}

	/// The handle map for the side of its segment that the given handle sits on.
	fn handle_side_of(&mut self, handle: HandleId) -> &mut HashMap<SegmentId, Option<DVec2>> {
		match handle.ty {
			HandleType::Primary => &mut self.handle_primary,
			HandleType::End => &mut self.handle_end,
		}
	}

	/// The endpoint map for the side of its segment that the given handle sits on.
	fn endpoint_side_of(&mut self, handle: HandleId) -> &mut HashMap<SegmentId, PointId> {
		match handle.ty {
			HandleType::Primary => &mut self.start_point,
			HandleType::End => &mut self.end_point,
		}
	}

	fn push(&mut self, id: SegmentId, points: [PointId; 2], handles: [Option<DVec2>; 2]) {
		self.remove.remove(&id);
		self.add.push(id);
		self.start_point.insert(id, points[0]);
		self.end_point.insert(id, points[1]);
		self.handle_primary.insert(id, handles[0]);
		self.handle_end.insert(id, handles[1]);
	}

	fn remove(&mut self, id: SegmentId) {
		self.remove.insert(id);
		self.add.retain(|&add| add != id);
		self.start_point.remove(&id);
		self.end_point.remove(&id);
		self.handle_primary.remove(&id);
		self.handle_end.remove(&id);
	}
}

/// Represents a procedural change to the [`Vector`].
#[derive(Clone, Debug, Default, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct VectorModification {
	points: PointModification,
	segments: SegmentModification,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashset"))]
	add_g1_continuous: HashSet<[HandleId; 2]>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashset"))]
	remove_g1_continuous: HashSet<[HandleId; 2]>,
	/// The added segments that each begin a path of their own, even where one sets out from the end of another segment.
	// TODO: Stop always writing this as a marker when switching to the new document format and Ctrl-C node serialization format
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashset_as_some"))]
	path_breaks: HashSet<SegmentId>,
	/// The added segments turned around when this modification was loaded as a soup, which other modifications naming them by side follow.
	// TODO: Eventually remove this document upgrade code
	#[cfg_attr(feature = "serde", serde(skip))]
	turned_on_load: HashSet<SegmentId>,
}

// TODO: Eventually remove this document upgrade code
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for VectorModification {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		// Modifications stored before paths recorded where they begin lack `path_breaks` and hold their segments as an unordered soup, which is chained into paths on load
		#[derive(serde::Deserialize)]
		struct StoredVectorModification {
			points: PointModification,
			segments: SegmentModification,
			add_g1_continuous: HashSet<[HandleId; 2]>,
			remove_g1_continuous: HashSet<[HandleId; 2]>,
			#[serde(default)]
			path_breaks: Option<HashSet<SegmentId>>,
		}

		let stored = StoredVectorModification::deserialize(deserializer)?;
		let legacy = stored.path_breaks.is_none();
		let mut modification = Self {
			points: stored.points,
			segments: stored.segments,
			add_g1_continuous: stored.add_g1_continuous,
			remove_g1_continuous: stored.remove_g1_continuous,
			path_breaks: stored.path_breaks.unwrap_or_default(),
			turned_on_load: HashSet::new(),
		};
		if legacy {
			modification.chain_legacy_segments();
		}

		Ok(modification)
	}
}

/// A modification type that can be added to a [`VectorModification`].
#[derive(PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VectorModificationType {
	InsertSegment {
		id: SegmentId,
		points: [PointId; 2],
		handles: [Option<DVec2>; 2],
	},
	InsertPoint {
		id: PointId,
		position: DVec2,
	},

	RemoveSegment {
		id: SegmentId,
	},
	RemovePoint {
		id: PointId,
	},

	SetG1Continuous {
		handles: [HandleId; 2],
		enabled: bool,
	},
	SetHandles {
		segment: SegmentId,
		handles: [Option<DVec2>; 2],
	},
	SetHandlePosition {
		handle: HandleId,
		relative_position: DVec2,
	},
	/// Reconnects one end of the handle's segment to another point: the start for a primary handle, the end for an end handle.
	SetEndpoint {
		handle: HandleId,
		id: PointId,
	},
	/// Marks or unmarks a segment this modification adds as beginning a path of its own. Segments from upstream are unaffected.
	SetPathBreak {
		segment: SegmentId,
		breaks: bool,
	},

	ApplyPointDelta {
		point: PointId,
		delta: DVec2,
	},
}

impl VectorModificationType {
	/// Whether every position, handle, and delta carried by this modification is finite.
	fn is_finite(&self) -> bool {
		match self {
			Self::InsertSegment { handles, .. } | Self::SetHandles { handles, .. } => handles.iter().flatten().all(|handle| handle.is_finite()),
			Self::InsertPoint { position: value, .. } | Self::SetHandlePosition { relative_position: value, .. } | Self::ApplyPointDelta { delta: value, .. } => value.is_finite(),
			Self::RemoveSegment { .. } | Self::RemovePoint { .. } | Self::SetG1Continuous { .. } | Self::SetEndpoint { .. } | Self::SetPathBreak { .. } => true,
		}
	}
}

/// Per-category `[added, removed, modified]` counts for a [`VectorModification`].
struct ModificationCategoryCounts {
	points: [usize; 3],
	segments: [usize; 3],
	smooth_handles: [usize; 3],
}

impl ModificationCategoryCounts {
	/// Returns the `[added, removed, modified]` totals across all categories.
	fn totals(&self) -> [usize; 3] {
		let mut totals = [0; 3];
		for [a, r, m] in [self.points, self.segments, self.smooth_handles] {
			totals[0] += a;
			totals[1] += r;
			totals[2] += m;
		}
		totals
	}

	/// Iterates over each named category and its `[added, removed, modified]` counts.
	fn iter_categories(&self) -> impl Iterator<Item = (&str, [usize; 3])> {
		[("Points", self.points), ("Segments", self.segments), ("Smooth Handles", self.smooth_handles)].into_iter()
	}
}

impl VectorModification {
	/// Computes per-category counts of additions, removals, and modifications.
	fn category_counts(&self) -> ModificationCategoryCounts {
		// Build sets of added IDs so we can distinguish true modifications from initial values stored for newly added items
		let add_points: HashSet<_> = self.points.add.iter().copied().collect();
		let add_segments: HashSet<_> = self.segments.add.iter().copied().collect();

		let point_modifications = self.points.delta.keys().filter(|id| !add_points.contains(id)).count();

		// Count unique modified segment IDs across all field maps
		let mut modified_segments: HashSet<&SegmentId> = HashSet::with_capacity(self.segments.start_point.len());
		let not_added_segment = |id: &&SegmentId| !add_segments.contains(id);
		modified_segments.extend(self.segments.start_point.keys().filter(not_added_segment));
		modified_segments.extend(self.segments.end_point.keys().filter(not_added_segment));
		modified_segments.extend(self.segments.handle_primary.keys().filter(not_added_segment));
		modified_segments.extend(self.segments.handle_end.keys().filter(not_added_segment));

		ModificationCategoryCounts {
			points: [self.points.add.len(), self.points.remove.len(), point_modifications],
			segments: [self.segments.add.len(), self.segments.remove.len(), modified_segments.len()],
			smooth_handles: [self.add_g1_continuous.len(), self.remove_g1_continuous.len(), 0],
		}
	}

	/// Returns a short human-readable summary string like "+6 / −1 / Δ1".
	pub fn summary_label(&self) -> String {
		let counts = self.category_counts();
		let [additions, removals, modifications] = counts.totals();

		let mut parts = Vec::new();
		if additions > 0 {
			parts.push(format!("+{additions}"));
		}
		if removals > 0 {
			parts.push(format!("\u{2212}{removals}"));
		}
		if modifications > 0 {
			parts.push(format!("\u{0394}{modifications}"));
		}
		if parts.is_empty() { "No Differential Edits".to_string() } else { parts.join(" / ") }
	}

	/// Returns a detailed multi-line tooltip describing all the changes.
	pub fn summary_tooltip(&self) -> String {
		let counts = self.category_counts();

		let mut lines = Vec::new();

		for (name, [added, removed, modified]) in counts.iter_categories() {
			let mut parts = Vec::new();
			if added > 0 {
				parts.push(format!("+{added}"));
			}
			if removed > 0 {
				parts.push(format!("\u{2212}{removed}"));
			}
			if modified > 0 {
				parts.push(format!("\u{0394}{modified}"));
			}
			if !parts.is_empty() {
				lines.push(format!("{name}: {}", parts.join(" / ")));
			}
		}

		if lines.is_empty() { "None".to_string() } else { lines.join("\n") }
	}

	/// Whether this modification changes nothing at all.
	pub fn is_empty(&self) -> bool {
		self.points == PointModification::default() && self.segments == SegmentModification::default() && self.add_g1_continuous.is_empty() && self.remove_g1_continuous.is_empty()
	}

	/// Whether this modification only moves existing points and handles, adding, removing, and reconnecting nothing.
	fn only_moves_points(&self) -> bool {
		// Every field is named so that adding one forces a decision here
		let Self {
			points: PointModification {
				add: added_points,
				remove: removed_points,
				delta: _,
			},
			segments,
			add_g1_continuous,
			remove_g1_continuous,
			// Path beginnings only concern added segments, and any of those already rules this out
			path_breaks: _,
			turned_on_load: _,
		} = self;
		let SegmentModification {
			add: added_segments,
			remove: removed_segments,
			start_point,
			end_point,
			handle_primary: _,
			handle_end: _,
		} = segments;

		added_points.is_empty()
			&& removed_points.is_empty()
			&& added_segments.is_empty()
			&& removed_segments.is_empty()
			&& start_point.is_empty()
			&& end_point.is_empty()
			&& add_g1_continuous.is_empty()
			&& remove_g1_continuous.is_empty()
	}

	/// Apply this modification to the specified [`Vector`] and its paths.
	pub fn apply(&self, vector: &mut Vector, paths: &mut List<Path>) {
		// An untouched vector keeps its incoming path structure instead of being flattened and rewoven
		if self.is_empty() {
			return;
		}

		// Moving points and handles leaves the weave intact, so the pool is edited where it lies in O(points + references), keeping
		// upstream storage as it is. A handle shared between segments would move once for each there, so that weave is rewoven instead.
		if self.only_moves_points() && self.segments.handle_edits_keep_curve_types(paths) {
			let (handles, handles_unshared) = vector.point_roles(paths);
			if handles_unshared {
				self.points.move_in_place(vector, paths, &handles);
				self.segments.set_handles_in_place(vector, paths);
				return;
			}
		}

		self.apply_through_working_form(vector, paths);
	}

	/// Applies this modification by flattening to the legacy working form, editing that, and weaving the result back.
	fn apply_through_working_form(&self, vector: &mut Vector, paths: &mut List<Path>) {
		let mut working = WorkingVector::flatten(vector, paths);
		self.points.apply(&mut working);
		self.segments.apply(&mut working, &self.path_breaks);

		let surviving_segments: FxHashSet<SegmentId> = working.segment_ids.iter().copied().collect();
		let valid = |val: &[HandleId; 2]| surviving_segments.contains(&val[0].segment) && surviving_segments.contains(&val[1].segment);
		working
			.colinear
			.retain(|val| !self.remove_g1_continuous.contains(val) && !self.remove_g1_continuous.contains(&[val[1], val[0]]) && valid(val));

		// Both orientations go in, since a pair and its reverse name the same tangent
		let mut present: FxHashSet<[HandleId; 2]> = working.colinear.iter().flat_map(|&[first, second]| [[first, second], [second, first]]).collect();
		for handles in &self.add_g1_continuous {
			if !present.contains(handles) && valid(handles) {
				working.colinear.push(*handles);
				present.extend([*handles, [handles[1], handles[0]]]);
			}
		}

		working.reweave(vector, paths);
	}

	/// Add a [`VectorModificationType`] to this modification.
	pub fn modify(&mut self, vector_modification: &VectorModificationType) {
		// Saving would write a non-finite value as `null`, which can't be read back when opening the document
		if !vector_modification.is_finite() {
			warn!("Ignoring a vector modification with a non-finite value: {vector_modification:?}");
			return;
		}

		match vector_modification {
			VectorModificationType::InsertSegment { id, points, handles } => self.segments.push(*id, *points, *handles),
			VectorModificationType::InsertPoint { id, position } => self.points.push(*id, *position),

			VectorModificationType::RemoveSegment { id } => {
				self.segments.remove(*id);
				self.path_breaks.remove(id);
			}
			VectorModificationType::RemovePoint { id } => self.points.remove(*id),

			VectorModificationType::SetG1Continuous { handles, enabled } => {
				if *enabled {
					if !self.add_g1_continuous.contains(&[handles[1], handles[0]]) {
						self.add_g1_continuous.insert(*handles);
					}
					self.remove_g1_continuous.remove(handles);
					self.remove_g1_continuous.remove(&[handles[1], handles[0]]);
				} else {
					if !self.remove_g1_continuous.contains(&[handles[1], handles[0]]) {
						self.remove_g1_continuous.insert(*handles);
					}
					self.add_g1_continuous.remove(handles);
					self.add_g1_continuous.remove(&[handles[1], handles[0]]);
				}
			}
			VectorModificationType::SetHandles { segment, handles } => {
				self.segments.handle_primary.insert(*segment, handles[0]);
				self.segments.handle_end.insert(*segment, handles[1]);
			}
			VectorModificationType::SetHandlePosition { handle, relative_position } => {
				self.segments.handle_side_of(*handle).insert(handle.segment, Some(*relative_position));
			}
			VectorModificationType::SetEndpoint { handle, id } => {
				self.segments.endpoint_side_of(*handle).insert(handle.segment, *id);
			}
			VectorModificationType::SetPathBreak { segment, breaks } => {
				// Only a segment this modification adds can begin a path of its own
				if *breaks && self.segments.add.contains(segment) {
					self.path_breaks.insert(*segment);
				} else {
					self.path_breaks.remove(segment);
				}
			}

			VectorModificationType::ApplyPointDelta { point, delta } => {
				*self.points.delta.entry(*point).or_default() += *delta;
			}
		}
	}

	/// Create a new modification that will convert an empty [`Vector`] into the target [`Vector`].
	pub fn create_from_vector(vector: &Vector, paths: &List<Path>) -> Self {
		let working = WorkingVector::flatten(vector, paths);
		let begins_path = working.segment_ids.iter().zip(&working.segment_begins_path).filter(|&(_, &begins)| begins).map(|(&id, _)| id);
		Self {
			points: PointModification::create_from_working(&working),
			segments: SegmentModification::create_from_working(&working),
			add_g1_continuous: working.colinear.iter().copied().collect(),
			remove_g1_continuous: HashSet::new(),
			path_breaks: begins_path.collect(),
			turned_on_load: HashSet::new(),
		}
	}

	// TODO: Eventually remove this document upgrade code
	/// Chains the added segments of a modification stored as an unordered soup into paths: reorders them, turns around
	/// the ones running against their chain along with their handles and G1 pairs, and records where each chain begins.
	fn chain_legacy_segments(&mut self) {
		// Segments missing any of their parts are never applied, so they sit out of the chaining and follow the chained ones
		let segments = &self.segments;
		let mut seen = FxHashSet::default();
		let (mut complete, mut unchainable) = (Vec::new(), Vec::new());
		for &id in &segments.add {
			if !seen.insert(id) {
				continue;
			}
			let parts = (
				segments.start_point.get(&id),
				segments.end_point.get(&id),
				segments.handle_primary.get(&id),
				segments.handle_end.get(&id),
			);
			match parts {
				(Some(&start), Some(&end), Some(&primary), Some(&end_handle)) => complete.push((id, start, end, primary, end_handle)),
				_ => unchainable.push(id),
			}
		}

		// Points are numbered the way they used to be stored, upstream ones first and then the added ones in the order they
		// were added. Only the added ones have positions here. What else meets an upstream point is unknown, so each
		// segment end there is numbered apart and pairs with nothing, leaving it to be joined once upstream is known.
		let added: FxHashMap<PointId, (usize, DVec2)> = self.points.add.iter().enumerate().filter_map(|(place, id)| Some((*id, (place, *self.points.delta.get(id)?)))).collect();
		let upstream_ends = complete.iter().flat_map(|&(_, start, end, ..)| [start, end]).filter(|point| !added.contains_key(point)).count();
		let mut next_upstream_end = 0;
		let mut index_of = |point: PointId| match added.get(&point) {
			Some(&(place, _)) => upstream_ends + place,
			None => {
				next_upstream_end += 1;
				next_upstream_end - 1
			}
		};

		let mut positions = vec![None; upstream_ends + self.points.add.len()];
		for &(place, position) in added.values() {
			positions[upstream_ends + place] = Some(position);
		}
		let chainable: Vec<_> = complete
			.iter()
			.map(|&(id, start, end, primary, end_handle)| (id, index_of(start), index_of(end), primary, end_handle))
			.collect();
		let endpoints: Vec<(usize, usize)> = chainable.iter().map(|&(_, start, end, ..)| (start, end)).collect();

		// A cubic's handles are stored relative to their own anchors, and a quadratic's one handle relative to its start. A segment
		// leaves an anchor toward its nearest control point not sitting on that anchor, as legacy vectors are chained too.
		let nonzero = |along: DVec2| (along.length_squared() > 1e-12).then_some(along);
		let direction = |segment: usize, arriving: bool| {
			let (_, start, end, primary, end_handle) = chainable[segment];
			let (start, end) = (positions[start], positions[end]);
			let across = |from: Option<DVec2>, offset: DVec2, to: Option<DVec2>| from.zip(to).map(|(from, to)| from + offset - to);

			// Each handle as an offset from this end's anchor, nearest first, then the other anchor
			let toward = match (primary, end_handle) {
				(Some(primary), Some(end_handle)) if arriving => [Some(end_handle), across(start, primary, end)],
				(Some(primary), Some(end_handle)) => [Some(primary), across(end, end_handle, start)],
				(Some(handle), None) | (None, Some(handle)) if arriving => [across(start, handle, end), None],
				(Some(handle), None) | (None, Some(handle)) => [Some(handle), None],
				(None, None) => [None, None],
			};
			let chord = if arriving { across(start, DVec2::ZERO, end) } else { across(end, DVec2::ZERO, start) };

			toward.into_iter().chain([chord]).flatten().find_map(nonzero)
		};

		// A segment touching an upstream point keeps its direction, since what upstream carries on into it is unknown, and so
		// does a quadratic, since another modification's offset for its lone handle could not be re-based onto the other end
		let reversible = |segment: usize| {
			let (_, start, end, primary, end_handle) = chainable[segment];
			positions[start].is_some() && positions[end].is_some() && primary.is_some() == end_handle.is_some()
		};
		let chained = crate::vector::flatten::chain_order(positions.len(), &endpoints, direction, reversible);

		let mut reversed_ids = FxHashSet::default();
		let mut path_breaks = HashSet::new();
		let mut add = Vec::with_capacity(segments.add.len());
		let mut turned = Vec::new();
		for (&(segment, reversed), &begins) in chained.order.iter().zip(&chained.begins_path) {
			let (id, start, end, primary, end_handle) = chainable[segment];
			add.push(id);

			// A chain setting out from an upstream point may be carrying on a path that ends there, so only the rest begin paths
			let sets_out_from = if reversed { end } else { start };
			if begins && positions[sets_out_from].is_some() {
				path_breaks.insert(id);
			}

			if reversed {
				reversed_ids.insert(id);
				turned.push((id, (end_handle, primary)));
			}
		}
		add.extend(unchainable);

		self.segments.add = add;
		for (id, (primary, end_handle)) in turned {
			let (start, end) = (self.segments.start_point.remove(&id), self.segments.end_point.remove(&id));
			if let (Some(start), Some(end)) = (start, end) {
				self.segments.start_point.insert(id, end);
				self.segments.end_point.insert(id, start);
			}
			self.segments.handle_primary.insert(id, primary);
			self.segments.handle_end.insert(id, end_handle);
		}
		self.path_breaks = path_breaks;

		// A turned segment's handles swap sides, so the G1 pairs naming them follow
		let follow = |pairs: &mut HashSet<[HandleId; 2]>| {
			*pairs = pairs
				.drain()
				.map(|pair| pair.map(|handle| if reversed_ids.contains(&handle.segment) { handle.opposite() } else { handle }))
				.collect();
		};
		follow(&mut self.add_g1_continuous);
		follow(&mut self.remove_g1_continuous);

		self.turned_on_load = reversed_ids.into_iter().collect();
	}

	// TODO: Eventually remove this document upgrade code
	/// Records the segments a legacy conversion turned around before this modification was captured from its result.
	pub fn with_segments_turned_on_load(mut self, turned: HashSet<SegmentId>) -> Self {
		self.turned_on_load = turned;
		self
	}

	// TODO: Eventually remove this document upgrade code
	/// Takes the record of the segments this modification turned around when it was loaded.
	pub fn take_segments_turned_on_load(&mut self) -> HashSet<SegmentId> {
		std::mem::take(&mut self.turned_on_load)
	}

	// TODO: Eventually remove this document upgrade code
	/// The segments this modification turned around when it was loaded.
	pub fn segments_turned_on_load(&self) -> &HashSet<SegmentId> {
		&self.turned_on_load
	}

	// TODO: Eventually remove this document upgrade code
	/// Turns this modification's edits around to match segments another modification turned when it was loaded, so each edit
	/// stays on the side of its segment it was made on. Returns whether anything changed.
	pub fn follow_segments_turned_elsewhere(&mut self, turned: &HashSet<SegmentId>) -> bool {
		let own: HashSet<SegmentId> = self.segments.add.iter().copied().collect();
		let named_elsewhere = |segment: &SegmentId| turned.contains(segment) && !own.contains(segment);
		let mut changed = false;

		// Only the segments this modification names by side can need turning, so this is linear in its own edits
		let segments = &self.segments;
		let named: HashSet<SegmentId> = [&segments.handle_primary, &segments.handle_end]
			.into_iter()
			.flat_map(|handles| handles.keys())
			.chain([&segments.start_point, &segments.end_point].into_iter().flat_map(|points| points.keys()))
			.copied()
			.filter(named_elsewhere)
			.collect();

		for segment in named {
			let (primary, end_handle) = (self.segments.handle_primary.remove(&segment), self.segments.handle_end.remove(&segment));
			changed |= primary.is_some() || end_handle.is_some();
			if let Some(primary) = primary {
				self.segments.handle_end.insert(segment, primary);
			}
			if let Some(end_handle) = end_handle {
				self.segments.handle_primary.insert(segment, end_handle);
			}

			let (start, end) = (self.segments.start_point.remove(&segment), self.segments.end_point.remove(&segment));
			changed |= start.is_some() || end.is_some();
			if let Some(start) = start {
				self.segments.end_point.insert(segment, start);
			}
			if let Some(end) = end {
				self.segments.start_point.insert(segment, end);
			}
		}

		for pairs in [&mut self.add_g1_continuous, &mut self.remove_g1_continuous] {
			if pairs.iter().flatten().any(|handle| named_elsewhere(&handle.segment)) {
				changed = true;
				*pairs = pairs
					.drain()
					.map(|pair| pair.map(|handle| if named_elsewhere(&handle.segment) { handle.opposite() } else { handle }))
					.collect();
			}
		}

		changed
	}
}

// Intentionally non-deterministic: fields contain HashMaps with non-deterministic iteration order,
// so we use a UUID to always bust the cache and force re-evaluation when any modification is present.
// This will not actually lead to a cache invalidation in most cases due to the
// graph inputs being wrapped in a `MemoHash` wrapper.
impl graphene_hash::CacheHash for VectorModification {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(&generate_uuid(), state);
	}
}

// TODO: Do we want to enforce that all serialized/deserialized hashmaps are a vec of tuples?
// TODO: Eventually remove this document upgrade code
/// Serializes as sorted `[[key, value], ...]` (sequence of pairs)
pub fn serialize_hashmap<K, V, S, H>(hashmap: &HashMap<K, V, H>, serializer: S) -> Result<S::Ok, S::Error>
where
	K: Serialize + Eq + Hash + Ord,
	V: Serialize,
	S: Serializer,
	H: BuildHasher,
{
	// Sort entries by key so the serialized output is deterministic across runs (HashMap iteration order is randomized).
	// Removes a major source of churn in saved-document diffs without affecting load behavior.
	let mut entries: Vec<_> = hashmap.iter().collect();
	entries.sort_by(|a, b| a.0.cmp(b.0));

	let mut seq = serializer.serialize_seq(Some(entries.len()))?;
	for (key, value) in entries {
		seq.serialize_element(&(key, value))?;
	}
	seq.end()
}

/// Serializes as sorted `{"key": value, ...}` (JSON object)
pub fn serialize_hashmap_as_sorted_object<K, V, S, H>(hashmap: &HashMap<K, V, H>, serializer: S) -> Result<S::Ok, S::Error>
where
	K: Serialize + Eq + Hash + Ord,
	V: Serialize,
	S: Serializer,
	H: BuildHasher,
{
	use serde::ser::SerializeMap;

	let mut entries: Vec<_> = hashmap.iter().collect();
	entries.sort_by(|a, b| a.0.cmp(b.0));

	let mut map = serializer.serialize_map(Some(entries.len()))?;
	for (key, value) in entries {
		map.serialize_entry(key, value)?;
	}
	map.end()
}

/// Serializes as sorted `[value, ...]` (JSON array)
pub(crate) fn serialize_hashset<T, S, H>(set: &HashSet<T, H>, serializer: S) -> Result<S::Ok, S::Error>
where
	T: Serialize + Eq + Hash + Ord,
	S: Serializer,
	H: BuildHasher,
{
	let mut entries: Vec<_> = set.iter().collect();
	entries.sort();

	let mut seq = serializer.serialize_seq(Some(entries.len()))?;
	for value in entries {
		seq.serialize_element(value)?;
	}
	seq.end()
}

/// Serializes sorted `[value, ...]` as a present optional, matching the field being read back as an `Option` in formats that tag one.
pub(crate) fn serialize_hashset_as_some<T, S, H>(set: &HashSet<T, H>, serializer: S) -> Result<S::Ok, S::Error>
where
	T: Serialize + Eq + Hash + Ord,
	S: Serializer,
	H: BuildHasher,
{
	let mut entries: Vec<_> = set.iter().collect();
	entries.sort();

	serializer.serialize_some(&entries)
}

pub fn deserialize_hashmap<'de, K, V, D, H>(deserializer: D) -> Result<HashMap<K, V, H>, D::Error>
where
	K: Deserialize<'de> + Eq + Hash,
	V: Deserialize<'de>,
	D: Deserializer<'de>,
	H: BuildHasher + Default,
{
	struct HashMapVisitor<K, V, H> {
		#[allow(clippy::type_complexity)]
		marker: std::marker::PhantomData<fn() -> HashMap<K, V, H>>,
	}

	impl<'de, K, V, H> Visitor<'de> for HashMapVisitor<K, V, H>
	where
		K: Deserialize<'de> + Eq + Hash,
		V: Deserialize<'de>,
		H: BuildHasher + Default,
	{
		type Value = HashMap<K, V, H>;

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			formatter.write_str("a sequence of tuples")
		}

		fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
		where
			A: SeqAccess<'de>,
		{
			let mut hashmap = HashMap::default();
			while let Some((key, value)) = seq.next_element()? {
				hashmap.insert(key, value);
			}
			Ok(hashmap)
		}
	}

	let visitor = HashMapVisitor { marker: std::marker::PhantomData };
	deserializer.deserialize_seq(visitor)
}

pub trait HandleExt {
	/// Set the handle's position relative to the anchor which is the start anchor for the primary handle and end anchor for the end handle.
	#[must_use]
	fn set_relative_position(self, relative_position: DVec2) -> VectorModificationType;
}

impl HandleExt for HandleId {
	fn set_relative_position(self, relative_position: DVec2) -> VectorModificationType {
		VectorModificationType::SetHandlePosition { handle: self, relative_position }
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::vector::algorithms::shapes::{ellipse_bezpath, rectangle_bezpath};
	use kurbo::{BezPath, PathSeg, Point, QuadBez};

	fn anchor_id(vector: &Vector, paths: &List<Path>, anchor_index: usize) -> PointId {
		vector.anchor_points(paths).nth(anchor_index).expect("anchor exists")
	}

	fn anchor_position(vector: &Vector, paths: &List<Path>, anchor_index: usize) -> DVec2 {
		let id = anchor_id(vector, paths, anchor_index);
		vector.position(vector.index_of_point_id(id).unwrap() as usize).unwrap()
	}

	/// What the tests compare of a modification's result: anchors by ID, segments by ID, the stroke contours, and G1 pairs.
	fn describe(vector: &Vector, paths: &List<Path>) -> String {
		let mut anchors: Vec<String> = vector
			.anchor_points(paths)
			.map(|id| format!("{id:?}@{:?}", vector.index_of_point_id(id).and_then(|index| vector.position(index as usize))))
			.collect();
		anchors.sort();

		let mut segments: Vec<String> = vector.segment_iter(paths).map(|(id, segment, start, end)| format!("{id:?}:{start:?}>{end:?}:{segment:?}")).collect();
		segments.sort();

		let strokes: Vec<String> = vector.stroke_bezpath_iter(paths).map(|bezpath| bezpath.to_svg()).collect();

		let mut pairs: Vec<String> = vector
			.colinear_pairs(paths)
			.into_iter()
			.map(|pair| {
				let mut handles = pair.map(|handle| format!("{handle:?}"));
				handles.sort();
				handles.join("~")
			})
			.collect();
		pairs.sort();

		format!("{anchors:?}\n{segments:?}\n{strokes:?}\n{pairs:?}")
	}

	/// Every curve type, a weld where they change, a smooth anchor, and a second subpath.
	fn mixed_geometry() -> (Vector, List<Path>) {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.curve_to((10., -10.), (20., -10.), (30., 0.));
		bezpath.line_to((30., 30.));
		bezpath.quad_to((15., 45.), (0., 30.));
		bezpath.close_path();
		bezpath.move_to((100., 100.));
		bezpath.curve_to((110., 90.), (120., 90.), (130., 100.));

		let (mut vector, mut paths) = Vector::from_bezpath(&bezpath);
		let smooth_anchor = anchor_id(&vector, &paths, 1);
		vector.set_smooth(&mut paths, smooth_anchor, true);

		(vector, paths)
	}

	// Moving points and handles where they lie gives the same result as flattening and reweaving
	#[test]
	fn moving_in_place_matches_the_working_form() {
		let (vector, paths) = mixed_geometry();
		let anchors: Vec<PointId> = vector.anchor_points(&paths).collect();
		let segments: Vec<SegmentId> = vector.segment_iter(&paths).map(|(id, ..)| id).collect();

		let mut modification = VectorModification::default();
		for (index, &anchor) in anchors.iter().enumerate() {
			let delta = DVec2::new(index as f64, -(index as f64) * 2.);
			modification.modify(&VectorModificationType::ApplyPointDelta { point: anchor, delta });
		}
		let handle_edits = [
			(HandleId::primary(segments[0]), DVec2::new(3., 4.)),
			(HandleId::primary(segments[2]), DVec2::new(-5., 6.)),
			(HandleId::end(segments[4]), DVec2::new(7., -8.)),
		];
		for (handle, relative_position) in handle_edits {
			modification.modify(&VectorModificationType::SetHandlePosition { handle, relative_position });
		}

		let (mut in_place, mut in_place_paths) = (vector.clone(), paths.clone());
		modification.apply(&mut in_place, &mut in_place_paths);
		let (mut rewoven, mut rewoven_paths) = (vector.clone(), paths.clone());
		modification.apply_through_working_form(&mut rewoven, &mut rewoven_paths);

		assert_eq!(describe(&in_place, &in_place_paths), describe(&rewoven, &rewoven_paths));
		assert_ne!(describe(&in_place, &in_place_paths), describe(&vector, &paths));
		assert_eq!(in_place_paths, paths, "moving points leaves the weave untouched");
	}

	// An edit that turns a line into a curve changes the weave, so it still goes through the working form
	#[test]
	fn curve_type_changes_still_apply() {
		let (mut vector, mut paths) = mixed_geometry();
		let line = vector.segment_iter(&paths).nth(1).map(|(id, ..)| id).unwrap();

		let mut modification = VectorModification::default();
		let handles = [Some(DVec2::new(1., 1.)), Some(DVec2::new(-1., -1.))];
		modification.modify(&VectorModificationType::SetHandles { segment: line, handles });
		modification.apply(&mut vector, &mut paths);

		let curved = vector.segment_iter(&paths).find(|&(id, ..)| id == line).map(|(_, segment, ..)| segment);
		assert!(matches!(curved, Some(PathSeg::Cubic(_))));
	}

	// Every way of editing a segment's two handles gives the same result in place as through the working form
	#[test]
	fn every_handle_edit_matches_the_working_form() {
		let mut bezpath = BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.line_to((10., 0.));
		bezpath.move_to((0., 20.));
		bezpath.quad_to((5., 30.), (10., 20.));
		bezpath.move_to((0., 40.));
		bezpath.curve_to((3., 50.), (7., 50.), (10., 40.));
		let (vector, paths) = Vector::from_bezpath(&bezpath);

		let edits = [None, Some(None), Some(Some(DVec2::new(2., 3.)))];
		for (segment, ..) in vector.segment_iter(&paths) {
			for (primary, end) in edits.into_iter().flat_map(|primary| edits.map(|end| (primary, end))) {
				let mut modification = VectorModification::default();
				if let Some(primary) = primary {
					modification.segments.handle_primary.insert(segment, primary);
				}
				if let Some(end) = end {
					modification.segments.handle_end.insert(segment, end);
				}

				let (mut in_place, mut in_place_paths) = (vector.clone(), paths.clone());
				modification.apply(&mut in_place, &mut in_place_paths);
				let (mut rewoven, mut rewoven_paths) = (vector.clone(), paths.clone());
				modification.apply_through_working_form(&mut rewoven, &mut rewoven_paths);

				assert_eq!(
					describe(&in_place, &in_place_paths),
					describe(&rewoven, &rewoven_paths),
					"{segment:?} edited by {primary:?} and {end:?}"
				);
			}
		}
	}

	// A handle shared by two segments is unwelded by a move, as the working form does, instead of moving for both
	#[test]
	fn a_shared_handle_is_rewoven_instead_of_moved_twice() {
		use crate::vector::path::push_path;
		use core_types::list::Item;

		let ids = [PointId::generate(), PointId::generate(), PointId::generate(), PointId::generate()];
		let anchors = [DVec2::new(0., 0.), DVec2::new(10., 0.), DVec2::new(0., 10.), DVec2::new(10., 10.)];
		let mut vector = Vector::default();
		for (&position, id) in anchors.iter().zip(ids) {
			vector.0.push(Item::new_from_element(position).with_attribute(ATTR_ID, id));
		}
		for position in [DVec2::new(5., 5.), DVec2::new(8., -2.), DVec2::new(8., 12.)] {
			vector.0.push(Item::new_from_element(position).with_attribute(ATTR_ID, PointId::UNNAMED));
		}
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Cubic, [0, 4, 5, 1], [SegmentId::generate()]);
		push_path(&mut paths, CurveType::Cubic, [2, 4, 6, 3], [SegmentId::generate()]);

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::ApplyPointDelta { point: ids[0], delta: DVec2::X });
		modification.modify(&VectorModificationType::ApplyPointDelta { point: ids[2], delta: DVec2::Y });

		let (mut applied, mut applied_paths) = (vector.clone(), paths.clone());
		modification.apply(&mut applied, &mut applied_paths);
		let (mut rewoven, mut rewoven_paths) = (vector.clone(), paths.clone());
		modification.apply_through_working_form(&mut rewoven, &mut rewoven_paths);

		assert_eq!(describe(&applied, &applied_paths), describe(&rewoven, &rewoven_paths));
	}

	// A delta that isn't finite moves nothing, and neither do the handles beside it, while a finite one beside it still applies
	#[test]
	fn non_finite_deltas_are_ignored_in_place() {
		let (vector, paths) = mixed_geometry();
		let [skipped, moved] = [anchor_id(&vector, &paths, 0), anchor_id(&vector, &paths, 1)];

		// Stored directly, since recording an edit already discards a non-finite value
		let mut modification = VectorModification::default();
		modification.points.delta.insert(skipped, DVec2::new(f64::NAN, 0.));
		modification.points.delta.insert(moved, DVec2::new(3., 4.));

		let (mut in_place, mut in_place_paths) = (vector.clone(), paths.clone());
		modification.apply(&mut in_place, &mut in_place_paths);
		let (mut rewoven, mut rewoven_paths) = (vector.clone(), paths.clone());
		modification.apply_through_working_form(&mut rewoven, &mut rewoven_paths);

		assert_eq!(describe(&in_place, &in_place_paths), describe(&rewoven, &rewoven_paths));
		assert_eq!(anchor_position(&in_place, &in_place_paths, 0), anchor_position(&vector, &paths, 0));
		assert_eq!(anchor_position(&in_place, &in_place_paths, 1), anchor_position(&vector, &paths, 1) + DVec2::new(3., 4.));
	}

	// A point that is a handle in one path and an anchor in another has two ways to move, so it goes through the working form
	#[test]
	fn a_point_both_handle_and_anchor_is_rewoven_instead_of_moved_in_place() {
		use crate::vector::path::push_path;
		use core_types::list::Item;

		let ids: [PointId; 4] = std::array::from_fn(|_| PointId::generate());
		let anchors = [DVec2::new(0., 0.), DVec2::new(10., 0.), DVec2::new(5., 5.), DVec2::new(5., 20.)];
		let mut vector = Vector::default();
		for (position, id) in anchors.into_iter().zip(ids) {
			vector.0.push(Item::new_from_element(position).with_attribute(ATTR_ID, id));
		}
		vector.0.push(Item::new_from_element(DVec2::new(8., -2.)).with_attribute(ATTR_ID, PointId::UNNAMED));

		// Point 2 is the cubic's first handle and the line's start
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Cubic, [0, 2, 4, 1], [SegmentId::generate()]);
		push_path(&mut paths, CurveType::Linear, [2, 3], [SegmentId::generate()]);
		assert!(!vector.point_roles(&paths).1, "a point with both roles should rule out moving in place");

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::ApplyPointDelta { point: ids[0], delta: DVec2::X });
		modification.modify(&VectorModificationType::ApplyPointDelta { point: ids[2], delta: DVec2::Y });

		let (mut applied, mut applied_paths) = (vector.clone(), paths.clone());
		modification.apply(&mut applied, &mut applied_paths);
		let (mut rewoven, mut rewoven_paths) = (vector.clone(), paths.clone());
		modification.apply_through_working_form(&mut rewoven, &mut rewoven_paths);

		assert_eq!(describe(&applied, &applied_paths), describe(&rewoven, &rewoven_paths));
	}

	// A free point and an anchor where three segments meet move in place just as the working form moves them
	#[test]
	fn free_points_and_junctions_move_in_place_like_the_working_form() {
		use crate::vector::path::push_path;

		let (mut vector, mut paths) = mixed_geometry();
		for position in [(200., 200.), (-20., -20.)] {
			let mut lone_point = BezPath::new();
			lone_point.move_to(position);
			vector.append_bezpath(&mut paths, &lone_point);
		}

		// The closed loop starts at pool point 0, so a line from there to the second lone point makes it a junction
		let spur_end = vector.point_count() as u32 - 1;
		push_path(&mut paths, CurveType::Linear, [0, spur_end], [SegmentId::generate()]);

		let mut modification = VectorModification::default();
		for (index, anchor) in vector.anchor_points(&paths).enumerate() {
			let delta = DVec2::new(index as f64 + 1., 2.);
			modification.modify(&VectorModificationType::ApplyPointDelta { point: anchor, delta });
		}

		let (mut in_place, mut in_place_paths) = (vector.clone(), paths.clone());
		modification.apply(&mut in_place, &mut in_place_paths);
		let (mut rewoven, mut rewoven_paths) = (vector.clone(), paths.clone());
		modification.apply_through_working_form(&mut rewoven, &mut rewoven_paths);

		assert_eq!(describe(&in_place, &in_place_paths), describe(&rewoven, &rewoven_paths));
		assert_eq!(in_place_paths, paths, "moving points leaves the weave untouched");
	}

	// Removing the segment a path begins with leaves the rest of that path beginning one of its own, rather than joining the path it touches
	#[test]
	fn a_path_keeps_its_start_when_its_first_segment_is_removed() {
		use crate::vector::path::push_path;
		use core_types::list::Item;

		let mut vector = Vector::default();
		for position in [(0., 0.), (10., 0.), (20., 0.), (20., 10.), (30., 0.)] {
			vector.0.push(Item::new_from_element(DVec2::from(position)).with_attribute(ATTR_ID, PointId::generate()));
		}
		let mut paths = List::new();
		let removed = SegmentId::generate();
		push_path(&mut paths, CurveType::Linear, [0, 1, 2], [SegmentId::generate(), SegmentId::generate()]);
		push_path(&mut paths, CurveType::Linear, [3, 2, 4], [removed, SegmentId::generate()]);

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::RemoveSegment { id: removed });
		modification.apply(&mut vector, &mut paths);

		assert_eq!(paths.len(), 2, "the remainder of the second path stays a path of its own");
	}

	// Re-adding a segment or point under an ID the geometry already holds adds no second copy under that ID
	#[test]
	fn duplicate_adds_are_skipped() {
		use kurbo::{Rect, Shape};

		let (mut vector, mut paths) = Vector::from_bezpath(&Rect::new(0., 0., 10., 10.).to_path(0.));
		let segment = vector.segment_iter(&paths).next().map(|(id, ..)| id).unwrap();
		let [start, end] = [anchor_id(&vector, &paths, 0), anchor_id(&vector, &paths, 2)];

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::InsertSegment {
			id: segment,
			points: [start, end],
			handles: [None, None],
		});
		modification.modify(&VectorModificationType::InsertPoint {
			id: start,
			position: DVec2::new(99., 99.),
		});
		modification.apply(&mut vector, &mut paths);

		assert_eq!(vector.segment_iter(&paths).count(), 4);
		assert_eq!(vector.anchor_points(&paths).count(), 4);
		let ids: HashSet<_> = vector.segment_iter(&paths).map(|(id, ..)| id).collect();
		assert_eq!(ids.len(), 4);
	}

	// An end handle addresses the end of its segment, both where its handle sits and which point that end connects to
	#[test]
	fn end_handles_address_the_end_of_their_segment() {
		let mut open = BezPath::new();
		open.move_to(Point::new(0., 0.));
		open.curve_to(Point::new(1., 1.), Point::new(2., 1.), Point::new(3., 0.));

		let (vector, paths) = Vector::from_bezpath(&open);
		let segment = vector.segment_iter(&paths).next().map(|(id, ..)| id).unwrap();
		let new_end = PointId::generate();

		let mut modification = VectorModification::create_from_vector(&vector, &paths);
		modification.modify(&VectorModificationType::InsertPoint {
			id: new_end,
			position: DVec2::new(10., 5.),
		});
		modification.modify(&VectorModificationType::SetEndpoint {
			handle: HandleId::end(segment),
			id: new_end,
		});
		modification.modify(&HandleId::end(segment).set_relative_position(DVec2::new(1., 2.)));

		let (mut applied, mut applied_paths) = (Vector::default(), List::new());
		modification.apply(&mut applied, &mut applied_paths);

		let (_, curve, start, end) = applied.segment_iter(&applied_paths).find(|&(id, ..)| id == segment).unwrap();
		assert_eq!(curve, PathSeg::Cubic(kurbo::CubicBez::new((0., 0.), (1., 1.), (11., 7.), (10., 5.))));
		assert_eq!(start, anchor_id(&vector, &paths, 0));
		assert_eq!(end, new_end);
	}

	// A pair and its reverse name the same tangent, so removing either order clears a pair the incoming geometry holds
	#[test]
	fn g1_pair_removal_ignores_the_order_of_its_handles() {
		let mut open = BezPath::new();
		open.move_to(Point::new(0., 0.));
		open.curve_to(Point::new(1., 1.), Point::new(2., 1.), Point::new(3., 0.));
		open.curve_to(Point::new(4., -1.), Point::new(5., -1.), Point::new(6., 0.));

		let (mut smooth, mut smooth_paths) = Vector::from_bezpath(&open);
		let middle = anchor_id(&smooth, &smooth_paths, 1);
		smooth.set_smooth(&mut smooth_paths, middle, true);
		let pairs = smooth.colinear_pairs(&smooth_paths);
		let [pair] = pairs[..] else { panic!("the middle anchor should be smooth, not {pairs:?}") };

		for removed in [pair, [pair[1], pair[0]]] {
			let (mut vector, mut paths) = (smooth.clone(), smooth_paths.clone());
			let mut modification = VectorModification::default();
			modification.remove_g1_continuous.insert(removed);
			modification.apply(&mut vector, &mut paths);

			assert!(vector.colinear_pairs(&paths).is_empty(), "removing {removed:?} should clear the tangent");
		}
	}

	#[test]
	fn modify_new() {
		let (mut vector, mut paths) = Vector::from_bezpath(&ellipse_bezpath(DVec2::ZERO, DVec2::ONE));
		vector.append_bezpath(&mut paths, &rectangle_bezpath(DVec2::NEG_ONE, DVec2::ZERO));

		let modify = VectorModification::create_from_vector(&vector, &paths);

		let (mut new, mut new_paths) = (Vector::default(), List::new());
		modify.apply(&mut new, &mut new_paths);

		assert_eq!(
			vector.segment_iter(&paths).collect::<Vec<_>>(),
			new.segment_iter(&new_paths).collect::<Vec<_>>(),
			"replaying the capture should rebuild every segment"
		);
		assert_eq!(vector.anchor_points(&paths).collect::<Vec<_>>(), new.anchor_points(&new_paths).collect::<Vec<_>>());
	}

	#[test]
	fn modify_existing() {
		let mut open_quads = BezPath::new();
		open_quads.move_to(Point::new(0., 0.));
		open_quads.quad_to(Point::new(5., 10.), Point::new(10., 0.));
		open_quads.quad_to(Point::new(15., 10.), Point::new(20., 0.));

		let (mut vector, mut paths) = Vector::from_bezpath(&ellipse_bezpath(DVec2::ZERO, DVec2::ONE));
		vector.append_bezpath(&mut paths, &rectangle_bezpath(DVec2::NEG_ONE, DVec2::ZERO));
		vector.append_bezpath(&mut paths, &open_quads);

		let mut modify_new = VectorModification::create_from_vector(&vector, &paths);
		let mut modify_original = VectorModification::default();

		for modification in [&mut modify_new, &mut modify_original] {
			let point = anchor_id(&vector, &paths, 0);
			modification.modify(&VectorModificationType::ApplyPointDelta { point, delta: DVec2::X * 0.5 });
			let point = anchor_id(&vector, &paths, 9);
			modification.modify(&VectorModificationType::ApplyPointDelta { point, delta: DVec2::X });
		}

		let (mut new, mut new_paths) = (Vector::default(), List::new());
		modify_new.apply(&mut new, &mut new_paths);

		modify_original.apply(&mut vector, &mut paths);

		assert_eq!(vector.segment_iter(&paths).collect::<Vec<_>>(), new.segment_iter(&new_paths).collect::<Vec<_>>());
		assert_eq!(anchor_position(&vector, &paths, 0), DVec2::X);
		assert_eq!(anchor_position(&vector, &paths, 9), DVec2::new(11., 0.));
		assert_eq!(
			vector.segment_iter(&paths).nth(8).unwrap().1,
			PathSeg::Quad(QuadBez::new(Point::new(0., 0.), Point::new(5., 10.), Point::new(11., 0.)))
		);
		assert_eq!(
			vector.segment_iter(&paths).nth(9).unwrap().1,
			PathSeg::Quad(QuadBez::new(Point::new(11., 0.), Point::new(16., 10.), Point::new(20., 0.)))
		);
	}

	#[test]
	fn non_finite_modifications_are_ignored() {
		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::SetHandlePosition {
			handle: HandleId::primary(SegmentId::generate()),
			relative_position: DVec2::NAN,
		});
		modification.modify(&VectorModificationType::ApplyPointDelta {
			point: PointId::generate(),
			delta: DVec2::new(f64::INFINITY, 0.),
		});

		assert_eq!(modification, VectorModification::default());
	}

	/// A modification placing the points in a row and drawing a straight segment between each pair of them given.
	fn drawn(points: &[PointId], segments: &[(SegmentId, usize, usize)]) -> VectorModification {
		let mut modification = VectorModification::default();
		for (index, &id) in points.iter().enumerate() {
			let position = DVec2::new(index as f64 * 10., 0.);
			modification.modify(&VectorModificationType::InsertPoint { id, position });
		}
		for &(id, start, end) in segments {
			let points = [points[start], points[end]];
			modification.modify(&VectorModificationType::InsertSegment { id, points, handles: [None, None] });
		}

		modification
	}

	/// The anchors each path visits once the modification is applied to nothing, as places in the given points.
	fn woven_paths(modification: &VectorModification, points: &[PointId]) -> Vec<Vec<usize>> {
		let (mut vector, mut paths) = (Vector::default(), List::new());
		modification.apply(&mut vector, &mut paths);

		let place = |point: u32| vector.point_id_at(point).and_then(|id| points.iter().position(|&candidate| candidate == id)).expect("a known anchor");
		paths.iter_element_values().map(|path| path.references().map(place).collect()).collect()
	}

	fn generated<T>(count: usize, generate: fn() -> T) -> Vec<T> {
		(0..count).map(|_| generate()).collect()
	}

	// A modification stored before paths recorded where they begin is a soup, which loading chains into one path and turns around where needed
	#[test]
	fn a_stored_soup_is_chained_into_paths_on_load() {
		let (points, ids) = (generated(5, PointId::generate), generated(4, SegmentId::generate));
		let soup = drawn(&points, &[(ids[0], 1, 2), (ids[1], 1, 0), (ids[2], 3, 2), (ids[3], 3, 4)]);
		assert_eq!(woven_paths(&soup, &points).len(), 4);

		let mut stored = serde_json::to_value(&soup).unwrap();
		stored.as_object_mut().unwrap().remove("path_breaks");
		let loaded: VectorModification = serde_json::from_value(stored).unwrap();

		assert_eq!(woven_paths(&loaded, &points), [[0, 1, 2, 3, 4]]);
		assert_eq!(loaded.path_breaks, HashSet::from([ids[1]]));
	}

	// A modification stored with its record of path beginnings loads exactly as it was saved, without being chained again
	#[test]
	fn the_path_record_survives_a_round_trip() {
		let (points, ids) = (generated(3, PointId::generate), generated(2, SegmentId::generate));
		let mut modification = drawn(&points, &[(ids[0], 1, 2), (ids[1], 1, 0)]);
		modification.modify(&VectorModificationType::SetPathBreak { segment: ids[1], breaks: true });

		let stored = serde_json::to_string(&modification).unwrap();
		let loaded: VectorModification = serde_json::from_str(&stored).unwrap();

		assert_eq!(loaded, modification);
		assert_eq!(woven_paths(&loaded, &points), [vec![1, 2], vec![1, 0]]);
	}

	// Splitting the first side of an upstream closed path, as the Path tool inserts a point, keeps one closed path beginning where it did
	#[test]
	fn splitting_an_upstream_path_beginning_keeps_the_path_whole() {
		use kurbo::{Rect, Shape};

		let (mut vector, mut paths) = Vector::from_bezpath(&Rect::new(0., 0., 10., 10.).to_path(0.));
		let (first, _, start, end) = vector.segment_iter(&paths).next().unwrap();
		let middle = PointId::generate();
		let halves = [SegmentId::generate(), SegmentId::generate()];

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::RemoveSegment { id: first });
		modification.modify(&VectorModificationType::InsertPoint {
			id: middle,
			position: DVec2::new(5., 0.),
		});
		for (id, points) in [(halves[0], [start, middle]), (halves[1], [middle, end])] {
			modification.modify(&VectorModificationType::InsertSegment { id, points, handles: [None, None] });
		}
		modification.modify(&VectorModificationType::SetPathBreak { segment: halves[0], breaks: true });
		modification.apply(&mut vector, &mut paths);

		assert_eq!(paths.len(), 1);
		assert!(paths.element(0).unwrap().is_closed());
		assert_eq!(vector.segment_iter(&paths).count(), 5);
		assert_eq!(vector.segment_iter(&paths).next().map(|(id, ..)| id), Some(halves[0]));
	}

	// Deleting a segment cuts its path, so what is left past it stays apart from another path ending at the same junction
	#[test]
	fn deleting_a_segment_beside_a_junction_keeps_the_paths_apart() {
		use crate::vector::path::push_path;
		use core_types::list::Item;

		let mut vector = Vector::default();
		for index in 0..8 {
			vector.0.push(Item::new_from_element(DVec2::new(index as f64, 0.)).with_attribute(ATTR_ID, PointId::generate()));
		}
		let mut paths = List::new();
		for references in [vec![0, 1, 2, 3], vec![5, 2], vec![6, 2, 7]] {
			let ids: Vec<SegmentId> = (1..references.len()).map(|_| SegmentId::generate()).collect();
			push_path(&mut paths, CurveType::Linear, references, ids);
		}
		let removed = vector.segment_iter(&paths).nth(1).map(|(id, ..)| id).unwrap();

		let mut modification = VectorModification::default();
		modification.modify(&VectorModificationType::RemoveSegment { id: removed });
		modification.apply(&mut vector, &mut paths);

		// Anchors are named by their x positions, which were their original pool indices
		let drawn: Vec<Vec<i32>> = paths
			.iter_element_values()
			.map(|path| path.references().map(|point| vector.position(point as usize).unwrap().x as i32).collect())
			.collect();
		assert!(drawn.contains(&vec![2, 3]), "{drawn:?}");
		assert!(drawn.contains(&vec![5, 2]), "{drawn:?}");
	}

	// A segment marked as beginning a path stays apart from the one it sets out from the end of
	#[test]
	fn a_path_break_keeps_a_stroke_apart() {
		let (points, ids) = (generated(3, PointId::generate), generated(2, SegmentId::generate));
		let mut modification = drawn(&points, &[(ids[0], 0, 1), (ids[1], 1, 2)]);
		assert_eq!(woven_paths(&modification, &points), [[0, 1, 2]]);

		modification.modify(&VectorModificationType::SetPathBreak { segment: ids[1], breaks: true });
		assert_eq!(woven_paths(&modification, &points), [vec![0, 1], vec![1, 2]]);
	}

	// A stroke drawn out of an upstream point and one drawn into it are not paired there, and each keeps its direction
	#[test]
	fn a_stored_soup_pairs_nothing_at_an_upstream_point() {
		let (points, ids) = (generated(4, PointId::generate), generated(3, SegmentId::generate));
		let mut soup = drawn(&points[1..], &[]);
		for (id, start, end) in [(ids[0], 0, 1), (ids[1], 1, 2), (ids[2], 3, 0)] {
			let points = [points[start], points[end]];
			soup.modify(&VectorModificationType::InsertSegment { id, points, handles: [None, None] });
		}

		let mut stored = serde_json::to_value(&soup).unwrap();
		stored.as_object_mut().unwrap().remove("path_breaks");
		let loaded: VectorModification = serde_json::from_value(stored).unwrap();

		// Paired at the upstream point, the stroke arriving there would have led the one leaving it
		assert_eq!(loaded.segments.add, vec![ids[0], ids[1], ids[2]]);
		assert_eq!(loaded.segments.start_point[&ids[0]], points[0]);
		assert_eq!(loaded.segments.start_point[&ids[2]], points[3]);
		assert_eq!(loaded.path_breaks, HashSet::from([ids[2]]));
	}

	// Loading turns a stored cubic around along with its handles and G1 pair, and an edit elsewhere naming it by side follows
	#[test]
	fn a_segment_turned_on_load_keeps_its_handles_and_edits_on_their_anchors() {
		let (points, ids) = (generated(3, PointId::generate), generated(2, SegmentId::generate));
		let (toward, away) = (DVec2::new(1., 2.), DVec2::new(3., 4.));
		let mut soup = drawn(&points, &[]);
		soup.modify(&VectorModificationType::InsertSegment {
			id: ids[0],
			points: [points[1], points[0]],
			handles: [Some(toward), Some(away)],
		});
		soup.modify(&VectorModificationType::InsertSegment {
			id: ids[1],
			points: [points[1], points[2]],
			handles: [None, None],
		});
		let pair = [HandleId::primary(ids[0]), HandleId::primary(ids[1])];
		soup.modify(&VectorModificationType::SetG1Continuous { handles: pair, enabled: true });

		let mut stored = serde_json::to_value(&soup).unwrap();
		stored.as_object_mut().unwrap().remove("path_breaks");
		let mut loaded: VectorModification = serde_json::from_value(stored).unwrap();

		// The first segment now runs from point 0 into point 1, each handle still on the anchor it was drawn from
		assert_eq!(loaded.segments.start_point[&ids[0]], points[0]);
		assert_eq!(loaded.segments.handle_primary[&ids[0]], Some(away));
		assert_eq!(loaded.segments.handle_end[&ids[0]], Some(toward));
		assert!(loaded.add_g1_continuous.contains(&[HandleId::end(ids[0]), HandleId::primary(ids[1])]));

		let turned = loaded.take_segments_turned_on_load();
		assert_eq!(turned, HashSet::from([ids[0]]));

		let mut downstream = VectorModification::default();
		downstream.modify(&VectorModificationType::SetHandlePosition {
			handle: HandleId::primary(ids[0]),
			relative_position: DVec2::X,
		});
		assert!(downstream.follow_segments_turned_elsewhere(&turned));
		assert_eq!(downstream.segments.handle_end.get(&ids[0]), Some(&Some(DVec2::X)));
		assert!(!downstream.segments.handle_primary.contains_key(&ids[0]));
	}

	// Capturing a vector records where its paths begin, so two paths welded head to tail come back as two
	#[test]
	fn a_captured_vector_keeps_its_paths() {
		use crate::vector::path::push_path;
		use core_types::list::Item;

		let points = generated(3, PointId::generate);
		let mut vector = Vector::default();
		for (index, &id) in points.iter().enumerate() {
			vector.0.push(Item::new_from_element(DVec2::new(index as f64 * 10., 0.)).with_attribute(ATTR_ID, id));
		}
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], [SegmentId::generate()]);
		push_path(&mut paths, CurveType::Linear, [1, 2], [SegmentId::generate()]);

		let captured = VectorModification::create_from_vector(&vector, &paths);
		assert_eq!(woven_paths(&captured, &points), [vec![0, 1], vec![1, 2]]);
	}

	// Splitting the segment a closed path begins with hands that beginning to its first half, so the path keeps its start
	#[test]
	fn a_point_inserted_on_a_paths_first_segment_keeps_its_start() {
		let (points, ids) = (generated(5, PointId::generate), generated(6, SegmentId::generate));
		let mut modification = drawn(&points[..4], &[(ids[0], 0, 1), (ids[1], 1, 2), (ids[2], 2, 3), (ids[3], 3, 0)]);
		modification.modify(&VectorModificationType::SetPathBreak { segment: ids[0], breaks: true });

		modification.modify(&VectorModificationType::InsertPoint {
			id: points[4],
			position: DVec2::new(5., 5.),
		});
		for (id, points) in [(ids[4], [points[0], points[4]]), (ids[5], [points[4], points[1]])] {
			modification.modify(&VectorModificationType::InsertSegment { id, points, handles: [None, None] });
		}
		modification.modify(&VectorModificationType::SetPathBreak { segment: ids[4], breaks: true });
		modification.modify(&VectorModificationType::RemoveSegment { id: ids[0] });

		assert_eq!(woven_paths(&modification, &points), [[0, 4, 1, 2, 3, 0]]);
	}

	// Splitting a segment removes it and stores its two halves last, and the path they sit in stays one path from the same start
	#[test]
	fn a_point_inserted_into_a_path_leaves_it_one_path() {
		let (points, ids) = (generated(5, PointId::generate), generated(6, SegmentId::generate));
		let mut modification = drawn(&points[..4], &[(ids[0], 0, 1), (ids[1], 1, 2), (ids[2], 2, 3), (ids[3], 3, 0)]);
		assert_eq!(woven_paths(&modification, &points), [[0, 1, 2, 3, 0]]);

		modification.modify(&VectorModificationType::RemoveSegment { id: ids[1] });
		modification.modify(&VectorModificationType::InsertPoint {
			id: points[4],
			position: DVec2::new(15., 5.),
		});
		for (id, points) in [(ids[4], [points[1], points[4]]), (ids[5], [points[4], points[2]])] {
			modification.modify(&VectorModificationType::InsertSegment { id, points, handles: [None, None] });
		}

		assert_eq!(woven_paths(&modification, &points), [[0, 1, 4, 2, 3, 0]]);
	}
}
