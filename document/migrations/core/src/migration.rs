use crate::{AnchorContext, Declaration, Edits, MigrationError, Region};

/// Steps every node of one declaration to its successor. Implemented by `#[migration]` and `migration!`, not by hand.
pub trait NodeMigration {
	/// The declaration whose nodes are anchors.
	fn source(&self) -> Declaration;
	/// The declaration the runner switches anchors to.
	fn target(&self) -> Declaration;
	/// Declarations the body names, which order it after the migration producing each and before the one consuming it.
	fn pins(&self) -> &'static [Declaration] {
		&[]
	}
	/// The union of the regions the migration's parameters read and edit.
	fn footprint(&self) -> Region;
	/// Run on one anchor, returning what its views recorded.
	fn run(&self, context: &AnchorContext) -> Result<Edits, MigrationError>;
}

/// Switches anchors to another declaration and changes nothing else.
pub struct Rename {
	source: Declaration,
	target: Declaration,
}

impl Rename {
	pub const fn new(source: Declaration, target: Declaration) -> Self {
		Self { source, target }
	}
}

impl NodeMigration for Rename {
	fn source(&self) -> Declaration {
		self.source
	}

	fn target(&self) -> Declaration {
		self.target
	}

	fn footprint(&self) -> Region {
		Region::ANCHOR
	}

	fn run(&self, _context: &AnchorContext) -> Result<Edits, MigrationError> {
		Ok(Edits::default())
	}
}

/// Declarative migrations: `migration!(rename "old::node"@1 => "new::node"@2)`.
#[macro_export]
macro_rules! migration {
	(rename $source:literal @ $source_version:literal => $target:literal @ $target_version:literal) => {
		$crate::Rename::new($crate::decl!($source @ $source_version), $crate::decl!($target @ $target_version))
	};
}
