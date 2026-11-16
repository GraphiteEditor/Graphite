//! The areas of a laid out graph that react to the pointer, as plain boxes in node graph pixels.

use crate::{CHAIN_WIDTH, Graph, LAYER_HEIGHT, Layout};
use glam::DVec2;

pub const GRID_SIZE: f64 = 24.;
const HALF_GRID_SIZE: f64 = GRID_SIZE / 2.;
const NODE_WIDTH: f64 = 5. * GRID_SIZE;
const SIDE_PORT_HALF_SIZE: DVec2 = DVec2::new(12., 12.);
const STACK_PORT_HALF_SIZE: DVec2 = DVec2::new(12., 8.);
const GRIP_WIDTH: f64 = 8.;
const NAME_LEFT_OFFSET: f64 = HALF_GRID_SIZE + 3. * GRID_SIZE + 8.;

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
	pub node: u32,
	pub part: Part,
}

impl ClickTarget {
	fn around(center: DVec2, half_size: DVec2, node: u32, part: Part) -> Self {
		Self {
			min: center - half_size,
			max: center + half_size,
			node,
			part,
		}
	}

	pub fn contains(&self, point: DVec2) -> bool {
		point.cmpge(self.min).all() && point.cmplt(self.max).all()
	}
}

/// Every click target of the graph, with the parts of a node after its body so they take precedence over it.
pub fn click_targets(graph: &Graph, layout: &Layout) -> Vec<ClickTarget> {
	let mut chain_lengths = vec![0_u32; graph.nodes.len()];
	for layer in layout.chain_layers.iter().flatten() {
		chain_lengths[*layer as usize] += 1;
	}

	let mut targets = Vec::with_capacity(graph.nodes.len() * 4);
	for (index, node) in graph.nodes.iter().enumerate() {
		let top_left = layout.positions[index].as_dvec2() * GRID_SIZE;
		let index = index as u32;

		if !node.is_layer {
			let body_top_left = top_left + DVec2::new(0., HALF_GRID_SIZE);
			let height = node.rows as f64 * GRID_SIZE;
			targets.push(ClickTarget {
				min: body_top_left,
				max: body_top_left + DVec2::new(NODE_WIDTH, height),
				node: index,
				part: Part::Body,
			});

			for input in 0..node.inputs {
				let center = top_left + DVec2::new(0., GRID_SIZE * (input as f64 + 1.));
				targets.push(ClickTarget::around(center, SIDE_PORT_HALF_SIZE, index, Part::Input(input)));
			}
			for output in 0..node.outputs {
				let center = top_left + DVec2::new(NODE_WIDTH, GRID_SIZE * (output as f64 + 1.));
				targets.push(ClickTarget::around(center, SIDE_PORT_HALF_SIZE, index, Part::Output(output)));
			}
			continue;
		}

		let width = node.width as f64 * GRID_SIZE;
		let height = LAYER_HEIGHT as f64 * GRID_SIZE;
		let chain_width = if node.inputs > 1 {
			(chain_lengths[index as usize] as i32 * CHAIN_WIDTH + 1) as f64 * GRID_SIZE
		} else {
			0.
		};
		targets.push(ClickTarget {
			min: top_left - DVec2::new(chain_width, 0.),
			max: top_left + DVec2::new(width, height),
			node: index,
			part: Part::Body,
		});

		targets.push(ClickTarget::around(top_left + DVec2::new(2. * GRID_SIZE, height + 8.), STACK_PORT_HALF_SIZE, index, Part::Input(0)));
		if node.inputs > 1 {
			targets.push(ClickTarget::around(top_left + DVec2::new(0., GRID_SIZE), SIDE_PORT_HALF_SIZE, index, Part::Input(1)));
		}
		targets.push(ClickTarget::around(top_left + DVec2::new(2. * GRID_SIZE, -8.), STACK_PORT_HALF_SIZE, index, Part::Output(0)));

		let icon_half_size = DVec2::splat(HALF_GRID_SIZE);
		let icons_width = if node.locked { GRID_SIZE } else { 0. };
		targets.push(ClickTarget::around(top_left + DVec2::new(width, GRID_SIZE), icon_half_size, index, Part::Visibility));
		if node.locked {
			targets.push(ClickTarget::around(top_left + DVec2::new(width - GRID_SIZE, GRID_SIZE), icon_half_size, index, Part::Lock));
		}

		let grip_right = top_left.x + width - HALF_GRID_SIZE - icons_width;
		targets.push(ClickTarget {
			min: DVec2::new(grip_right - GRIP_WIDTH, top_left.y + HALF_GRID_SIZE),
			max: DVec2::new(grip_right, top_left.y + GRID_SIZE + HALF_GRID_SIZE),
			node: index,
			part: Part::Grip,
		});

		let name_left = top_left.x + NAME_LEFT_OFFSET;
		let name_right = (name_left + node.name_width).min(top_left.x + width - HALF_GRID_SIZE - icons_width - GRIP_WIDTH);
		if name_right > name_left {
			targets.push(ClickTarget {
				min: DVec2::new(name_left, top_left.y + HALF_GRID_SIZE),
				max: DVec2::new(name_right, top_left.y + GRID_SIZE + HALF_GRID_SIZE),
				node: index,
				part: Part::Name,
			});
		}
	}

	targets
}

/// The last added target containing `point`, which is a node's part before its body and a later node before an earlier one.
pub fn target_at(targets: &[ClickTarget], point: DVec2) -> Option<&ClickTarget> {
	targets.iter().rev().find(|target| target.contains(point))
}
