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

impl From<DocumentNodePersistentMetadataInputNames> for DocumentNodePersistentMetadataPropertiesRow {
	fn from(old: DocumentNodePersistentMetadataInputNames) -> Self {
		DocumentNodePersistentMetadataPropertiesRow {
			reference: old.reference,
			input_properties: Vec::new(),
			display_name: old.display_name,
			output_names: old.output_names,
			has_primary_output: old.has_primary_output,
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

impl From<DocumentNodePersistentMetadataPropertiesRow> for DocumentNodePersistentMetadataHasPrimaryOutput {
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
		DocumentNodePersistentMetadataHasPrimaryOutput {
			reference: old.reference,
			display_name: old.display_name,
			input_metadata: Vec::new(),
			output_names: old.output_names,
			has_primary_output: old.has_primary_output,
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

impl From<DocumentNodePersistentMetadataHasPrimaryOutput> for DocumentNodePersistentMetadataStringReference {
	fn from(old: DocumentNodePersistentMetadataHasPrimaryOutput) -> Self {
		DocumentNodePersistentMetadataStringReference {
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct DocumentNodePersistentMetadataStringReference {
	pub reference: Option<String>,
	#[serde(default)]
	pub display_name: String,
	pub input_metadata: Vec<InputMetadata>,
	pub output_names: Vec<String>,
	#[serde(default)]
	pub locked: bool,
	#[serde(default)]
	pub pinned: bool,
	pub node_type_metadata: NodeTypePersistentMetadata,
	pub network_metadata: Option<NodeNetworkMetadata>,
}

impl From<DocumentNodePersistentMetadataStringReference> for DocumentNodePersistentMetadata {
	fn from(mut old: DocumentNodePersistentMetadataStringReference) -> Self {
		if let Some(metadata) = old.network_metadata.as_mut() {
			metadata.persistent_metadata.reference = old.reference;
		}
		DocumentNodePersistentMetadata {
			display_name: old.display_name,
			input_metadata: old.input_metadata,
			output_names: old.output_names,
			locked: old.locked,
			pinned: old.pinned,
			collapsed: false,
			node_type_metadata: old.node_type_metadata,
			network_metadata: old.network_metadata,
		}
	}
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum DocumentNodePersistentMetadataVersioned {
	// Newest first
	Current(DocumentNodePersistentMetadata),
	StringReference(DocumentNodePersistentMetadataStringReference),
	HasPrimaryOutput(DocumentNodePersistentMetadataHasPrimaryOutput),
	PropertiesRow(DocumentNodePersistentMetadataPropertiesRow),
	InputNames(DocumentNodePersistentMetadataInputNames),
}

pub fn deserialize_node_persistent_metadata<'de, D>(deserializer: D) -> Result<DocumentNodePersistentMetadata, D::Error>
where
	D: serde::Deserializer<'de>,
{
	use serde::Deserialize;

	let value = Value::deserialize(deserializer)?;

	let versioned_document = serde_json::from_value::<DocumentNodePersistentMetadataVersioned>(value).map_err(serde::de::Error::custom)?;

	let current: DocumentNodePersistentMetadata = match versioned_document {
		DocumentNodePersistentMetadataVersioned::Current(v) => v,
		DocumentNodePersistentMetadataVersioned::StringReference(v) => {
			let v: DocumentNodePersistentMetadataStringReference = v;
			v.into()
		}
		DocumentNodePersistentMetadataVersioned::HasPrimaryOutput(v) => {
			let v: DocumentNodePersistentMetadataStringReference = v.into();
			v.into()
		}
		DocumentNodePersistentMetadataVersioned::PropertiesRow(v) => {
			let v: DocumentNodePersistentMetadataHasPrimaryOutput = v.into();
			let v: DocumentNodePersistentMetadataStringReference = v.into();
			v.into()
		}
		DocumentNodePersistentMetadataVersioned::InputNames(v) => {
			let v: DocumentNodePersistentMetadataPropertiesRow = v.into();
			let v: DocumentNodePersistentMetadataHasPrimaryOutput = v.into();
			let v: DocumentNodePersistentMetadataStringReference = v.into();
			v.into()
		}
	};

	Ok(current)
}
