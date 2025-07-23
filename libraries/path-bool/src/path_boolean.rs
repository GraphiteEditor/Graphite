//! Implements boolean operations on paths using graph-based algorithms.
//!
//! This module uses concepts from graph theory to efficiently perform boolean
//! operations on complex paths. The main algorithms involve creating a graph
//! representation of the paths, simplifying this graph, and then working with
//! its dual graph to determine the result of the boolean operation.
//!
//! ## Graph Minor
//!
//! A graph minor is a simplified version of a graph, obtained by contracting edges
//! (merging connected vertices) and removing isolated vertices. In the context of
//! path boolean operations, we use a graph minor to simplify the initial graph
//! representation of the paths. This simplification involves:
//!
//! 1. Merging collinear segments into single edges.
//! 2. Removing vertices that don't represent significant features (like intersections
//!    or endpoints).
//!
//! The resulting graph minor preserves the topological structure of the paths while
//! reducing computational complexity.
//!
//! For more information on graph minors, see:
//! <https://en.wikipedia.org/wiki/Graph_minor>
//!
//! ## Dual Graph
//!
//! The dual graph is a graph derived from another graph (the primal graph). In the
//! context of path boolean operations, we construct the dual graph as follows:
//!
//! 1. Each face (region) in the primal graph becomes a vertex in the dual graph.
//! 2. Each edge in the primal graph becomes an edge in the dual graph, connecting
//!    the vertices that represent the faces on either side of the original edge.
//!
//! The dual graph allows us to efficiently determine which regions are inside or
//! outside the original paths, which is crucial for performing boolean operations.
//!
//! For more information on dual graphs, see:
//! <https://en.wikipedia.org/wiki/Dual_graph>
//!
//! ## Algorithm Overview
//!
//! The boolean operation algorithm follows these main steps:
//!
//! 1. Create a graph representation of both input paths (MajorGraph).
//! 2. Simplify this graph to create a graph minor (MinorGraph).
//! 3. Construct the dual graph of the MinorGraph.
//! 4. Use the dual graph to determine which regions should be included in the result,
//!    based on the specific boolean operation being performed.
//! 5. Reconstruct the resulting path(s) from the selected regions.
//!
//! This approach allows for efficient and accurate boolean operations, even on
//! complex paths with many intersections or self-intersections.

new_key_type! {
	pub struct MajorVertexKey;
	pub struct MajorEdgeKey;
	pub struct MinorVertexKey;
	pub struct MinorEdgeKey;
	pub struct DualVertexKey;
	pub struct DualEdgeKey;
}
// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::aabb::{Aabb, bounding_box_around_point, bounding_box_max_extent, merge_bounding_boxes};
use crate::epsilons::Epsilons;
use crate::intersection_path_segment::{path_segment_intersection, segments_equal};
use crate::path::Path;
use crate::path_cubic_segment_self_intersection::path_cubic_segment_self_intersection;
use crate::path_segment::PathSegment;
#[cfg(feature = "logging")]
use crate::path_to_path_data;
use crate::quad_tree::QuadTree;
use glam::DVec2;
use slotmap::{SlotMap, new_key_type};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;

/// Represents the types of boolean operations that can be performed on paths.
#[derive(Debug, Clone, Copy)]
pub enum PathBooleanOperation {
	/// Computes the union of two paths.
	///
	/// The result contains all areas that are inside either path A or path B (or both).
	/// This operation is useful for combining shapes or creating complex outlines.
	Union,

	/// Computes the difference between two paths (A minus B).
	///
	/// The result contains all areas that are inside path A but not inside path B.
	/// This operation is useful for cutting holes or subtracting shapes from each other.
	Difference,

	/// Computes the intersection of two paths.
	///
	/// The result contains only the areas that are inside both path A and path B.
	/// This operation is useful for finding overlapping regions between shapes.
	Intersection,

	/// Computes the symmetric difference (exclusive or) of two paths.
	///
	/// The result contains areas that are inside either path A or path B, but not in both.
	/// This operation is useful for creating non-overlapping regions or finding boundaries.
	Exclusion,

	/// Divides the first path using the second path as a "knife".
	///
	/// This operation splits path A wherever it intersects with path B, but keeps all
	/// parts of path A. It's useful for creating segments or partitioning shapes.
	Division,

	/// Breaks both paths into separate pieces where they intersect.
	///
	/// This operation splits both path A and path B at their intersection points,
	/// resulting in all possible non-overlapping segments from both paths.
	/// It's useful for creating detailed breakdowns of overlapping shapes.
	Fracture,
}

/// Specifies how to determine the "inside" of a path for filling.
#[derive(Debug, Clone, Copy)]
pub enum FillRule {
	/// A point is inside if a ray from the point to infinity crosses an odd number of path segments.
	NonZero,
	/// A point is inside if a ray from the point to infinity crosses an even number of path segments.
	EvenOdd,
}

const INTERSECTION_TREE_DEPTH: usize = 8;
const POINT_TREE_DEPTH: usize = 8;

pub const EPS: Epsilons = Epsilons {
	point: 1e-5,
	linear: 1e-4,
	param: 1e-8,
};

type MajorGraphEdgeStage1 = (PathSegment, u8);
type MajorGraphEdgeStage2 = (PathSegment, u8, Aabb);

#[derive(Debug, Clone)]
pub struct MajorGraphEdge {
	seg: PathSegment,
	parent: u8,
	incident_vertices: [MajorVertexKey; 2],
	direction_flag: Direction,
	twin: Option<MajorEdgeKey>,
}

#[derive(Debug, Clone, Default)]
pub struct MajorGraphVertex {
	#[cfg_attr(not(feature = "logging"), expect(dead_code))]
	pub point: DVec2,
	outgoing_edges: Vec<MajorEdgeKey>,
}

/// Represents the initial graph structure used in boolean operations.
///
/// This graph contains all segments from both input paths.
#[derive(Debug, Clone)]
struct MajorGraph {
	edges: SlotMap<MajorEdgeKey, MajorGraphEdge>,
	vertices: SlotMap<MajorVertexKey, MajorGraphVertex>,
}

#[derive(Debug, Clone, PartialEq)]
struct MinorGraphEdge {
	segments: Vec<PathSegment>,
	parent: u8,
	incident_vertices: [MinorVertexKey; 2],
	direction_flag: Direction,
	twin: Option<MinorEdgeKey>,
}

impl MinorGraphEdge {
	fn start_segment(&self) -> PathSegment {
		let segment = self.segments[0];
		match self.direction_flag {
			Direction::Forward => segment,
			Direction::Backwards => segment.reverse(),
		}
	}
}

// Compares Segments based on their derivative at the start. If the derivative
// is equal, check the curvature instead. This should correctly sort most instances.
fn compare_segments(a: &PathSegment, b: &PathSegment) -> Ordering {
	let angle_a = a.start_angle();
	let angle_b = b.start_angle();

	// Normalize angles to [0, 2π)
	let angle_a = (angle_a * 1000.).round() / 1000.;
	let angle_b = (angle_b * 1000.).round() / 1000.;

	// Compare angles first
	match angle_b.partial_cmp(&angle_a) {
		Some(Ordering::Equal) => {
			// If angles are equal (or very close), compare curvatures
			let curvature_a = a.start_curvature();
			let curvature_b = b.start_curvature();
			curvature_a.partial_cmp(&curvature_b).unwrap_or(Ordering::Equal)
		}
		Some(ordering) => ordering,
		None => Ordering::Equal, // Handle NaN cases
	}
}

impl PartialOrd for MinorGraphEdge {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(compare_segments(&self.start_segment(), &other.start_segment()))
	}
}

#[derive(Debug, Clone, Default)]
struct MinorGraphVertex {
	outgoing_edges: Vec<MinorEdgeKey>,
}

#[derive(Debug, Clone)]
struct MinorGraphCycle {
	segments: Vec<PathSegment>,
	parent: u8,
	direction_flag: Direction,
}

/// Represents a simplified graph structure derived from the MajorGraph.
///
/// This graph combines collinear segments and removes unnecessary vertices.
#[derive(Debug, Clone)]
struct MinorGraph {
	edges: SlotMap<MinorEdgeKey, MinorGraphEdge>,
	vertices: SlotMap<MinorVertexKey, MinorGraphVertex>,
	cycles: Vec<MinorGraphCycle>,
}

#[derive(Debug, Clone, PartialEq)]
struct DualGraphHalfEdge {
	segments: Vec<PathSegment>,
	parent: u8,
	incident_vertex: DualVertexKey,
	direction_flag: Direction,
	twin: Option<DualEdgeKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DualGraphVertex {
	incident_edges: Vec<DualEdgeKey>,
}

/// Represents a component in the dual graph.
///
/// A component is a connected subset of the dual graph, typically corresponding
/// to a distinct region in the original paths.
#[derive(Debug, Clone)]
struct DualGraphComponent {
	edges: Vec<DualEdgeKey>,
	vertices: Vec<DualVertexKey>,
	outer_face: Option<DualVertexKey>,
}

/// Represents the dual graph of the MinorGraph.
///
/// In this graph, faces of the MinorGraph become vertices, and edges represent
/// adjacency between faces. This structure is crucial for determining the
/// inside/outside regions of the paths.
#[derive(Debug, Clone)]
struct DualGraph {
	components: Vec<DualGraphComponent>,
	edges: SlotMap<DualEdgeKey, DualGraphHalfEdge>,
	vertices: SlotMap<DualVertexKey, DualGraphVertex>,
}

/// Represents the hierarchical nesting of regions in the paths.
///
/// This tree structure captures how different regions of the paths are contained
/// within each other
#[derive(Debug, Clone)]
struct NestingTree {
	component: DualGraphComponent,
	outgoing_edges: HashMap<DualVertexKey, Vec<NestingTree>>,
}

#[cfg(feature = "logging")]
fn major_graph_to_dot(graph: &MajorGraph) -> String {
	let mut dot = String::from("digraph {\n");
	for (vertex_key, vertex) in &graph.vertices {
		dot.push_str(&format!("  {:?} [label=\"{:.1},{:.1}\"]\n", (vertex_key.0.as_ffi() & 0xFF), vertex.point.x, vertex.point.y));
	}
	for (_, edge) in &graph.edges {
		dot.push_str(&format!(
			"  {:?} -> {:?}: {:0b}\n",
			(edge.incident_vertices[0].0.as_ffi() & 0xFF),
			(edge.incident_vertices[1].0.as_ffi() & 0xFF),
			edge.parent
		));
	}
	dot.push_str("}\n");
	dot
}

#[cfg(feature = "logging")]
fn minor_graph_to_dot(edges: &SlotMap<MinorEdgeKey, MinorGraphEdge>) -> String {
	let mut dot = String::from("digraph {\n");
	for edge in edges.values() {
		dot.push_str(&format!(
			"  {:?} -> {:?}: {:0b}\n",
			(edge.incident_vertices[0].0.as_ffi() & 0xFF),
			(edge.incident_vertices[1].0.as_ffi() & 0xFF),
			edge.parent
		));
	}
	dot.push_str("}\n");
	dot
}

#[cfg(feature = "logging")]
fn dual_graph_to_dot(components: &[DualGraphComponent], edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>) -> String {
	let mut dot = String::from("strict graph {\n");
	for component in components {
		for &edge_key in &component.edges {
			let edge = &edges[edge_key];
			dot.push_str(&format!(
				"  {:?} -- {:?}\n",
				(edge.incident_vertex.0.as_ffi() & 0xFF),
				(edges[edge.twin.unwrap()].incident_vertex.0.as_ffi() & 0xFF)
			));
		}
	}
	dot.push_str("}\n");
	dot
}

fn segment_to_edge(parent: u8) -> impl Fn(&PathSegment) -> Option<MajorGraphEdgeStage1> {
	move |seg| {
		if bounding_box_max_extent(&seg.bounding_box()) < EPS.point {
			return None;
		}

		match seg {
			// Convert Line Segments expressed as cubic beziers to proper line segments
			PathSegment::Cubic(start, _, _, end) => {
				let direction = seg.sample_at(0.1);
				if (*end - *start).angle_to(direction - *start).abs() < EPS.point * 4. {
					Some((PathSegment::Line(*start, *end), parent))
				} else {
					Some((*seg, parent))
				}
			}
			seg => Some((*seg, parent)),
		}
	}
}

fn split_at_self_intersections(edges: &mut Vec<MajorGraphEdgeStage1>) {
	let mut new_edges = Vec::new();
	for (seg, parent) in edges.iter_mut() {
		if let PathSegment::Cubic(..) = seg {
			if let Some(intersection) = path_cubic_segment_self_intersection(seg) {
				let mut intersection = intersection;
				if intersection[0] > intersection[1] {
					intersection.swap(0, 1);
				}
				let [t1, t2] = intersection;
				if (t1 - t2).abs() < EPS.param {
					let (seg1, seg2) = seg.split_at(t1);
					*seg = seg1;
					new_edges.push((seg2, *parent));
				} else {
					let (seg1, tmp_seg) = seg.split_at(t1);
					let (seg2, seg3) = &tmp_seg.split_at((t2 - t1) / (1. - t1));
					*seg = seg1;
					new_edges.push((*seg2, *parent));
					new_edges.push((*seg3, *parent));
				}
			}
		}
	}
	edges.extend(new_edges);
}

/// Splits path segments at their intersections with other segments.
///
/// This function performs the following steps:
/// 1. Computes bounding boxes for all input edges.
/// 2. Creates a spatial index (quad tree) of edges for efficient intersection checks.
/// 3. For each edge:
///    a. Finds potential intersecting edges using the spatial index.
///    b. Computes precise intersections with these candidates.
///    c. Records the intersection points as split locations.
/// 4. Splits the original edges at the recorded intersection points.
/// 5. Returns the split edges along with an overall bounding box.
///
/// The function uses an epsilon value to handle floating-point imprecision
/// when determining if intersections occur at endpoints.
///
/// # Arguments
///
/// * `edges` - A slice of initial path segments (MajorGraphEdgeStage1).
///
/// # Returns
///
/// A tuple containing:
/// * A vector of split edges (MajorGraphEdgeStage2).
/// * An optional overall bounding box (AaBb) for all edges.
fn split_at_intersections(edges: &[MajorGraphEdgeStage1]) -> (Vec<MajorGraphEdgeStage2>, Option<Aabb>) {
	// Step 1: Add bounding boxes to edges
	let with_bounding_box: Vec<MajorGraphEdgeStage2> = edges.iter().map(|(seg, parent)| (*seg, *parent, seg.bounding_box())).collect();

	// Step 2: Calculate total bounding box
	let total_bounding_box = with_bounding_box.iter().fold(None, |acc, (_, _, bb)| Some(merge_bounding_boxes(acc, bb)));

	let total_bounding_box = match total_bounding_box {
		Some(bb) => bb,
		None => return (Vec::new(), None),
	};

	// Step 3: Create edge tree for efficient intersection checks
	let mut edge_tree = QuadTree::new(total_bounding_box, INTERSECTION_TREE_DEPTH, 8);

	let mut splits_per_edge: HashMap<usize, Vec<f64>> = HashMap::new();

	fn add_split(splits_per_edge: &mut HashMap<usize, Vec<f64>>, i: usize, t: f64) {
		splits_per_edge.entry(i).or_default().push(t);
	}

	// Step 4: Find intersections and record split points
	for (i, edge) in with_bounding_box.iter().enumerate() {
		let candidates = edge_tree.find(&edge.2);
		for &j in &candidates {
			let candidate: &(PathSegment, u8) = &edges[j];
			let include_endpoints = edge.1 != candidate.1 || !(candidate.0.end().abs_diff_eq(edge.0.start(), EPS.point) || candidate.0.start().abs_diff_eq(edge.0.end(), EPS.point));
			let intersection = path_segment_intersection(&edge.0, &candidate.0, include_endpoints, &EPS);
			for [t0, t1] in intersection {
				add_split(&mut splits_per_edge, i, t0);
				add_split(&mut splits_per_edge, j, t1);
			}
		}
		edge_tree.insert(edge.2, i);
	}

	// Step 5: Apply splits to create new edges
	let mut new_edges = Vec::new();

	for (i, (seg, parent, _)) in with_bounding_box.into_iter().enumerate() {
		if let Some(splits) = splits_per_edge.get(&i) {
			let mut splits = splits.clone();
			splits.sort_by(|a, b| a.partial_cmp(b).unwrap());
			let mut tmp_seg = seg;
			let mut prev_t = 0.;
			for &t in splits.iter() {
				if t > 1. - EPS.param {
					break;
				}
				let tt = (t - prev_t) / (1. - prev_t);
				prev_t = t;
				if tt < EPS.param {
					continue;
				}
				if tt > 1. - EPS.param {
					continue;
				}
				let (seg1, seg2) = tmp_seg.split_at(tt);
				new_edges.push((seg1, parent, seg1.bounding_box()));
				tmp_seg = seg2;
			}
			new_edges.push((tmp_seg, parent, tmp_seg.bounding_box()));
		} else {
			new_edges.push((seg, parent, seg.bounding_box()));
		}
	}

	(new_edges, Some(total_bounding_box))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
	Forward,
	Backwards,
}

impl std::ops::Neg for Direction {
	type Output = Self;

	fn neg(self) -> Self::Output {
		match self {
			Self::Forward => Self::Backwards,
			Self::Backwards => Self::Forward,
		}
	}
}
impl std::ops::Not for Direction {
	type Output = Self;

	fn not(self) -> Self::Output {
		match self {
			Self::Forward => Self::Backwards,
			Self::Backwards => Self::Forward,
		}
	}
}

impl Direction {
	pub fn forward(self) -> bool {
		self == Self::Forward
	}
}

// TODO:(@TrueDoctor) Optimize this by rounding each vertex up and down and then inserting them in a hashmap. This should remove the need for bbox calculations and the quad tree
fn find_vertices(edges: &[MajorGraphEdgeStage2], bounding_box: Aabb) -> MajorGraph {
	let mut vertex_tree = QuadTree::new(bounding_box, POINT_TREE_DEPTH, 8);
	let mut graph = MajorGraph {
		edges: SlotMap::with_key(),
		vertices: SlotMap::with_key(),
	};

	let mut parents: HashMap<MajorEdgeKey, u8> = HashMap::new();

	let mut vertex_pair_id_to_edges: HashMap<_, Vec<(MajorGraphEdgeStage2, MajorEdgeKey, MajorEdgeKey)>> = HashMap::new();

	for (seg, parent, bounding_box) in edges {
		let mut get_vertex = |point: DVec2| -> MajorVertexKey {
			let box_around_point = bounding_box_around_point(point, EPS.point);
			if let Some(&existing_vertex) = vertex_tree.find(&box_around_point).iter().next() {
				existing_vertex
			} else {
				let vertex_key = graph.vertices.insert(MajorGraphVertex { point, outgoing_edges: Vec::new() });
				vertex_tree.insert(box_around_point, vertex_key);
				vertex_key
			}
		};

		let start_vertex = get_vertex(seg.start());
		let end_vertex = get_vertex(seg.end());

		if start_vertex == end_vertex {
			match seg {
				PathSegment::Line(..) => continue,
				PathSegment::Cubic(_, c1, c2, _) => {
					if c1.abs_diff_eq(*c2, EPS.point) {
						continue;
					}
				}
				PathSegment::Quadratic(_, c, _) => {
					if seg.start().abs_diff_eq(*c, EPS.point) {
						continue;
					}
				}
				PathSegment::Arc(_, _, _, _, _, false, _) => continue,
				_ => {}
			}
		}

		let vertex_pair_id = (start_vertex.min(end_vertex), start_vertex.max(end_vertex));
		if let Some(existing_edges) = vertex_pair_id_to_edges.get(&vertex_pair_id) {
			if let Some(existing_edge) = existing_edges
				.iter()
				.find(|(other_seg, ..)| segments_equal(seg, &other_seg.0, EPS.point) || segments_equal(&seg.reverse(), &other_seg.0, EPS.point))
			{
				*parents.entry(existing_edge.1).or_default() |= parent;
				*parents.entry(existing_edge.2).or_default() |= parent;
				continue;
			}
		}

		let fwd_edge_key = graph.edges.insert(MajorGraphEdge {
			seg: *seg,
			parent: *parent,
			incident_vertices: [start_vertex, end_vertex],
			direction_flag: Direction::Forward,
			twin: None,
		});

		let bwd_edge_key = graph.edges.insert(MajorGraphEdge {
			seg: *seg,
			parent: *parent,
			incident_vertices: [end_vertex, start_vertex],
			direction_flag: Direction::Backwards,
			twin: Some(fwd_edge_key),
		});

		graph.edges[fwd_edge_key].twin = Some(bwd_edge_key);

		graph.vertices[start_vertex].outgoing_edges.push(fwd_edge_key);
		graph.vertices[end_vertex].outgoing_edges.push(bwd_edge_key);

		vertex_pair_id_to_edges
			.entry(vertex_pair_id)
			.or_default()
			.push(((*seg, *parent, *bounding_box), fwd_edge_key, bwd_edge_key));
	}
	for (edge_key, parent) in parents {
		graph.edges[edge_key].parent |= parent;
	}

	graph
}

fn get_order(vertex: &MajorGraphVertex) -> usize {
	vertex.outgoing_edges.len()
}

/// Computes the minor graph from the major graph.
///
/// This function simplifies the graph structure by performing the following steps:
/// 1. Iterates through vertices of the major graph.
/// 2. For vertices with exactly two edges (degree 2):
///    a. Combines the two edges into a single edge if they have the same parent.
///    b. Updates the endpoints of the new edge to skip the current vertex.
/// 3. For vertices with degree != 2:
///    a. Creates a new vertex in the minor graph.
///    b. Creates new edges in the minor graph for each outgoing edge.
/// 4. Handles any cyclic components (closed loops with no high-degree vertices).
///
/// The resulting minor graph preserves the topological structure of the paths
/// while reducing the number of vertices and edges.
///
/// # Arguments
///
/// * `major_graph` - A reference to the MajorGraph.
///
/// # Returns
///
/// A new MinorGraph representing the simplified structure.
fn compute_minor(major_graph: &MajorGraph) -> MinorGraph {
	let mut new_edges = SlotMap::with_key();
	let mut new_vertices = SlotMap::with_key();
	let mut to_minor_vertex = HashMap::new();
	let mut id_to_edge = HashMap::new();
	let mut visited = HashSet::new();

	// Handle components that are not cycles
	for (major_vertex_key, vertex) in &major_graph.vertices {
		// Edges are contracted
		if get_order(vertex) == 2 {
			continue;
		}
		let start_vertex = *to_minor_vertex
			.entry(major_vertex_key)
			.or_insert_with(|| new_vertices.insert(MinorGraphVertex { outgoing_edges: Vec::new() }));

		for &start_edge_key in &vertex.outgoing_edges {
			let mut segments = Vec::new();
			let mut edge_key = start_edge_key;
			let mut edge = &major_graph.edges[edge_key];

			while edge.parent == major_graph.edges[start_edge_key].parent
				&& edge.direction_flag == major_graph.edges[start_edge_key].direction_flag
				&& get_order(&major_graph.vertices[edge.incident_vertices[1]]) == 2
			{
				segments.push(edge.seg);
				visited.insert(edge.incident_vertices[1]);
				let next_vertex = &major_graph.vertices[edge.incident_vertices[1]];
				// Choose the edge which is not our twin so we can make progress
				edge_key = *next_vertex.outgoing_edges.iter().find(|&&e| Some(e) != edge.twin).unwrap();
				edge = &major_graph.edges[edge_key];
			}
			segments.push(edge.seg);

			let end_vertex = *to_minor_vertex
				.entry(edge.incident_vertices[1])
				.or_insert_with(|| new_vertices.insert(MinorGraphVertex { outgoing_edges: Vec::new() }));
			assert!(major_graph.edges[start_edge_key].twin.is_some());
			assert!(edge.twin.is_some());

			let edge_id = (start_edge_key, edge_key);
			let twin_id = (edge.twin.unwrap(), major_graph.edges[start_edge_key].twin.unwrap());

			let twin_key = id_to_edge.get(&twin_id);

			let new_edge_key = new_edges.insert(MinorGraphEdge {
				segments,
				parent: major_graph.edges[start_edge_key].parent,
				incident_vertices: [start_vertex, end_vertex],
				direction_flag: major_graph.edges[start_edge_key].direction_flag,
				twin: twin_key.copied(),
			});
			if let Some(&twin_key) = twin_key {
				new_edges[twin_key].twin = Some(new_edge_key);
			}
			id_to_edge.insert(edge_id, new_edge_key);
			new_vertices[start_vertex].outgoing_edges.push(new_edge_key);
		}
	}

	// Handle cyclic components (if any)
	let mut cycles = Vec::new();
	for (major_vertex_key, vertex) in &major_graph.vertices {
		if vertex.outgoing_edges.len() != 2 || visited.contains(&major_vertex_key) {
			continue;
		}
		let mut edge_key = vertex.outgoing_edges[0];
		let mut edge = &major_graph.edges[edge_key];
		let mut cycle = MinorGraphCycle {
			segments: Vec::new(),
			parent: edge.parent,
			direction_flag: edge.direction_flag,
		};
		loop {
			cycle.segments.push(edge.seg);
			visited.insert(edge.incident_vertices[0]);
			assert_eq!(major_graph.vertices[edge.incident_vertices[1]].outgoing_edges.len(), 2, "Found an unvisited vertex of order != 2.");
			let next_vertex = &major_graph.vertices[edge.incident_vertices[1]];
			edge_key = *next_vertex.outgoing_edges.iter().find(|&&e| Some(e) != edge.twin).unwrap();
			edge = &major_graph.edges[edge_key];
			if edge.incident_vertices[0] == major_vertex_key {
				break;
			}
		}
		cycles.push(cycle);
	}

	MinorGraph {
		edges: new_edges,
		vertices: new_vertices,
		cycles,
	}
}

fn remove_dangling_edges(graph: &mut MinorGraph) {
	// Basically DFS for each parent with BFS number
	fn walk(parent: u8, graph: &MinorGraph) -> HashSet<MinorVertexKey> {
		let mut kept_vertices = HashSet::new();
		let mut vertex_to_level = HashMap::new();

		fn visit(
			vertex: MinorVertexKey,
			incoming_edge: Option<MinorEdgeKey>,
			level: usize,
			graph: &MinorGraph,
			vertex_to_level: &mut HashMap<MinorVertexKey, usize>,
			kept_vertices: &mut HashSet<MinorVertexKey>,
			parent: u8,
		) -> usize {
			if let Some(&existing_level) = vertex_to_level.get(&vertex) {
				return existing_level;
			}
			vertex_to_level.insert(vertex, level);

			let mut min_level = usize::MAX;
			for &edge_key in &graph.vertices[vertex].outgoing_edges {
				let edge = &graph.edges[edge_key];
				if edge.parent & parent != 0 && Some(edge_key) != incoming_edge {
					min_level = min_level.min(visit(edge.incident_vertices[1], edge.twin, level + 1, graph, vertex_to_level, kept_vertices, parent));
				}
			}

			if min_level <= level {
				kept_vertices.insert(vertex);
			}

			min_level
		}

		for edge in graph.edges.values() {
			if edge.parent & parent != 0 {
				visit(edge.incident_vertices[0], None, 0, graph, &mut vertex_to_level, &mut kept_vertices, parent);
			}
		}

		kept_vertices
	}

	let kept_vertices_a = walk(1, graph);
	let kept_vertices_b = walk(2, graph);

	graph.vertices.retain(|k, _| kept_vertices_a.contains(&k) || kept_vertices_b.contains(&k));

	for vertex in graph.vertices.values_mut() {
		vertex.outgoing_edges.retain(|&edge_key| {
			let edge = &graph.edges[edge_key];
			(edge.parent & 1 == 1 && kept_vertices_a.contains(&edge.incident_vertices[0]) && kept_vertices_a.contains(&edge.incident_vertices[1]))
				|| (edge.parent & 2 == 2 && kept_vertices_b.contains(&edge.incident_vertices[0]) && kept_vertices_b.contains(&edge.incident_vertices[1]))
		});
	}
	// TODO(@TrueDoctor): merge
	graph.edges.retain(|_, edge| {
		(edge.parent & 1 == 1 && kept_vertices_a.contains(&edge.incident_vertices[0]) && kept_vertices_a.contains(&edge.incident_vertices[1]))
			|| (edge.parent & 2 == 2 && kept_vertices_b.contains(&edge.incident_vertices[0]) && kept_vertices_b.contains(&edge.incident_vertices[1]))
	});
}

fn sort_outgoing_edges_by_angle(graph: &mut MinorGraph) {
	for (vertex_key, vertex) in graph.vertices.iter_mut() {
		if vertex.outgoing_edges.len() > 2 {
			vertex.outgoing_edges.sort_by(|&a, &b| graph.edges[a].partial_cmp(&graph.edges[b]).unwrap());
			if cfg!(feature = "logging") {
				eprintln!("Outgoing edges for {:?}:", vertex_key);
				for &edge_key in &vertex.outgoing_edges {
					let edge = &graph.edges[edge_key];
					let angle = edge.start_segment().start_angle();
					eprintln!("{:?}: {}°", edge_key.0, angle.to_degrees())
				}
			}
		}
	}
}

fn face_to_polygon(face: &DualGraphVertex, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>) -> Vec<DVec2> {
	const CNT: usize = 3;

	face.incident_edges
		.iter()
		.flat_map(|&edge_key| {
			let edge = &edges[edge_key];
			edge.segments.iter().flat_map(move |seg| {
				(0..CNT).map(move |i| {
					let t0 = i as f64 / CNT as f64;
					let t = if edge.direction_flag.forward() { t0 } else { 1. - t0 };
					seg.sample_at(t)
				})
			})
		})
		.collect()
}

fn interval_crosses_point(a: f64, b: f64, p: f64) -> bool {
	let dy1 = a >= p;
	let dy2 = b < p;
	dy1 == dy2
}

fn line_segment_intersects_horizontal_ray(a: DVec2, b: DVec2, point: DVec2) -> bool {
	if !interval_crosses_point(a.y, b.y, point.y) {
		return false;
	}
	let x = crate::math::lin_map(point.y, a.y, b.y, a.x, b.x);
	x >= point.x
}

fn compute_point_winding(polygon: &[DVec2], tested_point: DVec2) -> i32 {
	if polygon.len() <= 2 {
		return 0;
	}
	let mut prev_point = polygon[polygon.len() - 1];
	let mut winding = 0;
	for &point in polygon {
		if line_segment_intersects_horizontal_ray(prev_point, point, tested_point) {
			winding += if point.y > prev_point.y { -1 } else { 1 };
		}
		prev_point = point;
	}
	winding
}

fn compute_winding(face: &DualGraphVertex, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>) -> Option<i32> {
	let polygon = face_to_polygon(face, edges);

	for i in 0..polygon.len() {
		let a = polygon[i];
		let b = polygon[(i + 1) % polygon.len()];
		let c = polygon[(i + 2) % polygon.len()];
		let center = (a + b + c) / 3.;
		let winding = compute_point_winding(&polygon, center);
		if winding != 0 {
			return Some(winding);
		}
	}

	None
}

fn compute_signed_area(face: &DualGraphVertex, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>) -> f64 {
	let polygon = face_to_polygon(face, edges);
	if polygon.len() <= 4 {
		return -1.;
	}

	#[cfg(feature = "logging")]
	eprintln!("vertex: {:?}", face);
	#[cfg(feature = "logging")]
	for point in &polygon {
		eprintln!("{}, {}", point.x, point.y);
	}
	let mut area = 0.;

	for i in 0..polygon.len() {
		let a = polygon[i];
		let b = polygon[(i + 1) % polygon.len()];
		area += a.x * b.y;
		area -= b.x * a.y;
	}

	#[cfg(feature = "logging")]
	eprintln!("winding: {}", area);
	area
}

/// Computes the dual graph from the minor graph.
///
/// This function creates the dual graph by following these steps:
/// 1. Initializes empty structures for dual graph vertices and edges.
/// 2. For each edge in the minor graph:
///    a. Creates a new face (dual vertex) if not already created.
///    b. Traverses around the face, creating dual edges for each minor edge.
///    c. Connects dual edges to their twins (edges representing the same minor edge).
/// 3. Handles special cases like isolated cycles.
/// 4. Groups dual graph elements into connected components.
/// 5. Determines the outer face for each component.
///
/// The dual graph represents faces of the minor graph as vertices and adjacencies
/// between faces as edges, effectively flipping the concepts of vertices and faces.
///
/// # Arguments
///
/// * `minor_graph` - A reference to the MinorGraph.
///
/// # Returns
///
/// A Result containing either the computed DualGraph or a BooleanError if the
/// operation cannot be completed successfully.
fn compute_dual(minor_graph: &MinorGraph) -> Result<DualGraph, BooleanError> {
	let mut new_vertices: Vec<DualVertexKey> = Vec::new();
	let mut minor_to_dual_edge: HashMap<MinorEdgeKey, DualEdgeKey> = HashMap::new();
	let mut dual_edges = SlotMap::with_key();
	let mut dual_vertices = SlotMap::with_key();

	for (start_edge_key, start_edge) in &minor_graph.edges {
		#[cfg(feature = "logging")]
		eprintln!("Processing start edge: {}", (start_edge_key.0.as_ffi() & 0xFF));
		if minor_to_dual_edge.contains_key(&start_edge_key) {
			continue;
		}

		let face_key = dual_vertices.insert(DualGraphVertex { incident_edges: Vec::new() });

		let mut edge_key = start_edge_key;
		let mut edge = start_edge;

		loop {
			#[cfg(feature = "logging")]
			eprintln!("Processing edge: {}", (edge_key.0.as_ffi() & 0xFF));
			let twin = edge.twin.expect("Edge doesn't have a twin");
			let twin_dual_key = minor_to_dual_edge.get(&twin).copied();

			let new_edge_key = dual_edges.insert(DualGraphHalfEdge {
				segments: edge.segments.clone(),
				parent: edge.parent,
				incident_vertex: face_key,
				direction_flag: edge.direction_flag,
				twin: twin_dual_key,
			});

			if let Some(twin_key) = twin_dual_key {
				dual_edges[twin_key].twin = Some(new_edge_key);
			}

			minor_to_dual_edge.insert(edge_key, new_edge_key);

			dual_vertices[face_key].incident_edges.push(new_edge_key);

			edge_key = get_next_edge(edge_key, minor_graph);
			#[cfg(feature = "logging")]
			eprintln!("Next edge: {}", (edge_key.0.as_ffi() & 0xFF));
			edge = &minor_graph.edges[edge_key];

			if edge.incident_vertices[0] == start_edge.incident_vertices[0] {
				break;
			}
		}

		new_vertices.push(face_key);
	}

	for cycle in &minor_graph.cycles {
		let inner_face_key = dual_vertices.insert(DualGraphVertex { incident_edges: Vec::new() });
		let outer_face_key = dual_vertices.insert(DualGraphVertex { incident_edges: Vec::new() });

		let inner_half_edge_key = dual_edges.insert(DualGraphHalfEdge {
			segments: cycle.segments.clone(),
			parent: cycle.parent,
			incident_vertex: inner_face_key,
			direction_flag: cycle.direction_flag,
			twin: None,
		});

		let outer_half_edge_key = dual_edges.insert(DualGraphHalfEdge {
			segments: cycle.segments.iter().cloned().rev().collect(),
			parent: cycle.parent,
			incident_vertex: outer_face_key,
			direction_flag: !cycle.direction_flag,
			twin: Some(inner_half_edge_key),
		});

		dual_edges[inner_half_edge_key].twin = Some(outer_half_edge_key);
		dual_vertices[inner_face_key].incident_edges.push(inner_half_edge_key);
		dual_vertices[outer_face_key].incident_edges.push(outer_half_edge_key);
		new_vertices.push(inner_face_key);
		new_vertices.push(outer_face_key);
	}

	let mut components = Vec::new();
	let mut visited_vertices = HashSet::new();
	let mut visited_edges = HashSet::new();

	if cfg!(feature = "logging") {
		eprintln!("faces: {}, dual-edges: {}, cycles: {}", new_vertices.len(), dual_edges.len(), minor_graph.cycles.len())
	}

	// This can be very useful for debugging:
	// Copy the face outlines to a file called faces_combined.csv and then use this gnuplot command:
	// ```
	// plot 'faces_combined.csv' i 0:99 w l, 'faces_combined.csv' index 0 w l lc 'red'
	// ```
	// The first part of the command plots all faces to the graph and the second comand plots one surface,
	// specified by the index, in red. This allows you to check if all surfaces are closed paths and can
	// be used in conjunction with the flag debugging to identify issues later down the line as well.
	#[cfg(feature = "logging")]
	for (vertex_key, vertex) in &dual_vertices {
		eprintln!("\n\n#{:?}", vertex_key.0);
		let polygon = face_to_polygon(vertex, &dual_edges);
		for point in polygon.iter() {
			eprintln!("{}, {}", point.x, point.y);
		}
		eprintln!("{}, {}", polygon[0].x, polygon[0].y);
	}

	for &start_vertex_key in &new_vertices {
		if visited_vertices.contains(&start_vertex_key) {
			continue;
		}

		let mut component_vertices = Vec::new();
		let mut component_edges = Vec::new();

		let mut stack = vec![start_vertex_key];
		while let Some(vertex_key) = stack.pop() {
			if visited_vertices.insert(vertex_key) {
				component_vertices.push(vertex_key);
			}

			for &edge_key in &dual_vertices[vertex_key].incident_edges {
				if !visited_edges.insert(edge_key) {
					continue;
				}

				let edge = &dual_edges[edge_key];
				let twin_key = edge.twin.expect("Edge doesn't have a twin.");
				component_edges.push(edge_key);
				component_edges.push(twin_key);
				visited_edges.insert(twin_key);
				stack.push(dual_edges[twin_key].incident_vertex);
			}
		}
		#[cfg(feature = "logging")]
		eprintln!("component_vertices: {}", component_vertices.len());

		let windings: Option<Vec<_>> = component_vertices
			.iter()
			.map(|face_key| compute_winding(&dual_vertices[*face_key], &dual_edges).map(|w| (face_key, w)))
			.collect();
		let Some(windings) = windings else {
			return Err(BooleanError::NoEarInPolygon);
		};

		let areas: Vec<_> = component_vertices
			.iter()
			.map(|face_key| (face_key, compute_signed_area(&dual_vertices[*face_key], &dual_edges)))
			.collect();
		#[cfg(feature = "logging")]
		dbg!(&areas);

		#[cfg(feature = "logging")]
		if cfg!(feature = "logging") {
			eprintln!(
				"{}",
				dual_graph_to_dot(
					&[DualGraphComponent {
						vertices: component_vertices.clone(),
						edges: component_edges.clone(),
						outer_face: None,
					}],
					&dual_edges,
				)
			);
		}

		let mut count = windings.iter().filter(|(_, winding)| winding < &0).count();
		let mut reverse_winding = false;
		// If the paths are reversed use positive winding as outer face
		if windings.len() > 2 && count == windings.len() - 1 {
			count = 1;
			reverse_winding = true;
		}
		let outer_face_key = if count != 1 {
			#[cfg(feature = "logging")]
			eprintln!("Found multiple outer faces: {areas:?}, falling back to area calculation");
			let (key, _) = *areas.iter().max_by_key(|(_, area)| (area.abs() * 1000.) as u64).unwrap();
			*key
		} else {
			*windings
				.iter()
				.find(|&&(&_, ref winding)| (winding < &0) ^ reverse_winding)
				.expect("No outer face of a component found.")
				.0
		};
		#[cfg(feature = "logging")]
		dbg!(outer_face_key);

		components.push(DualGraphComponent {
			vertices: component_vertices,
			edges: component_edges,
			outer_face: Some(outer_face_key),
		});
	}

	Ok(DualGraph {
		vertices: dual_vertices,
		edges: dual_edges,
		components,
	})
}

fn get_next_edge(edge_key: MinorEdgeKey, graph: &MinorGraph) -> MinorEdgeKey {
	let edge = &graph.edges[edge_key];
	let vertex = &graph.vertices[edge.incident_vertices[1]];
	#[cfg(feature = "logging")]
	eprintln!("{edge_key:?}, twin: {:?}, {:?}", edge.twin, vertex.outgoing_edges);
	let index = vertex.outgoing_edges.iter().position(|&e| Some(edge_key) == graph.edges[e].twin).unwrap();
	vertex.outgoing_edges[(index + 1) % vertex.outgoing_edges.len()]
}

fn test_inclusion(a: &DualGraphComponent, b: &DualGraphComponent, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>, vertices: &SlotMap<DualVertexKey, DualGraphVertex>) -> Option<DualVertexKey> {
	let tested_point = edges[a.edges[0]].segments[0].start();
	for (face_key, face) in b.vertices.iter().map(|&key| (key, &vertices[key])) {
		if Some(face_key) == b.outer_face {
			continue;
		}
		let mut count = 0;
		for &edge_key in &face.incident_edges {
			let edge = &edges[edge_key];
			for seg in &edge.segments {
				count += path_segment_horizontal_ray_intersection_count(seg, tested_point);
			}
		}
		if count % 2 == 1 {
			return Some(face_key);
		}
	}
	None
}
fn bounding_box_intersects_horizontal_ray(bounding_box: &Aabb, point: DVec2) -> bool {
	interval_crosses_point(bounding_box.top, bounding_box.bottom, point[1]) && bounding_box.right >= point[0]
}

struct IntersectionSegment {
	bounding_box: Aabb,
	seg: PathSegment,
}

pub fn path_segment_horizontal_ray_intersection_count(orig_seg: &PathSegment, point: DVec2) -> usize {
	let total_bounding_box = orig_seg.bounding_box();

	if !bounding_box_intersects_horizontal_ray(&total_bounding_box, point) {
		return 0;
	}

	let mut segments = vec![IntersectionSegment {
		bounding_box: total_bounding_box,
		seg: *orig_seg,
	}];
	let mut count = 0;

	while !segments.is_empty() {
		let mut next_segments = Vec::new();
		for segment in segments {
			if bounding_box_max_extent(&segment.bounding_box) < EPS.linear {
				if line_segment_intersects_horizontal_ray(segment.seg.start(), segment.seg.end(), point) {
					count += 1;
				}
			} else {
				let split = &segment.seg.split_at(0.5);
				let bounding_box0 = split.0.bounding_box();
				let bounding_box1 = split.1.bounding_box();

				if bounding_box_intersects_horizontal_ray(&bounding_box0, point) {
					next_segments.push(IntersectionSegment {
						bounding_box: bounding_box0,
						seg: split.0,
					});
				}
				if bounding_box_intersects_horizontal_ray(&bounding_box1, point) {
					next_segments.push(IntersectionSegment {
						bounding_box: bounding_box1,
						seg: split.1,
					});
				}
			}
		}
		segments = next_segments;
	}

	count
}

/// Computes the nesting tree of the dual graph components.
///
/// This function builds a hierarchical structure representing how the components
/// of the dual graph are nested within each other. It does this by:
/// 1. Initializing an empty list of top-level nesting trees.
/// 2. For each component in the dual graph:
///    a. Tests for inclusion against existing nesting trees.
///    b. If included in an existing tree, recursively inserts it at the appropriate level.
///    c. If not included, creates a new top-level tree.
///    d. Checks if any existing trees should become children of the new tree.
/// 3. Continues this process until all components are placed in the nesting structure.
///
/// The resulting nesting tree captures the containment relationships between
/// different regions of the original paths.
///
/// # Arguments
///
/// * `dual_graph` - A reference to the DualGraph.
///
/// # Returns
///
/// A vector of NestingTree structures representing the top-level components and their nested subcomponents.
fn compute_nesting_tree(DualGraph { components, vertices, edges }: &DualGraph) -> Vec<NestingTree> {
	let mut nesting_trees = Vec::new();

	for component in components {
		insert_component(&mut nesting_trees, component, edges, vertices);
	}

	nesting_trees
}

fn insert_component(trees: &mut Vec<NestingTree>, component: &DualGraphComponent, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>, vertices: &SlotMap<DualVertexKey, DualGraphVertex>) {
	for tree in trees.iter_mut() {
		if let Some(face_key) = test_inclusion(component, &tree.component, edges, vertices) {
			if let Some(children) = tree.outgoing_edges.get_mut(&face_key) {
				insert_component(children, component, edges, vertices);
			} else {
				tree.outgoing_edges.insert(
					face_key,
					vec![NestingTree {
						component: component.clone(),
						outgoing_edges: HashMap::new(),
					}],
				);
			}
			return;
		}
	}

	let mut new_tree = NestingTree {
		component: component.clone(),
		outgoing_edges: HashMap::new(),
	};

	let mut i = 0;
	while i < trees.len() {
		if let Some(face_key) = test_inclusion(&trees[i].component, &new_tree.component, edges, vertices) {
			// TODO: (@TrueDoctor) use swap remove
			let tree = trees.remove(i);
			new_tree.outgoing_edges.entry(face_key).or_default().push(tree);
		} else {
			i += 1;
		}
	}

	trees.push(new_tree);
}

fn get_flag(count: i32, fill_rule: FillRule) -> u8 {
	match fill_rule {
		FillRule::NonZero => {
			if count == 0 {
				0
			} else {
				1
			}
		}
		FillRule::EvenOdd => (count % 2).unsigned_abs() as u8,
	}
}

/// Determines which faces should be included in the result based on the boolean operation.
///
/// This function applies the specified boolean operation and fill rules to decide
/// which regions of the dual graph should be part of the resulting path.
fn flag_faces(
	nesting_trees: &[NestingTree],
	a_fill_rule: FillRule,
	b_fill_rule: FillRule,
	edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>,
	vertices: &SlotMap<DualVertexKey, DualGraphVertex>,
	flags: &mut HashMap<DualVertexKey, u8>,
) {
	for tree in nesting_trees.iter() {
		let mut tree_stack = vec![(tree, 0, 0)];

		while let Some((current_tree, a_running_count, b_running_count)) = tree_stack.pop() {
			let mut visited_faces = HashSet::new();
			let mut face_stack = VecDeque::new();

			let outer_face_key = current_tree.component.outer_face.expect("Component doesn't have an outer face.");
			face_stack.push_back((outer_face_key, a_running_count, b_running_count));

			while let Some((face_key, a_count, b_count)) = face_stack.pop_front() {
				if visited_faces.contains(&face_key) {
					continue;
				}
				visited_faces.insert(face_key);

				let a_flag = get_flag(a_count, a_fill_rule);
				let b_flag = get_flag(b_count, b_fill_rule);
				*flags.entry(face_key).or_default() = a_flag | (b_flag << 1);

				for edge_key in &vertices[face_key].incident_edges {
					let edge = &edges[*edge_key];
					let twin_key = edge.twin.expect("Edge doesn't have a twin");
					#[cfg(feature = "logging")]
					eprintln!("Processing edge: {:?} to: {:?}", edge_key.0, edges[twin_key].incident_vertex.0);
					let mut next_a_count = a_count;
					if edge.parent & 1 != 0 {
						next_a_count += if edge.direction_flag.forward() { 1 } else { -1 };
					}
					let mut next_b_count = b_count;
					if edge.parent & 2 != 0 {
						next_b_count += if edge.direction_flag.forward() { 1 } else { -1 };
					}
					#[cfg(feature = "logging")]
					eprintln!("next_count a: {}, b:{}", next_a_count, next_b_count);
					face_stack.push_back((edges[twin_key].incident_vertex, next_a_count, next_b_count));
				}

				// Collect subtrees to be processed later
				if let Some(subtrees) = current_tree.outgoing_edges.get(&face_key) {
					for subtree in subtrees {
						tree_stack.push((subtree, a_count, b_count));
					}
				}
			}
		}
	}
}

fn get_selected_faces<'a>(predicate: &'a impl Fn(u8) -> bool, flags: &'a HashMap<DualVertexKey, u8>) -> impl Iterator<Item = DualVertexKey> + 'a {
	flags.iter().filter_map(|(key, &flag)| predicate(flag).then_some(*key))
}

fn walk_faces<'a>(faces: &'a [DualVertexKey], edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>, vertices: &SlotMap<DualVertexKey, DualGraphVertex>) -> impl Iterator<Item = PathSegment> + use<'a> {
	let face_set: HashSet<_> = faces.iter().copied().collect();
	// TODO: Try using a binary search to avoid the hashset construction
	let is_removed_edge = |edge: &DualGraphHalfEdge| face_set.contains(&edge.incident_vertex) == face_set.contains(&edges[edge.twin.unwrap()].incident_vertex);

	let mut edge_to_next = HashMap::new();
	for face_key in faces {
		let face = &vertices[*face_key];
		let mut prev_edge = *face.incident_edges.last().unwrap();
		for &edge in &face.incident_edges {
			edge_to_next.insert(prev_edge, edge);
			prev_edge = edge;
		}
	}

	let mut visited_edges = HashSet::new();
	let mut result = Vec::new();

	for &face_key in faces {
		let face = &vertices[face_key];
		for &start_edge in &face.incident_edges {
			if is_removed_edge(&edges[start_edge]) || visited_edges.contains(&start_edge) {
				continue;
			}
			let mut edge = start_edge;
			loop {
				let current_edge = &edges[edge];
				if current_edge.direction_flag.forward() {
					result.extend(current_edge.segments.iter().cloned());
				} else {
					result.extend(current_edge.segments.iter().map(PathSegment::reverse));
				}
				visited_edges.insert(edge);
				edge = *edge_to_next.get(&edge).unwrap();
				while is_removed_edge(&edges[edge]) {
					edge = *edge_to_next.get(&edges[edge].twin.unwrap()).unwrap();
				}
				if edge == start_edge {
					break;
				}
			}
		}
	}

	result.into_iter()
}

/// Reconstructs the resulting path(s) from the selected faces of the dual graph.
///
/// This function takes the faces that were flagged for inclusion and reconstructs
/// the path segments that form the boundaries of these faces, resulting in the
/// final output of the boolean operation.
fn dump_faces(
	nesting_trees: &[NestingTree],
	predicate: impl Fn(u8) -> bool + Copy,
	edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>,
	vertices: &SlotMap<DualVertexKey, DualGraphVertex>,
	flags: &HashMap<DualVertexKey, u8>,
) -> Vec<Path> {
	let mut paths = Vec::new();

	fn visit(
		tree: &NestingTree,
		predicate: impl Fn(u8) -> bool + Copy,
		paths: &mut Vec<Path>,
		edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>,
		vertices: &SlotMap<DualVertexKey, DualGraphVertex>,
		flags: &HashMap<DualVertexKey, u8>,
	) {
		for &face_key in tree.component.vertices.iter() {
			let face = &vertices[face_key];
			let flag = flags[&face_key];
			if !predicate(flag) || Some(face_key) == tree.component.outer_face {
				continue;
			}

			let mut path = Vec::new();

			for &edge_key in &face.incident_edges {
				let edge = &edges[edge_key];
				if edge.direction_flag.forward() {
					path.extend(edge.segments.iter().cloned());
				} else {
					path.extend(edge.segments.iter().map(PathSegment::reverse));
				}
			}

			// Poke holes in the face
			if let Some(subtrees) = tree.outgoing_edges.get(&face_key) {
				for subtree in subtrees {
					let outer_face_key = subtree.component.outer_face.unwrap();
					for &edge_key in &vertices[outer_face_key].incident_edges {
						let edge = &edges[edge_key];
						if edge.direction_flag.forward() {
							path.extend(edge.segments.iter().cloned());
						} else {
							path.extend(edge.segments.iter().map(PathSegment::reverse));
						}
					}
				}
			}

			paths.push(path);
		}

		for subtrees in tree.outgoing_edges.values() {
			for subtree in subtrees {
				visit(subtree, predicate, paths, edges, vertices, flags);
			}
		}
	}

	for tree in nesting_trees {
		visit(tree, predicate, &mut paths, edges, vertices, flags);
	}

	paths
}

const OPERATION_PREDICATES: [fn(u8) -> bool; 6] = [
	|flag: u8| flag > 0,               // Union
	|flag: u8| flag == 1,              // Difference
	|flag: u8| flag == 0b11,           // Intersection
	|flag: u8| flag == 1 || flag == 2, // Exclusion
	|flag: u8| (flag & 1) == 1,        // Division
	|flag: u8| flag > 0,               // Fracture
];

/// Represents errors that can occur during boolean operations on paths.
#[derive(Debug)]
pub enum BooleanError {
	/// Indicates that multiple outer faces were found where only one was expected.
	MultipleOuterFaces,
	/// Indicates that no valid ear was found in a polygon during triangulation. <https://en.wikipedia.org/wiki/Vertex_(geometry)#Ears>
	NoEarInPolygon,
	InvalidPathCommand(char),
}

impl Display for BooleanError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::MultipleOuterFaces => f.write_str("Found multiple candidates for the outer face in a connected component of the dual graph."),
			Self::NoEarInPolygon => f.write_str("Failed to compute winding order for one of the faces, this usually happens when the polygon is malformed."),
			Self::InvalidPathCommand(cmd) => f.write_fmt(format_args!("Encountered a '{cmd}' while parsing the svg data which was not recognized")),
		}
	}
}

/// Performs boolean operations on two paths.
///
/// Takes two paths, applies specified fill rules, and performs a boolean operation,
/// returning the resulting path(s).
///
/// # Examples
///
/// ```
/// use path_bool::{path_boolean, FillRule, PathBooleanOperation, path_from_path_data, path_to_path_data};
///
/// let path_a = path_from_path_data("M 10 10 L 50 10 L 30 40 Z").unwrap();
/// let path_b = path_from_path_data("M 20 30 L 60 30 L 60 50 L 20 50 Z").unwrap();
///
/// let result = path_boolean(
///     &path_a,
///     FillRule::NonZero,
///     &path_b,
///     FillRule::NonZero,
///     PathBooleanOperation::Intersection
/// ).unwrap();
///
/// let result_data = path_to_path_data(&result[0], 0.001);
/// assert_eq!(result_data, "M 36.666666666667,30.000000000000 L 23.333333333333,30.000000000000 L 30.000000000000,40.000000000000 L 36.666666666667,30.000000000000 Z");
/// ```
///
/// # Operations
///
/// The function supports various boolean operations:
/// - Union
/// - Difference
/// - Intersection
/// - Exclusion
/// - Division
/// - Fracture
///
/// See [`PathBooleanOperation`] for more details on each operation.
///
/// # Algorithm
///
/// The boolean operation is performed in several steps:
///
/// 1. Preprocessing: Convert input paths to edges and split at intersections.
/// 2. Graph Construction: Build a graph representation of path segments.
/// 3. Intersection Analysis: Compute intersections between path segments.
/// 4. Graph Transformation: Convert the initial graph into the graph minor using edge contractions.
/// 5. Nesting Analysis: Determine nesting relationships between path parts.
/// 6. Boolean Evaluation: Apply the specified operation based on nesting.
/// 7. Result Construction: Generate final path(s) based on the operation result.
///
/// # Errors
///
/// Returns a [`BooleanError`] if:
/// - Input paths are invalid or cannot be processed.
/// - The operation encounters an unsolvable geometric configuration.
/// - Issues arise in determining the nesting structure of the paths.
pub fn path_boolean(a: &Path, a_fill_rule: FillRule, b: &Path, b_fill_rule: FillRule, op: PathBooleanOperation) -> Result<Vec<Path>, BooleanError> {
	let mut unsplit_edges: Vec<MajorGraphEdgeStage1> = a.iter().map(segment_to_edge(1)).chain(b.iter().map(segment_to_edge(2))).flatten().collect();

	split_at_self_intersections(&mut unsplit_edges);

	let (split_edges, total_bounding_box) = split_at_intersections(&unsplit_edges);

	#[cfg(feature = "logging")]
	for (edge, _, _) in split_edges.iter() {
		eprintln!("{}", path_to_path_data(&vec![*edge], 0.001));
	}

	let total_bounding_box = match total_bounding_box {
		Some(bb) => bb,
		None => return Ok(Vec::new()), // Input geometry is empty
	};

	let major_graph = find_vertices(&split_edges, total_bounding_box);

	#[cfg(feature = "logging")]
	eprintln!("Major graph:");
	#[cfg(feature = "logging")]
	eprintln!("{}", major_graph_to_dot(&major_graph));

	let mut minor_graph = compute_minor(&major_graph);

	#[cfg(feature = "logging")]
	eprintln!("Minor graph:");
	#[cfg(feature = "logging")]
	eprintln!("{}", minor_graph_to_dot(&minor_graph.edges));

	remove_dangling_edges(&mut minor_graph);
	#[cfg(feature = "logging")]
	eprintln!("After removing dangling edges:");
	#[cfg(feature = "logging")]
	eprintln!("{}", minor_graph_to_dot(&minor_graph.edges));

	#[cfg(feature = "logging")]
	for (key, edge) in minor_graph.edges.iter() {
		eprintln!("{key:?}:\n{}", path_to_path_data(&edge.segments, 0.001));
	}
	#[cfg(feature = "logging")]
	for vertex in minor_graph.vertices.values() {
		eprintln!("{:?}", vertex);
	}
	sort_outgoing_edges_by_angle(&mut minor_graph);
	#[cfg(feature = "logging")]
	for vertex in minor_graph.vertices.values() {
		eprintln!("{:?}", vertex);
	}

	for (edge_key, edge) in &minor_graph.edges {
		assert!(minor_graph.vertices.contains_key(edge.incident_vertices[0]), "Edge {:?} has invalid start vertex", edge_key);
		assert!(minor_graph.vertices.contains_key(edge.incident_vertices[1]), "Edge {:?} has invalid end vertex", edge_key);
		assert!(edge.twin.is_some(), "Edge {:?} should have a twin", edge_key);
		let twin = &minor_graph.edges[edge.twin.unwrap()];
		assert_eq!(twin.twin.unwrap(), edge_key, "Twin relationship should be symmetrical for edge {:?}", edge_key);
	}

	let dual_graph = compute_dual(&minor_graph)?;

	let nesting_trees = compute_nesting_tree(&dual_graph);

	#[cfg(feature = "logging")]
	for tree in &nesting_trees {
		eprintln!("nesting_trees: {:?}", tree);
	}

	let DualGraph { edges, vertices, .. } = &dual_graph;

	#[cfg(feature = "logging")]
	eprintln!("Dual Graph:");
	#[cfg(feature = "logging")]
	eprintln!("{}", dual_graph_to_dot(&dual_graph.components, edges));

	let mut flags = HashMap::new();
	flag_faces(&nesting_trees, a_fill_rule, b_fill_rule, edges, vertices, &mut flags);

	#[cfg(feature = "logging")]
	for (face, flag) in &flags {
		eprintln!("{:?}: {:b}", face.0, flag);
	}

	let predicate = OPERATION_PREDICATES[op as usize];

	match op {
		PathBooleanOperation::Division | PathBooleanOperation::Fracture => Ok(dump_faces(&nesting_trees, predicate, edges, vertices, &flags)),
		_ => {
			let mut selected_faces: Vec<DualVertexKey> = get_selected_faces(&predicate, &flags).collect();
			selected_faces.sort_unstable();
			Ok(vec![walk_faces(&selected_faces, edges, vertices).collect()])
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use glam::DVec2;
	use std::f64::consts::TAU; // Assuming DVec2 is defined in your crate

	#[test]
	fn test_split_at_intersections() {
		let unsplit_edges = unsplit_edges();
		let (split_edges, total_bounding_box) = split_at_intersections(&unsplit_edges);

		// Check that we have a valid bounding box
		assert!(total_bounding_box.is_some());

		// Check that we have more edges after splitting (due to intersections)
		assert!(split_edges.len() >= unsplit_edges.len());

		// Check that all edges have a valid bounding box
		for (_, _, bb) in &split_edges {
			assert!(bb.left <= bb.right);
			assert!(bb.top <= bb.bottom);
		}

		// You might want to add more specific checks based on the expected behavior
		// of your split_at_intersections function
	}

	fn unsplit_edges() -> Vec<(PathSegment, u8)> {
		let unsplit_edges = vec![
			(PathSegment::Arc(DVec2::new(39., 20.), 19., 19., 0., false, true, DVec2::new(20., 39.)), 1),
			(PathSegment::Arc(DVec2::new(20., 39.), 19., 19., 0., false, true, DVec2::new(1., 20.)), 1),
			(PathSegment::Arc(DVec2::new(1., 20.), 19., 19., 0., false, true, DVec2::new(20., 1.)), 1),
			(PathSegment::Arc(DVec2::new(20., 1.), 19., 19., 0., false, true, DVec2::new(39., 20.)), 1),
			(PathSegment::Arc(DVec2::new(47., 28.), 19., 19., 0., false, true, DVec2::new(28., 47.)), 2),
			(PathSegment::Arc(DVec2::new(28., 47.), 19., 19., 0., false, true, DVec2::new(9., 28.)), 2),
			(PathSegment::Arc(DVec2::new(9., 28.), 19., 19., 0., false, true, DVec2::new(28., 9.)), 2),
			(PathSegment::Arc(DVec2::new(28., 9.), 19., 19., 0., false, true, DVec2::new(47., 28.)), 2),
		];
		unsplit_edges
	}

	#[test]
	fn test_compute_minor() {
		// Set up the initial graph
		let unsplit_edges = unsplit_edges();
		let (split_edges, total_bounding_box) = split_at_intersections(&unsplit_edges);
		let major_graph = find_vertices(&split_edges, total_bounding_box.unwrap());

		// Compute minor graph
		let minor_graph = compute_minor(&major_graph);

		// Print minor graph state
		eprintln!("Minor Graph:");
		print_minor_graph_state(&minor_graph);

		// Assertions
		assert_eq!(minor_graph.edges.len(), 8, "Expected 8 edges in minor graph");
		assert_eq!(minor_graph.vertices.len(), 2, "Expected 2 vertices in minor graph");
		assert!(minor_graph.cycles.is_empty(), "Expected no cycles in minor graph");

		// Check that each vertex has 4 outgoing edges
		for (vertex_key, vertex) in &minor_graph.vertices {
			assert_eq!(vertex.outgoing_edges.len(), 4, "Vertex {:?} should have 4 outgoing edges", vertex_key);
		}

		// Check that all edges have valid incident vertices and twins
		for (edge_key, edge) in &minor_graph.edges {
			assert!(minor_graph.vertices.contains_key(edge.incident_vertices[0]), "Edge {:?} has invalid start vertex", edge_key);
			assert!(minor_graph.vertices.contains_key(edge.incident_vertices[1]), "Edge {:?} has invalid end vertex", edge_key);
			assert!(edge.twin.is_some(), "Edge {:?} should have a twin", edge_key);
			let twin = &minor_graph.edges[edge.twin.unwrap()];
			assert_eq!(twin.twin.unwrap(), edge_key, "Twin relationship should be symmetrical for edge {:?}", edge_key);
		}

		// Check that parents are correctly assigned
		assert_eq!(minor_graph.edges.values().filter(|e| e.parent == 1).count(), 4, "Expected 4 edges with parent 1");
		assert_eq!(minor_graph.edges.values().filter(|e| e.parent == 2).count(), 4, "Expected 4 edges with parent 2");
	}

	fn print_minor_graph_state(graph: &MinorGraph) {
		eprintln!("  Vertices: {}", graph.vertices.len());
		eprintln!("  Edges: {}", graph.edges.len());
		eprintln!("  Cycles: {}", graph.cycles.len());

		for (vertex_key, vertex) in &graph.vertices {
			eprintln!("    Vertex {:?}: {} outgoing edges", vertex_key, vertex.outgoing_edges.len());
		}

		for (edge_key, edge) in &graph.edges {
			eprintln!("    Edge {:?}:", edge_key);
			eprintln!("      Parent: {}", edge.parent);
			eprintln!("      Twin: {:?}", edge.twin);
			eprintln!("      Incident vertices: {:?}", edge.incident_vertices);
		}
	}

	#[test]
	fn test_sort_outgoing_edges_by_angle() {
		// Set up the initial graph
		let unsplit_edges = unsplit_edges();
		let (split_edges, total_bounding_box) = split_at_intersections(&unsplit_edges);
		let major_graph = find_vertices(&split_edges, total_bounding_box.unwrap());
		let mut minor_graph = compute_minor(&major_graph);

		// Print initial state
		eprintln!("Initial Minor Graph:");
		print_minor_graph_state(&minor_graph);

		// Store initial edge order
		let initial_edge_order: HashMap<MinorVertexKey, Vec<MinorEdgeKey>> = minor_graph.vertices.iter().map(|(k, v)| (k, v.outgoing_edges.clone())).collect();

		// Apply sort_outgoing_edges_by_angle
		sort_outgoing_edges_by_angle(&mut minor_graph);

		// Print final state
		eprintln!("\nAfter sort_outgoing_edges_by_angle:");
		print_minor_graph_state(&minor_graph);

		// Assertions
		assert_eq!(minor_graph.edges.len(), 8, "Number of edges should remain unchanged");
		assert_eq!(minor_graph.vertices.len(), 2, "Number of vertices should remain unchanged");
		assert!(minor_graph.cycles.is_empty(), "Expected no cycles");

		// Check that each vertex still has 4 outgoing edges
		for (vertex_key, vertex) in &minor_graph.vertices {
			assert_eq!(vertex.outgoing_edges.len(), 4, "Vertex {:?} should have 4 outgoing edges", vertex_key);
		}

		// Check that the edges are sorted by angle
		for (vertex_key, vertex) in &minor_graph.vertices {
			let angles: Vec<f64> = vertex.outgoing_edges.iter().map(|&edge_key| get_incidence_angle(&minor_graph.edges[edge_key])).collect();

			// Check if angles are in ascending order
			for i in 1..angles.len() {
				assert!(angles[i] >= angles[i - 1], "Edges for vertex {:?} are not sorted by angle {} {}", vertex_key, angles[i], angles[i - 1]);
			}

			// Check that the set of edges is the same as before, just in different order
			let initial_edges: HashSet<_> = initial_edge_order[&vertex_key].iter().collect();
			let sorted_edges: HashSet<_> = vertex.outgoing_edges.iter().collect();
			assert_eq!(initial_edges, sorted_edges, "Set of edges for vertex {:?} changed after sorting", vertex_key);
		}

		// Check that all edges still have valid incident vertices and twins
		for (edge_key, edge) in &minor_graph.edges {
			assert!(minor_graph.vertices.contains_key(edge.incident_vertices[0]), "Edge {:?} has invalid start vertex", edge_key);
			assert!(minor_graph.vertices.contains_key(edge.incident_vertices[1]), "Edge {:?} has invalid end vertex", edge_key);
			assert!(edge.twin.is_some(), "Edge {:?} should have a twin", edge_key);
			let twin = &minor_graph.edges[edge.twin.unwrap()];
			assert_eq!(twin.twin.unwrap(), edge_key, "Twin relationship should be symmetrical for edge {:?}", edge_key);
		}
	}

	fn get_incidence_angle(edge: &MinorGraphEdge) -> f64 {
		let seg = &edge.segments[0]; // First segment is always the incident one in both fwd and bwd
		let (p0, p1) = if edge.direction_flag.forward() {
			(seg.sample_at(0.), seg.sample_at(0.1))
		} else {
			(seg.sample_at(1.), seg.sample_at(1. - 0.1))
		};
		((p1.y - p0.y).atan2(p1.x - p0.x) + TAU) % TAU
	}

	#[test]
	fn test_path_segment_horizontal_ray_intersection_count() {
		let orig_seg = PathSegment::Arc(DVec2::new(24., 10.090978), 13.909023, 13.909023, 0., false, true, DVec2::new(47., 24.));

		let point = DVec2::new(37.99, 24.);

		eprintln!("Starting test with segment: {:?}", orig_seg);
		eprintln!("Test point: {:?}", point);

		let count = path_segment_horizontal_ray_intersection_count(&orig_seg, point);

		eprintln!("Final intersection count: {}", count);

		let expected_count = 1;
		assert_eq!(count, expected_count, "Intersection count mismatch");
	}

	#[test]
	fn test_bounding_box_intersects_horizontal_ray() {
		let bbox = Aabb {
			top: 10.,
			right: 40.,
			bottom: 30.,
			left: 20.,
		};

		assert!(bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(0., 30.)));
		assert!(bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(20., 30.)));
		assert!(bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(10., 20.)));
		assert!(!bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(30., 40.)));
	}
}
