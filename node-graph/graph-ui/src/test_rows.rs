use crate::card::{Card, Connector, DataType, Highlight, Selection};
use crate::dot::Dot;
use crate::error_badge::ErrorBadge;
use crate::layer::Layer;
use crate::network_connector::{NetworkConnector, NetworkConnectorKind};
use crate::wire::{Wire, WireKey};
use core_types::uuid::NodeId;
use glam::{DVec2, IVec2};
use vello_gpu::kurbo::BezPath;

pub(crate) fn card(key: u64) -> Card {
	Card {
		key: NodeId(key),
		cell: IVec2::ZERO,
		name: String::new(),
		icon: String::new(),
		primary_input: None,
		primary_output: None,
		secondary_inputs: Vec::new(),
		secondary_outputs: Vec::new(),
		selection: Selection::None,
		visible: true,
		previewed: false,
		glide: DVec2::ZERO,
	}
}

pub(crate) fn layer(key: u64) -> Layer {
	Layer {
		key: NodeId(key),
		cell: IVec2::ZERO,
		name: String::new(),
		renaming: false,
		width: 8,
		chain_width: 0,
		has_left_input_wire: false,
		primary_input: None,
		primary_output: None,
		secondary_input: None,
		primary_input_connected_to_layer: false,
		primary_output_connected_to_layer: false,
		visible: true,
		locked: false,
		selection: Selection::None,
		previewed: false,
		hovered_button: None,
		hovered_button_clicked: false,
		glide: DVec2::ZERO,
	}
}

pub(crate) fn dot(key: u64) -> Dot {
	Dot {
		key: NodeId(key),
		cell: IVec2::ZERO,
		data_type: DataType::General,
		wired: false,
		selected: false,
		previewed: false,
		highlight: Highlight::None,
		output_highlight: Highlight::None,
		glide: DVec2::ZERO,
	}
}

pub(crate) fn wire(key: WireKey) -> Wire {
	Wire {
		key,
		path: BezPath::new(),
		data_type: DataType::General,
		thick: false,
		center_line: BezPath::new(),
		list: false,
		insertion_target: false,
	}
}

pub(crate) fn network_connector(kind: NetworkConnectorKind, index: usize) -> NetworkConnector {
	NetworkConnector {
		kind,
		index,
		position: IVec2::ZERO,
		connector: None,
		add_highlight: Highlight::None,
		editable: false,
		renaming: false,
		glide: DVec2::ZERO,
	}
}

pub(crate) fn badge(key: u64, text: &str) -> ErrorBadge {
	ErrorBadge {
		key: NodeId(key),
		cell: IVec2::ZERO,
		on_layer: false,
		text: text.to_string(),
		hovered: false,
		glide: DVec2::ZERO,
	}
}

pub(crate) fn connector(name: &str) -> Connector {
	Connector {
		name: name.to_string(),
		data_type: DataType::General,
		connected: false,
		highlight: Highlight::None,
	}
}
