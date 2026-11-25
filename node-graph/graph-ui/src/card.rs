use crate::metrics::{CONNECTOR_SIZE, GRID_SIZE, INPUT_CONNECTOR_LEFT, NODE_WIDTH, OUTPUT_CONNECTOR_RIGHT};
use crate::replicated_map::Keyed;
use core_types::uuid::NodeId;
use glam::{DVec2, IVec2};
use graphene_hash::CacheHash;

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct Card {
	pub key: NodeId,
	pub cell: IVec2,
	pub name: String,
	pub icon: String,
	pub primary_input: Option<Connector>,
	pub primary_output: Option<Connector>,
	pub secondary_inputs: Vec<Connector>,
	pub secondary_outputs: Vec<Connector>,
	pub selection: Selection,
	pub visible: bool,
	pub previewed: bool,
	pub glide: DVec2,
}

#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub enum Selection {
	None,
	Inactive,
	Active,
}

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct Connector {
	pub name: String,
	pub data_type: DataType,
	pub connected: bool,
	pub highlight: Highlight,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, CacheHash)]
pub enum Highlight {
	#[default]
	None,
	Hovered,
	WireSource,
	WireTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub enum DataType {
	General,
	Number,
	Artboard,
	Graphic,
	Raster,
	Vector,
	Color,
	Gradient,
	Typography,
	Invalid,
}

impl Keyed for Card {
	type Key = NodeId;

	fn key(&self) -> NodeId {
		self.key
	}
}

impl Card {
	pub fn rows(&self) -> usize {
		1 + self.secondary_inputs.len().max(self.secondary_outputs.len())
	}

	pub fn body(&self) -> [DVec2; 2] {
		let top_left = (self.cell.as_dvec2() + DVec2::new(0., 0.5)) * GRID_SIZE + self.glide;
		[top_left, top_left + DVec2::new(NODE_WIDTH, self.rows() as f64 * GRID_SIZE)]
	}

	pub fn inputs(&self) -> impl Iterator<Item = (DVec2, &Connector)> {
		self.connectors(&self.primary_input, &self.secondary_inputs, INPUT_CONNECTOR_LEFT)
	}

	pub fn outputs(&self) -> impl Iterator<Item = (DVec2, &Connector)> {
		self.connectors(&self.primary_output, &self.secondary_outputs, NODE_WIDTH + OUTPUT_CONNECTOR_RIGHT - CONNECTOR_SIZE)
	}

	fn connectors<'a>(&self, primary: &'a Option<Connector>, secondary: &'a [Connector], left: f64) -> impl Iterator<Item = (DVec2, &'a Connector)> {
		let [top_left, _] = self.body();
		let rows = primary
			.iter()
			.map(|connector| (0, connector))
			.chain(secondary.iter().enumerate().map(|(index, connector)| (index + 1, connector)));
		rows.map(move |(row, connector)| (top_left + DVec2::new(left, row as f64 * GRID_SIZE + (GRID_SIZE - CONNECTOR_SIZE) / 2.), connector))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::{card, connector};

	#[test]
	fn a_node_body_is_five_cells_wide_and_starts_half_a_cell_below_its_cell() {
		let mut card = Card {
			cell: IVec2::new(2, -1),
			primary_output: Some(connector("Shape")),
			..card(0)
		};
		assert_eq!(card.body(), [DVec2::new(48., -12.), DVec2::new(168., 12.)]);

		card.secondary_inputs = vec![connector("Width"), connector("Height")];
		card.secondary_outputs = vec![connector("Area")];
		assert_eq!(card.body(), [DVec2::new(48., -12.), DVec2::new(168., 60.)], "a row for each secondary input");

		let top_left = |(top_left, _): (DVec2, &Connector)| top_left;
		assert_eq!(card.inputs().map(top_left).collect::<Vec<_>>(), [DVec2::new(45., 20.), DVec2::new(45., 44.)], "no primary input");
		assert_eq!(card.outputs().map(top_left).collect::<Vec<_>>(), [DVec2::new(165., -4.), DVec2::new(165., 20.)]);
	}

	#[test]
	fn a_gliding_card_is_drawn_its_glide_away() {
		let glide = DVec2::new(3., -4.);
		let gliding = Card { glide, ..card(0) };
		assert_eq!(gliding.body(), card(0).body().map(|corner| corner + glide));
	}
}
