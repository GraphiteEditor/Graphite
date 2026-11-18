use crate::card::{Connector, Highlight};
use crate::metrics::{BORDER_WIDTH, EXPORT_NAME_INSET, GRID_SIZE, IMPORT_EXPORT_CONNECTOR, IMPORT_EXPORT_NAME_RAISE, IMPORT_EXPORT_SEPARATOR_OVERHANG, IMPORT_NAME_INSET, REORDER_BAR_SIZE};
use crate::replicated_map::Keyed;
use glam::{DVec2, IVec2};
use graphene_hash::CacheHash;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, CacheHash)]
pub enum NetworkConnectorKind {
	Import,
	Export,
}

#[derive(Clone, Debug, PartialEq, CacheHash)]
pub struct NetworkConnector {
	pub kind: NetworkConnectorKind,
	pub index: usize,
	pub position: IVec2,
	pub connector: Option<Connector>,
	pub add_highlight: Highlight,
	pub editable: bool,
	pub renaming: bool,
	pub glide: DVec2,
}

impl Keyed for NetworkConnector {
	type Key = (NetworkConnectorKind, usize);

	fn key(&self) -> (NetworkConnectorKind, usize) {
		(self.kind, self.index)
	}
}

impl NetworkConnector {
	pub fn anchor(&self) -> DVec2 {
		self.position.as_dvec2() + self.glide
	}

	pub fn connector_top_left(&self) -> DVec2 {
		self.anchor() + IMPORT_EXPORT_CONNECTOR
	}

	pub fn name_box(&self, width: f64) -> [DVec2; 2] {
		let anchor = self.anchor();
		let left = match self.kind {
			NetworkConnectorKind::Import => anchor.x - IMPORT_NAME_INSET - width,
			NetworkConnectorKind::Export => anchor.x + EXPORT_NAME_INSET,
		};
		let top_left = DVec2::new(left, anchor.y - GRID_SIZE / 2. - IMPORT_EXPORT_NAME_RAISE);
		[top_left, top_left + DVec2::new(width, GRID_SIZE)]
	}

	pub fn separator(&self, name_width: f64) -> [DVec2; 2] {
		let [name_min, name_max] = self.name_box(name_width);
		let toward_the_line = if self.index == 0 { GRID_SIZE } else { -GRID_SIZE };
		let top = self.anchor().y + toward_the_line - BORDER_WIDTH;
		[
			DVec2::new(name_min.x - IMPORT_EXPORT_SEPARATOR_OVERHANG, top),
			DVec2::new(name_max.x + IMPORT_EXPORT_SEPARATOR_OVERHANG, top + BORDER_WIDTH),
		]
	}
}

pub fn reorder_bar(kind: NetworkConnectorKind, anchor: DVec2) -> [DVec2; 2] {
	let left = match kind {
		NetworkConnectorKind::Import => anchor.x - IMPORT_NAME_INSET - REORDER_BAR_SIZE.x,
		NetworkConnectorKind::Export => anchor.x + EXPORT_NAME_INSET,
	};
	let top_left = DVec2::new(left, anchor.y - GRID_SIZE / 2.);
	[top_left, top_left + REORDER_BAR_SIZE]
}

pub fn import_export_row_offset(index: usize) -> f64 {
	let rows = if index == 0 { 0 } else { index + 1 };
	rows as f64 * GRID_SIZE
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_rows::network_connector;

	#[test]
	fn the_name_of_an_import_ends_left_of_its_connector_and_the_name_of_an_export_starts_right_of_it() {
		let primary = NetworkConnector {
			position: IVec2::new(48, -24),
			..network_connector(NetworkConnectorKind::Import, 0)
		};
		let secondary = NetworkConnector {
			position: IVec2::new(48, 24),
			..network_connector(NetworkConnectorKind::Import, 1)
		};
		assert_eq!(secondary.connector_top_left(), DVec2::new(45., 20.), "the connector box beside the anchor");
		let gliding = NetworkConnector {
			glide: DVec2::new(3., -4.),
			..secondary.clone()
		};
		assert_eq!(gliding.anchor(), DVec2::new(51., 20.), "a gliding import is drawn its glide away");
		assert_eq!(primary.name_box(30.), [DVec2::new(10., -37.), DVec2::new(40., -13.)], "an import's name ends left of its connector");
		let export = NetworkConnector {
			kind: NetworkConnectorKind::Export,
			..primary.clone()
		};
		assert_eq!(export.name_box(30.), [DVec2::new(57., -37.), DVec2::new(87., -13.)], "an export's name starts right of its connector");
		assert_eq!(primary.separator(30.), [DVec2::new(6., -1.), DVec2::new(44., 0.)], "the line under the primary name");
		assert_eq!(secondary.separator(20.), [DVec2::new(16., -1.), DVec2::new(44., 0.)], "the line over the next name, the same line");
		assert_eq!(
			reorder_bar(NetworkConnectorKind::Import, secondary.anchor()),
			[DVec2::new(-10., 12.), DVec2::new(40., 14.)],
			"the bar where an import would be dropped, above its row"
		);
		assert_eq!(
			reorder_bar(NetworkConnectorKind::Export, primary.anchor()),
			[DVec2::new(57., -36.), DVec2::new(107., -34.)],
			"the bar where an export would be dropped"
		);
	}
}
