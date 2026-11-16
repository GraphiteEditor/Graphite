//! The areas of a laid out graph that react to the pointer, in node graph pixels. A hit test scans each node's reach, a box around
//! everything the node draws, and builds the exact parts of only the nodes it lands in.

use crate::{CHAIN_WIDTH, Graph, LAYER_HEIGHT, Layout};
use glam::DVec2;

pub const GRID_SIZE: f64 = 24.;
const HALF_GRID_SIZE: f64 = GRID_SIZE / 2.;
const NODE_WIDTH: f64 = 5. * GRID_SIZE;
const SIDE_PORT_HALF_SIZE: DVec2 = DVec2::new(12., 12.);
const STACK_PORT_HALF_SIZE: DVec2 = DVec2::new(12., 8.);
const GRIP_WIDTH: f64 = 8.;
const NAME_LEFT_OFFSET: f64 = HALF_GRID_SIZE + 3. * GRID_SIZE + 8.;
/// How far any part of a node reaches past its body, which is a stack port's offset plus its half height.
const REACH_MARGIN: f64 = 16.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
	Body,
	Input(u32),
	Output(u32),
	Visibility,
	Lock,
	Grip,
	Name,
}

#[derive(Clone, Copy, Debug)]
pub struct ClickTarget {
	pub min: DVec2,
	pub max: DVec2,
	pub corner_radius: f64,
	pub node: u32,
	pub part: Part,
}

impl ClickTarget {
	fn around(center: DVec2, half_size: DVec2, corner_radius: f64, node: u32, part: Part) -> Self {
		Self {
			min: center - half_size,
			max: center + half_size,
			corner_radius,
			node,
			part,
		}
	}

	/// Whether `point` is inside the rounded rectangle.
	pub fn contains(&self, point: DVec2) -> bool {
		if !point.cmpge(self.min).all() || !point.cmplt(self.max).all() {
			return false;
		}

		let radius = self.corner_radius.min((self.max - self.min).min_element() / 2.);
		let inner = point.clamp(self.min + radius, self.max - radius);
		point.distance_squared(inner) <= radius * radius
	}
}

/// A box around everything `node` draws, its body, ports, handles, and a layer's chain span.
pub fn reach(graph: &Graph, layout: &Layout, node: usize) -> [DVec2; 2] {
	let top_left = layout.positions[node].as_dvec2() * GRID_SIZE;
	let data = &graph.nodes[node];

	let (chain_width, size) = if data.is_layer {
		(chain_width(graph, layout, node), DVec2::new(data.width as f64, LAYER_HEIGHT as f64) * GRID_SIZE)
	} else {
		(0., DVec2::new(NODE_WIDTH, (data.rows.max(data.inputs).max(data.outputs) as f64 + 1.) * GRID_SIZE))
	};

	[top_left - DVec2::new(chain_width, 0.) - REACH_MARGIN, top_left + size + REACH_MARGIN]
}

fn chain_width(graph: &Graph, layout: &Layout, node: usize) -> f64 {
	if graph.nodes[node].inputs > 1 {
		(layout.chain_lengths[node] as i32 * CHAIN_WIDTH + 1) as f64 * GRID_SIZE
	} else {
		0.
	}
}

/// Calls `visit` with each part of `node`, its body last.
pub fn node_parts(graph: &Graph, layout: &Layout, node: usize, mut visit: impl FnMut(ClickTarget)) {
	let top_left = layout.positions[node].as_dvec2() * GRID_SIZE;
	let data = &graph.nodes[node];
	let index = node as u32;

	if !data.is_layer {
		for input in 0..data.inputs {
			let center = top_left + DVec2::new(0., GRID_SIZE * (input as f64 + 1.));
			visit(ClickTarget::around(center, SIDE_PORT_HALF_SIZE, 4., index, Part::Input(input)));
		}
		for output in 0..data.outputs {
			let center = top_left + DVec2::new(NODE_WIDTH, GRID_SIZE * (output as f64 + 1.));
			visit(ClickTarget::around(center, SIDE_PORT_HALF_SIZE, 4., index, Part::Output(output)));
		}

		let body_top_left = top_left + DVec2::new(0., HALF_GRID_SIZE);
		visit(ClickTarget {
			min: body_top_left,
			max: body_top_left + DVec2::new(NODE_WIDTH, data.rows as f64 * GRID_SIZE),
			corner_radius: 3.,
			node: index,
			part: Part::Body,
		});
		return;
	}

	let width = data.width as f64 * GRID_SIZE;
	let height = LAYER_HEIGHT as f64 * GRID_SIZE;

	visit(ClickTarget::around(top_left + DVec2::new(2. * GRID_SIZE, height + 8.), STACK_PORT_HALF_SIZE, 4., index, Part::Input(0)));
	if data.inputs > 1 {
		visit(ClickTarget::around(top_left + DVec2::new(0., GRID_SIZE), SIDE_PORT_HALF_SIZE, 4., index, Part::Input(1)));
	}
	visit(ClickTarget::around(top_left + DVec2::new(2. * GRID_SIZE, -8.), STACK_PORT_HALF_SIZE, 4., index, Part::Output(0)));

	let icon_half_size = DVec2::splat(HALF_GRID_SIZE);
	let icons_width = if data.locked { GRID_SIZE } else { 0. };
	visit(ClickTarget::around(top_left + DVec2::new(width, GRID_SIZE), icon_half_size, 3., index, Part::Visibility));
	if data.locked {
		visit(ClickTarget::around(top_left + DVec2::new(width - GRID_SIZE, GRID_SIZE), icon_half_size, 3., index, Part::Lock));
	}

	let grip_right = top_left.x + width - HALF_GRID_SIZE - icons_width;
	visit(ClickTarget {
		min: DVec2::new(grip_right - GRIP_WIDTH, top_left.y + HALF_GRID_SIZE),
		max: DVec2::new(grip_right, top_left.y + GRID_SIZE + HALF_GRID_SIZE),
		corner_radius: 0.,
		node: index,
		part: Part::Grip,
	});

	let name_left = top_left.x + NAME_LEFT_OFFSET;
	let name_right = (name_left + data.name_width).min(top_left.x + width - HALF_GRID_SIZE - icons_width - GRIP_WIDTH);
	if name_right > name_left {
		visit(ClickTarget {
			min: DVec2::new(name_left, top_left.y + HALF_GRID_SIZE),
			max: DVec2::new(name_right, top_left.y + GRID_SIZE + HALF_GRID_SIZE),
			corner_radius: 3.,
			node: index,
			part: Part::Name,
		});
	}

	visit(ClickTarget {
		min: top_left - DVec2::new(chain_width(graph, layout, node), 0.),
		max: top_left + DVec2::new(width, height),
		corner_radius: 10.,
		node: index,
		part: Part::Body,
	});
}

/// The part under `point` of the topmost node with one there, where a node's ports and handles take precedence over its body.
pub fn target_at(graph: &Graph, layout: &Layout, point: DVec2) -> Option<ClickTarget> {
	for node in (0..graph.nodes.len()).rev() {
		let [min, max] = reach(graph, layout, node);
		if !point.cmpge(min).all() || !point.cmplt(max).all() {
			continue;
		}

		let mut hit = None;
		node_parts(graph, layout, node, |target| {
			if hit.is_none() && target.contains(point) {
				hit = Some(target);
			}
		});
		if hit.is_some() {
			return hit;
		}
	}

	None
}
