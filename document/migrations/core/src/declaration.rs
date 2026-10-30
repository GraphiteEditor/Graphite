use document_graph_storage::{ResourceEntry, ResourceId};
use serde::Serialize;

/// A proto-node declaration at one version, written `identifier@version`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct Declaration {
	pub identifier: &'static str,
	pub version: u32,
}

impl Declaration {
	pub const fn new(identifier: &'static str, version: u32) -> Self {
		Self { identifier, version }
	}
}

impl std::fmt::Display for Declaration {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}@{}", self.identifier, self.version)
	}
}

/// Names a declaration as `decl!("module::node"@2)`.
#[macro_export]
macro_rules! decl {
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
}
