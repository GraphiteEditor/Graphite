use super::*;
use crate::vector::flatten::WorkingVector;
use crate::vector::misc::{BezierHandles, HandleId, HandleType};
use crate::vector::path::Path;
use core_types::list::List;
use core_types::uuid::generate_uuid;
use dyn_any::DynAny;
use glam::DVec2;
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
		let mut moved: HashMap<usize, DVec2> = HashMap::new();
		for (index, id) in working.point_ids.iter().enumerate() {
			let Some(&delta) = self.delta.get(id) else { continue };
			if !delta.is_finite() {
				warn!("Invalid delta when applying a point modification");
				continue;
			}

			working.point_positions[index] += delta;
			moved.insert(index, delta);
		}

		// Handles travel with their anchor, carried in one pass over the segment ends
		if !moved.is_empty() {
			for index in 0..working.segment_ids.len() {
				if let Some(&delta) = moved.get(&working.segment_start[index]) {
					working.segment_handles[index].move_start(delta);
				}
				if let Some(&delta) = moved.get(&working.segment_end[index]) {
					working.segment_handles[index].move_end(delta);
				}
			}
		}

		let mut named: HashSet<PointId> = if self.add.is_empty() { HashSet::new() } else { working.point_ids.iter().copied().collect() };
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
	/// Apply this modification to the working form's segments.
	fn apply(&self, working: &mut WorkingVector) {
		// Remove the deleted segments, plus any segment left referencing a removed point
		let point_count = working.point_ids.len();
		let keep: Vec<bool> = (0..working.segment_ids.len())
			.map(|index| !self.remove.contains(&working.segment_ids[index]) && working.segment_start[index] < point_count && working.segment_end[index] < point_count)
			.collect();
		let mut keep_iter = keep.iter();
		working.segment_start.retain(|_| *keep_iter.next().unwrap_or(&false));
		let mut keep_iter = keep.iter();
		working.segment_end.retain(|_| *keep_iter.next().unwrap_or(&false));
		let mut keep_iter = keep.iter();
		working.segment_handles.retain(|_| *keep_iter.next().unwrap_or(&false));
		let mut keep_iter = keep.iter();
		working.segment_ids.retain(|_| *keep_iter.next().unwrap_or(&false));

		// The points no longer change here, so one index serves every endpoint lookup below.
		// Nothing looks an endpoint up in the common pure-handle case, so it skips the build.
		let mut point_index: HashMap<PointId, usize> = HashMap::new();
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

		let mut named: HashSet<SegmentId> = if self.add.is_empty() { HashSet::new() } else { working.segment_ids.iter().copied().collect() };
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
		}

		assert!(working.segment_start.iter().all(|&index| index < working.point_ids.len()), "index should be in range");
		assert!(working.segment_end.iter().all(|&index| index < working.point_ids.len()), "index should be in range");
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorModification {
	points: PointModification,
	segments: SegmentModification,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashset"))]
	add_g1_continuous: HashSet<[HandleId; 2]>,
	#[cfg_attr(feature = "serde", serde(serialize_with = "serialize_hashset"))]
	remove_g1_continuous: HashSet<[HandleId; 2]>,
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
			Self::RemoveSegment { .. } | Self::RemovePoint { .. } | Self::SetG1Continuous { .. } | Self::SetEndpoint { .. } => true,
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

	/// Apply this modification to the specified [`Vector`] and its paths.
	pub fn apply(&self, vector: &mut Vector, paths: &mut List<Path>) {
		// An untouched vector keeps its incoming path structure instead of being flattened and rewoven
		if self.is_empty() {
			return;
		}

		let mut working = WorkingVector::flatten(vector, paths);
		self.points.apply(&mut working);
		self.segments.apply(&mut working);

		let surviving_segments: HashSet<SegmentId> = working.segment_ids.iter().copied().collect();
		let valid = |val: &[HandleId; 2]| surviving_segments.contains(&val[0].segment) && surviving_segments.contains(&val[1].segment);
		working
			.colinear
			.retain(|val| !self.remove_g1_continuous.contains(val) && !self.remove_g1_continuous.contains(&[val[1], val[0]]) && valid(val));

		// Both orientations go in, since a pair and its reverse name the same tangent
		let mut present: HashSet<[HandleId; 2]> = working.colinear.iter().flat_map(|&[first, second]| [[first, second], [second, first]]).collect();
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

			VectorModificationType::RemoveSegment { id } => self.segments.remove(*id),
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

			VectorModificationType::ApplyPointDelta { point, delta } => {
				*self.points.delta.entry(*point).or_default() += *delta;
			}
		}
	}

	/// Create a new modification that will convert an empty [`Vector`] into the target [`Vector`].
	pub fn create_from_vector(vector: &Vector, paths: &List<Path>) -> Self {
		let working = WorkingVector::flatten(vector, paths);
		Self {
			points: PointModification::create_from_working(&working),
			segments: SegmentModification::create_from_working(&working),
			add_g1_continuous: working.colinear.iter().copied().collect(),
			remove_g1_continuous: HashSet::new(),
		}
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
}
