use super::*;
use crate::Node;
use bezier_rs::BezierHandles;
use dyn_any::{DynAny, StaticType};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointModification {
	add: Vec<PointId>,
	remove: HashSet<PointId>,
	delta: HashMap<PointId, DVec2>,
	g1_continous: HashMap<PointId, HashMap<[SegmentId; 2], bool>>,
}

impl PointModification {
	pub fn apply(&self, point_domain: &mut PointDomain) {
		point_domain.retain(|id| !self.remove.contains(id));

		for (id, pos) in point_domain.positions_mut() {
			let Some(&delta) = self.delta.get(&id) else { continue };
			if !delta.is_finite() {
				warn!("invalid delta");
				continue;
			}
			*pos += delta;
		}
		for (id, g1_continous) in point_domain.g1_continous_mut() {
			let Some(change) = self.g1_continous.get(&id) else { continue };
			g1_continous.retain(|current| change.get(current) != Some(&false));
			for (&add, _) in change.iter().filter(|(_, enable)| **enable) {
				g1_continous.push(add);
			}
		}

		for &add_id in &self.add {
			let Some(&position) = self.delta.get(&add_id) else { continue };
			if !position.is_finite() {
				warn!("invalid position");
				continue;
			}
			let get_continous = |continous: &HashMap<[SegmentId; 2], bool>| continous.iter().filter(|(_, enabled)| **enabled).map(|(val, _)| *val).collect();
			let g1_continous = self.g1_continous.get(&add_id).map(get_continous).unwrap_or_default();
			point_domain.push(add_id, position, g1_continous);
		}
	}
	fn push(&mut self, id: PointId, pos: DVec2) {
		self.add.push(id);
		self.delta.insert(id, pos);
	}
	fn remove(&mut self, id: PointId) {
		self.remove.insert(id);
		self.add.retain(|&add| add != id);
		self.delta.remove(&id);
	}
}
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SegmentModification {
	add: Vec<SegmentId>,
	remove: HashSet<SegmentId>,
	start_point: HashMap<SegmentId, PointId>,
	end_point: HashMap<SegmentId, PointId>,
	handles: HashMap<SegmentId, bezier_rs::BezierHandles>,
	stroke: HashMap<SegmentId, StrokeId>,
}

impl SegmentModification {
	pub fn apply(&self, segment_domain: &mut SegmentDomain, point_domain: &PointDomain) {
		segment_domain.retain(|id| !self.remove.contains(id));

		for (id, point) in segment_domain.start_point_mut() {
			let Some(&new) = self.start_point.get(&id) else { continue };
			if !point_domain.ids().contains(&new) {
				warn!("invalid start id");
				continue;
			}
			*point = new;
		}
		for (id, point) in segment_domain.end_point_mut() {
			let Some(&new) = self.end_point.get(&id) else { continue };
			if !point_domain.ids().contains(&new) {
				warn!("invalid end id");
				continue;
			}
			*point = new;
		}
		for (id, handles) in segment_domain.handles_mut() {
			let Some(&new) = self.handles.get(&id) else { continue };
			if !new.is_finite() {
				warn!("invalid handles");
				continue;
			}
			*handles = new;
		}
		for (id, stroke) in segment_domain.stroke_mut() {
			let Some(&new) = self.stroke.get(&id) else { continue };
			*stroke = new;
		}

		for &add_id in &self.add {
			let Some(&start) = self.start_point.get(&add_id) else { continue };
			let Some(&end) = self.end_point.get(&add_id) else { continue };
			let Some(&handles) = self.handles.get(&add_id) else { continue };
			let Some(&stroke) = self.stroke.get(&add_id) else { continue };
			if !point_domain.ids().contains(&start) {
				warn!("invalid start id");
				continue;
			}
			if !point_domain.ids().contains(&end) {
				warn!("invalid end id");
				continue;
			}
			if !handles.is_finite() {
				warn!("invalid handles");
				continue;
			}
			segment_domain.push(add_id, start, end, handles, stroke);
		}
	}
	fn push(&mut self, id: SegmentId, start: PointId, end: PointId, handles: bezier_rs::BezierHandles, stroke: StrokeId) {
		self.add.push(id);
		self.start_point.insert(id, start);
		self.end_point.insert(id, end);
		self.handles.insert(id, handles);
		self.stroke.insert(id, stroke);
	}
	fn remove(&mut self, id: SegmentId) {
		self.remove.insert(id);
		self.add.retain(|&add| add != id);
		self.start_point.remove(&id);
		self.end_point.remove(&id);
		self.handles.remove(&id);
		self.stroke.remove(&id);
	}
}
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegionModification {
	add: Vec<RegionId>,
	remove: HashSet<RegionId>,
	segment_range: HashMap<RegionId, core::ops::RangeInclusive<SegmentId>>,
	fill: HashMap<RegionId, FillId>,
}

impl RegionModification {
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
}
#[derive(Clone, Debug, Default, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorModification {
	points: PointModification,
	segments: SegmentModification,
	regions: RegionModification,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorModificationType {
	InsertSegment { id: SegmentId, start: PointId, end: PointId, handles: BezierHandles },
	InsertPoint { id: PointId, pos: DVec2 },

	RemoveSegment { id: SegmentId },
	RemovePoint { id: PointId },

	SetG1Continous { point: PointId, segments: [SegmentId; 2], enabled: bool },
	ApplyDelta { point: PointId, delta: DVec2 },
	SetHandles { segment: SegmentId, handles: BezierHandles },
}

impl VectorModification {
	pub fn apply(&self, vector_data: &mut VectorData) {
		self.points.apply(&mut vector_data.point_domain);
		self.segments.apply(&mut vector_data.segment_domain, &vector_data.point_domain);
		self.regions.apply(&mut vector_data.region_domain);
	}
	pub fn modify(&mut self, vector_data_modification: &VectorModificationType) {
		match vector_data_modification {
			VectorModificationType::InsertSegment { id, start, end, handles } => self.segments.push(*id, *start, *end, *handles, StrokeId::ZERO),
			VectorModificationType::InsertPoint { id, pos } => self.points.push(*id, *pos),

			VectorModificationType::RemoveSegment { id } => self.segments.remove(*id),
			VectorModificationType::RemovePoint { id } => self.points.remove(*id),

			VectorModificationType::SetG1Continous { point, segments, enabled } => {
				self.points.g1_continous.entry(*point).or_default().insert(*segments, *enabled);
			}

			VectorModificationType::ApplyDelta { point, delta } => {
				*self.points.delta.entry(*point).or_default() += *delta;
			}
			VectorModificationType::SetHandles { segment, handles } => {
				self.segments.handles.insert(*segment, *handles);
			}
		}
	}
}

impl core::hash::Hash for VectorModification {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		// TODO: properly implement (hashing a hashset is difficult because ordering is unstable)
		PointId::generate().hash(state);
	}
}

#[derive(Debug, Clone, Copy)]
pub struct PathModify<VectorModificationNode> {
	modification: VectorModificationNode,
}

#[node_macro::node_fn(PathModify)]
fn path_modify(mut vector_data: VectorData, modification: VectorModification) -> VectorData {
	modification.apply(&mut vector_data);
	vector_data
}
