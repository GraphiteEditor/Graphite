use crate::messages::portfolio::document::utility_types::network_interface::{DocumentNodePersistentMetadata, InputMetadata, InputPersistentMetadata, NodeNetworkMetadata, NodeTypePersistentMetadata};
use serde_json::Value;
use std::collections::HashMap;

/// Persistent metadata for each node in the network, which must be included when creating, serializing, and deserializing saving a node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodePersistentMetadataInputNames {
	pub reference: Option<String>,
	#[serde(default)]
	pub display_name: String,
	pub input_names: Vec<String>,
	pub output_names: Vec<String>,
	pub has_primary_output: bool,
	#[serde(default)]
	pub locked: bool,
	#[serde(default)]
	pub pinned: bool,
	pub node_type_metadata: NodeTypePersistentMetadata,
	pub network_metadata: Option<NodeNetworkMetadata>,
}

impl From<DocumentNodePersistentMetadataInputNames> for DocumentNodePersistentMetadata {
	fn from(old: DocumentNodePersistentMetadataInputNames) -> Self {
		DocumentNodePersistentMetadata {
			input_metadata: Vec::new(),
			reference: old.reference,
			display_name: old.display_name,
			output_names: old.output_names,
			locked: old.locked,
			pinned: old.pinned,
			node_type_metadata: old.node_type_metadata,
			network_metadata: old.network_metadata,
		}
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodePersistentMetadataPropertiesRow {
	pub reference: Option<String>,
	#[serde(default)]
	pub display_name: String,
	pub input_properties: Vec<PropertiesRow>,
	pub output_names: Vec<String>,
	pub has_primary_output: bool,
	#[serde(default)]
	pub locked: bool,
	#[serde(default)]
	pub pinned: bool,
	pub node_type_metadata: NodeTypePersistentMetadata,
	pub network_metadata: Option<NodeNetworkMetadata>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PropertiesRow {
	pub input_data: HashMap<String, Value>,
	pub widget_override: Option<String>,
	#[serde(skip)]
	pub input_name: String,
	#[serde(skip)]
	pub input_description: String,
}

impl From<DocumentNodePersistentMetadataPropertiesRow> for DocumentNodePersistentMetadata {
	fn from(old: DocumentNodePersistentMetadataPropertiesRow) -> Self {
		let mut input_metadata = Vec::new();
		for properties_row in old.input_properties {
			input_metadata.push(InputMetadata {
				persistent_metadata: InputPersistentMetadata {
					input_data: properties_row.input_data,
					widget_override: properties_row.widget_override,
					input_name: properties_row.input_name,
					input_description: properties_row.input_description,
				},
				..Default::default()
			})
		}
		DocumentNodePersistentMetadata {
			reference: old.reference,
			display_name: old.display_name,
			input_metadata: Vec::new(),
			output_names: old.output_names,
			locked: old.locked,
			pinned: old.pinned,
			node_type_metadata: old.node_type_metadata,
			network_metadata: old.network_metadata,
		}
	}
}

/// Persistent metadata for each node in the network, which must be included when creating, serializing, and deserializing saving a node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodePersistentMetadataHasPrimaryOutput {
	pub reference: Option<String>,
	#[serde(default)]
	pub display_name: String,
	pub input_metadata: Vec<InputMetadata>,
	pub output_names: Vec<String>,
	pub has_primary_output: bool,
	#[serde(default)]
	pub locked: bool,
	#[serde(default)]
	pub pinned: bool,
	pub node_type_metadata: NodeTypePersistentMetadata,
	pub network_metadata: Option<NodeNetworkMetadata>,
}

impl From<DocumentNodePersistentMetadataHasPrimaryOutput> for DocumentNodePersistentMetadata {
	fn from(old: DocumentNodePersistentMetadataHasPrimaryOutput) -> Self {
		DocumentNodePersistentMetadata {
			reference: old.reference,
			display_name: old.display_name,
			input_metadata: old.input_metadata,
			output_names: old.output_names,
			locked: old.locked,
			pinned: old.pinned,
			node_type_metadata: old.node_type_metadata,
			network_metadata: old.network_metadata,
		}
	}
}

pub fn deserialize_node_persistent_metadata<'de, D>(deserializer: D) -> Result<DocumentNodePersistentMetadata, D::Error>
where
	D: serde::Deserializer<'de>,
{
	use serde::Deserialize;

	let value = Value::deserialize(deserializer)?;
	if let Ok(document) = serde_json::from_value::<DocumentNodePersistentMetadataHasPrimaryOutput>(value.clone()) {
		return Ok(document.into());
	};
	if let Ok(document) = serde_json::from_value::<DocumentNodePersistentMetadata>(value.clone()) {
		return Ok(document);
	};
	if let Ok(document) = serde_json::from_value::<DocumentNodePersistentMetadataPropertiesRow>(value.clone()) {
		return Ok(document.into());
	};
	match serde_json::from_value::<DocumentNodePersistentMetadataInputNames>(value.clone()) {
		Ok(document) => Ok(document.into()),
		Err(e) => Err(serde::de::Error::custom(e)),
	}
}
