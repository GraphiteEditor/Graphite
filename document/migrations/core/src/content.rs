use crate::{MigrationError, MigrationId};
use graph_storage::{Node, NodeId, Registry, ResourceId};

/// Identity of a proto-node declaration, as read from the declaration resource body.
#[derive(Clone, Debug)]
pub struct DeclarationInfo {
	pub identifier: String,
	/// Explicit behavioral version; 0 for declarations predating versioning.
	pub version: u32,
}

/// Which registry entities a content migration applies to.
#[derive(Clone, Debug)]
pub enum Selector {
	/// Every node whose proto-node declaration matches.
	Node(NodeSelector),
	/// Every node carrying this `ui::reference` attribute value (how legacy wrapper networks
	/// like "Brush" are identified).
	Reference(&'static str),
	/// Once per document, gated by the [`crate::APPLIED_ATTRIBUTE`] provenance list.
	Document,
}

/// Matches nodes by declaration identifier (with historic aliases) and version range.
#[derive(Clone, Debug)]
pub struct NodeSelector {
	/// Current declaration identifier plus historic aliases.
	pub names: &'static [&'static str],
	/// Upgrade target: nodes with a declaration version strictly below this match.
	pub below_version: u32,
}

impl NodeSelector {
	/// Whether a node with this declaration identity is matched.
	pub fn matches(&self, declaration: &DeclarationInfo) -> bool {
		// TODO(Dennis): Matching semantics to decide here:
		// - Legacy identifiers can carry generic type arguments (`...MemoNode<T>`); the old system
		//   compared with `identifier.split('<').next()`. Strip them, or require exact matches?
		// - Should `below_version` also gate name-alias matches, or do aliases (which only appear in
		//   pre-versioning documents, version 0) always match regardless?
		let _ = declaration;
		todo!("decide and implement declaration matching")
	}
}

/// One matched entity, produced by scanning a [`Selector`] over the registry.
#[derive(Copy, Clone, Debug)]
pub enum Target {
	Node(NodeId),
	Document,
}

/// Byte-store and catalog services only the host (editor or CLI) can provide.
pub trait MigrationHost {
	/// Read a declaration resource's identity (identifier and version).
	fn declaration_info(&self, id: ResourceId) -> Option<DeclarationInfo>;
	/// The decoded declaration body, for inspection beyond the identity.
	fn declaration(&self, id: ResourceId) -> Option<serde_json::Value>;
	/// Instantiate the current catalog default node for a declaration identifier.
	fn resolve_definition(&mut self, identifier: &str) -> Option<Node>;
}

/// Everything a content migration can reach beyond the registry: host services plus ID minting.
pub trait MigrationContext: MigrationHost {
	/// Mint a fresh peer-scoped node ID for inserted nodes.
	fn mint_node_id(&mut self) -> NodeId;
}

/// One node-usage upgrade within the current format version.
///
/// `migrate` mutates a registry clone in place; the mutation is diffed into deltas and committed to
/// history as one retired gesture, so implementations never construct deltas by hand. Timestamps
/// written into the clone are placeholders the commit path re-stamps. Implementations must be
/// idempotent: a document can round-trip through an editor build that lacks a later migration.
pub trait ContentMigration {
	/// Stable identifier recorded in provenance.
	fn id(&self) -> MigrationId;
	/// Which registry entities to run on.
	fn selector(&self) -> Selector;
	/// Upgrade one matched target in place.
	fn migrate(&self, target: Target, registry: &mut Registry, context: &mut dyn MigrationContext) -> Result<(), MigrationError>;
}
