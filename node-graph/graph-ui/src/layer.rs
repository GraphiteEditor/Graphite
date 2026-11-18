use crate::card::{Connector, Selection};
use crate::metrics::{
	CONNECTOR_SIZE, GRID_SIZE, INPUT_CONNECTOR_LEFT, LAYER_BUTTON_SIZE, LAYER_DETAILS_MARGIN, LAYER_GRIP_WIDTH, LAYER_HEIGHT, LAYER_LEFT_MARGIN, LAYER_NAME_LEFT, LAYER_THUMBNAIL_IMAGE_INSET,
	LAYER_THUMBNAIL_MARGIN, LAYER_THUMBNAIL_WIDTH, STACK_CONNECTOR_SIZE,
};
use crate::replicated_map::Keyed;
use core_types::uuid::NodeId;
use glam::{DVec2, IVec2};
use graphene_hash::CacheHash;

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct Layer {
	pub key: NodeId,
	pub cell: IVec2,
	pub name: String,
	pub renaming: bool,
	pub width: u32,
	pub chain_width: u32,
	pub has_left_input_wire: bool,
	pub primary_input: Option<Connector>,
	pub primary_output: Option<Connector>,
	pub secondary_input: Option<Connector>,
	pub primary_input_connected_to_layer: bool,
	pub primary_output_connected_to_layer: bool,
	pub visible: bool,
	pub locked: bool,
	pub selection: Selection,
	pub previewed: bool,
	pub hovered_button: Option<LayerButton>,
	pub hovered_button_clicked: bool,
	pub glide: DVec2,
}

#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
pub enum LayerButton {
	Visibility,
	Lock,
	Grip,
}

impl Keyed for Layer {
	type Key = NodeId;

	fn key(&self) -> NodeId {
		self.key
	}
}

impl Layer {
	fn top_left(&self) -> DVec2 {
		self.cell.as_dvec2() * GRID_SIZE + self.glide
	}

	pub fn body(&self) -> [DVec2; 2] {
		let top_left = self.top_left();
		let left = if self.chain_width > 0 { -(self.chain_width as f64) * GRID_SIZE } else { LAYER_LEFT_MARGIN };
		[top_left + DVec2::X * left, top_left + DVec2::new(self.width as f64 * GRID_SIZE, LAYER_HEIGHT)]
	}

	pub fn left_input_wire(&self) -> Option<DVec2> {
		let [min, _] = self.body();
		(self.has_left_input_wire && self.chain_width > 0).then(|| DVec2::new(min.x, min.y + LAYER_HEIGHT / 2.))
	}

	pub fn thumbnail_block(&self) -> [DVec2; 2] {
		let left = self.top_left() + DVec2::X * LAYER_LEFT_MARGIN;
		[left, left + DVec2::new(LAYER_THUMBNAIL_WIDTH, LAYER_HEIGHT)]
	}

	pub fn thumbnail(&self) -> [DVec2; 2] {
		let [min, max] = self.thumbnail_block();
		[min + DVec2::X * LAYER_THUMBNAIL_MARGIN, max - DVec2::X * LAYER_THUMBNAIL_MARGIN]
	}

	pub fn thumbnail_image(&self) -> [DVec2; 2] {
		let [min, max] = self.thumbnail();
		[min + LAYER_THUMBNAIL_IMAGE_INSET, max - LAYER_THUMBNAIL_IMAGE_INSET]
	}

	pub fn stack_output(&self) -> DVec2 {
		let [min, max] = self.thumbnail_block();
		DVec2::new((min.x + max.x - STACK_CONNECTOR_SIZE.x) / 2., min.y - STACK_CONNECTOR_SIZE.y)
	}

	pub fn stack_input(&self) -> DVec2 {
		let [min, max] = self.thumbnail_block();
		DVec2::new((min.x + max.x - STACK_CONNECTOR_SIZE.x) / 2., max.y)
	}

	pub fn secondary_input_connector(&self) -> DVec2 {
		self.top_left() + DVec2::new(INPUT_CONNECTOR_LEFT, (LAYER_HEIGHT - CONNECTOR_SIZE) / 2.)
	}

	pub fn name_box(&self) -> [DVec2; 2] {
		let top_left = self.top_left() + DVec2::new(LAYER_NAME_LEFT, (LAYER_HEIGHT - GRID_SIZE) / 2.);
		[top_left, top_left + DVec2::new(self.name_width(), GRID_SIZE)]
	}

	pub fn name_width(&self) -> f64 {
		self.width as f64 * GRID_SIZE - LAYER_NAME_LEFT - LAYER_DETAILS_MARGIN
	}

	pub fn visibility(&self) -> [DVec2; 2] {
		let [_, max] = self.body();
		let top_left = DVec2::new(max.x - LAYER_BUTTON_SIZE / 2., self.top_left().y + (LAYER_HEIGHT - LAYER_BUTTON_SIZE) / 2.);
		[top_left, top_left + LAYER_BUTTON_SIZE]
	}

	pub fn grip(&self) -> [DVec2; 2] {
		let [right_of_grip, _] = self.lock().unwrap_or(self.visibility());
		[right_of_grip - DVec2::X * LAYER_GRIP_WIDTH, right_of_grip + DVec2::Y * LAYER_BUTTON_SIZE]
	}

	pub fn lock(&self) -> Option<[DVec2; 2]> {
		let [min, max] = self.visibility();
		self.locked.then(|| [min - DVec2::X * LAYER_BUTTON_SIZE, max - DVec2::X * LAYER_BUTTON_SIZE])
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_rows::layer;

	#[test]
	fn a_layer_spans_its_width_from_half_a_cell_into_its_cell() {
		let layer = Layer {
			cell: IVec2::new(2, -1),
			has_left_input_wire: true,
			..layer(0)
		};
		assert_eq!(layer.body(), [DVec2::new(60., -24.), DVec2::new(240., 24.)]);
		assert_eq!(layer.thumbnail_block(), [DVec2::new(60., -24.), DVec2::new(132., 24.)]);
		assert_eq!(layer.thumbnail(), [DVec2::new(61., -24.), DVec2::new(131., 24.)], "a pixel in from each side of its block");
		assert_eq!(
			layer.thumbnail_image(),
			[DVec2::new(63., -22.), DVec2::new(129., 22.)],
			"two pixels in from the thumbnail, at its aspect ratio"
		);
		assert_eq!(
			layer.name_box(),
			[DVec2::new(140., -12.), DVec2::new(232., 12.)],
			"the name follows the thumbnail and ends a margin short of the right edge"
		);
		assert_eq!(layer.stack_output(), DVec2::new(92., -36.), "the stack connectors are centred above and below the thumbnail");
		assert_eq!(layer.stack_input(), DVec2::new(92., 24.));
		assert_eq!(layer.secondary_input_connector(), DVec2::new(45., -4.), "the input on the left side sits where a node's would");
		assert_eq!(
			layer.visibility(),
			[DVec2::new(228., -12.), DVec2::new(252., 12.)],
			"the visibility button is centred on the right edge"
		);
		assert_eq!(layer.left_input_wire(), None, "no chain for a wire to enter");
		let gliding = Layer {
			glide: DVec2::new(3., -4.),
			..layer.clone()
		};
		assert_eq!(
			gliding.visibility(),
			layer.visibility().map(|corner| corner + DVec2::new(3., -4.)),
			"a gliding layer is drawn its glide away"
		);
		assert_eq!(layer.lock(), None, "an unlocked layer has no lock button");
		let locked = Layer { locked: true, ..layer.clone() };
		assert_eq!(locked.lock(), Some([DVec2::new(204., -12.), DVec2::new(228., 12.)]), "the lock button is left of the visibility button");
		assert_eq!(layer.grip(), [DVec2::new(220., -12.), DVec2::new(228., 12.)], "the grip is left of the visibility button");
		assert_eq!(locked.grip(), [DVec2::new(196., -12.), DVec2::new(204., 12.)], "the grip is left of the lock button");

		let chained = Layer { chain_width: 8, ..layer };
		assert_eq!(chained.body(), [DVec2::new(-144., -24.), DVec2::new(240., 24.)], "the chain reaches left of the cell");
		assert_eq!(chained.thumbnail_block()[0], DVec2::new(60., -24.), "the thumbnail stays where it is");
		assert_eq!(chained.left_input_wire(), Some(DVec2::new(-144., 0.)));
	}
}
