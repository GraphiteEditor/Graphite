use super::*;
use crate::Node;

use bezier_rs::BezierHandles;
use dyn_any::{DynAny, StaticType};

use std::collections::{HashMap, HashSet};

/// Represents a procedural change to the [`PointDomain`] in [`VectorData`].
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointModification {
	add: Vec<PointId>,
	remove: HashSet<PointId>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	delta: HashMap<PointId, DVec2>,
}

impl Hash for PointModification {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.add.hash(state);

		let mut remove = self.remove.iter().collect::<Vec<_>>();
		remove.sort_unstable();
		remove.hash(state);

		let mut delta = self.delta.iter().map(|(&a, &b)| (a, [b.x.to_bits(), b.y.to_bits()])).collect::<Vec<_>>();
		delta.sort_unstable();
		delta.hash(state);
	}
}

impl PointModification {
	/// Apply this modification to the specified [`PointDomain`].
	pub fn apply(&self, point_domain: &mut PointDomain, segment_domain: &mut SegmentDomain) {
		point_domain.retain(|id| !self.remove.contains(id));

		for (id, position) in point_domain.positions_mut() {
			let Some(&delta) = self.delta.get(&id) else { continue };
			if !delta.is_finite() {
				warn!("Invalid delta when applying a point modification");
				continue;
			}

			*position += delta;

			for (_, handles, start, end) in segment_domain.handles_mut() {
				if start == id {
					handles.move_start(delta);
				}
				if end == id {
					handles.move_end(delta);
				}
			}
		}

		for &add_id in &self.add {
			let Some(&position) = self.delta.get(&add_id) else { continue };
			if !position.is_finite() {
				warn!("Invalid position when applying a point modification");
				continue;
			}

			point_domain.push(add_id, position);
		}
	}

	/// Create a new modification that will convert an empty [`VectorData`] into the target [`VectorData`].
	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			add: vector_data.point_domain.ids().to_vec(),
			remove: HashSet::new(),
			delta: vector_data.point_domain.ids().iter().copied().zip(vector_data.point_domain.positions().iter().cloned()).collect(),
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

/// Represents a procedural change to the [`SegmentDomain`] in [`VectorData`].
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SegmentModification {
	add: Vec<SegmentId>,
	remove: HashSet<SegmentId>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	start_point: HashMap<SegmentId, PointId>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	end_point: HashMap<SegmentId, PointId>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	handle_primary: HashMap<SegmentId, Option<DVec2>>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	handle_end: HashMap<SegmentId, Option<DVec2>>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	stroke: HashMap<SegmentId, StrokeId>,
}

impl Hash for SegmentModification {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.add.hash(state);

		let mut remove = self.remove.iter().collect::<Vec<_>>();
		remove.sort_unstable();
		remove.hash(state);

		let mut start_point = self.start_point.iter().map(|(&a, &b)| (a, b)).collect::<Vec<_>>();
		start_point.sort_unstable();
		start_point.hash(state);

		let mut end_point = self.end_point.iter().map(|(&a, &b)| (a, b)).collect::<Vec<_>>();
		end_point.sort_unstable();
		end_point.hash(state);

		let mut handle_primary = self.handle_primary.iter().map(|(&a, &b)| (a, b.map(|b| [b.x.to_bits(), b.y.to_bits()]))).collect::<Vec<_>>();
		handle_primary.sort_unstable();
		handle_primary.hash(state);

		let mut handle_end = self.handle_end.iter().map(|(&a, &b)| (a, b.map(|b| [b.x.to_bits(), b.y.to_bits()]))).collect::<Vec<_>>();
		handle_end.sort_unstable();
		handle_end.hash(state);

		let mut stroke = self.stroke.iter().map(|(&a, &b)| (a, b)).collect::<Vec<_>>();
		stroke.sort_unstable();
		stroke.hash(state);
	}
}

impl SegmentModification {
	/// Apply this modification to the specified [`SegmentDomain`].
	pub fn apply(&self, segment_domain: &mut SegmentDomain, point_domain: &PointDomain) {
		segment_domain.retain(|id| !self.remove.contains(id));

		for (id, point) in segment_domain.start_point_mut() {
			let Some(&new) = self.start_point.get(&id) else { continue };
			if !point_domain.ids().contains(&new) {
				warn!("Invalid start ID when applying a segment modification");
				continue;
			}

			*point = new;
		}

		for (id, point) in segment_domain.end_point_mut() {
			let Some(&new) = self.end_point.get(&id) else { continue };
			if !point_domain.ids().contains(&new) {
				warn!("Invalid end ID when applying a segment modification");
				continue;
			}

			*point = new;
		}

		for (id, handles, start, end) in segment_domain.handles_mut() {
			let Some(start) = point_domain.position_from_id(start) else { continue };
			let Some(end) = point_domain.position_from_id(end) else { continue };

			// Compute the actual start and end position based on the offset from the anchor
			let start = self.handle_primary.get(&id).copied().map(|handle| handle.map(|handle| handle + start));
			let end = self.handle_end.get(&id).copied().map(|handle| handle.map(|handle| handle + end));

			if !start.unwrap_or_default().map_or(true, |start| start.is_finite()) || !end.unwrap_or_default().map_or(true, |end| end.is_finite()) {
				warn!("Invalid handles when applying a segment modification");
				continue;
			}

			match (start, end) {
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

		for (id, stroke) in segment_domain.stroke_mut() {
			let Some(&new) = self.stroke.get(&id) else { continue };
			*stroke = new;
		}

		for &add_id in &self.add {
			let Some(&start) = self.start_point.get(&add_id) else { continue };
			let Some(&end) = self.end_point.get(&add_id) else { continue };
			let Some(&handle_start) = self.handle_primary.get(&add_id) else { continue };
			let Some(&handle_end) = self.handle_end.get(&add_id) else { continue };
			let Some(&stroke) = self.stroke.get(&add_id) else { continue };

			if !point_domain.ids().contains(&start) {
				warn!("invalid start id");
				continue;
			}
			if !point_domain.ids().contains(&end) {
				warn!("invalid end id");
				continue;
			}

			let Some(start_position) = point_domain.position_from_id(start) else { continue };
			let Some(end_position) = point_domain.position_from_id(end) else { continue };
			let handles = match (handle_start, handle_end) {
				(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic {
					handle_start: handle_start + start_position,
					handle_end: handle_end + end_position,
				},
				(Some(handle), None) | (None, Some(handle)) => BezierHandles::Quadratic { handle: handle + start_position },
				(None, None) => BezierHandles::Linear,
			};

			if !handles.is_finite() {
				warn!("invalid handles");
				continue;
			}

			segment_domain.push(add_id, start, end, handles, stroke);
		}
	}

	/// Create a new modification that will convert an empty [`VectorData`] into the target [`VectorData`].
	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			add: vector_data.segment_domain.ids().to_vec(),
			remove: HashSet::new(),
			start_point: vector_data.segment_domain.ids().iter().copied().zip(vector_data.segment_domain.start_point().iter().cloned()).collect(),
			end_point: vector_data.segment_domain.ids().iter().copied().zip(vector_data.segment_domain.end_point().iter().cloned()).collect(),
			handle_primary: vector_data.segment_bezier_iter().map(|(id, b, _, _)| (id, b.handle_start().map(|handle| handle - b.start))).collect(),
			handle_end: vector_data.segment_bezier_iter().map(|(id, b, _, _)| (id, b.handle_end().map(|handle| handle - b.end))).collect(),
			stroke: vector_data.segment_domain.ids().iter().copied().zip(vector_data.segment_domain.stroke().iter().cloned()).collect(),
		}
	}

	fn push(&mut self, id: SegmentId, points: [PointId; 2], handles: [Option<DVec2>; 2], stroke: StrokeId) {
		self.remove.remove(&id);
		self.add.push(id);
		self.start_point.insert(id, points[0]);
		self.end_point.insert(id, points[1]);
		self.handle_primary.insert(id, handles[0]);
		self.handle_end.insert(id, handles[1]);
		self.stroke.insert(id, stroke);
	}

	fn remove(&mut self, id: SegmentId) {
		self.remove.insert(id);
		self.add.retain(|&add| add != id);
		self.start_point.remove(&id);
		self.end_point.remove(&id);
		self.handle_primary.remove(&id);
		self.handle_end.remove(&id);
		self.stroke.remove(&id);
	}
}

/// Represents a procedural change to the [`RegionDomain`] in [`VectorData`].
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegionModification {
	add: Vec<RegionId>,
	remove: HashSet<RegionId>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	segment_range: HashMap<RegionId, core::ops::RangeInclusive<SegmentId>>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	fill: HashMap<RegionId, FillId>,
}

impl Hash for RegionModification {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.add.hash(state);

		let mut remove = self.remove.iter().collect::<Vec<_>>();
		remove.sort_unstable();
		remove.hash(state);

		let mut segment_range = self.segment_range.iter().map(|(&a, b)| (a, (*b.start(), *b.end()))).collect::<Vec<_>>();
		segment_range.sort_unstable();
		segment_range.hash(state);

		let mut fill = self.fill.iter().map(|(&a, &b)| (a, b)).collect::<Vec<_>>();
		fill.sort_unstable();
		fill.hash(state);
	}
}

impl RegionModification {
	/// Apply this modification to the specified [`RegionDomain`].
	pub fn apply(&self, region_domain: &mut RegionDomain) {
		region_domain.retain(|id| !self.remove.contains(id));

		for (id, segment_range) in region_domain.segment_range_mut() {
			let Some(new) = self.segment_range.get(&id) else { continue };
			*segment_range = new.clone(); // Range inclusive is not copy
		}

		for (id, fill) in region_domain.fill_mut() {
			let Some(&new) = self.fill.get(&id) else { continue };
			*fill = new;
		}

		for &add_id in &self.add {
			let Some(segment_range) = self.segment_range.get(&add_id) else { continue };
			let Some(&fill) = self.fill.get(&add_id) else { continue };
			region_domain.push(add_id, segment_range.clone(), fill);
		}
	}

	/// Create a new modification that will convert an empty [`VectorData`] into the target [`VectorData`].
	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			add: vector_data.region_domain.ids().to_vec(),
			remove: HashSet::new(),
			segment_range: vector_data.region_domain.ids().iter().copied().zip(vector_data.region_domain.segment_range().iter().cloned()).collect(),
			fill: vector_data.region_domain.ids().iter().copied().zip(vector_data.region_domain.fill().iter().cloned()).collect(),
		}
	}
}

/// Represents a procedural change to the [`VectorData`].
#[derive(Clone, Debug, Default, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorModification {
	points: PointModification,
	segments: SegmentModification,
	regions: RegionModification,
	add_g1_continuous: HashSet<[HandleId; 2]>,
	remove_g1_continuous: HashSet<[HandleId; 2]>,
}

/// A modification type that can be added to a [`VectorModification`].
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorModificationType {
	InsertSegment { id: SegmentId, points: [PointId; 2], handles: [Option<DVec2>; 2] },
	InsertPoint { id: PointId, position: DVec2 },

	RemoveSegment { id: SegmentId },
	RemovePoint { id: PointId },

	SetG1Continuous { handles: [HandleId; 2], enabled: bool },
	SetHandles { segment: SegmentId, handles: [Option<DVec2>; 2] },
	SetPrimaryHandle { segment: SegmentId, relative_position: DVec2 },
	SetEndHandle { segment: SegmentId, relative_position: DVec2 },
	SetStartPoint { segment: SegmentId, id: PointId },
	SetEndPoint { segment: SegmentId, id: PointId },

	ApplyPointDelta { point: PointId, delta: DVec2 },
	ApplyPrimaryDelta { segment: SegmentId, delta: DVec2 },
	ApplyEndDelta { segment: SegmentId, delta: DVec2 },
}

impl VectorModification {
	/// Apply this modification to the specified [`VectorData`].
	pub fn apply(&self, vector_data: &mut VectorData) {
		self.points.apply(&mut vector_data.point_domain, &mut vector_data.segment_domain);
		self.segments.apply(&mut vector_data.segment_domain, &vector_data.point_domain);
		self.regions.apply(&mut vector_data.region_domain);

		let valid = |val: &[HandleId; 2]| vector_data.segment_domain.ids().contains(&val[0].segment) && vector_data.segment_domain.ids().contains(&val[1].segment);
		vector_data
			.colinear_manipulators
			.retain(|val| !self.remove_g1_continuous.contains(val) && !self.remove_g1_continuous.contains(&[val[1], val[0]]) && valid(val));

		for handles in &self.add_g1_continuous {
			if !vector_data.colinear_manipulators.iter().any(|test| test == handles || test == &[handles[1], handles[0]]) && valid(handles) {
				vector_data.colinear_manipulators.push(*handles);
			}
		}
	}

	/// Add a [`VectorModificationType`] to this modification.
	pub fn modify(&mut self, vector_data_modification: &VectorModificationType) {
		match vector_data_modification {
			VectorModificationType::InsertSegment { id, points, handles } => self.segments.push(*id, *points, *handles, StrokeId::ZERO),
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
			VectorModificationType::SetPrimaryHandle { segment, relative_position } => {
				self.segments.handle_primary.insert(*segment, Some(*relative_position));
			}
			VectorModificationType::SetEndHandle { segment, relative_position } => {
				self.segments.handle_end.insert(*segment, Some(*relative_position));
			}
			VectorModificationType::SetStartPoint { segment, id } => {
				self.segments.start_point.insert(*segment, *id);
			}
			VectorModificationType::SetEndPoint { segment, id } => {
				self.segments.end_point.insert(*segment, *id);
			}

			VectorModificationType::ApplyPointDelta { point, delta } => {
				*self.points.delta.entry(*point).or_default() += *delta;
			}
			VectorModificationType::ApplyPrimaryDelta { segment, delta } => {
				let position = self.segments.handle_primary.entry(*segment).or_default();
				*position = Some(position.unwrap_or_default() + *delta);
			}
			VectorModificationType::ApplyEndDelta { segment, delta } => {
				let position = self.segments.handle_end.entry(*segment).or_default();
				*position = Some(position.unwrap_or_default() + *delta);
			}
		}
	}

	/// Create a new modification that will convert an empty [`VectorData`] into the target [`VectorData`].
	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			points: PointModification::create_from_vector(vector_data),
			segments: SegmentModification::create_from_vector(vector_data),
			regions: RegionModification::create_from_vector(vector_data),
			add_g1_continuous: vector_data.colinear_manipulators.iter().copied().collect(),
			remove_g1_continuous: HashSet::new(),
		}
	}
}

impl core::hash::Hash for VectorModification {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.points.hash(state);

		self.segments.hash(state);

		self.regions.hash(state);

		let mut add_g1_continuous = self.add_g1_continuous.iter().copied().collect::<Vec<_>>();
		add_g1_continuous.sort_unstable();
		add_g1_continuous.hash(state);

		let mut remove_g1_continuous = self.remove_g1_continuous.iter().copied().collect::<Vec<_>>();
		remove_g1_continuous.sort_unstable();
		remove_g1_continuous.hash(state);
	}
}

/// A node that applies a procedural modification to some [`VectorData`].
#[derive(Debug, Clone, Copy)]
pub struct PathModify<VectorModificationNode> {
	modification: VectorModificationNode,
}

#[node_macro::node_fn(PathModify)]
fn path_modify(mut vector_data: VectorData, modification: VectorModification) -> VectorData {
	modification.apply(&mut vector_data);
	vector_data
}

#[test]
fn modify_new() {
	let vector_data = VectorData::from_subpaths(
		[bezier_rs::Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE), bezier_rs::Subpath::new_rect(DVec2::NEG_ONE, DVec2::ZERO)],
		false,
	);

	let modify = VectorModification::create_from_vector(&vector_data);

	let mut new = VectorData::empty();
	modify.apply(&mut new);
	assert_eq!(vector_data, new);
}

#[test]
fn modify_existing() {
	use bezier_rs::{Bezier, Subpath};
	let subpaths = [
		Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE),
		Subpath::new_rect(DVec2::NEG_ONE, DVec2::ZERO),
		Subpath::from_beziers(
			&[
				Bezier::from_quadratic_dvec2(DVec2::new(0., 0.), DVec2::new(5., 10.), DVec2::new(10., 0.)),
				Bezier::from_quadratic_dvec2(DVec2::new(10., 0.), DVec2::new(15., 10.), DVec2::new(20., 0.)),
			],
			false,
		),
	];
	let mut vector_data = VectorData::from_subpaths(subpaths, false);

	let mut modify_new = VectorModification::create_from_vector(&vector_data);
	let mut modify_original = VectorModification::default();

	for modification in [&mut modify_new, &mut modify_original] {
		let point = vector_data.point_domain.ids()[0];
		modification.modify(&VectorModificationType::ApplyPointDelta { point, delta: DVec2::X * 0.5 });
		let point = vector_data.point_domain.ids()[9];
		modification.modify(&VectorModificationType::ApplyPointDelta { point, delta: DVec2::X });
	}

	let mut new = VectorData::empty();
	modify_new.apply(&mut new);

	modify_original.apply(&mut vector_data);

	assert_eq!(vector_data, new);
	assert_eq!(vector_data.point_domain.positions()[0], DVec2::X);
	assert_eq!(vector_data.point_domain.positions()[9], DVec2::new(11., 0.));
	assert_eq!(
		vector_data.segment_bezier_iter().nth(8).unwrap().1,
		Bezier::from_quadratic_dvec2(DVec2::new(0., 0.), DVec2::new(5., 10.), DVec2::new(11., 0.))
	);
	assert_eq!(
		vector_data.segment_bezier_iter().nth(9).unwrap().1,
		Bezier::from_quadratic_dvec2(DVec2::new(11., 0.), DVec2::new(16., 10.), DVec2::new(20., 0.))
	);
}

// TODO: Eventually remove this (probably starting late 2024)
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::Hash;
fn serialize_hashmap<K, V, S>(hashmap: &HashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
where
	K: Serialize + Eq + Hash,
	V: Serialize,
	S: Serializer,
{
	let mut seq = serializer.serialize_seq(Some(hashmap.len()))?;
	for (key, value) in hashmap {
		seq.serialize_element(&(key, value))?;
	}
	seq.end()
}

fn deserialize_hashmap<'de, K, V, D>(deserializer: D) -> Result<HashMap<K, V>, D::Error>
where
	K: Deserialize<'de> + Eq + Hash,
	V: Deserialize<'de>,
	D: Deserializer<'de>,
{
	struct HashMapVisitor<K, V> {
		marker: std::marker::PhantomData<fn() -> HashMap<K, V>>,
	}

	impl<'de, K, V> Visitor<'de> for HashMapVisitor<K, V>
	where
		K: Deserialize<'de> + Eq + Hash,
		V: Deserialize<'de>,
	{
		type Value = HashMap<K, V>;

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			formatter.write_str("a sequence of tuples")
		}

		fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
		where
			A: SeqAccess<'de>,
		{
			let mut hashmap = HashMap::new();
			while let Some((key, value)) = seq.next_element()? {
				hashmap.insert(key, value);
			}
			Ok(hashmap)
		}
	}

	let visitor = HashMapVisitor { marker: std::marker::PhantomData };
	deserializer.deserialize_seq(visitor)
}
