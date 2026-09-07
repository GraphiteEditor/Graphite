use core::fmt;
use std::{
	array,
	cmp::Ordering,
	collections::{HashMap, VecDeque},
};

use glam::{DAffine2, DVec2};
use vector_types::{
	GradientInterpolation, GradientSpace, MeshGradient,
	gradient::MeshGradientEvaluator,
	mesh_gradient::{BicubicBezierNet, MeshGradientEvaluatorError, evaluate_cubic_bezier_bernstein},
};

use crate::mesh_gradient::tessellate::MeshGradientTessellatorError::Evaluator;

/// Maximum allowed geometry approximation error in viewport pixels.
const MESH_POSITION_DEVIATION_TOLERANCE: PositionDeviationBound = PositionDeviationBound(2.);

const MAX_SUBDIVISION_DEPTH: u32 = 31;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct InterpolationSetting {
	/// Color space to interpolate in: 0 => gamma sRGB, 1 => linear sRGB, 2 => OKLab, 3 => Lab
	pub space: u32,
	/// Interpolation method: 0 => Stepped, 1 => Linear, 2 => Smooth
	pub method: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PatchData {
	pub colors: [[f32; 4]; 4],
	pub color_u_derivatives: [[f32; 4]; 4],
	pub color_v_derivatives: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct MeshVertex {
	pub patch_index: u32,
	pub uv: [f32; 2],
	pub position: [f32; 2],
}

pub(super) struct MeshGradientTessellator {
	evaluator: MeshGradientEvaluator,
	mesh_to_output: DAffine2,
}

#[derive(Debug)]
pub(super) enum MeshGradientTessellatorError {
	Evaluator(MeshGradientEvaluatorError),
	Subpatches,
}

impl From<MeshGradientEvaluatorError> for MeshGradientTessellatorError {
	fn from(evaluator_error: MeshGradientEvaluatorError) -> Self {
		Evaluator(evaluator_error)
	}
}

impl MeshGradientTessellator {
	pub(super) fn try_new(
		mesh_gradient: &MeshGradient,
		color_space: GradientSpace,
		interpolation_method: GradientInterpolation,
		mesh_to_output: DAffine2,
	) -> Result<Self, MeshGradientTessellatorError> {
		let evaluator = mesh_gradient.evaluator(color_space, interpolation_method)?;
		Ok(Self { evaluator, mesh_to_output })
	}

	pub(super) fn tessellate(&self) -> Result<(Vec<MeshVertex>, Vec<u32>), MeshGradientTessellatorError> {
		const RECT_TO_TRIANGLE_INDICES: [u32; 6] = [0, 1, 3, 1, 3, 2];

		#[derive(Copy, Clone)]
		struct BoundaryVertex {
			uv: DVec2,
			position: DVec2,
		}

		let subpatches = self.subdivide_mesh_adaptive()?;

		let mut vertices = vec![];
		let mut indices = vec![];
		for subpatch in subpatches {
			let patch_index = subpatch.patch_index as u32;
			let base_vertex_index = vertices.len() as u32;
			let [uv_min, uv_max] = subpatch.uv_bounds;

			// [top left, top right, bottom right, bottom left]
			let corner_uvs_clockwise = [uv_min, DVec2::new(uv_max.x, uv_min.y), uv_max, DVec2::new(uv_min.x, uv_max.y)];
			let corner_vertices: [BoundaryVertex; 4] = array::from_fn(|index| BoundaryVertex {
				uv: corner_uvs_clockwise[index],
				position: subpatch.position_bezier_net.corners_clockwise()[index],
			});

			let corner_indices = |edge_index: usize| [edge_index, (edge_index + 1) % 4];
			let junction_vertex = |edge_index: usize| {
				let [corner1_index, corner2_index] = corner_indices(edge_index);
				let uv = (corner_uvs_clockwise[corner1_index] + corner_uvs_clockwise[corner2_index]) / 2.;
				let position = evaluate_cubic_bezier_bernstein(&bezier_edge(&subpatch.position_bezier_net, edge_index), 0.5);
				BoundaryVertex { uv, position }
			};

			// When the current subpatch has at least one junction, tessellate it by a triangle fan using the first junction as a pivot.
			if let Some(pivot_edge_index) = subpatch.junctions.iter().enumerate().find(|(_, has_junction)| **has_junction).map(|(edge_index, _)| edge_index) {
				let boundary_vertices = ALL_EDGES
					.iter()
					.flat_map(|edge| {
						let edge_index = *edge as usize;
						[Some(corner_vertices[edge_index]), subpatch.junctions[edge_index].then(|| junction_vertex(edge_index))]
					})
					.flatten()
					.collect::<Vec<_>>();
				let vertex_count = boundary_vertices.len();

				// Rotate to make the first vertex becomes the pivot
				boundary_vertices.iter().cycle().skip(pivot_edge_index + 1).take(vertex_count).for_each(|vertex| {
					vertices.push(MeshVertex {
						patch_index,
						uv: vertex.uv.as_vec2().to_array(),
						position: vertex.position.as_vec2().to_array(),
					});
				});
				let local_vertex_indices = (1..vertex_count).collect::<Vec<_>>();
				indices.extend(
					local_vertex_indices
						.windows(2)
						.flat_map(|pair| [base_vertex_index, base_vertex_index + pair[0] as u32, base_vertex_index + pair[1] as u32]),
				);
			} else {
				vertices.extend(subpatch.position_bezier_net.corners_clockwise().iter().zip(corner_uvs_clockwise.iter()).map(|(pos, uv)| MeshVertex {
					patch_index: subpatch.patch_index as u32,
					uv: uv.as_vec2().to_array(),
					position: pos.as_vec2().to_array(),
				}));
				indices.extend(RECT_TO_TRIANGLE_INDICES.iter().map(|&corner_index| corner_index + base_vertex_index));
			}
		}

		Ok((vertices, indices))
	}

	fn subdivide_mesh_adaptive(&self) -> Result<Vec<Subpatch>, MeshGradientTessellatorError> {
		let mut state = self.initialize_adaptive_subdivision()?;

		while let Some(key) = state.next_refinement_key() {
			self.refine_subpatch(&mut state, key);
		}

		self.mark_t_junctions(&mut state);

		Ok(state.into_leaf_subpatches())
	}

	fn initialize_adaptive_subdivision(&self) -> Result<AdaptiveSubdivisionState, MeshGradientTessellatorError> {
		let mut state = AdaptiveSubdivisionState::default();

		for patch in self.evaluator.patch_evaluators() {
			let root = Subpatch {
				patch_index: patch.index(),
				position_bezier_net: patch.position_bezier_net(),
				uv_bounds: [DVec2::new(0., 0.), DVec2::new(1., 1.)],
				is_subdivided: false,
				balance_queued: false,
				junctions: [false; 4],
			};
			let deviation = self
				.subpatch_tessellation_error_bound_px(&root.position_bezier_net)
				.map_err(|_| MeshGradientTessellatorError::Subpatches)?;
			let patch_root_key = (root.patch_index, MortonCode::root());
			if deviation > MESH_POSITION_DEVIATION_TOLERANCE {
				state.deviation_queue.push_back((deviation, patch_root_key));
			};
			state.subpatches.insert(patch_root_key, root);
		}

		Ok(state)
	}

	fn refine_subpatch(&self, state: &mut AdaptiveSubdivisionState, key: SubpatchKey) {
		let morton_code = key.1;
		if morton_code.depth() == MAX_SUBDIVISION_DEPTH {
			return;
		}

		let (patch_index, subdivided_uv_bounds, subdivided_nets) = {
			let Some(target) = state.subpatches.get_mut(&key) else { return };
			if target.is_subdivided {
				return;
			};
			target.is_subdivided = true;
			let patch_index = key.0;
			let subdivided_uv_bounds = subdivide_uv_bounds(target.uv_bounds);
			let subdivided_nets = target.position_bezier_net.subdivide();
			(patch_index, subdivided_uv_bounds, subdivided_nets)
		};

		for quadrant in ALL_QUADRANTS {
			let i = quadrant as usize;
			let child_morton_code = morton_code.child(quadrant);
			let Ok(child_deviation) = self.subpatch_tessellation_error_bound_px(&subdivided_nets[i]) else {
				continue;
			};
			let child_subpatch = Subpatch {
				patch_index,
				position_bezier_net: subdivided_nets[i],
				uv_bounds: subdivided_uv_bounds[i],
				is_subdivided: false,
				balance_queued: false,
				junctions: [false; 4],
			};
			let child_key = (patch_index, child_morton_code);
			state.subpatches.insert(child_key, child_subpatch);

			if child_deviation > MESH_POSITION_DEVIATION_TOLERANCE {
				state.deviation_queue.push_back((child_deviation, child_key));
			}
		}

		self.enqueue_balance_refinements(state, patch_index, morton_code);
	}

	fn enqueue_balance_refinements(&self, state: &mut AdaptiveSubdivisionState, patch_index: usize, morton_code: MortonCode) {
		// Target neighbors of the parent cell of the [NW, NE, SW, SE] quadrant.
		const BALANCING_EDGES_BY_QUADRANT: [[Edge; 2]; 4] = [[Edge::Top, Edge::Left], [Edge::Top, Edge::Right], [Edge::Bottom, Edge::Left], [Edge::Bottom, Edge::Right]];

		let quadrant = morton_code.quadrant();
		let Some(parent_morton_code) = morton_code.try_parent() else { return };

		for edge in BALANCING_EDGES_BY_QUADRANT[quadrant as usize] {
			let Some(parent_neighbor_key) = self.neighbor_key(patch_index, parent_morton_code, edge) else {
				continue;
			};
			let Some(parent_neighbor) = state.subpatches.get_mut(&parent_neighbor_key) else { continue };
			if parent_neighbor.is_subdivided || parent_neighbor.balance_queued {
				continue;
			};
			parent_neighbor.balance_queued = true;
			state.balance_queue.push_back(parent_neighbor_key);
		}
	}

	fn mark_t_junctions(&self, state: &mut AdaptiveSubdivisionState) {
		// Collect if a subpatch has neighboring subdivided subpatches that create T-junctions.
		// These are necessary to be vertices after the tessellation process.
		let junctions = state
			.subpatches
			.iter()
			.filter(|(_, subpatch)| !subpatch.is_subdivided)
			.map(|(key, _)| {
				let (patch_index, morton_code) = *key;
				let current_junctions = ALL_EDGES.map(|edge| {
					let Some(neighbor_key) = self.neighbor_key(patch_index, morton_code, edge) else { return false };
					let neighbor = state.subpatches.get(&neighbor_key);
					neighbor.is_some_and(|n| n.is_subdivided)
				});

				(*key, current_junctions)
			})
			.collect::<Vec<_>>();

		junctions.into_iter().for_each(|(key, junctions)| {
			let Some(subpatch) = state.subpatches.get_mut(&key) else { return };
			subpatch.junctions = junctions;
		});
	}

	fn neighbor_key(&self, patch_index: usize, morton_code: MortonCode, edge: Edge) -> Option<SubpatchKey> {
		let (patch_rows, patch_columns) = self.evaluator.patch_dimension();
		let patch_col = patch_index as i64 % patch_columns;
		let patch_row = patch_index as i64 / patch_columns;

		let [cell_x, cell_y] = morton_code.coordinates();
		let [dx, dy] = match edge {
			Edge::Top => [0, -1],
			Edge::Bottom => [0, 1],
			Edge::Left => [-1, 0],
			Edge::Right => [1, 0],
		};
		let neighbor_cell_x = cell_x as i64 + dx;
		let neighbor_cell_y = cell_y as i64 + dy;

		let depth = morton_code.depth();
		let cells_per_axis = 2_i64.pow(depth);

		// The parent's neighbor is possibly not within the same quadtree
		let neighbor_patch_x = patch_col + neighbor_cell_x.div_euclid(cells_per_axis);
		let neighbor_patch_y = patch_row + neighbor_cell_y.div_euclid(cells_per_axis);
		if neighbor_patch_x < 0 || neighbor_patch_x >= patch_columns || neighbor_patch_y < 0 || neighbor_patch_y >= patch_rows {
			return None;
		}
		let neighbor_patch_index = neighbor_patch_x + neighbor_patch_y * patch_columns;
		let neighbor_subpatch_x = neighbor_cell_x.rem_euclid(cells_per_axis) as u64;
		let neighbor_subpatch_y = neighbor_cell_y.rem_euclid(cells_per_axis) as u64;
		let neighbor_morton_code = MortonCode::from_coordinates(depth, neighbor_subpatch_x, neighbor_subpatch_y);

		Some((neighbor_patch_index as usize, neighbor_morton_code))
	}

	/// Measures how far the rendered approximation of one subpatch goes from the target.
	fn subpatch_tessellation_error_bound_px(&self, subpatch_net: &BicubicBezierNet<DVec2>) -> Result<PositionDeviationBound, NotFiniteError> {
		// Compute an upper bound on the shape error between the target subpatch and the bilinear patch defined by its four corners, using the convex hull property of Bézier surfaces.
		let approximated = BicubicBezierNet::from_quadrilateral(&subpatch_net.corners());
		let differences = *subpatch_net - approximated;
		let shape_error_px = differences
			.iter()
			.flatten()
			.map(|diff| (self.mesh_to_output.matrix2 * diff).length())
			.max_by(|a, b| a.total_cmp(b))
			.expect("DIfferences length must be greater than 0");

		let [top_left, top_right, bottom_left, bottom_right] = subpatch_net.corners();
		// Calculate the maximum error between the bilinear quadrilateral mapping and the piecewise-linear mapping produced by barycentric interpolation over two triangles.
		// Their difference is `u * v * D` in one triangle and `(1 - u) * (1 - v) * D` in the other, where `D` is the bilinear mixed difference.
		// Both scalar factors have a maximum of 1/4 at the midpoint of the shared diagonal.
		// https://gpuopen.com/learn/bilinear-interpolation-quadrilateral-barycentric-coordinates/
		// TODO: possibly better to do the reverse calculation in the fragment shader
		let bilerp_error_px = (self.mesh_to_output.matrix2 * (top_left - top_right - bottom_left + bottom_right)).length() / 4.;

		PositionDeviationBound::new(shape_error_px + bilerp_error_px)
	}
}

#[repr(u8)]
#[derive(Copy, Clone)]
enum Quadrant {
	NW,
	NE,
	SW,
	SE,
}

const ALL_QUADRANTS: [Quadrant; 4] = [Quadrant::NW, Quadrant::NE, Quadrant::SW, Quadrant::SE];

#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
struct MortonCode(u64);

impl MortonCode {
	fn root() -> Self {
		// Use a leading 1 bit as a sentinel to preserve depth information in the Morton code.
		const MORTON_SENTINEL: u64 = 1;
		Self(MORTON_SENTINEL)
	}

	fn from_coordinates(depth: u32, x: u64, y: u64) -> Self {
		Self((1 << (depth * 2)) | expand_morton_bits(x) | expand_morton_bits(y) << 1)
	}

	fn depth(&self) -> u32 {
		self.0.ilog2() / 2
	}

	fn quadrant(&self) -> Quadrant {
		let index = self.0 & 0b11;
		match index {
			0 => Quadrant::NW,
			1 => Quadrant::NE,
			2 => Quadrant::SW,
			3 => Quadrant::SE,
			_ => unreachable!("Morton code cannot point outside of the quadrant"),
		}
	}

	fn child(&self, quadrant: Quadrant) -> Self {
		Self(self.0 << 2 | quadrant as u64)
	}

	fn try_parent(&self) -> Option<Self> {
		let parent_code = self.0 >> 2;
		if parent_code == 0 {
			return None;
		}
		Some(Self(parent_code))
	}

	fn coordinates(&self) -> [u64; 2] {
		let code = self.0;
		let without_sentinel = (1 << code.ilog2()) ^ code;
		let x = compact_morton_bits(without_sentinel & MORTON_COORDINATE_MASK);
		let y = compact_morton_bits((without_sentinel >> 1) & MORTON_COORDINATE_MASK);
		[x, y]
	}
}

const MORTON_COORDINATE_MASK: u64 = 0x5555_5555_5555_5555;

// Index subpatches by the pair of patch index and Morton code for expected O(1) lookup of given ancestors or neighbors, without storing parent pointers.
type SubpatchKey = (usize, MortonCode);

#[derive(Clone)]
struct Subpatch {
	patch_index: usize,
	position_bezier_net: BicubicBezierNet<DVec2>,
	uv_bounds: [DVec2; 2],
	is_subdivided: bool,
	balance_queued: bool,
	/// Stores if edges have T-junction. Use `Edge` to access.
	junctions: [bool; 4],
}

#[derive(Default)]
struct AdaptiveSubdivisionState {
	subpatches: HashMap<SubpatchKey, Subpatch>,
	/// Queue for deviation-based adaptive subdivision.
	deviation_queue: VecDeque<(PositionDeviationBound, SubpatchKey)>,
	/// Queue for balancing the quadtree.
	balance_queue: VecDeque<SubpatchKey>,
}

impl AdaptiveSubdivisionState {
	fn next_refinement_key(&mut self) -> Option<SubpatchKey> {
		self.balance_queue.pop_front().or_else(|| self.deviation_queue.pop_front().map(|(_, key)| key))
	}

	fn into_leaf_subpatches(self) -> Vec<Subpatch> {
		self.subpatches.into_values().filter(|subpatch| !subpatch.is_subdivided).collect()
	}
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq)]
enum Edge {
	Top,
	Right,
	Bottom,
	Left,
}

const ALL_EDGES: [Edge; 4] = [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left];

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PositionDeviationBound(f64);

impl PositionDeviationBound {
	pub fn new(value: f64) -> Result<Self, NotFiniteError> {
		if value.is_finite() { Ok(Self(value)) } else { Err(NotFiniteError(value)) }
	}

	pub const fn get(self) -> f64 {
		self.0
	}
}

impl Eq for PositionDeviationBound {}

impl PartialOrd for PositionDeviationBound {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for PositionDeviationBound {
	fn cmp(&self, other: &Self) -> Ordering {
		self.0.partial_cmp(&other.0).expect("Finite does not have NaN")
	}
}

impl fmt::Display for PositionDeviationBound {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl TryFrom<f64> for PositionDeviationBound {
	type Error = NotFiniteError;

	fn try_from(value: f64) -> Result<Self, Self::Error> {
		Self::new(value)
	}
}

impl From<PositionDeviationBound> for f64 {
	fn from(value: PositionDeviationBound) -> Self {
		value.0
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotFiniteError(pub f64);

impl fmt::Display for NotFiniteError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Not finite value: {}", self.0)
	}
}

impl std::error::Error for NotFiniteError {}

fn compact_morton_bits(n: u64) -> u64 {
	let n1 = (n >> 1 | n) & 0x3333_3333_3333_3333;
	let n2 = (n1 >> 2 | n1) & 0x0f0f_0f0f_0f0f_0f0f;
	let n3 = (n2 >> 4 | n2) & 0x00ff_00ff_00ff_00ff;
	let n4 = (n3 >> 8 | n3) & 0x0000_ffff_0000_ffff;
	(n4 >> 16 | n4) & 0x0000_0000_ffff_ffff
}

fn expand_morton_bits(n: u64) -> u64 {
	let n1 = (n << 16 | n) & 0x0000_ffff_0000_ffff;
	let n2 = (n1 << 8 | n1) & 0x00ff_00ff_00ff_00ff;
	let n3 = (n2 << 4 | n2) & 0x0f0f_0f0f_0f0f_0f0f;
	let n4 = (n3 << 2 | n3) & 0x3333_3333_3333_3333;
	(n4 << 1 | n4) & 0x5555_5555_5555_5555
}

fn subdivide_uv_bounds(uv_bounds: [DVec2; 2]) -> [[DVec2; 2]; 4] {
	let [uv_min, uv_max] = uv_bounds;
	let uv_mid = (uv_min + uv_max) / 2.;
	[
		[uv_min, uv_mid],
		[DVec2::new(uv_mid.x, uv_min.y), DVec2::new(uv_max.x, uv_mid.y)],
		[DVec2::new(uv_min.x, uv_mid.y), DVec2::new(uv_mid.x, uv_max.y)],
		[uv_mid, uv_max],
	]
}

fn bezier_edge(net: &BicubicBezierNet<DVec2>, edge_index: usize) -> [DVec2; 4] {
	match edge_index {
		0 => [net[0][0], net[0][1], net[0][2], net[0][3]],
		1 => [net[0][3], net[1][3], net[2][3], net[3][3]],
		2 => [net[3][3], net[3][2], net[3][1], net[3][0]],
		3 => [net[3][0], net[2][0], net[1][0], net[0][0]],
		_ => unreachable!(),
	}
}
