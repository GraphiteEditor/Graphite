use crate::card::Card;
use crate::dot::Dot;
use crate::error_badge::ErrorBadge;
use crate::layer::Layer;
use crate::network_connector::NetworkConnector;
use crate::replicated_map::{ReplicatedMap, ReplicatedMapDelta, ReplicatedMapEdit};
use crate::wire::Wire;
use graphene_hash::CacheHash;

#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct GraphUiScene {
	cards: ReplicatedMap<Card>,
	layers: ReplicatedMap<Layer>,
	dots: ReplicatedMap<Dot>,
	wires: ReplicatedMap<Wire>,
	network_connectors: ReplicatedMap<NetworkConnector>,
	errors: ReplicatedMap<ErrorBadge>,
}

impl GraphUiScene {
	pub fn cards(&self) -> &ReplicatedMap<Card> {
		&self.cards
	}

	pub fn layers(&self) -> &ReplicatedMap<Layer> {
		&self.layers
	}

	pub fn dots(&self) -> &ReplicatedMap<Dot> {
		&self.dots
	}

	pub fn wires(&self) -> &ReplicatedMap<Wire> {
		&self.wires
	}

	pub fn network_connectors(&self) -> &ReplicatedMap<NetworkConnector> {
		&self.network_connectors
	}

	pub fn errors(&self) -> &ReplicatedMap<ErrorBadge> {
		&self.errors
	}

	pub fn edit(self) -> GraphUiSceneEdit {
		GraphUiSceneEdit {
			cards: self.cards.edit(),
			layers: self.layers.edit(),
			dots: self.dots.edit(),
			wires: self.wires.edit(),
			network_connectors: self.network_connectors.edit(),
			errors: self.errors.edit(),
		}
	}

	pub fn apply(&mut self, delta: GraphUiSceneDelta) {
		self.cards.apply(delta.cards);
		self.layers.apply(delta.layers);
		self.dots.apply(delta.dots);
		self.wires.apply(delta.wires);
		self.network_connectors.apply(delta.network_connectors);
		self.errors.apply(delta.errors);
	}
}

pub struct GraphUiSceneEdit {
	pub cards: ReplicatedMapEdit<Card>,
	pub layers: ReplicatedMapEdit<Layer>,
	pub dots: ReplicatedMapEdit<Dot>,
	pub wires: ReplicatedMapEdit<Wire>,
	pub network_connectors: ReplicatedMapEdit<NetworkConnector>,
	pub errors: ReplicatedMapEdit<ErrorBadge>,
}

impl GraphUiSceneEdit {
	pub fn finish(self) -> (GraphUiScene, GraphUiSceneDelta) {
		let (cards, card_deltas) = self.cards.finish();
		let (layers, layer_deltas) = self.layers.finish();
		let (dots, dot_deltas) = self.dots.finish();
		let (wires, wire_deltas) = self.wires.finish();
		let (network_connectors, network_connector_deltas) = self.network_connectors.finish();
		let (errors, error_deltas) = self.errors.finish();
		(
			GraphUiScene {
				cards,
				layers,
				dots,
				wires,
				network_connectors,
				errors,
			},
			GraphUiSceneDelta {
				cards: card_deltas,
				layers: layer_deltas,
				dots: dot_deltas,
				wires: wire_deltas,
				network_connectors: network_connector_deltas,
				errors: error_deltas,
			},
		)
	}
}

#[derive(Debug)]
pub struct GraphUiSceneDelta {
	cards: Vec<ReplicatedMapDelta<Card>>,
	layers: Vec<ReplicatedMapDelta<Layer>>,
	dots: Vec<ReplicatedMapDelta<Dot>>,
	wires: Vec<ReplicatedMapDelta<Wire>>,
	network_connectors: Vec<ReplicatedMapDelta<NetworkConnector>>,
	errors: Vec<ReplicatedMapDelta<ErrorBadge>>,
}

impl GraphUiSceneDelta {
	pub fn is_empty(&self) -> bool {
		self.cards.is_empty() && self.layers.is_empty() && self.dots.is_empty() && self.wires.is_empty() && self.network_connectors.is_empty() && self.errors.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::network_connector::NetworkConnectorKind;
	use crate::test_rows::{badge, card, connector, dot, layer, network_connector, wire};
	use crate::wire::WireKey;
	use core_types::uuid::NodeId;
	use glam::IVec2;

	#[test]
	fn a_copy_that_applies_the_delta_of_an_edit_equals_the_edited_scene() {
		let mut edit = GraphUiScene::default().edit();
		edit.cards.replace_all([card(0), card(1)]);
		let (scene, _) = edit.finish();
		let mut copy = scene.clone();

		let mut edit = scene.edit();
		edit.cards.put(card(2));
		edit.cards.update(&NodeId(0), |card| card.cell = IVec2::ONE);
		edit.cards.remove(&NodeId(1));
		edit.layers.put(layer(4));
		edit.dots.put(dot(3));
		edit.wires.put(wire(WireKey::Input { node: NodeId(2), index: 0 }));
		edit.network_connectors.put(network_connector(NetworkConnectorKind::Import, 0));
		edit.network_connectors.put(NetworkConnector {
			connector: Some(connector("Canvas")),
			..network_connector(NetworkConnectorKind::Export, 0)
		});
		edit.errors.put(badge(2, "An error"));
		let (edited, delta) = edit.finish();

		assert!(!delta.is_empty());
		copy.apply(delta);
		assert_eq!(copy, edited);

		let mut edit = copy.edit();
		edit.errors.remove(&NodeId(2));
		assert!(!edit.finish().1.is_empty(), "a change to one map is a delta");

		assert!(edited.edit().finish().1.is_empty(), "an edit that changes nothing has no deltas");
	}
}
