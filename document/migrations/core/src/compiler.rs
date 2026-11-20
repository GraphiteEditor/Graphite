use crate::{DeclarationResolver, Node};
use document_graph_storage::Registry;

/// A value type as the compiler reports it. Only ever compared within one run, so nothing about it is stored.
pub trait ValueType {
	/// The `type_name` of the element an `Item` or `List` holds, or of the type itself when it's neither. `None` for what isn't concrete.
	fn element_name(&self) -> Option<&str>;
}

impl dyn ValueType {
	/// Whether the elements of this value type are `T`.
	pub fn element_is<T: 'static>(&self) -> bool {
		// The compiler's types carry no `TypeId`, and compare by this same name
		self.element_name() == Some(std::any::type_name::<T>())
	}
}

/// Answers what flows through one state of a document. A migration asks for it with an `Option<&dyn Compiler>` parameter, and handles `None`.
pub trait Compiler {
	/// The value type slot `slot` of `node` carries, or `None` where what feeds it doesn't compile.
	fn input_type(&self, node: &Node, slot: usize) -> Option<Box<dyn ValueType>>;
}

/// What the caller opening a document offers migrations beyond the stored data.
pub trait MigrationHost: Sync {
	/// A compiler describing `state`, whose declarations `resolver` names. `None` runs the migrations asking for one on their fallback.
	fn compiler(&self, state: &Registry, resolver: &dyn DeclarationResolver) -> Option<Box<dyn Compiler + '_>>;
}
