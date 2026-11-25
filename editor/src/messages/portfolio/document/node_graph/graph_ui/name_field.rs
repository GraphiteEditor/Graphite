use super::*;
use crate::messages::prelude::FrontendMessage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GraphName {
	Layer(NodeId),
	Import(usize),
	Export(usize),
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphNameField {
	pub name: String,
	pub x: f64,
	pub top: f64,
	#[serde(rename = "alignRight")]
	pub align_right: bool,
	pub dimmed: bool,
}

impl Facts<'_> {
	fn name_field(&self, name: GraphName) -> Option<GraphNameField> {
		match name {
			GraphName::Layer(key) => {
				let layer = self.layer(key)?;
				let [top_left, _] = layer.name_box();
				Some(GraphNameField {
					name: layer.name,
					x: top_left.x,
					top: top_left.y,
					align_right: false,
					dimmed: !layer.visible,
				})
			}
			GraphName::Import(index) => self.interface_name_field(NetworkConnectorKind::Import, index),
			GraphName::Export(index) => self.interface_name_field(NetworkConnectorKind::Export, index),
		}
	}

	fn interface_name_field(&self, kind: NetworkConnectorKind, index: usize) -> Option<GraphNameField> {
		let row = self.network_connectors().into_iter().find(|row| row.key() == (kind, index) && row.editable)?;
		let connector = row.connector.as_ref()?;
		let [top_left, _] = row.name_box(0.);
		Some(GraphNameField {
			name: connector.name.clone(),
			x: top_left.x,
			top: top_left.y,
			align_right: kind == NetworkConnectorKind::Import,
			dimmed: false,
		})
	}
}

impl GraphUiState {
	pub fn begin_name_edit(&mut self, name: GraphName, responses: &mut VecDeque<Message>) {
		if self.editing_name != Some(name) {
			self.editing_name = Some(name);
			responses.add(PortfolioMessage::UpdateGraphUi { part: GraphUiPart::All });
		}
	}

	pub fn end_name_edit(&mut self) -> Option<GraphName> {
		self.editing_name.take()
	}

	pub fn update_name_field(&mut self, edited: Option<(GraphName, GraphNameField)>, responses: &mut VecDeque<Message>) {
		let shown = self.name_field.as_ref();
		let same_row = |(name, field): &(GraphName, GraphNameField)| match (name, shown) {
			(GraphName::Import(_) | GraphName::Export(_), Some((shown, shown_field))) if shown == name => shown_field.name == field.name,
			_ => true,
		};
		let edited = edited.filter(same_row);
		if edited.is_none() {
			self.editing_name = None;
		}
		if edited == self.name_field {
			return;
		}
		let sent = self.name_field.as_ref().map(|(name, _)| *name);
		let opens = edited.as_ref().is_some_and(|(name, _)| sent != Some(*name));
		self.name_field.clone_from(&edited);
		responses.add(FrontendMessage::UpdateGraphNameField {
			field: edited.map(|(_, field)| field),
			opens,
		});
	}

	pub fn leave_network(&mut self, responses: &mut VecDeque<Message>) {
		if self.editing_name.is_some() || self.name_field.is_some() {
			self.drop_name_edit(responses);
		}
	}

	pub fn graph_view_toggled(&mut self, responses: &mut VecDeque<Message>) {
		self.tooltip = None;
		self.drop_name_edit(responses);
	}

	fn drop_name_edit(&mut self, responses: &mut VecDeque<Message>) {
		self.editing_name = None;
		self.name_field = None;
		responses.add(FrontendMessage::UpdateGraphNameField { field: None, opens: false });
	}
}

pub fn edited_name(document: &DocumentMessageHandler) -> Option<(GraphName, GraphNameField)> {
	let name = document.node_graph_handler.graph_ui.editing_name?;
	Some((name, Facts::of(document).name_field(name)?))
}
