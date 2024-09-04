use slotmap::{new_key_type, SlotMap};

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

use crate::aabb::{bounding_box_around_point, bounding_box_max_extent, merge_bounding_boxes, AaBb};
use crate::epsilons::Epsilons;
use crate::intersection_path_segment::{path_segment_intersection, segments_equal};
use crate::path::Path;
use crate::path_cubic_segment_self_intersection::path_cubic_segment_self_intersection;
use crate::path_segment::{get_end_point, get_start_point, path_segment_bounding_box, reverse_path_segment, sample_path_segment_at, split_segment_at, PathSegment};
use crate::path_to_path_data;
use crate::quad_tree::QuadTree;
use crate::vector::{vectors_equal, Vector};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy)]
pub enum PathBooleanOperation {
	Union,
	Difference,
	Intersection,
	Exclusion,
	Division,
	Fracture,
}

#[derive(Debug, Clone, Copy)]
pub enum FillRule {
	NonZero,
	EvenOdd,
}

const INTERSECTION_TREE_DEPTH: usize = 8;
const POINT_TREE_DEPTH: usize = 8;

const EPS: Epsilons = Epsilons {
	point: 1e-6,
	linear: 1e-4,
	param: 1e-8,
};

type MajorGraphEdgeStage1 = (PathSegment, u8);
type MajorGraphEdgeStage2 = (PathSegment, u8, AaBb);

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
	pub point: Vector,
	outgoing_edges: Vec<MajorEdgeKey>,
}

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

#[cfg(feature = "logging")]
impl MinorGraphEdge {
	fn format_path(&self) -> String {
		use std::fmt::Write;
		let mut output = String::new();
		let segments = self.segments.clone();
		for segment in segments.into_iter() {
			let _ = match segment {
				PathSegment::Line(mut start, mut end) | PathSegment::Cubic(mut start, _, _, mut end) => {
					if self.direction_flag.backwards() {
						(end, start) = (start, end);
					}
					write!(&mut output, "{:.1},{:.1}-{:.1},{:.1}  ", start.x, start.y, end.x, end.y)
				}
				x => write!(&mut output, "{:?}", x),
			};
		}
		output
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

#[derive(Debug, Clone)]
struct DualGraphComponent {
	edges: Vec<DualEdgeKey>,
	vertices: Vec<DualVertexKey>,
	outer_face: Option<DualVertexKey>,
}

#[derive(Debug, Clone)]
struct DualGraph {
	components: Vec<DualGraphComponent>,
	edges: SlotMap<DualEdgeKey, DualGraphHalfEdge>,
	vertices: SlotMap<DualVertexKey, DualGraphVertex>,
}

#[derive(Debug, Clone)]
struct NestingTree {
	component: DualGraphComponent,
	outgoing_edges: HashMap<DualVertexKey, Vec<NestingTree>>,
}

#[cfg(feature = "logging")]
fn major_graph_to_dot(graph: &MajorGraph) -> String {
	let mut dot = String::from("digraph {\n");
	for (vertex_key, vertex) in &graph.vertices {
		dot.push_str(&format!("  {:?} [label=\"{:.1},{:.1}\"]\n", (vertex_key.0.as_ffi() & 0xFF) - 1, vertex.point.x, vertex.point.y));
	}
	for (edge_key, edge) in &graph.edges {
		dot.push_str(&format!(
			"  {:?} -> {:?}: {:0b}\n",
			(edge.incident_vertices[0].0.as_ffi() & 0xFF) - 1,
			(edge.incident_vertices[1].0.as_ffi() & 0xFF) - 1,
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
			(edge.incident_vertices[0].0.as_ffi() & 0xFF) - 1,
			(edge.incident_vertices[1].0.as_ffi() & 0xFF) - 1,
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
				(edge.incident_vertex.0.as_ffi() & 0xFF) - 1,
				(edges[edge.twin.unwrap()].incident_vertex.0.as_ffi() & 0xFF) - 1
			));
		}
	}
	dot.push_str("}\n");
	dot
}

fn segment_to_edge(parent: u8) -> impl Fn(&PathSegment) -> MajorGraphEdgeStage1 {
	move |seg| (*seg, parent)
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
					let (seg1, seg2) = split_segment_at(seg, t1);
					*seg = seg1;
					new_edges.push((seg2, *parent));
				} else {
					let (seg1, tmp_seg) = split_segment_at(seg, t1);
					let (seg2, seg3) = split_segment_at(&tmp_seg, (t2 - t1) / (1.0 - t1));
					*seg = seg1;
					new_edges.push((seg2, *parent));
					new_edges.push((seg3, *parent));
				}
			}
		}
	}
	edges.extend(new_edges);
}

fn split_at_intersections(edges: &[MajorGraphEdgeStage1]) -> (Vec<MajorGraphEdgeStage2>, Option<AaBb>) {
	// Step 1: Add bounding boxes to edges
	let with_bounding_box: Vec<MajorGraphEdgeStage2> = edges.iter().map(|(seg, parent)| (*seg, *parent, path_segment_bounding_box(seg))).collect();

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
			let candidate: &(_, _) = &edges[j];
			let include_endpoints = edge.1 != candidate.1
				|| !(vectors_equal(get_end_point(&candidate.0), get_start_point(&edge.0), EPS.point) || vectors_equal(get_start_point(&candidate.0), get_end_point(&edge.0), EPS.point));
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
			let mut prev_t = 0.0;
			for &t in splits.iter() {
				if t > 1.0 - EPS.param {
					break;
				}
				let tt = (t - prev_t) / (1.0 - prev_t);
				prev_t = t;
				if tt < EPS.param {
					continue;
				}
				if tt > 1.0 - EPS.param {
					continue;
				}
				let (seg1, seg2) = split_segment_at(&tmp_seg, tt);
				new_edges.push((seg1, parent, path_segment_bounding_box(&seg1)));
				tmp_seg = seg2;
			}
			new_edges.push((tmp_seg, parent, path_segment_bounding_box(&tmp_seg)));
		} else {
			new_edges.push((seg, parent, path_segment_bounding_box(&seg)));
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
	pub fn backwards(self) -> bool {
		self == Self::Backwards
	}
}

// TODO:(@TrueDoctor) Optimize this by rounding each vertex up and down and then inserting them in a hashmap. This should remove the need for bbox calculations and the quad tree
fn find_vertices(edges: &[MajorGraphEdgeStage2], bounding_box: AaBb) -> MajorGraph {
	let mut vertex_tree = QuadTree::new(bounding_box, POINT_TREE_DEPTH, 8);
	let mut graph = MajorGraph {
		edges: SlotMap::with_key(),
		vertices: SlotMap::with_key(),
	};

	let mut parents: HashMap<MajorEdgeKey, u8> = HashMap::new();

	let mut vertex_pair_id_to_edges: HashMap<_, Vec<(MajorGraphEdgeStage2, MajorEdgeKey, MajorEdgeKey)>> = HashMap::new();

	for (seg, parent, bounding_box) in edges {
		let mut get_vertex = |point: Vector| -> MajorVertexKey {
			let box_around_point = bounding_box_around_point(point, EPS.point);
			if let Some(&existing_vertex) = vertex_tree.find(&box_around_point).iter().next() {
				existing_vertex
			} else {
				let vertex_key = graph.vertices.insert(MajorGraphVertex { point, outgoing_edges: Vec::new() });
				vertex_tree.insert(box_around_point, vertex_key);
				vertex_key
			}
		};

		let start_vertex = get_vertex(get_start_point(seg));
		let end_vertex = get_vertex(get_end_point(seg));

		if start_vertex == end_vertex {
			match seg {
				PathSegment::Line(..) => continue,
				PathSegment::Cubic(_, c1, c2, _) => {
					if vectors_equal(*c1, *c2, EPS.point) {
						continue;
					}
				}
				PathSegment::Quadratic(_, c, _) => {
					if vectors_equal(get_start_point(seg), *c, EPS.point) {
						continue;
					}
				}
				PathSegment::Arc(_, _, _, _, _, false, _) => continue,
				_ => {}
			}
		}

		let vertex_pair_id = (start_vertex.min(end_vertex), start_vertex.max(end_vertex));
		if let Some(existing_edges) = vertex_pair_id_to_edges.get(&vertex_pair_id) {
			if let Some(existing_edge) = existing_edges.iter().find(|(other_seg, ..)| segments_equal(seg, &other_seg.0, EPS.point)) {
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
				// choose the edge which is not our twin so we can make progress
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
	// Basically DFS for each parent with bfs number
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

fn get_incidence_angle(edge: &MinorGraphEdge) -> f64 {
	let seg = &edge.segments[0]; // TODO: explain in comment why this is always the incident one in both fwd and bwd

	// println!("{edge:?}"); //, edge.direction_flag.forward());
	let (p0, p1) = if edge.direction_flag.forward() {
		(sample_path_segment_at(seg, 0.0), sample_path_segment_at(seg, EPS.param))
	} else {
		(sample_path_segment_at(seg, 1.0), sample_path_segment_at(seg, 1.0 - EPS.param))
	};

	// println!("{p0:?} {p1:?}");
	let angle = (p1.y - p0.y).atan2(p1.x - p0.x);
	// println!("angle: {}", angle);
	(angle * 10000.).round() / 1000.
}

fn sort_outgoing_edges_by_angle(graph: &mut MinorGraph) {
	for vertex in graph.vertices.values_mut() {
		if vertex.outgoing_edges.len() > 2 {
			let edges: Vec<_> = vertex
				.outgoing_edges
				.iter()
				.map(|key| (*key, &graph.edges[*key]))
				.map(|(key, edge)| ((key.0.as_ffi() & 0xFF), get_incidence_angle(edge)))
				.collect();
			vertex.outgoing_edges.sort_by(|&a, &b| {
				// TODO(@TrueDoctor): Make more robust. The js version seems to sort the data slightly differently when the angles are reallly close. In that case put the edge wich was discovered later first.
				(get_incidence_angle(&graph.edges[a]) - (a.0.as_ffi() & 0xFFFFFF) as f64 / 1000000.)
					.partial_cmp(&(get_incidence_angle(&graph.edges[b]) - (b.0.as_ffi() & 0xFFFFFF) as f64 / 1000000.))
					.unwrap_or(b.cmp(&a))
			});
			let edges: Vec<_> = vertex
				.outgoing_edges
				.iter()
				.map(|key| (*key, &graph.edges[*key]))
				.map(|(key, edge)| ((key.0.as_ffi() & 0xFF), get_incidence_angle(edge)))
				.collect();
		}
	}
}

fn face_to_polygon(face: &DualGraphVertex, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>) -> Vec<Vector> {
	const CNT: usize = 3;
	#[cfg(feature = "logging")]
	println!("incident node counts {}", face.incident_edges.len());

	face.incident_edges
		.iter()
		.flat_map(|&edge_key| {
			let edge = &edges[edge_key];
			// println!("{}", path_to_path_data(&edge.segments, 0.001));
			edge.segments.iter().flat_map(move |seg| {
				(0..CNT).map(move |i| {
					let t0 = i as f64 / CNT as f64;
					let t = if edge.direction_flag.forward() { t0 } else { 1.0 - t0 };
					sample_path_segment_at(seg, t)
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

fn line_segment_intersects_horizontal_ray(a: Vector, b: Vector, point: Vector) -> bool {
	if !interval_crosses_point(a.y, b.y, point.y) {
		return false;
	}
	let x = crate::math::lin_map(point.y, a.y, b.y, a.x, b.x);
	x >= point.x
}

fn compute_point_winding(polygon: &[Vector], tested_point: Vector) -> i32 {
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

fn compute_winding(face: &DualGraphVertex, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>) -> (i32, Vector) {
	let polygon = face_to_polygon(face, edges);
	#[cfg(feature = "logging")]
	for point in &polygon {
		println!("[{}, {}]", point.x, point.y);
	}

	for i in 0..polygon.len() {
		let a = polygon[i];
		let b = polygon[(i + 1) % polygon.len()];
		let c = polygon[(i + 2) % polygon.len()];
		let center = (a + b + c) / 3.;
		let winding = compute_point_winding(&polygon, center);
		if winding != 0 {
			return (winding, center);
		}
	}

	panic!("No ear in polygon found.");
}

fn compute_dual(minor_graph: &MinorGraph) -> Option<DualGraph> {
	let mut new_vertices: Vec<DualVertexKey> = Vec::new();
	let mut minor_to_dual_edge: HashMap<MinorEdgeKey, DualEdgeKey> = HashMap::new();
	let mut dual_edges = SlotMap::with_key();
	let mut dual_vertices = SlotMap::with_key();

	for (start_edge_key, start_edge) in &minor_graph.edges {
		#[cfg(feature = "logging")]
		println!("Processing start edge: {}", (start_edge_key.0.as_ffi() & 0xFF) - 1);
		if minor_to_dual_edge.contains_key(&start_edge_key) {
			continue;
		}

		let face_key = dual_vertices.insert(DualGraphVertex { incident_edges: Vec::new() });

		let mut edge_key = start_edge_key;
		let mut edge = start_edge;

		loop {
			#[cfg(feature = "logging")]
			println!("Processing edge: {}", (start_edge_key.0.as_ffi() & 0xFF) - 1);
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
			println!("Next edge: {}", (start_edge_key.0.as_ffi() & 0xFF) - 1);
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
		println!("faces: {}, dual-edges: {}, cycles: {}", new_vertices.len(), dual_edges.len(), minor_graph.cycles.len())
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
		println!("component_vertices: {}", component_vertices.len());
		for edge in &dual_edges {
			// println!("{:?}", edge.incident_vertex);
		}

		let outer_face_key = *component_vertices
			.iter()
			.find(|&&face_key| compute_winding(&dual_vertices[face_key], &dual_edges).0 < 0)
			.expect("No outer face of a component found.");

		#[cfg(feature = "logging")]
		if cfg!(feature = "logging") {
			println!(
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

		if component_vertices.iter().filter(|&&face_key| compute_winding(&dual_vertices[face_key], &dual_edges).0 < 0).count() > 1 {
			return None;
		}
		// TODO: merge with previous iter
		assert_eq!(
			component_vertices.iter().filter(|&&face_key| compute_winding(&dual_vertices[face_key], &dual_edges).0 < 0).count(),
			1,
			"Multiple outer faces found."
		);

		components.push(DualGraphComponent {
			vertices: component_vertices,
			edges: component_edges,
			outer_face: Some(outer_face_key),
		});
	}

	Some(DualGraph {
		vertices: dual_vertices,
		edges: dual_edges,
		components,
	})
}

fn get_next_edge(edge_key: MinorEdgeKey, graph: &MinorGraph) -> MinorEdgeKey {
	let edge = &graph.edges[edge_key];
	let vertex = &graph.vertices[edge.incident_vertices[1]];
	let index = vertex.outgoing_edges.iter().position(|&e| Some(edge_key) == graph.edges[e].twin).unwrap();
	vertex.outgoing_edges[(index + 1) % vertex.outgoing_edges.len()]
}

fn test_inclusion(a: &DualGraphComponent, b: &DualGraphComponent, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>, vertices: &SlotMap<DualVertexKey, DualGraphVertex>) -> Option<DualVertexKey> {
	let tested_point = get_start_point(&edges[a.edges[0]].segments[0]);
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
fn bounding_box_intersects_horizontal_ray(bounding_box: &AaBb, point: Vector) -> bool {
	interval_crosses_point(bounding_box.top, bounding_box.bottom, point[1]) && bounding_box.right >= point[0]
}

struct IntersectionSegment {
	bounding_box: AaBb,
	seg: PathSegment,
}

pub fn path_segment_horizontal_ray_intersection_count(orig_seg: &PathSegment, point: Vector) -> usize {
	let total_bounding_box = path_segment_bounding_box(orig_seg);

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
				if line_segment_intersects_horizontal_ray(get_start_point(&segment.seg), get_end_point(&segment.seg), point) {
					count += 1;
				}
			} else {
				let split = split_segment_at(&segment.seg, 0.5);
				let bounding_box0 = path_segment_bounding_box(&split.0);
				let bounding_box1 = path_segment_bounding_box(&split.1);

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

			while let Some((face_key, a_count, b_count)) = face_stack.pop_back() {
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
					let mut next_a_count = a_count;
					if edge.parent & 1 != 0 {
						next_a_count += if edge.direction_flag.forward() { 1 } else { -1 };
					}
					let mut next_b_count = b_count;
					if edge.parent & 2 != 0 {
						next_b_count += if edge.direction_flag.forward() { 1 } else { -1 };
					}
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

// TODO(@Truedoctor): Check if we can just iterate over the flags
fn get_selected_faces<'a>(predicate: &'a impl Fn(u8) -> bool, flags: &'a HashMap<DualVertexKey, u8>) -> impl Iterator<Item = DualVertexKey> + 'a {
	flags.iter().filter_map(|(key, &flag)| predicate(flag).then_some(*key))
}

fn walk_faces<'a>(faces: &'a HashSet<DualVertexKey>, edges: &SlotMap<DualEdgeKey, DualGraphHalfEdge>, vertices: &SlotMap<DualVertexKey, DualGraphVertex>) -> impl Iterator<Item = PathSegment> + 'a {
	let is_removed_edge = |edge: &DualGraphHalfEdge| faces.contains(&edge.incident_vertex) == faces.contains(&edges[edge.twin.unwrap()].incident_vertex);

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
					result.extend(current_edge.segments.iter().map(reverse_path_segment));
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
					path.extend(edge.segments.iter().map(reverse_path_segment));
				}
			}

			// poke holes in the face
			if let Some(subtrees) = tree.outgoing_edges.get(&face_key) {
				for subtree in subtrees {
					let outer_face_key = subtree.component.outer_face.unwrap();
					for &edge_key in &vertices[outer_face_key].incident_edges {
						let edge = &edges[edge_key];
						if edge.direction_flag.forward() {
							path.extend(edge.segments.iter().cloned());
						} else {
							path.extend(edge.segments.iter().map(reverse_path_segment));
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

// TODO: Impl error trait
#[derive(Debug)]
pub enum BooleanError {
	MultipleOuterFaces,
}

pub fn path_boolean(a: &Path, a_fill_rule: FillRule, b: &Path, b_fill_rule: FillRule, op: PathBooleanOperation) -> Result<Vec<Path>, BooleanError> {
	let mut unsplit_edges: Vec<MajorGraphEdgeStage1> = a.iter().map(segment_to_edge(1)).chain(b.iter().map(segment_to_edge(2))).collect();

	split_at_self_intersections(&mut unsplit_edges);

	let (split_edges, total_bounding_box) = split_at_intersections(&unsplit_edges);

	let total_bounding_box = match total_bounding_box {
		Some(bb) => bb,
		None => return Ok(Vec::new()), // input geometry is empty
	};

	let major_graph = find_vertices(&split_edges, total_bounding_box);

	#[cfg(feature = "logging")]
	println!("Major graph:");
	#[cfg(feature = "logging")]
	println!("{}", major_graph_to_dot(&major_graph));

	let mut minor_graph = compute_minor(&major_graph);

	#[cfg(feature = "logging")]
	println!("Minor graph:");
	#[cfg(feature = "logging")]
	println!("{}", minor_graph_to_dot(&minor_graph.edges));

	remove_dangling_edges(&mut minor_graph);
	#[cfg(feature = "logging")]
	println!("After removing dangling edges:");
	#[cfg(feature = "logging")]
	println!("{}", minor_graph_to_dot(&minor_graph.edges));

	#[cfg(feature = "logging")]
	for (key, edge) in minor_graph.edges.iter() {
		// println!("{}", edge.format_path());
		println!("{key:?}:\n{}", path_to_path_data(&edge.segments, 0.001));
	}
	#[cfg(feature = "logging")]
	for vertex in minor_graph.vertices.values() {
		println!("{:?}", vertex);
	}
	sort_outgoing_edges_by_angle(&mut minor_graph);
	#[cfg(feature = "logging")]
	for vertex in minor_graph.vertices.values() {
		println!("{:?}", vertex);
	}

	for (edge_key, edge) in &minor_graph.edges {
		assert!(minor_graph.vertices.contains_key(edge.incident_vertices[0]), "Edge {:?} has invalid start vertex", edge_key);
		assert!(minor_graph.vertices.contains_key(edge.incident_vertices[1]), "Edge {:?} has invalid end vertex", edge_key);
		assert!(edge.twin.is_some(), "Edge {:?} should have a twin", edge_key);
		let twin = &minor_graph.edges[edge.twin.unwrap()];
		assert_eq!(twin.twin.unwrap(), edge_key, "Twin relationship should be symmetrical for edge {:?}", edge_key);
	}

	let dual_graph = compute_dual(&minor_graph).ok_or(BooleanError::MultipleOuterFaces)?;

	let nesting_trees = compute_nesting_tree(&dual_graph);

	let DualGraph { edges, vertices, .. } = &dual_graph;

	#[cfg(feature = "logging")]
	println!("Dual Graph:");
	#[cfg(feature = "logging")]
	println!("{}", dual_graph_to_dot(&dual_graph.components, edges));

	let mut flags = HashMap::new();
	flag_faces(&nesting_trees, a_fill_rule, b_fill_rule, edges, vertices, &mut flags);

	let predicate = OPERATION_PREDICATES[op as usize];

	match op {
		PathBooleanOperation::Division | PathBooleanOperation::Fracture => Ok(dump_faces(&nesting_trees, predicate, edges, vertices, &flags)),
		_ => {
			let selected_faces: HashSet<DualVertexKey> = get_selected_faces(&predicate, &flags).collect();
			Ok(vec![walk_faces(&selected_faces, edges, vertices).collect()])
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use glam::DVec2; // Assuming DVec2 is defined in your crate

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
			(PathSegment::Arc(DVec2::new(39.0, 20.0), 19.0, 19.0, 0.0, false, true, DVec2::new(20.0, 39.0)), 1),
			(PathSegment::Arc(DVec2::new(20.0, 39.0), 19.0, 19.0, 0.0, false, true, DVec2::new(1.0, 20.0)), 1),
			(PathSegment::Arc(DVec2::new(1.0, 20.0), 19.0, 19.0, 0.0, false, true, DVec2::new(20.0, 1.0)), 1),
			(PathSegment::Arc(DVec2::new(20.0, 1.0), 19.0, 19.0, 0.0, false, true, DVec2::new(39.0, 20.0)), 1),
			(PathSegment::Line(DVec2::new(39.0, 20.0), DVec2::new(39.0, 20.0)), 1),
			(PathSegment::Arc(DVec2::new(47.0, 28.0), 19.0, 19.0, 0.0, false, true, DVec2::new(28.0, 47.0)), 2),
			(PathSegment::Arc(DVec2::new(28.0, 47.0), 19.0, 19.0, 0.0, false, true, DVec2::new(9.0, 28.0)), 2),
			(PathSegment::Arc(DVec2::new(9.0, 28.0), 19.0, 19.0, 0.0, false, true, DVec2::new(28.0, 9.0)), 2),
			(PathSegment::Arc(DVec2::new(28.0, 9.0), 19.0, 19.0, 0.0, false, true, DVec2::new(47.0, 28.0)), 2),
			(PathSegment::Line(DVec2::new(47.0, 28.0), DVec2::new(47.0, 28.0)), 2),
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
		//     println!("Minor Graph:");
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
		println!("  Vertices: {}", graph.vertices.len());
		println!("  Edges: {}", graph.edges.len());
		println!("  Cycles: {}", graph.cycles.len());

		for (vertex_key, vertex) in &graph.vertices {
			println!("    Vertex {:?}: {} outgoing edges", vertex_key, vertex.outgoing_edges.len());
		}

		for (edge_key, edge) in &graph.edges {
			println!("    Edge {:?}:", edge_key);
			println!("      Parent: {}", edge.parent);
			println!("      Twin: {:?}", edge.twin);
			println!("      Incident vertices: {:?}", edge.incident_vertices);
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
		println!("Initial Minor Graph:");
		print_minor_graph_state(&minor_graph);

		// Store initial edge order
		let initial_edge_order: HashMap<MinorVertexKey, Vec<MinorEdgeKey>> = minor_graph.vertices.iter().map(|(k, v)| (k, v.outgoing_edges.clone())).collect();

		// Apply sort_outgoing_edges_by_angle
		sort_outgoing_edges_by_angle(&mut minor_graph);

		// Print final state
		println!("\nAfter sort_outgoing_edges_by_angle:");
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
				assert!(angles[i] >= angles[i - 1], "Edges for vertex {:?} are not sorted by angle", vertex_key);
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
			(sample_path_segment_at(seg, 0.0), sample_path_segment_at(seg, EPS.param))
		} else {
			(sample_path_segment_at(seg, 1.0), sample_path_segment_at(seg, 1.0 - EPS.param))
		};
		(p1.y - p0.y).atan2(p1.x - p0.x)
	}

	#[test]
	fn test_path_segment_horizontal_ray_intersection_count() {
		let orig_seg = PathSegment::Arc(DVec2::new(24.0, 10.090978), 13.909023, 13.909023, 0.0, false, true, DVec2::new(47., 24.0));

		let point = DVec2::new(37.99, 24.0);

		println!("Starting test with segment: {:?}", orig_seg);
		println!("Test point: {:?}", point);

		let count = path_segment_horizontal_ray_intersection_count(&orig_seg, point);

		println!("Final intersection count: {}", count);

		let expected_count = 1;
		assert_eq!(count, expected_count, "Intersection count mismatch");
	}

	#[test]
	fn test_bounding_box_intersects_horizontal_ray() {
		let bbox = AaBb {
			top: 10.0,
			right: 40.0,
			bottom: 30.0,
			left: 20.0,
		};

		assert!(bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(0.0, 30.0)));
		assert!(bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(20.0, 30.0)));
		assert!(bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(10.0, 20.0)));
		assert!(!bounding_box_intersects_horizontal_ray(&bbox, DVec2::new(30.0, 40.0)));
	}
}
