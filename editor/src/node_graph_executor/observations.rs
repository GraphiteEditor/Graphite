use super::{InspectResult, Observation, ObservedConnector};
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use graph_craft::document::NodeId;
use graphene_std::list::{Item, List};
use graphene_std::vector::{Vector, VectorGeometry, path, paths_from_list_row_attributes};
use graphene_std::{ContextOverrides, NodeParameter};
use std::collections::HashMap;

/// Names an observation so it can be changed or stopped without touching the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ObservationKey {
	/// The node the Data panel inspects.
	DataPanel,
	/// The content of a 'Path' node, which is the geometry the Pen and Path tools edit.
	PathContent(NodeId),
	// TODO: Eventually remove this document upgrade code
	/// The "Fill" node whose geometry a gradient migration measures.
	FillMeasurement,
}

pub(super) fn path_content_observations(network_interface: &NodeNetworkInterface) -> impl Iterator<Item = (ObservationKey, Observation)> {
	let path_nodes = network_interface.node_paths_with_reference(&DefinitionIdentifier::ProtoNode(path::IDENTIFIER));

	path_nodes.into_iter().filter_map(|node_path| {
		let key = ObservationKey::PathContent(*node_path.last()?);
		let observation = Observation {
			node_path,
			connector: ObservedConnector::Input(path::ContentInput::INDEX),
			overrides: ContextOverrides::default(),
		};
		Some((key, observation))
	})
}

/// The incoming geometry of each 'Path' node, read from its observed content.
pub(super) fn path_contents(observations: &HashMap<ObservationKey, InspectResult>) -> HashMap<NodeId, VectorGeometry> {
	observations
		.iter()
		.filter_map(|(key, result)| {
			let ObservationKey::PathContent(path_node) = *key else { return None };
			let value = result.data.as_ref()?;

			let geometry = match value.downcast_ref::<List<Vector>>() {
				Some(list) => VectorGeometry::new(list.element(0).cloned().unwrap_or_default(), paths_from_list_row_attributes(list, 0).cloned().unwrap_or_default()),
				None => VectorGeometry::from_item(value.downcast_ref::<Item<Vector>>()?),
			};
			Some((path_node, geometry))
		})
		.collect()
}
