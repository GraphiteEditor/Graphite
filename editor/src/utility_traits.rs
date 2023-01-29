pub use crate::dispatcher::*;
use crate::messages::prelude::*;

/// Implements a message handler struct for a separate message struct.
/// - The first generic argument (`M`) is that message struct type, representing a message enum variant to be matched and handled in `process_message()`.
/// - The second generic argument (`D`) is the type of data that can be passed along by the caller to `process_message()`.
pub trait MessageHandler<M: ToDiscriminant, D>
where
	M::Discriminant: AsMessage,
	<M::Discriminant as TransitiveChild>::TopParent: TransitiveChild<Parent = <M::Discriminant as TransitiveChild>::TopParent, TopParent = <M::Discriminant as TransitiveChild>::TopParent> + AsMessage,
{
	/// Return true if the Action is consumed.
	fn process_message(&mut self, message: M, responses: &mut VecDeque<Message>, data: D);

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
