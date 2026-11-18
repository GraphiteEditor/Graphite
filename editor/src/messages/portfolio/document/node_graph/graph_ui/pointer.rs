use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::network_interface::LayerClickTargetTypes;
use crate::messages::prelude::FrontendMessage;
use graphene_std::graph_ui::metrics::{GRID_SIZE, NODE_ROW_PADDING};
use std::cell::OnceCell;

impl GraphUiState {
	pub fn part_under_pointer(&mut self, hits: &PointerHits, errors: Vec<(NodeId, String)>, pointer_free: bool) -> Option<HoveredPart> {
		if let Some(badge) = self.error_badge_under(hits, errors) {
			return Some(HoveredPart::ErrorBadge(badge));
		}

		let (network_interface, network_path) = (hits.network_interface, hits.network_path);
		let dot = hits.node().filter(|node_id| network_interface.is_redirect_dot(node_id, network_path));
		if let Some(dot) = dot {
			if !pointer_free {
				return None;
			}
			if hits.output() == Some(OutputConnector::node(dot, 0)) {
				return Some(HoveredPart::DotOutput(dot));
			}
			let pulls_a_wire_in = hits.input() == Some(InputConnector::node_at_index(dot, 0));
			return (!pulls_a_wire_in).then_some(HoveredPart::Dot(dot));
		}

		let visibility = hits.visibility().map(HoveredPart::Visibility);
		let lock = || hits.lock().map(HoveredPart::Lock);
		let grip = || hits.grip().map(HoveredPart::Grip);
		visibility.or_else(lock).or_else(grip)
	}

	fn error_badge_under(&mut self, hits: &PointerHits, errors: Vec<(NodeId, String)>) -> Option<NodeId> {
		let (network_interface, network_path) = (hits.network_interface, hits.network_path);
		let mut badges = error_badges(errors, network_interface, network_path, &self.highlights, &network_interface.glide_offsets(network_path));
		badges.sort_by_key(|badge| (badge.hovered, badge.key));
		let text_sizes = &mut self.error_text_sizes;
		text_sizes.retain(|text, _| badges.iter().any(|badge| badge.text == *text));
		let mut text_size = |badge: &ErrorBadge| *text_sizes.entry(badge.text.clone()).or_insert_with(|| badge.text_size());
		badges.iter().rev().find(|badge| badge.covers(hits.point, text_size(badge))).map(|badge| badge.key)
	}

	pub fn reword_tooltip(&mut self) {
		self.reword_tooltip = true;
	}

	pub fn set_tooltip(&mut self, target: Option<TooltipTarget>, network_interface: &NodeNetworkInterface, network_path: &[NodeId], responses: &mut VecDeque<Message>) {
		let reword = std::mem::take(&mut self.reword_tooltip);
		if !reword && self.tooltip.as_ref().is_some_and(|(sent, _, _)| *sent == target) {
			return;
		}
		let (label, description) = target.map(|target| tooltip_text(target, network_interface, network_path)).unwrap_or_default();
		let tooltip = (target, label, description);
		if self.tooltip.as_ref() == Some(&tooltip) {
			return;
		}
		let (_, label, description) = self.tooltip.insert(tooltip).clone();
		responses.add(FrontendMessage::UpdateGraphTooltip { label, description });
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipTarget {
	Name(ImportOrExport),
	AddConnector(ImportOrExport),
	Connector(ConnectorKey),
	Visibility(NodeId),
	Lock(NodeId),
	Grip(NodeId),
	Dot(NodeId),
	Row { node: NodeId, row: usize },
	Node(NodeId),
}

pub struct PointerHits<'a> {
	network_interface: &'a NodeNetworkInterface,
	network_path: &'a [NodeId],
	viewport_location: DVec2,
	point: DVec2,
	found: FoundHits,
}

#[derive(Default)]
struct FoundHits {
	name: OnceCell<Option<ImportOrExport>>,
	empty_connector: OnceCell<Option<ImportOrExport>>,
	node: OnceCell<Option<NodeId>>,
	visibility: OnceCell<Option<NodeId>>,
	lock: OnceCell<Option<NodeId>>,
	grip: OnceCell<Option<NodeId>>,
	input: OnceCell<Option<InputConnector>>,
	output: OnceCell<Option<OutputConnector>>,
}

impl<'a> PointerHits<'a> {
	pub fn new(network_interface: &'a NodeNetworkInterface, network_path: &'a [NodeId], viewport_location: DVec2, point: DVec2) -> Self {
		Self {
			network_interface,
			network_path,
			viewport_location,
			point,
			found: FoundHits::default(),
		}
	}

	fn name(&self) -> Option<ImportOrExport> {
		*self.found.name.get_or_init(|| self.network_interface.import_export_name_from_point(self.point, self.network_path))
	}

	fn empty_connector(&self) -> Option<ImportOrExport> {
		*self.found.empty_connector.get_or_init(|| {
			let empty = |is_import| self.network_interface.empty_import_export_connector_at(self.point, is_import, self.network_path);
			let import = empty(true).map(|(index, _)| ImportOrExport::Import(index));
			import.or_else(|| empty(false).map(|(index, _)| ImportOrExport::Export(index)))
		})
	}

	fn node(&self) -> Option<NodeId> {
		*self.found.node.get_or_init(|| self.network_interface.node_from_click(self.viewport_location, self.network_path))
	}

	fn visibility(&self) -> Option<NodeId> {
		self.layer_button(&self.found.visibility, LayerClickTargetTypes::Visibility)
	}

	fn lock(&self) -> Option<NodeId> {
		self.layer_button(&self.found.lock, LayerClickTargetTypes::Lock)
	}

	fn grip(&self) -> Option<NodeId> {
		self.layer_button(&self.found.grip, LayerClickTargetTypes::Grip)
	}

	fn layer_button(&self, found: &OnceCell<Option<NodeId>>, button: LayerClickTargetTypes) -> Option<NodeId> {
		*found.get_or_init(|| self.network_interface.layer_click_target_from_click(self.viewport_location, button, self.network_path))
	}

	fn input(&self) -> Option<InputConnector> {
		*self.found.input.get_or_init(|| self.network_interface.input_port_from_click(self.viewport_location, self.network_path))
	}

	fn output(&self) -> Option<OutputConnector> {
		*self
			.found
			.output
			.get_or_init(|| self.network_interface.output_port_from_click(self.viewport_location, self.network_path))
	}
}

pub fn connector_under_pointer(hits: &PointerHits) -> Option<ConnectorKey> {
	if hits.name().is_some() || hits.visibility().is_some() || hits.lock().is_some() {
		return None;
	}

	let input = || hits.input().map(ConnectorKey::Input);
	let output = || hits.output().map(ConnectorKey::Output);
	hits.empty_connector().map(ConnectorKey::from).or_else(input).or_else(output)
}

pub fn tooltip_target(hits: &PointerHits) -> Option<TooltipTarget> {
	let (network_interface, network_path) = (hits.network_interface, hits.network_path);
	if let Some(name) = hits.name() {
		return Some(TooltipTarget::Name(name));
	}
	if let Some(add) = hits.empty_connector() {
		return Some(TooltipTarget::AddConnector(add));
	}

	let node = hits.node();
	if let Some(dot) = node.filter(|node| network_interface.is_redirect_dot(node, network_path)) {
		return Some(TooltipTarget::Dot(dot));
	}
	let input = || hits.input().map(|input| TooltipTarget::Connector(ConnectorKey::Input(input)));
	let output = || hits.output().map(|output| TooltipTarget::Connector(ConnectorKey::Output(output)));
	let part = hits
		.visibility()
		.map(TooltipTarget::Visibility)
		.or_else(|| hits.lock().map(TooltipTarget::Lock))
		.or_else(input)
		.or_else(output)
		.or_else(|| hits.grip().map(TooltipTarget::Grip));
	if part.is_some() {
		return part;
	}

	let node = node?;
	if network_interface.is_layer(&node, network_path) {
		return Some(TooltipTarget::Node(node));
	}
	let point = hits.point;
	let glide = network_interface.glide_offsets(network_path).node(&node);
	let [min, max] = network_interface.node_bounding_box(&node, network_path)?.map(|corner| corner + glide);
	let row = ((point.y - min.y) / GRID_SIZE).floor();
	let on_row_name = point.x >= min.x + NODE_ROW_PADDING && point.x <= max.x - NODE_ROW_PADDING;
	Some(if row >= 1. && on_row_name {
		TooltipTarget::Row { node, row: row as usize }
	} else {
		TooltipTarget::Node(node)
	})
}

fn tooltip_text(target: TooltipTarget, network_interface: &NodeNetworkInterface, network_path: &[NodeId]) -> (String, String) {
	let input = |input: InputConnector| network_interface.frontend_input_from_connector(&input, network_path);
	let output = |output: OutputConnector| network_interface.frontend_output_from_connector(&output, network_path);
	let input_tooltip = |key: InputConnector| {
		let lists_no_types = matches!(key, InputConnector::Node { .. });
		input(key).map(|input| connector_tooltip(input.name, &input.description, input.resolved_type, &input.valid_types, input.connected, lists_no_types))
	};
	let output_tooltip = |key: OutputConnector| output(key).map(|output| connector_tooltip(output.name, &output.description, output.resolved_type, &output.possible_types, output.connected, false));
	let label_only = |text: &str| (text.to_string(), String::new());

	match target {
		TooltipTarget::Name(ImportOrExport::Import(index)) => (network_interface.displayed_import_name(index, network_path), network_interface.import_description(index, network_path)),
		TooltipTarget::Name(ImportOrExport::Export(index)) => (network_interface.displayed_export_name(index, network_path), network_interface.export_description(index, network_path)),
		TooltipTarget::AddConnector(add) => {
			let (label, description) = match add {
				ImportOrExport::Import(index) => (
					if index == 0 { "Add Primary Import" } else { "Add Import" },
					"Drag a wire from here and connect it to an input, or drop one here from an input.",
				),
				ImportOrExport::Export(index) => (
					if index == 0 { "Add Primary Export" } else { "Add Export" },
					"Drag a wire from here and connect it to an output, or drop one here from an output.",
				),
			};
			(label.to_string(), description.to_string())
		}
		TooltipTarget::Connector(ConnectorKey::Input(key)) => input_tooltip(key).unwrap_or_default(),
		TooltipTarget::Connector(ConnectorKey::Output(key)) => output_tooltip(key).unwrap_or_default(),
		TooltipTarget::Visibility(layer) => label_only(if network_interface.is_visible(&layer, network_path) { "Hide" } else { "Show" }),
		TooltipTarget::Lock(_) => label_only("Unlock"),
		TooltipTarget::Grip(_) => (String::new(), "Drag only this layer without pushing others outside the stack".to_string()),
		TooltipTarget::Dot(dot) => output_tooltip(OutputConnector::node(dot, 0)).unwrap_or_else(|| label_only(&network_interface.display_name(&dot, network_path))),
		TooltipTarget::Row { node, row } => {
			let secondary_inputs = (1..network_interface.number_of_inputs(&node, network_path)).filter_map(|index| input(InputConnector::node_at_index(node, index)));
			let named_input = secondary_inputs.map(|input| (input.name, input.description)).nth(row - 1);
			let named_output = || output(OutputConnector::node(node, row)).map(|output| (output.name, output.description));
			named_input.or_else(named_output).unwrap_or_default()
		}
		TooltipTarget::Node(node) => {
			let (name, implementation_name) = (network_interface.display_name(&node, network_path), network_interface.implementation_name(&node, network_path));
			let label = if name == implementation_name { name } else { format!("{implementation_name} ({name})") };
			let definition = network_interface.reference(&node, network_path).and_then(|reference| resolve_document_node_type(&reference));
			(label, definition.map(|definition| definition.description.trim().to_string()).unwrap_or_default())
		}
	}
}

fn connector_tooltip(name: String, description: &str, resolved_type: String, types: &[String], connected: bool, lists_no_types: bool) -> (String, String) {
	let wired_type = connected.then(|| format!("Wired Type:\n`{resolved_type}`"));
	let possible_types = if types.is_empty() {
		lists_no_types.then(|| "Possible Types:\nNone".to_string())
	} else {
		Some(format!("Possible Types:\n{}", types.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join("\n")))
	};
	let types = [wired_type, possible_types].into_iter().flatten().collect::<Vec<_>>().join("\n\n");
	let description = [description.trim().to_string(), types].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join("\n---\n");
	(if name.is_empty() { resolved_type } else { name }, description)
}

#[cfg(test)]
mod tests {
	use super::connector_tooltip;

	#[test]
	fn a_connector_tooltip_tells_its_description_then_its_wired_and_possible_types() {
		let types = ["f64".to_string(), "u32".to_string()];
		assert_eq!(
			connector_tooltip("Radius".to_string(), " The size. ", "f64".to_string(), &types, true, false),
			("Radius".to_string(), "The size.\n---\nWired Type:\n`f64`\n\nPossible Types:\n`f64`\n`u32`".to_string())
		);
		assert_eq!(
			connector_tooltip(String::new(), "", "f64".to_string(), &[], false, true),
			("f64".to_string(), "Possible Types:\nNone".to_string()),
			"an unnamed node input is labeled with its type and tells that no type fits"
		);
		assert_eq!(
			connector_tooltip("Out".to_string(), "", "f64".to_string(), &[], false, false),
			("Out".to_string(), String::new()),
			"an output or export leaves out the types it does not know"
		);
	}
}
