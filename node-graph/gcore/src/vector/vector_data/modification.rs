use super::*;
use crate::Node;
use dyn_any::{DynAny, StaticType};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointMofication {
	add: Vec<PointId>,
	remove: HashSet<PointId>,
	delta: HashMap<PointId, DVec2>,
}

impl PointMofication {
	pub fn apply(&self, point_domain: &mut PointDomain) {
		point_domain.retain(|id| !self.remove.contains(id));

		for (id, pos) in point_domain.positions_mut() {
			let Some(&delta) = self.delta.get(&id) else { continue };
			*pos += delta;
		}

		for &add_id in &self.add {
			let Some(&position) = self.delta.get(&add_id) else { continue };
			point_domain.push(add_id, position);
		}
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
	pub fn apply(&self, segment_domain: &mut SegmentDomain) {
		segment_domain.retain(|id| !self.remove.contains(id));

		for (id, point) in segment_domain.start_point_mut() {
			let Some(&new) = self.start_point.get(&id) else { continue };
			*point = new;
		}
		for (id, point) in segment_domain.end_point_mut() {
			let Some(&new) = self.end_point.get(&id) else { continue };
			*point = new;
		}
		for (id, handles) in segment_domain.handles_mut() {
			let Some(&new) = self.handles.get(&id) else { continue };
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
			segment_domain.push(add_id, start, end, handles, stroke);
		}
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
	points: PointMofication,
	segments: SegmentModification,
	regions: RegionModification,
}

impl VectorModification {
	pub fn apply(&self, vector_data: &mut VectorData) {
		self.points.apply(&mut vector_data.point_domain);
		self.segments.apply(&mut vector_data.segment_domain);
		self.regions.apply(&mut vector_data.region_domain);
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
	info!("{vector_data:#?}");
	modification.apply(&mut vector_data);
	info!("{vector_data:#?}");
	vector_data
}
