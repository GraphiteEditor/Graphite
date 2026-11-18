use crate::card::{DataType, Highlight};
use crate::metrics::GRID_SIZE;
use crate::replicated_map::Keyed;
use core_types::uuid::NodeId;
use glam::{DVec2, IVec2};
use graphene_hash::CacheHash;

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct Dot {
	pub key: NodeId,
	pub cell: IVec2,
	pub data_type: DataType,
	pub wired: bool,
	pub selected: bool,
	pub previewed: bool,
	pub highlight: Highlight,
	pub output_highlight: Highlight,
	pub glide: DVec2,
}

impl Keyed for Dot {
	type Key = NodeId;

	fn key(&self) -> NodeId {
		self.key
	}
}

impl Dot {
	pub fn center(&self) -> DVec2 {
		(self.cell.as_dvec2() + DVec2::Y) * GRID_SIZE + self.glide
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_rows::dot;

	#[test]
	fn a_dot_sits_on_the_left_edge_of_its_cell_a_cell_down() {
		let dot = Dot { cell: IVec2::new(2, -1), ..dot(0) };
		assert_eq!(dot.center(), DVec2::new(48., 0.));
		let gliding = Dot { glide: DVec2::new(3., -4.), ..dot };
		assert_eq!(gliding.center(), DVec2::new(51., -4.), "a gliding dot is drawn its glide away");
	}
}
