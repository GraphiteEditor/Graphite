use super::storage_metadata::position_from_runtime;
use super::{DocumentNodeMetadata, DocumentNodePersistentMetadata, LayerPosition, NodePosition, NodeTypePersistentMetadata};
use super::{InputMetadata, InputPersistentMetadata};
use document_graph_storage::attr::network as network_attr;
use document_graph_storage::attr::node as node_attr;
use document_graph_storage::from_runtime::{ConversionError, DeclarationBytes};
use document_graph_storage::{AttributeDelta, Attributes, Implementation, NodeMetadataSource, PathResolver, Position, Registry, RegistryDelta, ScopedConversion, TimeStamp};
use document_graph_storage::{convert_input_attributes, convert_resource_entry, encode_input_ui_attributes, encode_node_ui_attributes, node_value_resource_refs, value_resource_ref};
use graph_craft::application_io::resource::{ResourceId, ResourceRegistry};
use graph_craft::document::NodeId;
use graph_craft::runtime_delta::RuntimeDelta;
use std::collections::{HashMap, HashSet};

/// A [`RuntimeDelta`] extended with the editor-only change kinds, which carry the `ui::*` metadata
/// the compiler has no use for. The compiler consumes only the `Graph` variant.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorDelta {
	Graph(RuntimeDelta),
	/// Every `ui::*` attribute of a node that has just been added, including everything nested under
	/// it, so one delta covers a group and its contents.
	///
	/// Asserting the whole set is sound here precisely because the node is new: every field was just
	/// written, so there is no concurrent peer holding state for it to clobber. An edit to an existing
	/// node must use [`EditorDelta::NodeMetadata`] instead.
	NodeMetadataSnapshot {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		metadata: Box<DocumentNodePersistentMetadata>,
	},
	/// One field of an existing node's metadata.
	NodeMetadata {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		change: NodeMetadataChange,
	},
	/// A node's whole input metadata array, paired with the `SetInputs` that changed the slots it
	/// indexes. Whole-array for the same reason: an insert shifts every later entry.
	NodeInputMetadata {
		network_path: Vec<NodeId>,
		node_id: NodeId,
		input_metadata: Vec<InputMetadata>,
	},
	/// One field of an existing network's metadata.
	NetworkMetadata {
		network_path: Vec<NodeId>,
		change: NetworkMetadataChange,
	},
}

/// One field of a node's persistent metadata, named the way the store writes it so each write has
/// exactly one delta. Every variant carries the post-change value, never a difference.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeMetadataChange {
	/// Whether the node is displayed as a layer and where it sits, which are one choice: a layer and a
	/// node do not have the same kinds of position. Encodes to `ui::position` and `ui::is_layer`, which
	/// merge independently, so a concurrent move and a concurrent layer toggle both survive.
	NodeType(NodeTypePersistentMetadata),
	DisplayName(String),
	Locked(bool),
	Pinned(bool),
	/// The whole vec, since the encoding is one array-valued attribute rather than a slot each.
	OutputNames(Vec<String>),
	InputName {
		index: usize,
		name: String,
	},
	WidgetOverride {
		index: usize,
		widget_override: Option<String>,
	},
}

/// One field of a network's persistent metadata. The navigation transform and width are absent
/// deliberately: they are per-peer view state, persisted to `session.json` rather than the registry.
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkMetadataChange {
	/// The definition the network was instantiated from, dropped once it is edited away from it.
	Reference(Option<String>),
	/// The whole order, since the encoding is one array-valued attribute rather than a slot each.
	PinnedOrder(Vec<NodeId>),
}

pub struct ConstructedOps {
	pub ops: Vec<RegistryDelta>,
	pub declarations: BatchDeclarations,
}

/// The proto-node declarations a batch extracted: the bytes for the caller's byte store, and the same
/// declarations decoded for its declaration cache, which a cursor rebuild reads instead of the bytes.
#[derive(Default)]
pub struct BatchDeclarations {
	pub bytes: DeclarationBytes,
	pub decoded: document_graph_storage::Declarations,
}

/// Constructs the storage ops for one gesture's deltas, in delta order. Removal closures and
/// resource liveness are computed against the whole batch, since several removals in one gesture
/// can jointly orphan a resource that each alone would not. Op timestamps are placeholders,
/// re-stamped by the staging clock.
///
/// `metadata` addresses each delta's node by the identity the interface holds for it, so the ops
/// land on the same nodes a whole-document conversion of that interface would produce.
pub fn construct_batch(
	deltas: &[EditorDelta],
	working: &Registry,
	resources: &ResourceRegistry,
	metadata: &dyn NodeMetadataSource,
	peer: document_graph_storage::PeerId,
) -> Result<ConstructedOps, ConversionError> {
	let identities = IdentitiesOnly(metadata);
	let context = ConversionContext {
		resources,
		identities: &identities,
		resolver: PathResolver::new(Some(&identities), peer),
		peer,
	};
	let mut batch = BatchRegistry::new(working);
	let mut ops = Vec::new();
	let mut declarations = BatchDeclarations::default();
	let mut batch_removed_nodes = Vec::new();

	// In delta order, so each delta reads what the ones before it did
	for delta in deltas {
		delta.construct(&context, &mut batch, &mut batch_removed_nodes, &mut ops, &mut declarations)?;
	}

	batch_removed_nodes.sort_by_key(|(id, _)| *id);
	batch_removed_nodes.dedup_by_key(|(id, _)| *id);
	construct_resource_removals(&batch_removed_nodes, &batch, &mut ops);
	Ok(ConstructedOps { ops, declarations })
}

/// What every delta in a batch converts against, fixed for the whole batch.
struct ConversionContext<'a> {
	resources: &'a ResourceRegistry,
	identities: &'a dyn NodeMetadataSource,
	resolver: PathResolver<'a>,
	peer: document_graph_storage::PeerId,
}

impl EditorDelta {
	fn construct(
		&self,
		context: &ConversionContext,
		batch: &mut BatchRegistry,
		batch_removed_nodes: &mut Vec<(document_graph_storage::NodeId, document_graph_storage::Node)>,
		ops: &mut Vec<RegistryDelta>,
		declarations: &mut BatchDeclarations,
	) -> Result<(), ConversionError> {
		match self {
			EditorDelta::Graph(RuntimeDelta::AddNode { network_path, node_id, node }) => {
				construct_structural_additions(network_path, *node_id, node, batch, context, ops, declarations)?;
			}

			EditorDelta::Graph(RuntimeDelta::ReplaceNode { network_path, node_id, node }) => {
				// Only what the previous implementation owned goes; the node stays so the addition below
				// updates it in place and its `ui::*` attributes survive the swap.
				construct_removals_below(context.resolver.node_id(network_path, *node_id), batch, batch_removed_nodes, ops);
				construct_structural_additions(network_path, *node_id, node, batch, context, ops, declarations)?;
			}

			EditorDelta::Graph(RuntimeDelta::RemoveNode { network_path, node_id }) => {
				construct_removals(context.resolver.node_id(network_path, *node_id), batch, batch_removed_nodes, ops);
			}

			EditorDelta::Graph(RuntimeDelta::SetVisibility { network_path, node_id, visible }) => {
				ops.push(RegistryDelta::ChangeNodeAttribute {
					id: context.resolver.node_id(network_path, *node_id),
					delta: AttributeDelta {
						key: node_attr::VISIBLE.to_string(),
						value: (!visible).then_some(serde_json::Value::Bool(false)),
					},
				});
			}

			EditorDelta::Graph(RuntimeDelta::SetInput {
				network_path,
				node_id,
				input_index,
				input,
			}) => {
				let new_input = context.resolver.convert_input_at(input, network_path)?;
				construct_referenced_resource(Some(context.resolver.node_id(network_path, *node_id)), &new_input, context, batch, ops)?;

				ops.push(RegistryDelta::ChangeNodeInput {
					id: context.resolver.node_id(network_path, *node_id),
					index: (*input_index).try_into().map_err(|_| ConversionError::IndexOverflow(*input_index))?,
					new_input,
				});
			}

			EditorDelta::Graph(RuntimeDelta::SetExport { network_path, export_index, input }) => {
				let export = input.as_ref().map(|input| context.resolver.convert_input_at(input, network_path)).transpose()?;
				if let Some(export) = export.as_ref() {
					construct_referenced_resource(None, export, context, batch, ops)?;
				}

				ops.push(RegistryDelta::SetNetworkExport {
					id: context.resolver.network_id(network_path),
					index: (*export_index).try_into().map_err(|_| ConversionError::IndexOverflow(*export_index))?,
					export,
				});
			}

			EditorDelta::Graph(RuntimeDelta::SetCallArgument { network_path, node_id, call_argument }) => {
				ops.push(RegistryDelta::ChangeNodeAttribute {
					id: context.resolver.node_id(network_path, *node_id),
					delta: document_graph_storage::from_runtime::encode_call_argument(call_argument)?,
				});
			}

			EditorDelta::Graph(RuntimeDelta::SetContextFeatures {
				network_path,
				node_id,
				context_features,
			}) => {
				ops.push(RegistryDelta::ChangeNodeAttribute {
					id: context.resolver.node_id(network_path, *node_id),
					delta: document_graph_storage::from_runtime::encode_context_features(context_features)?,
				});
			}

			EditorDelta::NodeMetadataSnapshot { network_path, node_id, metadata } => {
				construct_metadata_snapshot(network_path, *node_id, metadata, batch, &context.resolver, ops)?;
			}

			EditorDelta::Graph(RuntimeDelta::SetInputs { network_path, node_id, inputs }) => {
				// The slots' `ui::*` attributes arrive with the paired input metadata delta
				let inputs = inputs
					.iter()
					.map(|input| {
						Ok(document_graph_storage::InputSlot {
							input: context.resolver.convert_input_at(input, network_path)?,
							timestamp: TimeStamp::ORIGIN,
							attributes: convert_input_attributes(input)?,
						})
					})
					.collect::<Result<Vec<_>, ConversionError>>()?;

				for slot in &inputs {
					construct_referenced_resource(Some(context.resolver.node_id(network_path, *node_id)), &slot.input, context, batch, ops)?;
				}

				ops.push(RegistryDelta::SetNodeInputs {
					id: context.resolver.node_id(network_path, *node_id),
					inputs,
				});
			}

			EditorDelta::NodeMetadata { network_path, node_id, change } => {
				ops.extend(node_metadata_ops(context.resolver.node_id(network_path, *node_id), change)?);
			}

			EditorDelta::NodeInputMetadata {
				network_path,
				node_id,
				input_metadata,
			} => {
				let global_id = context.resolver.node_id(network_path, *node_id);
				let source = InputMetadataSource(input_metadata);
				let working_node = batch.node(global_id);

				for input_index in 0..input_metadata.len() {
					let mut encoded = Attributes::new();
					encode_input_ui_attributes(&mut encoded, &source, network_path, *node_id, input_index, TimeStamp::ORIGIN)?;
					let current = working_node.and_then(|node| node.inputs().get(input_index)).map(|slot| &slot.attributes);

					for delta in ui_attribute_writes(current, &encoded) {
						ops.push(RegistryDelta::ChangeNodeInputAttribute {
							id: global_id,
							index: input_index.try_into().map_err(|_| ConversionError::IndexOverflow(input_index))?,
							delta,
						});
					}
				}
			}

			EditorDelta::NetworkMetadata { network_path, change } => {
				let delta = match change {
					NetworkMetadataChange::Reference(reference) => AttributeDelta {
						key: node_attr::ui::REFERENCE.to_string(),
						value: reference.clone().map(serde_json::Value::String),
					},
					NetworkMetadataChange::PinnedOrder(order) => {
						let stored: Vec<_> = order.iter().map(|node_id| context.resolver.node_id(network_path, *node_id)).collect();
						AttributeDelta {
							key: network_attr::PINNED_ORDER.to_string(),
							value: (!stored.is_empty()).then(|| serialize_attribute(network_attr::PINNED_ORDER, &stored)).transpose()?,
						}
					}
				};

				ops.push(RegistryDelta::ChangeNetworkAttribute {
					id: context.resolver.network_id(network_path),
					delta,
				});
			}
		}

		Ok(())
	}
}

fn serialize_attribute<T: serde::Serialize>(key: &str, value: &T) -> Result<serde_json::Value, ConversionError> {
	serde_json::to_value(value).map_err(|error| ConversionError::SerializationError(format!("{key}: {error:?}")))
}

/// The attribute writes one metadata field makes, as the whole-document encoder would write them.
///
/// A field set back to its unset value clears the attribute, since absence is how that encoder spells
/// unset. Clearing rather than writing a sentinel keeps the two paths producing the same registry.
fn node_metadata_ops(global_id: document_graph_storage::NodeId, change: &NodeMetadataChange) -> Result<Vec<RegistryDelta>, ConversionError> {
	let node_attribute = |key: &str, value: Option<serde_json::Value>| RegistryDelta::ChangeNodeAttribute {
		id: global_id,
		delta: AttributeDelta { key: key.to_string(), value },
	};
	let input_attribute = |key: &str, index: usize, value: Option<serde_json::Value>| {
		Ok::<_, ConversionError>(RegistryDelta::ChangeNodeInputAttribute {
			id: global_id,
			index: index.try_into().map_err(|_| ConversionError::IndexOverflow(index))?,
			delta: AttributeDelta { key: key.to_string(), value },
		})
	};

	// The empty string is the runtime's own sentinel for an unset name
	let flag = |value: bool| value.then_some(serde_json::Value::Bool(true));
	let non_empty = |value: &str| (!value.is_empty()).then(|| serde_json::Value::String(value.to_string()));
	Ok(match change {
		NodeMetadataChange::NodeType(node_type) => {
			let position = position_from_runtime(node_type);
			vec![
				node_attribute(node_attr::ui::POSITION, Some(serialize_attribute(node_attr::ui::POSITION, &position)?)),
				node_attribute(node_attr::ui::IS_LAYER, flag(matches!(node_type, NodeTypePersistentMetadata::Layer(_)))),
			]
		}
		NodeMetadataChange::DisplayName(display_name) => vec![node_attribute(node_attr::ui::DISPLAY_NAME, non_empty(display_name))],
		NodeMetadataChange::Locked(locked) => vec![node_attribute(node_attr::ui::LOCKED, flag(*locked))],
		NodeMetadataChange::Pinned(pinned) => vec![node_attribute(node_attr::ui::PINNED, flag(*pinned))],
		NodeMetadataChange::OutputNames(output_names) => {
			let value = (!output_names.is_empty()).then(|| serialize_attribute(node_attr::ui::OUTPUT_NAMES, output_names)).transpose()?;
			vec![node_attribute(node_attr::ui::OUTPUT_NAMES, value)]
		}
		NodeMetadataChange::InputName { index, name } => vec![input_attribute(node_attr::input::ui::NAME, *index, non_empty(name))?],
		NodeMetadataChange::WidgetOverride { index, widget_override } => {
			let value = widget_override.as_deref().and_then(non_empty);
			vec![input_attribute(node_attr::input::ui::WIDGET_OVERRIDE, *index, value)?]
		}
	})
}

/// Converts through the same encoders as a whole-document conversion, with identities only as the
/// source: ui attributes arrive via the gesture's paired `NodeMetadata` delta.
fn construct_structural_additions(
	network_path: &[NodeId],
	node_id: NodeId,
	node: &graph_craft::document::DocumentNode,
	batch: &mut BatchRegistry,
	context: &ConversionContext,
	ops: &mut Vec<RegistryDelta>,
	declarations: &mut BatchDeclarations,
) -> Result<(), ConversionError> {
	let mut scoped = ScopedConversion::new(context.identities, context.peer);
	let mut scratch = Registry::default();
	scoped.convert_node_at(&mut scratch, network_path, node_id, node, true)?;
	let (bytes, decoded) = scoped.finish();
	declarations.bytes.extend(bytes);
	declarations.decoded.extend(decoded);

	let mut networks: Vec<_> = scratch.networks.iter().map(|(id, network)| (*id, network.clone())).collect();
	networks.sort_by_key(|(id, _)| *id);
	for (id, network) in networks {
		ops.push(RegistryDelta::AddNetwork { id, network: network.clone() });
		batch.record_network(id, network);
	}

	// Recorded as well as emitted, so a later delta in this batch can read these nodes back
	let mut nodes: Vec<_> = scratch.node_instances.iter().collect();
	nodes.sort_by_key(|(id, _)| **id);
	let added: Vec<_> = nodes.into_iter().map(|(id, node)| (*id, node.clone())).collect();
	for (id, node) in added {
		// A node the registry already holds is updated rather than rebuilt: rebuilding would clear the
		// `ui::*` attributes it carries, and restating those would clobber whatever a concurrent peer
		// wrote to its name, lock or pin.
		match batch.node(id).is_some() {
			true => {
				ops.push(RegistryDelta::SetNodeImplementation {
					id,
					implementation: node.implementation().clone(),
				});
				ops.push(RegistryDelta::SetNodeInputs { id, inputs: node.inputs().to_vec() });
			}
			false => ops.push(RegistryDelta::AddNode { id, node: node.clone() }),
		}
		batch.record_addition(id, node);
	}

	let mut new_resources: Vec<_> = scratch
		.resources
		.iter()
		.filter(|(id, _)| !batch.working.resources.contains_key(id))
		.map(|(id, entry)| (*id, entry.clone()))
		.collect();
	new_resources.sort_by_key(|(id, _)| *id);
	for (id, entry) in new_resources {
		if batch.record_resource(id, entry.clone()) {
			ops.push(RegistryDelta::AddResource { id, entry });
		}
	}
	let mut tagged: Vec<ResourceId> = scratch.node_instances.values().flat_map(node_value_resource_refs).collect();
	tagged.sort();
	tagged.dedup();
	for id in tagged {
		if batch.resource(id).is_none()
			&& !scratch.resources.contains_key(&id)
			&& let Some(entry) = convert_resource_entry(context.resources, id, context.peer)?
		{
			batch.record_resource(id, entry.clone());
			ops.push(RegistryDelta::AddResource { id, entry });
		}
	}

	Ok(())
}

/// Stages the resource a value input references, so setting an input to an asset persists the entry
/// alongside the reference rather than leaving the registry pointing at something it does not hold.
fn construct_referenced_resource(
	owner: Option<document_graph_storage::NodeId>,
	input: &document_graph_storage::NodeInput,
	context: &ConversionContext,
	batch: &mut BatchRegistry,
	ops: &mut Vec<RegistryDelta>,
) -> Result<(), ConversionError> {
	let Some(id) = value_resource_ref(input) else { return Ok(()) };
	if let Some(owner) = owner {
		batch.record_input_resource(owner, id);
	}
	if batch.resource(id).is_some() {
		return Ok(());
	}
	let Some(entry) = convert_resource_entry(context.resources, id, context.peer)? else {
		return Ok(());
	};

	batch.record_resource(id, entry.clone());
	ops.push(RegistryDelta::AddResource { id, entry });
	Ok(())
}

fn construct_metadata_snapshot(
	network_path: &[NodeId],
	node_id: NodeId,
	metadata: &DocumentNodePersistentMetadata,
	batch: &BatchRegistry,
	resolver: &PathResolver,
	ops: &mut Vec<RegistryDelta>,
) -> Result<(), ConversionError> {
	let source = MetadataCopySource {
		anchor_path: network_path,
		anchor_id: node_id,
		metadata,
	};

	let mut pending = vec![(network_path.to_vec(), node_id, metadata)];
	while let Some((path, id, node_metadata)) = pending.pop() {
		let global_id = resolver.node_id(&path, id);
		let working_node = batch.node(global_id);

		let mut encoded = Attributes::new();
		encode_node_ui_attributes(&mut encoded, &source, &path, id, TimeStamp::ORIGIN)?;
		for delta in ui_attribute_writes(working_node.map(|node| node.attributes()), &encoded) {
			ops.push(RegistryDelta::ChangeNodeAttribute { id: global_id, delta });
		}

		for input_index in 0..node_metadata.input_metadata.len() {
			let mut encoded = Attributes::new();
			encode_input_ui_attributes(&mut encoded, &source, &path, id, input_index, TimeStamp::ORIGIN)?;
			let current = working_node.and_then(|node| node.inputs().get(input_index)).map(|slot| &slot.attributes);
			for delta in ui_attribute_writes(current, &encoded) {
				ops.push(RegistryDelta::ChangeNodeInputAttribute {
					id: global_id,
					index: input_index.try_into().map_err(|_| ConversionError::IndexOverflow(input_index))?,
					delta,
				});
			}
		}

		if let Some(network_metadata) = &node_metadata.network_metadata {
			let mut nested_path = path.clone();
			nested_path.push(id);
			let network_id = resolver.network_id(&nested_path);

			let target = network_metadata.persistent_metadata.reference.clone().map(serde_json::Value::String);
			// Read through the batch rather than the pre-batch registry: a `ReplaceNode` earlier in this
			// batch rebuilds the network without a reference, so comparing against the old state would
			// suppress the write and leave the rebuilt network missing it.
			let current = batch
				.network(network_id)
				.and_then(|network| network.attributes.get(node_attr::ui::REFERENCE))
				.map(|value| value.value.clone());
			if current != target {
				ops.push(RegistryDelta::ChangeNetworkAttribute {
					id: network_id,
					delta: AttributeDelta {
						key: node_attr::ui::REFERENCE.to_string(),
						value: target,
					},
				});
			}

			// Restated alongside the reference, since a rebuild clears the network's attributes and the
			// order would otherwise be lost for every nested network the snapshot covers.
			let pinned_order: Vec<_> = network_metadata
				.persistent_metadata
				.pinned_node_order
				.iter()
				.map(|pinned| resolver.node_id(&nested_path, *pinned))
				.collect();
			ops.push(RegistryDelta::ChangeNetworkAttribute {
				id: network_id,
				delta: AttributeDelta {
					key: network_attr::PINNED_ORDER.to_string(),
					value: (!pinned_order.is_empty()).then(|| serialize_attribute(network_attr::PINNED_ORDER, &pinned_order)).transpose()?,
				},
			});

			for (child_id, child) in &network_metadata.persistent_metadata.node_metadata {
				pending.push((nested_path.clone(), *child_id, &child.persistent_metadata));
			}
		}
	}

	Ok(())
}

/// Every `ui::` attribute of `encoded`, plus a clear for each `ui::` key of `current` that `encoded`
/// does not carry. Values are compared only to decide what to clear, never to skip a write.
///
/// Restating a value that already matches looks redundant but is load-bearing: `current` is the
/// registry as it stood before the batch, and these writes follow an op in the same batch that
/// rebuilt what they describe. A value the pre-batch state agrees with may already have been cleared
/// by that op, so skipping it would leave the attribute missing.
fn ui_attribute_writes(current: Option<&Attributes>, encoded: &Attributes) -> Vec<AttributeDelta> {
	let owned = |key: &str| key.starts_with("ui::");
	let mut deltas = Vec::new();

	if let Some(current) = current {
		for key in current.keys() {
			if owned(key) && !encoded.contains_key(key) {
				deltas.push(AttributeDelta { key: key.clone(), value: None });
			}
		}
	}
	for (key, value) in encoded {
		deltas.push(AttributeDelta {
			key: key.clone(),
			value: Some(value.value.clone()),
		});
	}

	deltas.sort_by(|a, b| a.key.cmp(&b.key));
	deltas
}

/// Emits the removal of a node and everything nested under it, recording what was removed so the
/// batch's resource liveness accounts for all of it at the end.
fn construct_removals(
	node_id: document_graph_storage::NodeId,
	batch: &mut BatchRegistry,
	batch_removed_nodes: &mut Vec<(document_graph_storage::NodeId, document_graph_storage::Node)>,
	ops: &mut Vec<RegistryDelta>,
) {
	construct_removals_with(node_id, true, batch, batch_removed_nodes, ops)
}

/// Removes everything the node's implementation owns while leaving the node itself, for a swap that
/// keeps the node in place.
fn construct_removals_below(
	node_id: document_graph_storage::NodeId,
	batch: &mut BatchRegistry,
	batch_removed_nodes: &mut Vec<(document_graph_storage::NodeId, document_graph_storage::Node)>,
	ops: &mut Vec<RegistryDelta>,
) {
	construct_removals_with(node_id, false, batch, batch_removed_nodes, ops)
}

fn construct_removals_with(
	node_id: document_graph_storage::NodeId,
	include_node: bool,
	batch: &mut BatchRegistry,
	batch_removed_nodes: &mut Vec<(document_graph_storage::NodeId, document_graph_storage::Node)>,
	ops: &mut Vec<RegistryDelta>,
) {
	let (mut removed_nodes, mut removed_networks) = removal_closure(node_id, batch);

	removed_nodes.sort();
	removed_networks.sort();
	for id in &removed_nodes {
		if !include_node && *id == node_id {
			continue;
		}
		let Some(snapshot) = batch.node(*id).cloned() else { continue };
		ops.push(RegistryDelta::RemoveNode { id: *id, snapshot: snapshot.clone() });
		// Kept by value: a replacement later in this batch re-adds the same ID with different content, so
		// looking the node up again at the end would read what replaced it rather than what was removed.
		batch_removed_nodes.push((*id, snapshot));
		batch.record_removal(*id);
	}
	for id in &removed_networks {
		let Some(snapshot) = batch.network(*id) else { continue };
		ops.push(RegistryDelta::RemoveNetwork { id: *id, snapshot: snapshot.clone() });
	}
}

/// Emits removals for resources referenced only by the batch's removed nodes, checked after every
/// removal is known.
fn construct_resource_removals(batch_removed_nodes: &[(document_graph_storage::NodeId, document_graph_storage::Node)], batch: &BatchRegistry, ops: &mut Vec<RegistryDelta>) {
	// A node this batch wrote an input on and then removed orphans whatever that write referenced, which
	// the stored node does not mention.
	let mut candidates: Vec<ResourceId> = batch_removed_nodes
		.iter()
		.flat_map(|(id, _)| batch.input_resources.get(id).into_iter().flatten().copied())
		.chain(batch_removed_nodes.iter().map(|(_, node)| node).flat_map(|node| {
			let declaration = match node.implementation() {
				Implementation::ProtoNode(declaration) => Some(*declaration),
				Implementation::Network(_) => None,
			};
			declaration.into_iter().chain(node_value_resource_refs(node))
		}))
		.collect();
	candidates.sort();
	candidates.dedup();
	if candidates.is_empty() {
		return;
	}

	// Gathered in one pass so each candidate is a lookup rather than another scan of the whole registry
	let mut still_referenced: HashSet<ResourceId> = HashSet::new();
	for (id, node) in batch.live_nodes() {
		if let Implementation::ProtoNode(declaration) = node.implementation() {
			still_referenced.insert(*declaration);
		}
		still_referenced.extend(node_value_resource_refs(node));
		// What this batch wrote onto a node that survives it is a live reference, even though the stored
		// node still carries the input it had before.
		still_referenced.extend(batch.input_resources.get(&id).into_iter().flatten().copied());
	}
	// The batch's own networks count too: one added earlier in it can export a resource the removal would
	// otherwise see as orphaned.
	for network in batch.working.networks.values().chain(batch.added_networks.values()) {
		still_referenced.extend(network.exports.iter().filter_map(|slot| slot.target.as_ref().and_then(value_resource_ref)));
	}

	for candidate in candidates {
		if !still_referenced.contains(&candidate)
			&& let Some(entry) = batch.resource(candidate)
		{
			ops.push(RegistryDelta::RemoveResource {
				id: candidate,
				snapshot: entry.clone(),
			});
		}
	}
}

/// The registry a delta is constructed against: what the batch started from, plus the nodes deltas
/// earlier in the same batch have added, since a delta can read back or remove a node an earlier one
/// added.
///
/// Nodes are also indexed by network, so a removal closure walks only the subtree it removes instead
/// of rescanning every node for each nested network it meets.
struct BatchRegistry<'a> {
	working: &'a Registry,
	added: HashMap<document_graph_storage::NodeId, document_graph_storage::Node>,
	added_networks: HashMap<document_graph_storage::NetworkId, document_graph_storage::Network>,
	added_resources: HashMap<ResourceId, document_graph_storage::ResourceEntry>,
	by_network: HashMap<document_graph_storage::NetworkId, Vec<document_graph_storage::NodeId>>,
	/// Nodes the batch has removed and not since re-added, so a lookup answers with the state the ops so
	/// far have produced rather than the state the batch started from.
	removed: HashSet<document_graph_storage::NodeId>,
	/// Resources the batch's own input writes reference, keyed by the node written to. The stored node
	/// still holds its pre-batch inputs, so liveness would otherwise miss both the reference these writes
	/// create and the orphan they leave when that node is later removed.
	input_resources: HashMap<document_graph_storage::NodeId, HashSet<ResourceId>>,
}

impl<'a> BatchRegistry<'a> {
	fn new(working: &'a Registry) -> Self {
		let mut by_network: HashMap<_, Vec<_>> = HashMap::new();
		for (id, node) in &working.node_instances {
			by_network.entry(node.network()).or_default().push(*id);
		}
		Self {
			working,
			added: HashMap::new(),
			added_networks: HashMap::new(),
			added_resources: HashMap::new(),
			by_network,
			removed: HashSet::new(),
			input_resources: HashMap::new(),
		}
	}

	/// Records that an input write on `owner` references `resource`.
	fn record_input_resource(&mut self, owner: document_graph_storage::NodeId, resource: ResourceId) {
		self.input_resources.entry(owner).or_default().insert(resource);
	}

	/// Records a node an earlier delta in this batch added, so a later one can read it back.
	fn record_addition(&mut self, id: document_graph_storage::NodeId, node: document_graph_storage::Node) {
		self.by_network.entry(node.network()).or_default().push(id);
		self.removed.remove(&id);
		self.added.insert(id, node);
	}

	/// Records a node this batch removed, so a later delta reading it back finds it gone.
	fn record_removal(&mut self, id: document_graph_storage::NodeId) {
		self.added.remove(&id);
		self.removed.insert(id);
	}

	/// Records a network an earlier delta in this batch added, so removing the node that owns it finds
	/// the network to remove alongside it.
	fn record_network(&mut self, id: document_graph_storage::NetworkId, network: document_graph_storage::Network) {
		self.added_networks.insert(id, network);
	}

	/// Records a resource this batch added. Returns whether it is new to the batch, so the caller emits
	/// one addition for a resource several nodes reference.
	fn record_resource(&mut self, id: ResourceId, entry: document_graph_storage::ResourceEntry) -> bool {
		self.added_resources.insert(id, entry).is_none()
	}

	fn node(&self, id: document_graph_storage::NodeId) -> Option<&document_graph_storage::Node> {
		if self.removed.contains(&id) {
			return None;
		}
		self.added.get(&id).or_else(|| self.working.node_instances.get(&id))
	}

	/// Every node the batch leaves in place, which is what the working registry held plus what the batch
	/// added, less what it removed.
	fn live_nodes(&self) -> impl Iterator<Item = (document_graph_storage::NodeId, &document_graph_storage::Node)> {
		self.working
			.node_instances
			.keys()
			.chain(self.added.keys())
			.copied()
			.collect::<HashSet<_>>()
			.into_iter()
			.filter_map(|id| Some((id, self.node(id)?)))
	}

	fn network(&self, id: document_graph_storage::NetworkId) -> Option<&document_graph_storage::Network> {
		self.added_networks.get(&id).or_else(|| self.working.networks.get(&id))
	}

	fn resource(&self, id: ResourceId) -> Option<&document_graph_storage::ResourceEntry> {
		self.added_resources.get(&id).or_else(|| self.working.resources.get(&id))
	}

	fn children(&self, network_id: document_graph_storage::NetworkId) -> &[document_graph_storage::NodeId] {
		self.by_network.get(&network_id).map_or(&[], Vec::as_slice)
	}
}

/// The node together with everything nested under it, and the networks that nesting owns.
fn removal_closure(node_id: document_graph_storage::NodeId, batch: &BatchRegistry) -> (Vec<document_graph_storage::NodeId>, Vec<document_graph_storage::NetworkId>) {
	let mut nodes = Vec::new();
	let mut networks = Vec::new();
	collect_removal_closure(node_id, batch, &mut nodes, &mut networks);
	(nodes, networks)
}

fn collect_removal_closure(node_id: document_graph_storage::NodeId, batch: &BatchRegistry, nodes: &mut Vec<document_graph_storage::NodeId>, networks: &mut Vec<document_graph_storage::NetworkId>) {
	let Some(node) = batch.node(node_id) else { return };
	nodes.push(node_id);

	if let &Implementation::Network(network_id) = node.implementation() {
		networks.push(network_id);
		for &child_id in batch.children(network_id) {
			collect_removal_closure(child_id, batch, nodes, networks);
		}
	}
}

/// Narrows a metadata source to the node identities alone, so a scoped conversion addresses nodes the
/// way a whole-document conversion does without also re-encoding ui attributes the gesture's paired
/// `NodeMetadata` delta already carries.
struct IdentitiesOnly<'a>(&'a dyn NodeMetadataSource);

impl NodeMetadataSource for IdentitiesOnly<'_> {
	fn storage_node_id(&self, network_path: &[NodeId], local_id: NodeId) -> Option<document_graph_storage::NodeId> {
		self.0.storage_node_id(network_path, local_id)
	}
}

/// Serves one node's input metadata for encoding a [`EditorDelta::NodeInputMetadata`] delta. Only the
/// per-input accessors answer, since the node-level ones are never reached: the delta encodes through
/// `encode_input_ui_attributes` alone.
struct InputMetadataSource<'a>(&'a [InputMetadata]);

impl InputMetadataSource<'_> {
	fn persistent(&self, input_index: usize) -> Option<&InputPersistentMetadata> {
		self.0.get(input_index).map(|metadata| &metadata.persistent_metadata)
	}
}

impl NodeMetadataSource for InputMetadataSource<'_> {
	fn input_name(&self, _network_path: &[NodeId], _local_id: NodeId, input_index: usize) -> Option<&str> {
		Some(self.persistent(input_index)?.input_name.as_str())
	}

	fn input_description(&self, _network_path: &[NodeId], _local_id: NodeId, input_index: usize) -> Option<&str> {
		Some(self.persistent(input_index)?.input_description.as_str())
	}

	fn widget_override(&self, _network_path: &[NodeId], _local_id: NodeId, input_index: usize) -> Option<&str> {
		self.persistent(input_index)?.widget_override.as_deref()
	}

	fn input_data(&self, _network_path: &[NodeId], _local_id: NodeId, input_index: usize) -> HashMap<String, serde_json::Value> {
		self.persistent(input_index).map(|metadata| metadata.input_data.clone()).unwrap_or_default()
	}
}

/// Serves a metadata copy as the [`document_graph_storage::NodeMetadataSource`] for its own
/// encoding, resolving requested paths relative to the anchor node the copy was taken from.
struct MetadataCopySource<'a> {
	anchor_path: &'a [NodeId],
	anchor_id: NodeId,
	metadata: &'a DocumentNodePersistentMetadata,
}

impl MetadataCopySource<'_> {
	fn metadata_for(&self, metadata_path: &[NodeId], node_id: NodeId) -> Option<&DocumentNodePersistentMetadata> {
		let relative = metadata_path.strip_prefix(self.anchor_path)?;

		let (mut current, rest) = match relative.split_first() {
			None => return (node_id == self.anchor_id).then_some(self.metadata),
			Some((first, rest)) if *first == self.anchor_id => (self.metadata, rest),
			Some(_) => return None,
		};

		for step in rest {
			current = child_metadata(current, *step)?;
		}
		child_metadata(current, node_id)
	}
}

fn child_metadata(metadata: &DocumentNodePersistentMetadata, child_id: NodeId) -> Option<&DocumentNodePersistentMetadata> {
	metadata
		.network_metadata
		.as_ref()
		.and_then(|network| network.persistent_metadata.node_metadata.get(&child_id))
		.map(|child: &DocumentNodeMetadata| &child.persistent_metadata)
}

impl document_graph_storage::NodeMetadataSource for MetadataCopySource<'_> {
	fn position(&self, metadata_path: &[NodeId], node_id: NodeId) -> Option<Position> {
		match &self.metadata_for(metadata_path, node_id)?.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer) => Some(match layer.position {
				LayerPosition::Absolute(offset) => Position::Absolute([offset.x, offset.y]),
				LayerPosition::Stack(offset) => Position::Stack(offset),
			}),
			NodeTypePersistentMetadata::Node(node) => Some(match *node.position() {
				NodePosition::Absolute(offset) => Position::Absolute([offset.x, offset.y]),
				NodePosition::Chain => Position::Chain,
			}),
		}
	}

	fn is_layer(&self, metadata_path: &[NodeId], node_id: NodeId) -> bool {
		self.metadata_for(metadata_path, node_id)
			.is_some_and(|metadata| matches!(metadata.node_type_metadata, NodeTypePersistentMetadata::Layer(_)))
	}

	fn display_name(&self, metadata_path: &[NodeId], node_id: NodeId) -> Option<&str> {
		self.metadata_for(metadata_path, node_id).map(|metadata| metadata.display_name.as_str())
	}

	fn locked(&self, metadata_path: &[NodeId], node_id: NodeId) -> bool {
		self.metadata_for(metadata_path, node_id).is_some_and(|metadata| metadata.locked)
	}

	fn pinned(&self, metadata_path: &[NodeId], node_id: NodeId) -> bool {
		self.metadata_for(metadata_path, node_id).is_some_and(|metadata| metadata.pinned)
	}

	fn output_names(&self, metadata_path: &[NodeId], node_id: NodeId) -> Vec<String> {
		self.metadata_for(metadata_path, node_id).map(|metadata| metadata.output_names.clone()).unwrap_or_default()
	}

	fn input_name(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> Option<&str> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.map(|input| input.persistent_metadata.input_name.as_str())
	}

	fn input_description(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> Option<&str> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.map(|input| input.persistent_metadata.input_description.as_str())
	}

	fn widget_override(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> Option<&str> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.and_then(|input| input.persistent_metadata.widget_override.as_deref())
	}

	fn input_data(&self, metadata_path: &[NodeId], node_id: NodeId, input_index: usize) -> std::collections::HashMap<String, serde_json::Value> {
		self.metadata_for(metadata_path, node_id)
			.and_then(|metadata| metadata.input_metadata.get(input_index))
			.map(|input| input.persistent_metadata.input_data.clone())
			.unwrap_or_default()
	}

	fn reference(&self, network_path: &[NodeId]) -> Option<&str> {
		let (owner_path, owner_id) = network_path.split_last().map(|(last, rest)| (rest, *last))?;
		self.metadata_for(owner_path, owner_id)?
			.network_metadata
			.as_ref()
			.and_then(|network| network.persistent_metadata.reference.as_deref())
	}
}
