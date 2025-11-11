use super::*;
use crate::Ctx;
use crate::subpath::BezierHandles;
use crate::table::{Table, TableRow};
use crate::uuid::{NodeId, generate_uuid};
use crate::vector::misc::{HandleId, HandleType, point_to_dvec2};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use kurbo::{BezPath, PathEl, Point};
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

/// Represents a procedural change to the [`PointDomain`] in [`Vector`].
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PointModification {
	add: Vec<PointId>,
	remove: HashSet<PointId>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	delta: HashMap<PointId, DVec2>,
}

impl Hash for PointModification {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		generate_uuid().hash(state)
	}
}

impl PointModification {
	/// Apply this modification to the specified [`PointDomain`].
	pub fn apply(&self, point_domain: &mut PointDomain, segment_domain: &mut SegmentDomain) {
		point_domain.retain(segment_domain, |id| !self.remove.contains(id));

		for (index, (id, position)) in point_domain.positions_mut().enumerate() {
			let Some(&delta) = self.delta.get(&id) else { continue };
			if !delta.is_finite() {
				warn!("Invalid delta when applying a point modification");
				continue;
			}

			*position += delta;

			for (_, handles, start, end) in segment_domain.handles_mut() {
				if start == index {
					handles.move_start(delta);
				}
				if end == index {
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

	/// Create a new modification that will convert an empty [`Vector`] into the target [`Vector`].
	pub fn create_from_vector(vector: &Vector) -> Self {
		Self {
			add: vector.point_domain.ids().to_vec(),
			remove: HashSet::new(),
			delta: vector.point_domain.ids().iter().copied().zip(vector.point_domain.positions().iter().cloned()).collect(),
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

/// Represents a procedural change to the [`SegmentDomain`] in [`Vector`].
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
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

impl SegmentModification {
	/// Apply this modification to the specified [`SegmentDomain`].
	pub fn apply(&self, segment_domain: &mut SegmentDomain, point_domain: &PointDomain) {
		segment_domain.retain(|id| !self.remove.contains(id), point_domain.ids().len());

		for (id, point) in segment_domain.start_point_mut() {
			let Some(&new) = self.start_point.get(&id) else { continue };
			let Some(index) = point_domain.resolve_id(new) else {
				warn!("Invalid start ID when applying a segment modification");
				continue;
			};

			*point = index;
		}

		for (id, point) in segment_domain.end_point_mut() {
			let Some(&new) = self.end_point.get(&id) else { continue };
			let Some(index) = point_domain.resolve_id(new) else {
				warn!("Invalid end ID when applying a segment modification");
				continue;
			};

			*point = index;
		}

		for (id, handles, start, end) in segment_domain.handles_mut() {
			let Some(&start) = point_domain.positions().get(start) else { continue };
			let Some(&end) = point_domain.positions().get(end) else { continue };

			// Compute the actual start and end position based on the offset from the anchor
			let start = self.handle_primary.get(&id).copied().map(|handle| handle.map(|handle| handle + start));
			let end = self.handle_end.get(&id).copied().map(|handle| handle.map(|handle| handle + end));

			if !start.unwrap_or_default().is_none_or(|start| start.is_finite()) || !end.unwrap_or_default().is_none_or(|end| end.is_finite()) {
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

			let Some(start_index) = point_domain.resolve_id(start) else {
				warn!("invalid start id: {start:#?}");
				continue;
			};
			let Some(end_index) = point_domain.resolve_id(end) else {
				warn!("invalid end id: {end:#?}");
				continue;
			};

			let start_position = point_domain.positions()[start_index];
			let end_position = point_domain.positions()[end_index];
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

			segment_domain.push(add_id, start_index, end_index, handles, stroke);
		}

		assert!(
			segment_domain.start_point().iter().all(|&index| index < point_domain.ids().len()),
			"index should be in range {segment_domain:#?}"
		);
		assert!(
			segment_domain.end_point().iter().all(|&index| index < point_domain.ids().len()),
			"index should be in range {segment_domain:#?}"
		);
	}

	/// Create a new modification that will convert an empty [`Vector`] into the target [`Vector`].
	pub fn create_from_vector(vector: &Vector) -> Self {
		let point_id = |(&segment, &index)| (segment, vector.point_domain.ids()[index]);
		Self {
			add: vector.segment_domain.ids().to_vec(),
			remove: HashSet::new(),
			start_point: vector.segment_domain.ids().iter().zip(vector.segment_domain.start_point()).map(point_id).collect(),
			end_point: vector.segment_domain.ids().iter().zip(vector.segment_domain.end_point()).map(point_id).collect(),
			handle_primary: vector.segment_bezier_iter().map(|(id, b, _, _)| (id, b.handle_start().map(|handle| handle - b.start))).collect(),
			handle_end: vector.segment_bezier_iter().map(|(id, b, _, _)| (id, b.handle_end().map(|handle| handle - b.end))).collect(),
			stroke: vector.segment_domain.ids().iter().copied().zip(vector.segment_domain.stroke().iter().cloned()).collect(),
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

/// Represents a procedural change to the [`RegionDomain`] in [`Vector`].
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RegionModification {
	add: Vec<RegionId>,
	remove: HashSet<RegionId>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	segment_range: HashMap<RegionId, std::ops::RangeInclusive<SegmentId>>,
	#[serde(serialize_with = "serialize_hashmap", deserialize_with = "deserialize_hashmap")]
	fill: HashMap<RegionId, FillId>,
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

	/// Create a new modification that will convert an empty [`Vector`] into the target [`Vector`].
	pub fn create_from_vector(vector: &Vector) -> Self {
		Self {
			add: vector.region_domain.ids().to_vec(),
			remove: HashSet::new(),
			segment_range: vector.region_domain.ids().iter().copied().zip(vector.region_domain.segment_range().iter().cloned()).collect(),
			fill: vector.region_domain.ids().iter().copied().zip(vector.region_domain.fill().iter().cloned()).collect(),
		}
	}
}

/// Represents a procedural change to the [`Vector`].
#[derive(Clone, Debug, Default, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
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
	/// Apply this modification to the specified [`Vector`].
	pub fn apply(&self, vector: &mut Vector) {
		self.points.apply(&mut vector.point_domain, &mut vector.segment_domain);
		self.segments.apply(&mut vector.segment_domain, &vector.point_domain);
		self.regions.apply(&mut vector.region_domain);

		let valid = |val: &[HandleId; 2]| vector.segment_domain.ids().contains(&val[0].segment) && vector.segment_domain.ids().contains(&val[1].segment);
		vector
			.colinear_manipulators
			.retain(|val| !self.remove_g1_continuous.contains(val) && !self.remove_g1_continuous.contains(&[val[1], val[0]]) && valid(val));

		for handles in &self.add_g1_continuous {
			if !vector.colinear_manipulators.iter().any(|test| test == handles || test == &[handles[1], handles[0]]) && valid(handles) {
				vector.colinear_manipulators.push(*handles);
			}
		}
	}

	/// Add a [`VectorModificationType`] to this modification.
	pub fn modify(&mut self, vector_modification: &VectorModificationType) {
		match vector_modification {
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

	/// Create a new modification that will convert an empty [`Vector`] into the target [`Vector`].
	pub fn create_from_vector(vector: &Vector) -> Self {
		Self {
			points: PointModification::create_from_vector(vector),
			segments: SegmentModification::create_from_vector(vector),
			regions: RegionModification::create_from_vector(vector),
			add_g1_continuous: vector.colinear_manipulators.iter().copied().collect(),
			remove_g1_continuous: HashSet::new(),
		}
	}
}

impl Hash for VectorModification {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		generate_uuid().hash(state)
	}
}

/// Applies a differential modification to a vector path, associating changes made by the Pen and Path tools to indices of edited points and segments.
#[node_macro::node(category(""))]
async fn path_modify(_ctx: impl Ctx, mut vector: Table<Vector>, modification: Box<VectorModification>, node_path: Vec<NodeId>) -> Table<Vector> {
	if vector.is_empty() {
		vector.push(TableRow::default());
	}
	let row = vector.get_mut(0).expect("push should give one item");
	modification.apply(row.element);

	// Update the source node id
	let this_node_path = node_path.iter().rev().nth(1).copied();
	*row.source_node_id = row.source_node_id.or(this_node_path);

	if vector.len() > 1 {
		warn!("The path modify ran on {} vector rows. Only the first can be modified.", vector.len());
	}
	vector
}

/// Applies the vector path's local transformation to its geometry and resets the transform to the identity.
#[node_macro::node(category("Vector"))]
async fn apply_transform(_ctx: impl Ctx, mut vector: Table<Vector>) -> Table<Vector> {
	for row in vector.iter_mut() {
		let vector = row.element;
		let transform = *row.transform;

		for (_, point) in vector.point_domain.positions_mut() {
			*point = transform.transform_point2(*point);
		}

		*row.transform = DAffine2::IDENTITY;
	}

	vector
}

// Do we want to enforce that all serialized/deserialized hashmaps are a vec of tuples?
// TODO: Eventually remove this document upgrade code
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::Hash;
pub fn serialize_hashmap<K, V, S, H>(hashmap: &HashMap<K, V, H>, serializer: S) -> Result<S::Ok, S::Error>
where
	K: Serialize + Eq + Hash,
	V: Serialize,
	S: Serializer,
	H: BuildHasher,
{
	let mut seq = serializer.serialize_seq(Some(hashmap.len()))?;
	for (key, value) in hashmap {
		seq.serialize_element(&(key, value))?;
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

pub struct AppendBezpath<'a> {
	first_point: Option<Point>,
	last_point: Option<Point>,
	first_point_index: Option<usize>,
	last_point_index: Option<usize>,
	first_segment_id: Option<SegmentId>,
	last_segment_id: Option<SegmentId>,
	point_id: PointId,
	segment_id: SegmentId,
	vector: &'a mut Vector,
}

impl<'a> AppendBezpath<'a> {
	fn new(vector: &'a mut Vector) -> Self {
		Self {
			first_point: None,
			last_point: None,
			first_point_index: None,
			last_point_index: None,
			first_segment_id: None,
			last_segment_id: None,
			point_id: vector.point_domain.next_id(),
			segment_id: vector.segment_domain.next_id(),
			vector,
		}
	}

	fn append_segment_and_close_path(&mut self, point: Point, handle: BezierHandles) {
		let handle = if self.first_point.unwrap() != point {
			// If the first point is not the same as the last point of the path then we append the segment
			// with given handle and point and then close the path with linear handle.
			self.append_segment(point, handle);
			BezierHandles::Linear
		} else {
			// if the endpoints are the same then we close the path with given handle.
			handle
		};

		// Create a new segment.
		let next_segment_id = self.segment_id.next_id();
		self.vector
			.segment_domain
			.push(next_segment_id, self.last_point_index.unwrap(), self.first_point_index.unwrap(), handle, StrokeId::ZERO);

		// Create a new region.
		let next_region_id = self.vector.region_domain.next_id();
		let first_segment_id = self.first_segment_id.unwrap_or(next_segment_id);
		let last_segment_id = next_segment_id;

		self.vector.region_domain.push(next_region_id, first_segment_id..=last_segment_id, FillId::ZERO);
	}

	fn append_segment(&mut self, end_point: Point, handle: BezierHandles) {
		// Append the point.
		let next_point_index = self.vector.point_domain.ids().len();
		let next_point_id = self.point_id.next_id();

		self.vector.point_domain.push(next_point_id, point_to_dvec2(end_point));

		// Append the segment.
		let next_segment_id = self.segment_id.next_id();
		self.vector
			.segment_domain
			.push(next_segment_id, self.last_point_index.unwrap(), next_point_index, handle, StrokeId::ZERO);

		// Update the states.
		self.last_point = Some(end_point);
		self.last_point_index = Some(next_point_index);

		self.first_segment_id = Some(self.first_segment_id.unwrap_or(next_segment_id));
		self.last_segment_id = Some(next_segment_id);
	}

	fn append_first_point(&mut self, point: Point) {
		self.first_point = Some(point);
		self.last_point = Some(point);

		// Append the first point.
		let next_point_index = self.vector.point_domain.ids().len();
		self.vector.point_domain.push(self.point_id.next_id(), point_to_dvec2(point));

		// Update the state.
		self.first_point_index = Some(next_point_index);
		self.last_point_index = Some(next_point_index);
	}

	fn reset(&mut self) {
		self.first_point = None;
		self.last_point = None;
		self.first_point_index = None;
		self.last_point_index = None;
		self.first_segment_id = None;
		self.last_segment_id = None;
	}

	pub fn append_bezpath(vector: &'a mut Vector, bezpath: BezPath) {
		let mut this = Self::new(vector);
		let mut elements = bezpath.elements().iter().peekable();

		while let Some(element) = elements.next() {
			let close_path = elements.peek().is_some_and(|elm| **elm == PathEl::ClosePath);

			match *element {
				PathEl::MoveTo(point) => this.append_first_point(point),
				PathEl::LineTo(point) => {
					let handle = BezierHandles::Linear;
					if close_path {
						this.append_segment_and_close_path(point, handle);
					} else {
						this.append_segment(point, handle);
					}
				}
				PathEl::QuadTo(point, point1) => {
					let handle = BezierHandles::Quadratic { handle: point_to_dvec2(point) };
					if close_path {
						this.append_segment_and_close_path(point1, handle);
					} else {
						this.append_segment(point1, handle);
					}
				}
				PathEl::CurveTo(point, point1, point2) => {
					let handle = BezierHandles::Cubic {
						handle_start: point_to_dvec2(point),
						handle_end: point_to_dvec2(point1),
					};

					if close_path {
						this.append_segment_and_close_path(point2, handle);
					} else {
						this.append_segment(point2, handle);
					}
				}
				PathEl::ClosePath => {
					// Already handled using `append_segment_and_close_path()` hence we reset state and continue.
					this.reset();
				}
			}
		}
	}
}

pub trait VectorExt {
	fn append_bezpath(&mut self, bezpath: BezPath);
}

impl VectorExt for Vector {
	fn append_bezpath(&mut self, bezpath: BezPath) {
		AppendBezpath::append_bezpath(self, bezpath);
	}
}

pub trait HandleExt {
	/// Set the handle's position relative to the anchor which is the start anchor for the primary handle and end anchor for the end handle.
	#[must_use]
	fn set_relative_position(self, relative_position: DVec2) -> VectorModificationType;
}

impl HandleExt for HandleId {
	fn set_relative_position(self, relative_position: DVec2) -> VectorModificationType {
		let Self { ty, segment } = self;
		match ty {
			HandleType::Primary => VectorModificationType::SetPrimaryHandle { segment, relative_position },
			HandleType::End => VectorModificationType::SetEndHandle { segment, relative_position },
		}
	}
}

#[cfg(test)]
mod tests {
	use kurbo::{PathSeg, QuadBez};

	use super::*;

	use crate::subpath::{Bezier, Subpath};

	#[test]
	fn modify_new() {
		let vector = Vector::from_subpaths([Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE), Subpath::new_rect(DVec2::NEG_ONE, DVec2::ZERO)], false);

		let modify = VectorModification::create_from_vector(&vector);

		let mut new = Vector::default();
		modify.apply(&mut new);
		assert_eq!(vector, new);
	}

	#[test]
	fn modify_existing() {
		let subpaths = [
			Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE),
			Subpath::new_rect(DVec2::NEG_ONE, DVec2::ZERO),
			Subpath::from_beziers(
				&[
					PathSeg::Quad(QuadBez::new(Point::new(0., 0.), Point::new(5., 10.), Point::new(10., 0.))),
					PathSeg::Quad(QuadBez::new(Point::new(10., 0.), Point::new(15., 10.), Point::new(20., 0.))),
				],
				false,
			),
		];
		let mut vector = Vector::from_subpaths(subpaths, false);

		let mut modify_new = VectorModification::create_from_vector(&vector);
		let mut modify_original = VectorModification::default();

		for modification in [&mut modify_new, &mut modify_original] {
			let point = vector.point_domain.ids()[0];
			modification.modify(&VectorModificationType::ApplyPointDelta { point, delta: DVec2::X * 0.5 });
			let point = vector.point_domain.ids()[9];
			modification.modify(&VectorModificationType::ApplyPointDelta { point, delta: DVec2::X });
		}

		let mut new = Vector::default();
		modify_new.apply(&mut new);

		modify_original.apply(&mut vector);

		assert_eq!(vector, new);
		assert_eq!(vector.point_domain.positions()[0], DVec2::X);
		assert_eq!(vector.point_domain.positions()[9], DVec2::new(11., 0.));
		assert_eq!(
			vector.segment_bezier_iter().nth(8).unwrap().1,
			Bezier::from_quadratic_dvec2(DVec2::new(0., 0.), DVec2::new(5., 10.), DVec2::new(11., 0.))
		);
		assert_eq!(
			vector.segment_bezier_iter().nth(9).unwrap().1,
			Bezier::from_quadratic_dvec2(DVec2::new(11., 0.), DVec2::new(16., 10.), DVec2::new(20., 0.))
		);
	}
}
