mod name_field;
mod pointer;
#[cfg(test)]
mod tests;

pub use name_field::{GraphName, GraphNameField, edited_name};
pub use pointer::{PointerHits, connector_under_pointer, tooltip_target};

use super::node_graph_message_handler::NodeGraphMessageHandler;
use super::utility_types::FrontendGraphDataType;
use crate::messages::portfolio::document::DocumentMessageHandler;
use crate::messages::portfolio::document::utility_types::network_interface::{GlideOffsets, ImportOrExport, InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use crate::messages::prelude::{Message, PortfolioMessage, Responses};
use glam::DVec2;
use graph_craft::document::NodeId;
use graphene_std::graph_ui::GraphUiConfig;
use graphene_std::graph_ui::card::{Card, Connector, DataType, Highlight, Selection};
use graphene_std::graph_ui::dot::Dot;
use graphene_std::graph_ui::error_badge::ErrorBadge;
use graphene_std::graph_ui::layer::{Layer, LayerButton};
use graphene_std::graph_ui::network_connector::{NetworkConnector, NetworkConnectorKind, import_export_row_offset};
use graphene_std::graph_ui::replicated_map::{Keyed, ReplicatedMapEdit};
use graphene_std::graph_ui::scene::GraphUiSceneEdit;
use graphene_std::graph_ui::wire::{Wire, WireInProgress, WireKey};
use kurbo::BezPath;
use pointer::TooltipTarget;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorKey {
	Input(InputConnector),
	Output(OutputConnector),
}

impl ConnectorKey {
	fn node(&self) -> Option<NodeId> {
		match self {
			ConnectorKey::Input(input) => input.node_id(),
			ConnectorKey::Output(output) => output.node_id(),
		}
	}
}

impl From<ImportOrExport> for ConnectorKey {
	fn from(connector: ImportOrExport) -> Self {
		match connector {
			ImportOrExport::Import(index) => ConnectorKey::Output(OutputConnector::Import(index)),
			ImportOrExport::Export(index) => ConnectorKey::Input(InputConnector::Export(index)),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoveredPart {
	Dot(NodeId),
	DotOutput(NodeId),
	Visibility(NodeId),
	Lock(NodeId),
	Grip(NodeId),
	ErrorBadge(NodeId),
}

impl HoveredPart {
	fn node(&self) -> NodeId {
		match *self {
			HoveredPart::Dot(node) | HoveredPart::DotOutput(node) | HoveredPart::Visibility(node) | HoveredPart::Lock(node) | HoveredPart::Grip(node) | HoveredPart::ErrorBadge(node) => node,
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Highlights {
	pub hovered_connector: Option<ConnectorKey>,
	pub dragged_from: Option<ConnectorKey>,
	pub snapped_to: Option<ConnectorKey>,
	pub hovered_part: Option<HoveredPart>,
	pub pressed_part: Option<HoveredPart>,
	pub clicked: bool,
}

impl Highlights {
	fn nodes(&self) -> impl Iterator<Item = NodeId> {
		let parts = [self.hovered_part, self.pressed_part].into_iter().flatten().map(|part| part.node());
		self.connectors().filter_map(|key| key.node()).chain(parts)
	}

	fn dot(&self, dot: NodeId) -> [Highlight; 2] {
		let ends = [ConnectorKey::Input(InputConnector::node_at_index(dot, 0)), ConnectorKey::Output(OutputConnector::node(dot, 0))];
		let body = if self.snapped_to.is_some_and(|key| ends.contains(&key)) {
			Highlight::WireTarget
		} else if [self.hovered_part, self.pressed_part].contains(&Some(HoveredPart::Dot(dot))) {
			Highlight::Hovered
		} else {
			Highlight::None
		};
		let output = if self.dragged_from.is_some_and(|key| ends.contains(&key)) {
			Highlight::WireSource
		} else if self.hovered_part == Some(HoveredPart::DotOutput(dot)) {
			Highlight::Hovered
		} else {
			Highlight::None
		};
		[body, output]
	}

	fn layer_button(&self, layer: NodeId) -> Option<LayerButton> {
		match self.hovered_part? {
			HoveredPart::Visibility(node) if node == layer => Some(LayerButton::Visibility),
			HoveredPart::Lock(node) if node == layer => Some(LayerButton::Lock),
			HoveredPart::Grip(node) if node == layer => Some(LayerButton::Grip),
			_ => None,
		}
	}

	fn connector(&self, key: ConnectorKey) -> Highlight {
		let key = Some(key);
		if key == self.snapped_to {
			Highlight::WireTarget
		} else if key == self.hovered_connector {
			Highlight::Hovered
		} else if key == self.dragged_from {
			Highlight::WireSource
		} else {
			Highlight::None
		}
	}

	fn connectors(&self) -> impl Iterator<Item = ConnectorKey> {
		[self.hovered_connector, self.dragged_from, self.snapped_to].into_iter().flatten()
	}

	fn badge_hovered(&self, badge: NodeId) -> bool {
		self.hovered_part == Some(HoveredPart::ErrorBadge(badge))
	}
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GraphUiPart {
	All,
	Wires,
	NetworkConnectors,
	Errors,
	Highlights,
	Glide,
	Moved { nodes: Vec<NodeId> },
}

pub struct Facts<'a> {
	network_interface: &'a NodeNetworkInterface,
	network_path: &'a [NodeId],
	node_graph_handler: &'a NodeGraphMessageHandler,
	selected_nodes: Vec<NodeId>,
	selected: Selection,
	glide: GlideOffsets,
}

impl<'a> Facts<'a> {
	pub fn of(document: &'a DocumentMessageHandler) -> Self {
		let network_interface = &document.network_interface;
		let network_path = document.breadcrumb_network_path();
		let selected_nodes = network_interface.selected_nodes_in_nested_network(network_path).unwrap_or_default();
		Self {
			network_interface,
			network_path,
			node_graph_handler: &document.node_graph_handler,
			selected_nodes: selected_nodes.selected_nodes().copied().collect(),
			selected: if network_path == document.selection_network_path() {
				Selection::Active
			} else {
				Selection::Inactive
			},
			glide: network_interface.glide_offsets(network_path),
		}
	}

	pub fn update(&self, part: GraphUiPart, scene: &mut GraphUiSceneEdit, wire_style: GraphWireStyle) {
		match part {
			GraphUiPart::All => {
				scene.cards.replace_all(self.cards());
				scene.layers.replace_all(self.layers());
				scene.dots.replace_all(self.dots());
				scene.wires.replace_all(self.wires(wire_style));
				scene.network_connectors.replace_all(self.network_connectors());
				scene.errors.replace_all(self.errors());
			}
			GraphUiPart::Wires => scene.wires.replace_all(self.wires(wire_style)),
			GraphUiPart::NetworkConnectors => {
				let changed = self.replace_network_connectors(scene);
				self.put_wires_meeting([], changed, scene, wire_style);
			}
			GraphUiPart::Errors => scene.errors.replace_all(self.errors()),
			GraphUiPart::Highlights => self.update_highlights(scene),
			GraphUiPart::Glide => self.update_glide(scene, wire_style),
			GraphUiPart::Moved { nodes } => self.update_moved(scene, nodes, wire_style),
		}
	}

	fn nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
		self.network_interface.nested_network(self.network_path).into_iter().flat_map(|network| network.nodes.keys().copied())
	}

	fn selection(&self, key: NodeId) -> Selection {
		if self.selected_nodes.contains(&key) { self.selected } else { Selection::None }
	}

	fn cards(&self) -> Vec<Card> {
		self.nodes().filter_map(|key| self.card(key)).collect()
	}

	fn card(&self, key: NodeId) -> Option<Card> {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		if network_interface.is_layer(&key, network_path) || network_interface.is_redirect_dot(&key, network_path) {
			return None;
		}
		let cell = network_interface.position(&key, network_path)?;
		let name = network_interface.display_name(&key, network_path);
		Some(Card {
			key,
			cell,
			name,
			icon: network_interface.node_icon(&key, network_path).to_string(),
			primary_input: self.input(key, 0),
			primary_output: self.output(key, 0),
			secondary_inputs: (1..network_interface.number_of_inputs(&key, network_path)).filter_map(|index| self.input(key, index)).collect(),
			secondary_outputs: (1..network_interface.number_of_outputs(&key, network_path)).filter_map(|index| self.output(key, index)).collect(),
			selection: self.selection(key),
			visible: network_interface.is_visible(&key, network_path),
			previewed: network_interface.previewing(network_path).previews(key),
			glide: self.glide.node(&key),
		})
	}

	fn layers(&self) -> Vec<Layer> {
		self.nodes().filter_map(|key| self.layer(key)).collect()
	}

	fn layer(&self, key: NodeId) -> Option<Layer> {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		if !network_interface.is_layer(&key, network_path) {
			return None;
		}
		let highlights = &self.node_graph_handler.graph_ui.highlights;
		let hovered_button = highlights.layer_button(key);
		Some(Layer {
			key,
			cell: network_interface.position(&key, network_path)?,
			name: network_interface.display_name(&key, network_path),
			renaming: self.node_graph_handler.graph_ui.editing_name == Some(GraphName::Layer(key)),
			width: network_interface.layer_width(&key, network_path)?,
			chain_width: network_interface.chain_width(&key, network_path),
			has_left_input_wire: network_interface.has_left_input_wire(&key, network_path),
			primary_input: self.input(key, 0),
			primary_output: self.output(key, 0),
			secondary_input: (1..network_interface.number_of_inputs(&key, network_path)).find_map(|index| self.input(key, index)),
			primary_input_connected_to_layer: network_interface.primary_input_connected_to_layer(&key, network_path),
			primary_output_connected_to_layer: network_interface.primary_output_connected_to_layer(&key, network_path),
			visible: network_interface.is_visible(&key, network_path),
			locked: network_interface.is_locked(&key, network_path),
			selection: self.selection(key),
			previewed: network_interface.previewing(network_path).previews(key),
			hovered_button,
			hovered_button_clicked: hovered_button.is_some() && highlights.clicked,
			glide: self.glide.node(&key),
		})
	}

	fn dots(&self) -> Vec<Dot> {
		self.nodes().filter_map(|key| self.dot(key)).collect()
	}

	fn dot(&self, key: NodeId) -> Option<Dot> {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		if !network_interface.is_redirect_dot(&key, network_path) {
			return None;
		}
		let input = self.input(key, 0);
		let output = self.output(key, 0);
		let [highlight, output_highlight] = self.node_graph_handler.graph_ui.highlights.dot(key);
		Some(Dot {
			key,
			cell: network_interface.position(&key, network_path)?,
			data_type: output.as_ref().map_or(DataType::General, |output| output.data_type),
			wired: input.is_some_and(|input| input.connected) || output.is_some_and(|output| output.connected),
			selected: self.selected_nodes.contains(&key),
			previewed: network_interface.previewing(network_path).previews(key),
			highlight,
			output_highlight,
			glide: self.glide.node(&key),
		})
	}

	fn wires(&self, wire_style: GraphWireStyle) -> Vec<Wire> {
		let inputs = self.network_interface.node_graph_input_connectors(self.network_path).into_iter();
		inputs.filter_map(|input| self.wire(input, wire_style)).chain(self.preview_wire(wire_style)).collect()
	}

	fn wire(&self, input: InputConnector, wire_style: GraphWireStyle) -> Option<Wire> {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		let output = network_interface.upstream_output_connector(&input, network_path)?;
		let routed = network_interface.gliding_vector_wire_from_input(&input, wire_style, &self.glide, network_path)?;
		let insertion_target = Some(input) == self.node_graph_handler.wire_insertion_preview;
		Some(self.wire_row(wire_key(input), output, routed, insertion_target))
	}

	fn preview_wire(&self, wire_style: GraphWireStyle) -> Option<Wire> {
		let (output, path, center_line, thick) = self.network_interface.vector_wire_to_preview(wire_style, &self.glide, self.network_path)?;
		Some(self.wire_row(WireKey::Preview, output, (path, center_line, thick), false))
	}

	fn wire_row(&self, key: WireKey, output: OutputConnector, (path, center_line, thick): (BezPath, BezPath, bool), insertion_target: bool) -> Wire {
		let output_type = self.network_interface.output_type(&output, self.network_path);
		Wire {
			key,
			path,
			data_type: data_type(output_type.displayed_type()),
			thick,
			center_line,
			list: output_type.is_list(),
			insertion_target,
		}
	}

	fn network_connectors(&self) -> Vec<NetworkConnector> {
		[NetworkConnectorKind::Import, NetworkConnectorKind::Export]
			.into_iter()
			.flat_map(|kind| self.network_connectors_of(kind))
			.collect()
	}

	fn network_connectors_of(&self, kind: NetworkConnectorKind) -> Vec<NetworkConnector> {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		let (count, glide) = match kind {
			NetworkConnectorKind::Import => (network_interface.number_of_imports(network_path), self.glide.imports),
			NetworkConnectorKind::Export => (network_interface.number_of_exports(network_path), self.glide.exports),
		};
		let renamed = match (kind, self.node_graph_handler.graph_ui.editing_name) {
			(NetworkConnectorKind::Import, Some(GraphName::Import(index))) | (NetworkConnectorKind::Export, Some(GraphName::Export(index))) => Some(index),
			_ => None,
		};
		let row = |index, position: DVec2, connector: Option<Connector>| NetworkConnector {
			kind,
			index,
			position: position.as_ivec2(),
			add_highlight: if connector.is_some() { Highlight::None } else { self.highlight(interface_connector(kind, index)) },
			connector,
			editable: !network_path.is_empty(),
			renaming: renamed == Some(index),
			glide,
		};
		let named = (0..count).filter_map(|index| {
			let key = interface_connector(kind, index);
			Some(row(index, self.position(key)?, Some(self.connector(key)?)))
		});
		let empty = network_interface.empty_import_export_connectors(kind == NetworkConnectorKind::Import, network_path);
		named.chain(empty.into_iter().map(|(index, position)| row(index, position, None))).collect()
	}

	fn errors(&self) -> Vec<ErrorBadge> {
		let errors = self.node_graph_handler.node_graph_errors(self.network_interface, self.network_path);
		error_badges(errors, self.network_interface, self.network_path, &self.node_graph_handler.graph_ui.highlights, &self.glide)
	}

	fn update_glide(&self, scene: &mut GraphUiSceneEdit, wire_style: GraphWireStyle) {
		let glide = &self.glide;
		let mut moved: BTreeSet<NodeId> = glide_rows(&mut scene.cards, glide, |card| card.glide, |card| &mut card.glide).into_iter().collect();
		moved.extend(glide_rows(&mut scene.layers, glide, |layer| layer.glide, |layer| &mut layer.glide));
		moved.extend(glide_rows(&mut scene.dots, glide, |dot| dot.glide, |dot| &mut dot.glide));
		glide_rows(&mut scene.errors, glide, |badge| badge.glide, |badge| &mut badge.glide);

		let glide_of = |kind: NetworkConnectorKind| if kind == NetworkConnectorKind::Import { glide.imports } else { glide.exports };
		let gliding: Vec<(NetworkConnectorKind, usize)> = scene.network_connectors.iter().filter(|row| row.glide != glide_of(row.kind)).map(Keyed::key).collect();
		for key in &gliding {
			scene.network_connectors.update(key, |row| row.glide = glide_of(row.kind));
		}

		if moved.is_empty() && gliding.is_empty() {
			return;
		}
		self.put_wires_meeting(moved, gliding, scene, wire_style);
	}

	fn update_moved(&self, scene: &mut GraphUiSceneEdit, nodes: Vec<NodeId>, wire_style: GraphWireStyle) {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		let downstream_layers = nodes.iter().filter_map(|node| network_interface.downstream_layer_for_chain_node(node, network_path));
		let rows: BTreeSet<NodeId> = nodes.iter().copied().chain(downstream_layers).collect();
		self.put_node_rows(rows, scene);
		scene.errors.replace_all(self.errors());
		let changed = self.replace_network_connectors(scene);
		self.put_wires_meeting(nodes, changed, scene, wire_style);
	}

	fn put_node_rows(&self, nodes: impl IntoIterator<Item = NodeId>, scene: &mut GraphUiSceneEdit) {
		for node in nodes {
			if let Some(card) = self.card(node) {
				scene.cards.put(card);
			}
			if let Some(layer) = self.layer(node) {
				scene.layers.put(layer);
			}
			if let Some(dot) = self.dot(node) {
				scene.dots.put(dot);
			}
		}
	}

	fn replace_network_connectors(&self, scene: &mut GraphUiSceneEdit) -> Vec<(NetworkConnectorKind, usize)> {
		let replaced = self.network_connectors();
		let changed = replaced.iter().filter(|row| scene.network_connectors.get(&row.key()) != Some(row)).map(Keyed::key).collect();
		scene.network_connectors.replace_all(replaced);
		changed
	}

	fn put_wires_meeting(
		&self,
		nodes: impl IntoIterator<Item = NodeId>,
		network_connectors: impl IntoIterator<Item = (NetworkConnectorKind, usize)>,
		scene: &mut GraphUiSceneEdit,
		wire_style: GraphWireStyle,
	) {
		for input in self.inputs_meeting(nodes, network_connectors) {
			if let Some(wire) = self.wire(input, wire_style) {
				scene.wires.put(wire);
			}
		}
		if let Some(preview) = self.preview_wire(wire_style) {
			scene.wires.put(preview);
		}
	}

	fn inputs_meeting(&self, nodes: impl IntoIterator<Item = NodeId>, network_connectors: impl IntoIterator<Item = (NetworkConnectorKind, usize)>) -> HashSet<InputConnector> {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		let mut inputs = HashSet::new();
		let mut outputs = Vec::new();
		let network = network_interface.nested_network(network_path);
		for node_id in nodes.into_iter().filter(|node_id| network.is_some_and(|network| network.nodes.contains_key(node_id))) {
			inputs.extend((0..network_interface.number_of_inputs(&node_id, network_path)).map(|input_index| InputConnector::node_at_index(node_id, input_index)));
			outputs.extend((0..network_interface.number_of_outputs(&node_id, network_path)).map(|output_index| OutputConnector::node(node_id, output_index)));
		}

		for (kind, index) in network_connectors {
			match kind {
				NetworkConnectorKind::Import => outputs.push(OutputConnector::Import(index)),
				NetworkConnectorKind::Export => {
					inputs.insert(InputConnector::Export(index));
				}
			}
		}

		network_interface.with_outward_wires(network_path, |outward_wires| {
			for output in &outputs {
				inputs.extend(outward_wires.get(output).into_iter().flatten().copied());
			}
		});

		inputs
	}

	fn input(&self, node: NodeId, index: usize) -> Option<Connector> {
		self.connector(ConnectorKey::Input(InputConnector::node_at_index(node, index)))
	}

	fn output(&self, node: NodeId, index: usize) -> Option<Connector> {
		self.connector(ConnectorKey::Output(OutputConnector::node(node, index)))
	}

	fn connector(&self, key: ConnectorKey) -> Option<Connector> {
		let (network_interface, network_path) = (self.network_interface, self.network_path);
		let (name, connector_type, connected) = match key {
			ConnectorKey::Input(input) => network_interface.graph_ui_input(&input, network_path),
			ConnectorKey::Output(output) => network_interface.graph_ui_output(&output, network_path),
		}?;
		Some(Connector {
			name,
			data_type: data_type(connector_type),
			connected,
			highlight: self.highlight(key),
		})
	}

	fn position(&self, key: ConnectorKey) -> Option<DVec2> {
		match key {
			ConnectorKey::Input(input) => self.network_interface.input_position(&input, self.network_path),
			ConnectorKey::Output(output) => self.network_interface.output_position(&output, self.network_path),
		}
	}

	fn highlight(&self, key: ConnectorKey) -> Highlight {
		self.node_graph_handler.graph_ui.highlights.connector(key)
	}

	fn update_highlights(&self, scene: &mut GraphUiSceneEdit) {
		let highlights = &self.node_graph_handler.graph_ui.highlights;
		let highlighted = |connector: &Connector| connector.highlight != Highlight::None;
		let card_highlighted = |card: &Card| card.inputs().chain(card.outputs()).any(|(_, connector)| highlighted(connector));
		let layer_connectors = |layer: &Layer| [&layer.primary_input, &layer.primary_output, &layer.secondary_input].into_iter().flatten().any(highlighted);
		let layer_highlighted = |layer: &Layer| layer.hovered_button.is_some() || layer_connectors(layer);
		let dot_highlighted = |dot: &Dot| dot.highlight != Highlight::None || dot.output_highlight != Highlight::None;
		let mut nodes: BTreeSet<NodeId> = highlights.nodes().collect();
		nodes.extend(scene.cards.iter().filter(|card| card_highlighted(card)).map(|card| card.key));
		nodes.extend(scene.layers.iter().filter(|layer| layer_highlighted(layer)).map(|layer| layer.key));
		nodes.extend(scene.dots.iter().filter(|dot| dot_highlighted(dot)).map(|dot| dot.key));
		self.put_node_rows(nodes, scene);

		let highlight = |(kind, index)| self.highlight(interface_connector(kind, index));
		let shown = |row: &NetworkConnector| row.connector.as_ref().map_or(row.add_highlight, |connector| connector.highlight);
		let changed: Vec<(NetworkConnectorKind, usize)> = scene.network_connectors.iter().filter(|row| shown(row) != highlight(row.key())).map(Keyed::key).collect();
		for key in changed {
			scene.network_connectors.update(&key, |row| match &mut row.connector {
				Some(connector) => connector.highlight = highlight(key),
				None => row.add_highlight = highlight(key),
			});
		}

		let changed: Vec<NodeId> = scene
			.errors
			.iter()
			.filter(|badge| badge.hovered != highlights.badge_hovered(badge.key))
			.map(|badge| badge.key)
			.collect();
		for key in changed {
			scene.errors.update(&key, |badge| badge.hovered = highlights.badge_hovered(key));
		}
	}
}

#[derive(Clone, Debug, Default)]
pub struct GraphUiState {
	highlights: Highlights,
	tooltip: Option<(Option<TooltipTarget>, String, String)>,
	reword_tooltip: bool,
	editing_name: Option<GraphName>,
	name_field: Option<(GraphName, GraphNameField)>,
	error_text_sizes: HashMap<String, DVec2>,
}

impl GraphUiState {
	pub fn change_highlights(&mut self, change: impl FnOnce(&mut Highlights), responses: &mut VecDeque<Message>) {
		let mut highlights = self.highlights;
		change(&mut highlights);
		if self.highlights != highlights {
			self.highlights = highlights;
			responses.add(PortfolioMessage::UpdateGraphUi { part: GraphUiPart::Highlights });
		}
	}
}

fn error_badges(errors: Vec<(NodeId, String)>, network_interface: &NodeNetworkInterface, network_path: &[NodeId], highlights: &Highlights, glide: &GlideOffsets) -> Vec<ErrorBadge> {
	errors
		.into_iter()
		.filter_map(|(key, text)| {
			Some(ErrorBadge {
				key,
				cell: network_interface.position(&key, network_path)?,
				on_layer: network_interface.is_layer(&key, network_path),
				text,
				hovered: highlights.badge_hovered(key),
				glide: glide.node(&key),
			})
		})
		.collect()
}

fn glide_rows<Row: Keyed<Key = NodeId>>(rows: &mut ReplicatedMapEdit<Row>, glide: &GlideOffsets, drawn: fn(&Row) -> DVec2, field: fn(&mut Row) -> &mut DVec2) -> Vec<NodeId> {
	let moved: Vec<NodeId> = rows.iter().filter(|row| drawn(row) != glide.node(&row.key())).map(|row| row.key()).collect();
	for key in &moved {
		rows.update(key, |row| *field(row) = glide.node(key));
	}
	moved
}

pub fn config(document: &DocumentMessageHandler) -> Option<GraphUiConfig> {
	let node_graph_handler = &document.node_graph_handler;
	let graph_metadata = document.network_interface.network_metadata(document.breadcrumb_network_path());
	graph_metadata.filter(|_| document.graph_view_overlay_open).map(|metadata| GraphUiConfig {
		view: metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport,
		fade_artwork: document.graph_fade_artwork_percentage / 100.,
		wire_in_progress: wire_in_progress(node_graph_handler),
		reorder_bar: reorder_bar(document),
		box_selection: node_graph_handler.box_selection,
	})
}

fn reorder_bar(document: &DocumentMessageHandler) -> Option<(NetworkConnectorKind, DVec2)> {
	let node_graph_handler = &document.node_graph_handler;
	let index = node_graph_handler.end_index?;
	let kind = if node_graph_handler.reordering_import.is_some() {
		NetworkConnectorKind::Import
	} else if node_graph_handler.reordering_export.is_some() {
		NetworkConnectorKind::Export
	} else {
		return None;
	};
	let (imports, exports) = document.network_interface.import_export_position(document.breadcrumb_network_path())?;
	let first_row_center = match kind {
		NetworkConnectorKind::Import => imports,
		NetworkConnectorKind::Export => exports,
	};
	Some((kind, first_row_center.as_dvec2() + DVec2::new(0., import_export_row_offset(index))))
}

fn wire_in_progress(node_graph_handler: &NodeGraphMessageHandler) -> Option<WireInProgress> {
	let path = node_graph_handler.wire_in_progress_path.as_ref()?;
	Some(WireInProgress {
		path: path.elements().try_into().ok()?,
		data_type: data_type(node_graph_handler.wire_in_progress_type),
	})
}

fn interface_connector(kind: NetworkConnectorKind, index: usize) -> ConnectorKey {
	match kind {
		NetworkConnectorKind::Import => ImportOrExport::Import(index),
		NetworkConnectorKind::Export => ImportOrExport::Export(index),
	}
	.into()
}

fn wire_key(input: InputConnector) -> WireKey {
	match input {
		InputConnector::Node { node_id, input_index } => WireKey::Input { node: node_id, index: input_index },
		InputConnector::Export(index) => WireKey::Export(index),
	}
}

fn data_type(data_type: FrontendGraphDataType) -> DataType {
	match data_type {
		FrontendGraphDataType::General => DataType::General,
		FrontendGraphDataType::Number => DataType::Number,
		FrontendGraphDataType::Artboard => DataType::Artboard,
		FrontendGraphDataType::Graphic => DataType::Graphic,
		FrontendGraphDataType::Raster => DataType::Raster,
		FrontendGraphDataType::Vector => DataType::Vector,
		FrontendGraphDataType::Color => DataType::Color,
		FrontendGraphDataType::Gradient => DataType::Gradient,
		FrontendGraphDataType::Typography => DataType::Typography,
		FrontendGraphDataType::Invalid => DataType::Invalid,
	}
}
