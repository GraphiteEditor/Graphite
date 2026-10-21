use crate::vector::flatten::WorkingVector;
use crate::vector::misc::UnweldMethod;
use crate::vector::path::{CurveType, Path, continues_previous_path, curve_types_of, push_paths};
use crate::vector::vector_types::largest_point_id;
use crate::vector::{PointId, SegmentId, Vector};
use core_types::list::{ATTR_ID, List};
use glam::{DAffine2, DVec2};
use petgraph::graph::{EdgeIndex, NodeIndex, UnGraph};
use petgraph::prelude::UnGraphMap;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::ops::Range;

pub trait WeldExt {
	/// Collapses every segment shorter than the distance, joining its two ends.
	fn weld_connected(&mut self, paths: &mut List<Path>, distance: f64);
	/// Joins any points lying within the distance of each other, measured after the transform.
	fn weld_all(&mut self, paths: &mut List<Path>, transform: DAffine2, distance: f64);
	/// Joins the ends of open path flows lying within the distance of each other, measured after the transform, and leaves every other anchor where it is.
	fn weld_flow_ends(&mut self, paths: &mut List<Path>, transform: DAffine2, distance: f64);
	/// Gives every segment, or every path flow, its own copies of the points it shares, by the method chosen.
	fn unweld(&mut self, paths: &mut List<Path>, method: UnweldMethod);
}

impl WeldExt for Vector {
	fn weld_connected(&mut self, paths: &mut List<Path>, distance: f64) {
		let mut working = WorkingVector::flatten(self, paths);

		// Treat the working form as an undirected graph
		let indices = VectorIndex::build_from(&working);

		// TODO: We lose information on the winding order by using an undirected graph. Switch to a directed graph and fix the algorithm to handle that.
		// Graph containing only short edges, referencing the data graph
		let mut short_edges = UnGraphMap::new();

		for segment_id in working.segment_ids.iter().copied() {
			let length = indices.segment_chord_length(segment_id);
			if length < distance {
				let [start, end] = indices.segment_ends(segment_id);
				let start = indices.point_graph.node_weight(start).unwrap().id;
				let end = indices.point_graph.node_weight(end).unwrap().id;

				short_edges.add_node(start);
				short_edges.add_node(end);
				short_edges.add_edge(start, end, segment_id);
			}
		}

		// Group connected segments to collapse them into a single point
		// TODO: there are a few possible algorithms for this - perhaps test empirically to find fastest
		let collapse: Vec<FxHashSet<PointId>> = petgraph::algo::tarjan_scc(&short_edges).into_iter().map(|connected| connected.into_iter().collect()).collect();
		let average_position = collapse
			.iter()
			.map(|collapse_set| {
				let sum: DVec2 = collapse_set.iter().map(|&id| indices.point_position(id, &working)).sum();
				sum / collapse_set.len() as f64
			})
			.collect::<Vec<_>>();

		// Collect points and segments to delete at the end to avoid invalidating indices
		let mut points_to_delete = FxHashSet::default();
		let mut segments_to_delete = FxHashSet::default();
		for (mut collapse_set, average_pos) in collapse.into_iter().zip(average_position) {
			// Remove any segments where both endpoints are in the collapse set
			for segment_index in 0..working.segment_ids.len() {
				let start = working.point_ids[working.segment_start[segment_index]];
				let end = working.point_ids[working.segment_end[segment_index]];
				if collapse_set.contains(&start) && collapse_set.contains(&end) {
					segments_to_delete.insert(working.segment_ids[segment_index]);
				}
			}

			// Delete all points but the first, set its position to the average, and update segments
			let first_id = collapse_set.iter().copied().next().unwrap();
			collapse_set.remove(&first_id);
			let first_offset = indices.point_to_offset[&first_id];

			// Look for segments with endpoints in `collapse_set` and replace them with the point we are collapsing to
			for segment_index in 0..working.segment_ids.len() {
				let start_id = working.point_ids[working.segment_start[segment_index]];
				let end_id = working.point_ids[working.segment_end[segment_index]];

				// Update Bezier handles for moved points
				if start_id == first_id {
					let point_position = working.point_positions[working.segment_start[segment_index]];
					working.segment_handles[segment_index].move_start(average_pos - point_position);
				}
				if end_id == first_id {
					let point_position = working.point_positions[working.segment_end[segment_index]];
					working.segment_handles[segment_index].move_end(average_pos - point_position);
				}

				// Replace removed points with the collapsed point
				if collapse_set.contains(&start_id) {
					let point_position = working.point_positions[working.segment_start[segment_index]];
					working.segment_start[segment_index] = first_offset;
					working.segment_handles[segment_index].move_start(average_pos - point_position);
				}
				if collapse_set.contains(&end_id) {
					let point_position = working.point_positions[working.segment_end[segment_index]];
					working.segment_end[segment_index] = first_offset;
					working.segment_handles[segment_index].move_end(average_pos - point_position);
				}
			}

			// Update the position of the collapsed point
			working.point_positions[first_offset] = average_pos;

			points_to_delete.extend(collapse_set)
		}

		// Drop the collapsed segments, then the merged-away points, remapping segment endpoints
		let keep_segment: Vec<bool> = working.segment_ids.iter().map(|id| !segments_to_delete.contains(id)).collect();
		working.carry_path_beginnings(&keep_segment);
		retain_by(&mut working.segment_ids, &keep_segment);
		retain_by(&mut working.segment_start, &keep_segment);
		retain_by(&mut working.segment_end, &keep_segment);
		retain_by(&mut working.segment_handles, &keep_segment);
		retain_by(&mut working.segment_begins_path, &keep_segment);

		let mut point_map = Vec::with_capacity(working.point_ids.len());
		let mut new_index = 0;
		let keep_point: Vec<bool> = working
			.point_ids
			.iter()
			.map(|id| {
				let keep = !points_to_delete.contains(id);
				point_map.push(if keep { new_index } else { usize::MAX });
				new_index += usize::from(keep);
				keep
			})
			.collect();
		retain_by(&mut working.point_ids, &keep_point);
		retain_by(&mut working.point_positions, &keep_point);
		for index in working.segment_start.iter_mut().chain(working.segment_end.iter_mut()) {
			*index = point_map.get(*index).copied().unwrap_or(usize::MAX);
		}

		working.reweave(self, paths);
	}

	fn weld_all(&mut self, paths: &mut List<Path>, transform: DAffine2, distance: f64) {
		let mut working = WorkingVector::flatten(self, paths);
		let point_count = working.point_positions.len();

		// Find min x and y for grid cell normalization
		let mut min_x = f64::MAX;
		let mut min_y = f64::MAX;

		// Calculate mins without collecting all positions
		for &pos in &working.point_positions {
			let transformed_pos = transform.transform_point2(pos);
			min_x = min_x.min(transformed_pos.x);
			min_y = min_y.min(transformed_pos.y);
		}

		// Create a spatial grid with cell size of 'distance'
		use std::collections::HashMap;
		let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

		// Add points to grid cells without collecting all positions first
		for i in 0..point_count {
			let pos = transform.transform_point2(working.point_positions[i]);
			let grid_x = ((pos.x - min_x) / distance).floor() as i32;
			let grid_y = ((pos.y - min_y) / distance).floor() as i32;

			grid.entry((grid_x, grid_y)).or_default().push(i);
		}

		// Create point index mapping for merged points
		let mut point_index_map = vec![None; point_count];
		let mut merged_positions = Vec::new();
		let mut merged_indices = Vec::new();

		// Process each point
		for i in 0..point_count {
			// Skip points that have already been processed
			if point_index_map[i].is_some() {
				continue;
			}

			let pos_i = transform.transform_point2(working.point_positions[i]);
			let grid_x = ((pos_i.x - min_x) / distance).floor() as i32;
			let grid_y = ((pos_i.y - min_y) / distance).floor() as i32;

			let mut group = vec![i];

			// Check only neighboring cells (3x3 grid around current cell)
			for dx in -1..=1 {
				for dy in -1..=1 {
					let neighbor_cell = (grid_x + dx, grid_y + dy);

					if let Some(indices) = grid.get(&neighbor_cell) {
						for &j in indices {
							if j > i && point_index_map[j].is_none() {
								let pos_j = transform.transform_point2(working.point_positions[j]);
								if pos_i.distance(pos_j) <= distance {
									group.push(j);
								}
							}
						}
					}
				}
			}

			// Create merged point - calculate positions as needed
			let merged_position = group
				.iter()
				.map(|&idx| transform.transform_point2(working.point_positions[idx]))
				.fold(DVec2::ZERO, |sum, pos| sum + pos)
				/ group.len() as f64;

			let merged_position = transform.inverse().transform_point2(merged_position);
			let merged_index = merged_positions.len();

			merged_positions.push(merged_position);
			merged_indices.push(working.point_ids[group[0]]);

			// Update mapping for all points in the group
			for &idx in &group {
				point_index_map[idx] = Some(merged_index);
			}
		}

		// Segments with endpoints that collapsed together drop out, handing on any path they began
		let keep_segment: Vec<bool> = (0..working.segment_ids.len())
			.map(|segment_index| point_index_map[working.segment_start[segment_index]] != point_index_map[working.segment_end[segment_index]])
			.collect();
		working.carry_path_beginnings(&keep_segment);

		// Build the merged working form from the kept segments
		let mut merged = WorkingVector {
			point_ids: merged_indices,
			point_positions: merged_positions,
			colinear: working.colinear,
			..Default::default()
		};

		for segment_index in (0..working.segment_ids.len()).filter(|&segment_index| keep_segment[segment_index]) {
			let new_start = point_index_map[working.segment_start[segment_index]].unwrap();
			let new_end = point_index_map[working.segment_end[segment_index]].unwrap();

			merged.segment_ids.push(working.segment_ids[segment_index]);
			merged.segment_start.push(new_start);
			merged.segment_end.push(new_end);
			merged.segment_handles.push(working.segment_handles[segment_index]);
			merged.segment_begins_path.push(working.segment_begins_path.get(segment_index).copied().unwrap_or(false));
		}

		merged.reweave(self, paths);
	}

	fn weld_flow_ends(&mut self, paths: &mut List<Path>, transform: DAffine2, distance: f64) {
		let point_count = self.0.len();
		if !transform.matrix2.determinant().is_normal() {
			return;
		}
		let inverse = transform.inverse();

		// Every pool point an open path flow begins or ends at
		let mut is_end = vec![false; point_count];
		for flow in path_flows(paths) {
			if let Some((start, end)) = path_flow_ends(paths, &flow)
				&& start != end
			{
				for point in [start, end] {
					if let Some(is_end) = is_end.get_mut(point) {
						*is_end = true;
					}
				}
			}
		}
		let ends: Vec<usize> = (0..point_count).filter(|&point| is_end[point]).collect();

		// Ends within the distance of an earlier one join it, found through a grid of cells as wide as the distance in O(ends)
		let pool_positions = self.0.iter_element_values().as_slice();
		let positions: Vec<DVec2> = ends.iter().map(|&point| transform.transform_point2(pool_positions[point])).collect();
		let cell = |position: DVec2| ((position.x / distance).floor() as i64, (position.y / distance).floor() as i64);
		let mut grid: FxHashMap<(i64, i64), Vec<usize>> = FxHashMap::default();
		for (index, &position) in positions.iter().enumerate() {
			grid.entry(cell(position)).or_default().push(index);
		}

		let mut joined_into = vec![None; point_count];
		let mut moved_by = vec![None; point_count];
		let mut taken = vec![false; ends.len()];
		for index in 0..ends.len() {
			if taken[index] {
				continue;
			}
			taken[index] = true;

			let (x, y) = cell(positions[index]);
			let neighbors = (-1..=1).flat_map(|dx| (-1..=1).map(move |dy| (x.saturating_add(dx), y.saturating_add(dy))));
			let mut group = vec![index];
			for neighbor in neighbors {
				for &other in grid.get(&neighbor).into_iter().flatten() {
					if !taken[other] && positions[index].distance(positions[other]) <= distance {
						taken[other] = true;
						group.push(other);
					}
				}
			}
			if group.len() < 2 {
				continue;
			}

			// The group meets at its average position, which its earliest point keeps
			let average = inverse.transform_point2(group.iter().map(|&member| positions[member]).sum::<DVec2>() / group.len() as f64);
			let kept = ends[group[0]];
			for &member in &group {
				let point = ends[member];
				moved_by[point] = Some(average - pool_positions[point]);
				if point != kept {
					joined_into[point] = Some(kept);
				}
			}
		}
		if joined_into.iter().all(Option::is_none) {
			return;
		}

		// A cubic anchor's handles move along with it, keeping the curve's shape beside the end
		let mut handle_moved_by = vec![None; point_count];
		for (path, curve_type) in paths.iter_element_values().zip(curve_types_of(paths)) {
			if curve_type != CurveType::Cubic {
				continue;
			}
			let references = path.0.iter_element_values().as_slice();
			for (index, &anchor) in references.iter().enumerate().step_by(curve_type.stride()) {
				let Some(offset) = usize::try_from(anchor).ok().and_then(|anchor| moved_by.get(anchor).copied().flatten()) else {
					continue;
				};

				for &handle in [index.checked_sub(1), Some(index + 1)].into_iter().flatten().filter_map(|index| references.get(index)) {
					if let Some(slot) = usize::try_from(handle).ok().and_then(|handle| handle_moved_by.get_mut(handle)) {
						slot.get_or_insert(offset);
					}
				}
			}
		}
		for ((position, moved_by), handle_moved_by) in self.0.iter_element_values_mut().zip(&moved_by).zip(&handle_moved_by) {
			if let Some(offset) = moved_by.or(*handle_moved_by) {
				*position += offset;
			}
		}

		// The pool drops every end that joined another, and references name the end it joined, renumbered past those dropped
		let mut renumbered = Vec::with_capacity(point_count);
		let mut kept_count = 0;
		for joined in &joined_into {
			renumbered.push(kept_count);
			kept_count += usize::from(joined.is_none());
		}
		let rename = |reference: i64| {
			let point = usize::try_from(reference).ok()?;
			let point = joined_into.get(point).copied().flatten().unwrap_or(point);
			renumbered.get(point).map(|&point| point as i64)
		};
		let kept = joined_into.iter().map(Option::is_none);
		self.0 = std::mem::take(&mut self.0).into_iter().zip(kept).filter_map(|(point, kept)| kept.then_some(point)).collect();

		// A straight span whose two ends just joined has nothing left to draw, so it drops out with the reference it departs from
		let curve_types: Vec<CurveType> = curve_types_of(paths).collect();
		let mut kept_paths = vec![true; paths.len()];
		for ((path, curve_type), kept_path) in paths.iter_element_values_mut().zip(curve_types).zip(&mut kept_paths) {
			let mut collapsed = Vec::new();
			let mut previous: Option<(i64, i64)> = None;
			for (index, reference) in path.0.iter_element_values_mut().enumerate() {
				let renamed = rename(*reference).unwrap_or(*reference);
				if curve_type == CurveType::Linear
					&& let Some((previous_reference, previous_renamed)) = previous
					&& previous_reference != *reference
					&& previous_renamed == renamed
				{
					collapsed.push(index - 1);
				}

				previous = Some((*reference, renamed));
				*reference = renamed;
			}
			if collapsed.is_empty() {
				continue;
			}

			let mut collapsed = collapsed.into_iter().peekable();
			path.0 = std::mem::take(&mut path.0)
				.into_iter()
				.enumerate()
				.filter_map(|(index, reference)| collapsed.next_if_eq(&index).is_none().then_some(reference))
				.collect();
			*kept_path = path.0.len() >= 2;
		}
		if kept_paths.contains(&false) {
			*paths = std::mem::take(paths).into_iter().zip(kept_paths).filter_map(|(path, kept)| kept.then_some(path)).collect();
		}
	}

	// A path flow stored in several curve types counts as one and keeps any point it meets more than once, in O(points + references)
	fn unweld(&mut self, paths: &mut List<Path>, method: UnweldMethod) {
		if method == UnweldMethod::All {
			separate_segments(self, paths);
			return;
		}
		let point_count = self.0.len();

		// Each stored path's path flow, and whether that flow is open with two ends of its own
		let flows = path_flows(paths);
		let mut flow_of = vec![0; paths.len()];
		let mut open = vec![false; flows.len()];
		for (index, flow) in flows.iter().enumerate() {
			if let Some(stored) = flow_of.get_mut(flow.clone()) {
				stored.fill(index);
			}
			open[index] = path_flow_ends(paths, flow).is_some_and(|(start, end)| start != end);
		}
		let is_end = |path: usize, reference: usize, reference_count: usize| {
			let index = flow_of[path];
			open[index] && ((path == flows[index].start && reference == 0) || (path + 1 == flows[index].end && reference + 1 == reference_count))
		};

		// The first path flow to use each point, and under Flow Ends, which flows pass through it and which ends there first
		let flow_ends_only = method == UnweldMethod::FlowEnds;
		let mut first_through = vec![None; point_count];
		let mut first_ending = if flow_ends_only { vec![None; point_count] } else { Vec::new() };
		let mut passes_through = FxHashSet::default();
		for (path, stored) in paths.iter_element_values().enumerate() {
			let references = stored.0.iter_element_values().as_slice();
			for (reference, &point) in references.iter().enumerate() {
				let Some(point) = usize::try_from(point).ok().filter(|&point| point < point_count) else {
					continue;
				};
				let holder = flow_of[path];

				if !flow_ends_only {
					first_through[point].get_or_insert(holder);
				} else if is_end(path, reference, references.len()) {
					first_ending[point].get_or_insert(holder);
				} else {
					first_through[point].get_or_insert(holder);
					passes_through.insert((point, holder));
				}
			}
		}

		// Each path flow other than a point's keeper takes its own copy, and a copied anchor takes the next ID.
		// Under Flow Ends, a flow keeps every point it passes through, and a point only ends meet stays with the first flow to end there.
		let keeps = |point: usize, holder: usize, end: bool| {
			if flow_ends_only {
				!end || passes_through.contains(&(point, holder)) || (first_through[point].is_none() && first_ending[point] == Some(holder))
			} else {
				first_through[point] == Some(holder)
			}
		};
		let mut latest_id = largest_point_id(&self.0);
		let mut copies: FxHashMap<(usize, usize), i64> = FxHashMap::default();
		for (path, stored) in paths.iter_element_values_mut().enumerate() {
			let reference_count = stored.0.len();
			for (reference, point) in stored.0.iter_element_values_mut().enumerate() {
				let Some(original) = usize::try_from(*point).ok().filter(|&point| point < point_count) else {
					continue;
				};
				let holder = flow_of[path];
				if keeps(original, holder, flow_ends_only && is_end(path, reference, reference_count)) {
					continue;
				}

				*point = *copies.entry((original, holder)).or_insert_with(|| {
					let copy = self.0.len();
					let Some(item) = self.0.clone_item(original) else { return original as i64 };
					self.0.push(item);

					let named = self.0.attribute::<PointId>(ATTR_ID, original).is_some_and(|id| *id != PointId::UNNAMED);
					if named && let Some(latest) = &mut latest_id {
						self.0.set_attribute(ATTR_ID, copy, latest.next_id());
					}

					copy as i64
				});
			}
		}
	}
}

/// Rebuilds the paths as one per segment, each taking its own copy of any point an earlier segment used, in O(points + references).
/// Every anchor becomes an end, which drops what only an anchor between two segments carries, like smoothness.
fn separate_segments(vector: &mut Vector, paths: &mut List<Path>) {
	let point_count = vector.0.len();
	let in_pool = |reference: i64| usize::try_from(reference).ok().filter(|&point| point < point_count);

	let mut used = vec![false; point_count];
	let mut latest_id = largest_point_id(&vector.0);
	let mut separate_point = |pool: &mut List<DVec2>, reference: i64| {
		let point = in_pool(reference)?;
		if !std::mem::replace(&mut used[point], true) {
			return u32::try_from(point).ok();
		}

		let copy = pool.len();
		pool.push(pool.clone_item(point)?);
		if pool.attribute::<PointId>(ATTR_ID, point).is_some_and(|id| *id != PointId::UNNAMED)
			&& let Some(latest) = &mut latest_id
		{
			pool.set_attribute(ATTR_ID, copy, latest.next_id());
		}
		u32::try_from(copy).ok()
	};

	let mut segments = Vec::new();
	for (path, curve_type) in paths.iter_element_values().zip(curve_types_of(paths)) {
		let references = path.0.iter_element_values().as_slice();
		let ids = path.0.iter_attribute_values::<SegmentId>(ATTR_ID).map(|ids| ids.as_slice()).unwrap_or_default();
		let stride = curve_type.stride();

		for span in 0..path.segment_count(curve_type) {
			let Some(span_references) = references.get(span * stride..=span * stride + stride) else {
				continue;
			};

			// A span naming a point outside the pool is left out before it copies any of its points
			if !span_references.iter().all(|&reference| in_pool(reference).is_some()) {
				continue;
			}
			let Some(separated) = span_references.iter().map(|&reference| separate_point(&mut vector.0, reference)).collect::<Option<Vec<u32>>>() else {
				continue;
			};
			segments.push((curve_type, separated, ids.get(span * stride).copied().into_iter().collect()));
		}
	}

	*paths = List::new();
	push_paths(paths, segments);
}

/// The path flows, each a range of stored paths: a path, with those after it that only carry it on in another curve type.
fn path_flows(paths: &List<Path>) -> Vec<Range<usize>> {
	let mut flows: Vec<Range<usize>> = Vec::new();
	for index in 0..paths.len() {
		match flows.last_mut() {
			Some(flow) if continues_previous_path(paths, index) => flow.end = index + 1,
			_ => flows.push(index..index + 1),
		}
	}

	flows
}

/// The pool points a path flow begins and ends at.
fn path_flow_ends(paths: &List<Path>, flow: &Range<usize>) -> Option<(usize, usize)> {
	let start = *paths.element(flow.start)?.0.iter_element_values().as_slice().first()?;
	let end = *paths.element(flow.end.checked_sub(1)?)?.0.iter_element_values().as_slice().last()?;

	Some((usize::try_from(start).ok()?, usize::try_from(end).ok()?))
}

/// Keeps only the elements flagged true, in order.
fn retain_by<T>(values: &mut Vec<T>, keep: &[bool]) {
	let mut flags = keep.iter();
	values.retain(|_| *flags.next().unwrap_or(&false));
}

/// All the fixed fields of a point from the working form.
pub(crate) struct Point {
	pub id: PointId,
	pub position: DVec2,
}

/// Useful indexes to speed up various operations on the flattened working form.
///
/// Important: It is the user's responsibility to ensure the indexes remain valid after mutations to the data.
pub(crate) struct VectorIndex {
	/// Points and segments form a graph. Store it here in a form amenable to graph algorithms.
	///
	/// Currently, segment data is not stored as it is not used, but it could easily be added.
	pub(crate) point_graph: UnGraph<Point, ()>,
	pub(crate) segment_to_edge: FxHashMap<SegmentId, EdgeIndex>,
	/// Get the offset from the point ID.
	pub(crate) point_to_offset: FxHashMap<PointId, usize>,
	// TODO: faces
}

impl VectorIndex {
	/// Construct a [`VectorIndex`] by building indexes from the given working form. Takes `O(n)` time.
	fn build_from(working: &WorkingVector) -> Self {
		let point_to_offset = working.point_ids.iter().copied().enumerate().map(|(a, b)| (b, a)).collect::<FxHashMap<_, _>>();

		let mut point_to_node = FxHashMap::default();
		let mut segment_to_edge = FxHashMap::default();

		let mut graph = UnGraph::new_undirected();

		for (&point_id, &position) in working.point_ids.iter().zip(&working.point_positions) {
			let idx = graph.add_node(Point { id: point_id, position });
			point_to_node.insert(point_id, idx);
		}

		for (segment_index, &segment_id) in working.segment_ids.iter().enumerate() {
			let start_id = working.point_ids[working.segment_start[segment_index]];
			let end_id = working.point_ids[working.segment_end[segment_index]];
			let edge = graph.add_edge(point_to_node[&start_id], point_to_node[&end_id], ());

			segment_to_edge.insert(segment_id, edge);
		}

		Self {
			point_graph: graph,
			segment_to_edge,
			point_to_offset,
		}
	}

	/// Fetch the length of given segment's chord. Takes `O(1)` time.
	///
	/// # Panics
	///
	/// Will panic if no segment with the given ID is found.
	fn segment_chord_length(&self, id: SegmentId) -> f64 {
		let edge_idx = self.segment_to_edge[&id];
		let (start, end) = self.point_graph.edge_endpoints(edge_idx).unwrap();
		let start_position = self.point_graph.node_weight(start).unwrap().position;
		let end_position = self.point_graph.node_weight(end).unwrap().position;
		(start_position - end_position).length()
	}

	/// Get the ends of a segment. Takes `O(1)` time.
	///
	/// The IDs will be ordered [smallest, largest] so they can be used to find other segments with the same endpoints, regardless of direction.
	///
	/// # Panics
	///
	/// This function will panic if the ID is not present.
	fn segment_ends(&self, id: SegmentId) -> [NodeIndex; 2] {
		let (start, end) = self.point_graph.edge_endpoints(self.segment_to_edge[&id]).unwrap();
		if start < end { [start, end] } else { [end, start] }
	}

	/// Get the physical location of a point. Takes `O(1)` time.
	///
	/// # Panics
	///
	/// Will panic if `id` isn't in the data.
	fn point_position(&self, id: PointId, working: &WorkingVector) -> DVec2 {
		let offset = self.point_to_offset[&id];
		working.point_positions[offset]
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::vector::contour::contours;
	use crate::vector::path::push_path;
	use core_types::list::{ATTR_ID, Item};

	fn pool(positions: &[(f64, f64)]) -> Vector {
		let mut vector = Vector::default();
		let mut point_id = PointId::ZERO;
		for &(x, y) in positions {
			vector.0.push(Item::new_from_element(DVec2::new(x, y)).with_attribute(ATTR_ID, point_id.next_id()));
		}
		vector
	}

	fn near(position: Option<&DVec2>, expected: (f64, f64)) -> bool {
		position.is_some_and(|position| position.distance(DVec2::new(expected.0, expected.1)) < 1e-9)
	}

	fn references(paths: &List<Path>) -> Vec<Vec<i64>> {
		paths.iter_element_values().map(|path| path.0.iter_element_values().copied().collect()).collect()
	}

	fn point_ids(vector: &Vector) -> Vec<u64> {
		vector.0.iter_attribute_values::<PointId>(ATTR_ID).map(|ids| ids.map(|id| id.inner()).collect()).unwrap_or_default()
	}

	// Two lines meeting across a small gap join there, while two close points of another path stay apart since only one is an end
	#[test]
	fn flow_ends_join_without_touching_other_points() {
		let mut vector = pool(&[(0., 0.), (10., 0.), (10.05, 0.), (20., 0.), (0., 10.), (0.05, 10.), (10., 10.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], []);
		push_path(&mut paths, CurveType::Linear, [2, 3], []);
		push_path(&mut paths, CurveType::Linear, [4, 5, 6], []);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);

		assert_eq!(references(&paths), [vec![0, 1], vec![1, 2], vec![3, 4, 5]]);
		assert!(near(vector.0.element(1), (10.025, 0.)));
		assert_eq!(contours(vector.0.len(), &paths).len(), 2);
	}

	// A path whose end comes back within the distance of its start closes there
	#[test]
	fn a_path_ending_beside_its_start_closes() {
		let mut vector = pool(&[(0., 0.), (10., 0.), (10., 10.), (0.05, 0.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1, 2, 3], []);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);

		assert_eq!(references(&paths), [vec![0, 1, 2, 0]]);
		assert_eq!(vector.0.len(), 3);
	}

	// A short first segment whose two anchors both join drops out, and the next segment keeps its ID, while a line already of zero length stays
	#[test]
	fn a_segment_whose_anchors_join_drops_out() {
		let mut vector = pool(&[(0., 0.), (0.05, 0.), (10., 0.), (0.05, 10.), (50., 50.)]);
		let mut paths = List::new();
		let mut segment_id = SegmentId::ZERO;
		push_path(&mut paths, CurveType::Linear, [0, 1, 2], [(); 2].map(|_| segment_id.next_id()));
		push_path(&mut paths, CurveType::Linear, [3, 1], [segment_id.next_id()]);
		push_path(&mut paths, CurveType::Linear, [4, 4], [segment_id.next_id()]);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);

		assert_eq!(references(&paths), [vec![0, 1], vec![2, 0], vec![3, 3]]);
		assert_eq!(vector.segment_iter(&paths).map(|(id, ..)| id.inner()).collect::<Vec<_>>(), [2, 3, 4]);
		assert!(near(vector.0.element(0), (0.025, 0.)));
	}

	// A path whose end comes back to its own second anchor, beside its start, closes without a segment of zero length
	#[test]
	fn a_path_ending_on_its_own_anchor_beside_its_start_closes() {
		let mut vector = pool(&[(0., 0.), (0.05, 0.), (10., 0.), (10., 10.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1, 2, 3, 1], []);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);

		assert_eq!(references(&paths), [vec![0, 1, 2, 0]]);
		assert!(matches!(contours(vector.0.len(), &paths).as_slice(), [contour] if contour.closed));
	}

	// A path flow drawn as a line then a curve joins and closes only at the ends it was drawn with, not at the joint between its curve types
	#[test]
	fn a_path_in_several_curve_types_joins_only_at_its_ends() {
		let mut vector = pool(&[(0., 0.), (10., 0.), (20., 10.), (12., 5.), (18., 8.), (10.05, 0.), (20.05, 10.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], []);
		push_path(&mut paths, CurveType::Cubic, [1, 3, 4, 2], []);
		push_path(&mut paths, CurveType::Linear, [5, 6], []);
		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);
		assert_eq!(references(&paths), [vec![0, 1], vec![1, 3, 4, 2], vec![5, 2]]);

		let mut vector = pool(&[(0., 0.), (10., 0.), (12., 5.), (5., 8.), (0.05, 0.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], []);
		push_path(&mut paths, CurveType::Cubic, [1, 2, 3, 4], []);
		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);
		assert_eq!(references(&paths), [vec![0, 1], vec![1, 2, 3, 0]]);
		assert!(matches!(contours(vector.0.len(), &paths).as_slice(), [contour] if contour.closed));
	}

	// A closed path has no ends, so its anchors stay put beside each other and beside another path's end
	#[test]
	fn a_closed_path_has_no_ends_to_join() {
		let positions = [(0., 0.), (10., 0.), (10.05, 0.), (5., 10.), (0.05, 0.), (-10., 0.)];
		let mut vector = pool(&positions);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1, 2, 3, 0], []);
		push_path(&mut paths, CurveType::Linear, [5, 4], []);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);

		assert_eq!(references(&paths), [vec![0, 1, 2, 3, 0], vec![5, 4]]);
		assert_eq!(vector.0.len(), positions.len());
	}

	// Ends a small gap apart in the item's own space stay apart once the transform scales the gap past the distance
	#[test]
	fn flow_ends_join_by_their_distance_after_the_transform() {
		let mut vector = pool(&[(0., 0.), (1., 0.), (1.05, 0.), (2., 0.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], []);
		push_path(&mut paths, CurveType::Linear, [2, 3], []);
		let scale = DAffine2::from_scale(DVec2::splat(10.));

		vector.weld_flow_ends(&mut paths, scale, 0.1);
		assert_eq!(references(&paths), [vec![0, 1], vec![2, 3]]);

		vector.weld_flow_ends(&mut paths, scale, 1.);
		assert_eq!(references(&paths), [vec![0, 1], vec![1, 2]]);
		assert!(near(vector.0.element(1), (1.025, 0.)));
	}

	// Three ends within the distance of the first meet at their average
	#[test]
	fn three_flow_ends_meet_at_their_average() {
		let mut vector = pool(&[(0., 0.), (10., 0.), (10.06, 0.), (20., 0.), (10., 10.), (10., 0.06)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], []);
		push_path(&mut paths, CurveType::Linear, [2, 3], []);
		push_path(&mut paths, CurveType::Linear, [4, 5], []);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);

		assert_eq!(references(&paths), [vec![0, 1], vec![1, 2], vec![3, 1]]);
		assert!(near(vector.0.element(1), (10.02, 0.02)));
		assert_eq!(vector.0.len(), 4);
	}

	// Dropping a joined point and a collapsed line keeps every other point's ID and every other path's curve type
	#[test]
	fn joining_flow_ends_keeps_point_ids_and_curve_types() {
		let mut vector = pool(&[(0., 0.), (0., 5.), (10., 5.), (10., 0.), (30., 0.), (30.05, 0.), (40., 0.), (45., 5.), (50., 0.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Cubic, [0, 1, 2, 3], []);
		push_path(&mut paths, CurveType::Linear, [4, 5], []);
		push_path(&mut paths, CurveType::Quadratic, [6, 7, 8], []);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.1);

		assert_eq!(references(&paths), [vec![0, 1, 2, 3], vec![5, 6, 7]]);
		assert_eq!(curve_types_of(&paths).collect::<Vec<_>>(), [CurveType::Cubic, CurveType::Quadratic]);
		assert_eq!(point_ids(&vector), [1, 2, 3, 4, 5, 7, 8, 9]);
	}

	// A cubic end brings the handle beside it along to where it joins, and a straight segment whose two ends join leaves a lone point
	#[test]
	fn a_cubic_end_moves_with_its_handle_and_a_collapsed_line_goes() {
		let mut vector = pool(&[(0., 0.), (0., 5.), (10., 5.), (10., 0.), (10.1, 0.), (20., 0.), (30., 0.), (30.05, 0.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Cubic, [0, 1, 2, 3], []);
		push_path(&mut paths, CurveType::Linear, [4, 5], []);
		push_path(&mut paths, CurveType::Linear, [6, 7], []);

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.2);

		assert_eq!(references(&paths), [vec![0, 1, 2, 3], vec![3, 4]]);
		assert!(near(vector.0.element(3), (10.05, 0.)));
		assert!(near(vector.0.element(2), (10.05, 5.)));
		assert_eq!(vector.0.len(), 6);
	}

	fn diamond() -> (Vector, List<Path>) {
		let vector = pool(&[(0., 0.), (-10., 10.), (0., 20.), (10., 10.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1, 2], []);
		push_path(&mut paths, CurveType::Linear, [0, 3, 2], []);
		(vector, paths)
	}

	// Two halves of a diamond welded at both ends come apart into two open paths, each copied anchor named next in sequence
	#[test]
	fn unwelding_shared_points_gives_each_path_its_own_points() {
		let (mut vector, mut paths) = diamond();

		vector.unweld(&mut paths, UnweldMethod::SharedPoints);

		assert_eq!(references(&paths), [vec![0, 1, 2], vec![4, 3, 5]]);
		assert_eq!(point_ids(&vector), [1, 2, 3, 4, 5, 6]);
		assert!(near(vector.0.element(4), (0., 0.)) && near(vector.0.element(5), (0., 20.)));
		assert!(contours(vector.0.len(), &paths).iter().all(|contour| !contour.closed));
	}

	// A shape mixing lines and curves is stored as a path per curve type, which still count as one path flow
	#[test]
	fn unwelding_leaves_a_path_flow_in_several_curve_types_whole() {
		let mut bezpath = kurbo::BezPath::new();
		bezpath.move_to((0., 0.));
		bezpath.line_to((10., 0.));
		bezpath.curve_to((15., 5.), (15., 10.), (10., 15.));
		bezpath.close_path();
		let (mut vector, mut paths) = Vector::from_bezpath(&bezpath);
		assert_eq!(paths.len(), 3);
		let before = references(&paths);

		vector.unweld(&mut paths, UnweldMethod::SharedPoints);

		assert_eq!(references(&paths), before);
		assert_eq!(contours(vector.0.len(), &paths).len(), 1);
	}

	// Splitting only flow ends leaves two flows crossing at a shared anchor joined there, and parts a third flow ending on it
	#[test]
	fn unwelding_flow_ends_keeps_crossings() {
		let positions = [(-10., 0.), (10., 0.), (0., -10.), (0., 10.), (0., 0.), (10., 10.)];
		let crossing = || {
			let mut paths = List::new();
			push_path(&mut paths, CurveType::Linear, [0, 4, 1], []);
			push_path(&mut paths, CurveType::Linear, [2, 4, 3], []);
			push_path(&mut paths, CurveType::Linear, [5, 4], []);
			(pool(&positions), paths)
		};

		let (mut vector, mut paths) = crossing();
		vector.unweld(&mut paths, UnweldMethod::FlowEnds);
		assert_eq!(references(&paths), [vec![0, 4, 1], vec![2, 4, 3], vec![5, 6]]);

		let (mut vector, mut paths) = crossing();
		vector.unweld(&mut paths, UnweldMethod::SharedPoints);
		assert_eq!(references(&paths), [vec![0, 4, 1], vec![2, 6, 3], vec![5, 7]]);
	}

	// Splitting every anchor turns a closed triangle into three segments, each keeping the ID that named it
	#[test]
	fn unwelding_all_separates_every_segment() {
		let mut vector = pool(&[(0., 0.), (10., 0.), (0., 10.)]);
		let mut paths = List::new();
		let mut segment_id = SegmentId::ZERO;
		let ids = [(); 3].map(|_| segment_id.next_id());
		push_path(&mut paths, CurveType::Linear, [0, 1, 2, 0], ids);

		vector.unweld(&mut paths, UnweldMethod::All);

		assert_eq!(references(&paths), [vec![0, 1], vec![3, 2], vec![4, 5]]);
		let segment_ids: Vec<u64> = vector.segment_iter(&paths).map(|(id, ..)| id.inner()).collect();
		assert_eq!(segment_ids, [1, 2, 3]);
		assert_eq!(contours(vector.0.len(), &paths).len(), 3);
	}

	// Welding flow ends at the smallest distance puts back what unwelding flow ends split apart where only flow ends met
	#[test]
	fn welding_flow_ends_undoes_unwelding() {
		let (mut vector, mut paths) = diamond();

		vector.unweld(&mut paths, UnweldMethod::FlowEnds);
		assert_eq!(references(&paths), [vec![0, 1, 2], vec![4, 3, 5]]);
		assert_eq!(vector.0.len(), 6);
		assert!(contours(vector.0.len(), &paths).iter().all(|contour| !contour.closed));

		vector.weld_flow_ends(&mut paths, DAffine2::IDENTITY, 0.0001);

		let (original, original_paths) = diamond();
		assert_eq!(references(&paths), references(&original_paths));
		assert_eq!(vector.0.len(), original.0.len());
		assert!(contours(vector.0.len(), &paths).iter().all(|contour| contour.closed));
	}

	// A closed path takes one copy of the point it closes on for both of its references to it, so it stays closed
	#[test]
	fn unwelding_a_closed_path_keeps_it_closed() {
		let mut vector = pool(&[(0., 0.), (-10., 10.), (0., 20.), (10., -10.), (20., 0.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1, 2], []);
		push_path(&mut paths, CurveType::Linear, [0, 3, 4, 0], []);

		vector.unweld(&mut paths, UnweldMethod::SharedPoints);

		assert_eq!(references(&paths), [vec![0, 1, 2], vec![5, 3, 4, 5]]);
		let closed: Vec<bool> = contours(vector.0.len(), &paths).iter().map(|contour| contour.closed).collect();
		assert_eq!(closed, [false, true]);
	}

	// A copied anchor takes the next ID while a copied handle stays unnamed, like every handle
	#[test]
	fn unwelding_leaves_copied_handles_unnamed() {
		let mut vector = Vector::default();
		let mut point_id = PointId::ZERO;
		for (x, y, is_anchor) in [(0., 0., true), (0., 10., false), (10., 10., false), (10., 0., true)] {
			let id = if is_anchor { point_id.next_id() } else { PointId::UNNAMED };
			vector.0.push(Item::new_from_element(DVec2::new(x, y)).with_attribute(ATTR_ID, id));
		}
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Cubic, [0, 1, 2, 3], []);
		push_path(&mut paths, CurveType::Cubic, [3, 2, 1, 0], []);

		vector.unweld(&mut paths, UnweldMethod::SharedPoints);

		assert_eq!(references(&paths), [vec![0, 1, 2, 3], vec![4, 5, 6, 7]]);
		let unnamed = PointId::UNNAMED.inner();
		assert_eq!(point_ids(&vector), [1, unnamed, unnamed, 2, 3, unnamed, unnamed, 4]);
	}

	// Splitting only flow ends keeps a point with a flow that passes through it before ending there, parting the other flow ending there
	#[test]
	fn unwelding_flow_ends_keeps_an_end_with_the_flow_passing_through_it() {
		let positions = [(0., 0.), (10., 0.), (20., 0.), (20., 10.), (10., -10.)];
		let ending_on_itself = || {
			let mut paths = List::new();
			push_path(&mut paths, CurveType::Linear, [4, 1], []);
			push_path(&mut paths, CurveType::Linear, [0, 1, 2, 3, 1], []);
			(pool(&positions), paths)
		};

		let (mut vector, mut paths) = ending_on_itself();
		vector.unweld(&mut paths, UnweldMethod::FlowEnds);
		assert_eq!(references(&paths), [vec![4, 5], vec![0, 1, 2, 3, 1]]);

		// Splitting every shared point instead keeps it with the path stored first
		let (mut vector, mut paths) = ending_on_itself();
		vector.unweld(&mut paths, UnweldMethod::SharedPoints);
		assert_eq!(references(&paths), [vec![4, 1], vec![0, 5, 2, 3, 5]]);
	}

	// A segment naming a point outside the pool is left out without copying the points it shares
	#[test]
	fn unwelding_all_leaves_out_a_segment_outside_the_pool() {
		let mut vector = pool(&[(0., 0.), (10., 0.)]);
		let mut paths = List::new();
		push_path(&mut paths, CurveType::Linear, [0, 1], []);
		push_path(&mut paths, CurveType::Linear, [1, 99], []);

		vector.unweld(&mut paths, UnweldMethod::All);

		assert_eq!(references(&paths), [vec![0, 1]]);
		assert_eq!(vector.0.len(), 2);
	}
}
