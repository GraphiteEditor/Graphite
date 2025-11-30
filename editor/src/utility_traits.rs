pub use crate::dispatcher::*;
use crate::messages::prelude::*;

/// Implements a message handler struct for a separate message struct.
/// - The first type argument (`M`) is that message struct type, representing a message enum variant to be matched and handled in `process_message()`.
/// - The second type argument (`C`) is the type of the context struct that can be passed along by the caller to `process_message()`.
pub trait MessageHandler<M: ToDiscriminant, C>
where
	M::Discriminant: AsMessage,
	<M::Discriminant as TransitiveChild>::TopParent: TransitiveChild<Parent = <M::Discriminant as TransitiveChild>::TopParent, TopParent = <M::Discriminant as TransitiveChild>::TopParent> + AsMessage,
{
	fn process_message(&mut self, message: M, responses: &mut VecDeque<Message>, context: C);

	fn actions(&self) -> ActionList;
}

pub type ActionList = Vec<Vec<MessageDiscriminant>>;

pub trait AsMessage: TransitiveChild
where
	Self::TopParent: TransitiveChild<Parent = Self::TopParent, TopParent = Self::TopParent> + AsMessage,
{
	fn local_name(self) -> String;
	fn global_name(self) -> String {
		<Self as Into<Self::TopParent>>::into(self).local_name()
	}
}

// TODO: Add Send + Sync requirement
// Use something like rw locks for synchronization
pub trait MessageHandlerData {}

pub trait ToDiscriminant {
	type Discriminant;

	fn to_discriminant(&self) -> Self::Discriminant;
}

pub trait TransitiveChild: Into<Self::Parent> + Into<Self::TopParent> {
	type TopParent;
	type Parent;
}

pub trait Hint {
	fn hints(&self) -> HashMap<String, String>;
}

pub trait HierarchicalTree {
	fn build_message_tree() -> DebugMessageTree;

	fn message_handler_data_str() -> MessageData {
		MessageData::new(String::new(), Vec::new(), "", 0)
	}

	fn message_handler_str() -> MessageData {
		MessageData::new(String::new(), Vec::new(), "", 0)
	}

	fn path() -> &'static str {
		""
	}
}

pub trait ExtractField {
	fn field_types() -> Vec<(String, usize)>;
	fn path() -> &'static str;
	fn line_number() -> usize;
	fn print_field_types();
}
