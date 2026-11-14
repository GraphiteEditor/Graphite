use document_graph_storage::{Attributes, NetworkReference, ProtoNode, ResourceEntry, ResourceId};
use serde::Serialize;

/// What a declaration describes: a proto node, or a built-in network definition that instances copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub enum DeclarationKind {
	#[default]
	Proto,
	Network,
}

impl DeclarationKind {
	fn is_proto(&self) -> bool {
		*self == Self::Proto
	}
}

/// A declaration at one version, written `identifier@version`, or `network identifier@version` for a network definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct Declaration {
	pub identifier: &'static str,
	pub version: u32,
	/// Skipped for proto nodes, so the IDs derived from declarations predating network definitions stay the same.
	#[serde(skip_serializing_if = "DeclarationKind::is_proto")]
	pub kind: DeclarationKind,
}

impl Declaration {
	pub const fn new(identifier: &'static str, version: u32) -> Self {
		Self {
			identifier,
			version,
			kind: DeclarationKind::Proto,
		}
	}

	pub const fn network(identifier: &'static str, version: u32) -> Self {
		Self {
			identifier,
			version,
			kind: DeclarationKind::Network,
		}
	}

	pub fn is_network(&self) -> bool {
		self.kind == DeclarationKind::Network
	}

	/// Whether a network's attributes name this network definition.
	pub fn describes_network(&self, attributes: &Attributes) -> bool {
		NetworkReference::read(attributes).is_some_and(|reference| self.is_reference(&reference))
	}

	/// Whether `reference` names this network definition.
	pub fn is_reference(&self, reference: &NetworkReference) -> bool {
		self.is_network() && reference.identifier == self.identifier && reference.version == self.version
	}

	/// The reference a network of this definition stores. Meaningless for a proto node.
	pub fn network_reference(&self) -> NetworkReference {
		NetworkReference {
			identifier: self.identifier.to_string(),
			version: self.version,
		}
	}
}

impl std::fmt::Display for Declaration {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self.kind {
			DeclarationKind::Proto => write!(f, "{}@{}", self.identifier, self.version),
			DeclarationKind::Network => write!(f, "network {}@{}", self.identifier, self.version),
		}
	}
}

/// Names a declaration as `decl!("module::node"@2)`, or a network definition as `decl!(network "Merge"@2)`.
#[macro_export]
macro_rules! decl {
	(network $identifier:literal @ $version:literal) => {
		$crate::Declaration::network($identifier, $version)
	};
	($identifier:literal @ $version:literal) => {
		$crate::Declaration::new($identifier, $version)
	};
}

/// Maps the declaration resources nodes reference to the declarations they hold.
pub trait DeclarationResolver {
	/// The declaration a proto-node resource holds, if it is one a migration knows.
	fn declaration(&self, resource: ResourceId) -> Option<Declaration>;
	/// The resource a node of `declaration` references.
	fn resource(&self, declaration: Declaration) -> ResourceId;
	/// The registry entry of that resource, added when a migration first references it. `None` leaves the registry's resources alone.
	fn resource_entry(&self, _declaration: Declaration) -> Option<ResourceEntry> {
		None
	}
	/// What a resource this resolver knows decodes to, for converting a state to a runtime network.
	fn proto(&self, _resource: ResourceId) -> Option<ProtoNode> {
		None
	}
}
