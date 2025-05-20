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

pub trait HierarchicalTree {
	fn build_message_tree() -> DebugMessageTree;

	fn message_handler_data_str() -> MessageData {
		MessageData::new(String::new(), Vec::new())
	}

	fn message_handler_str() -> MessageData {
		MessageData::new(String::new(), Vec::new())
	}

	fn path() -> &'static str {
		""
	}
}

#[derive(Debug)]
pub struct MessageData {
	name: String,
	fields: Vec<(String, usize)>,
}

impl MessageData {
	pub fn new(name: String, fields: Vec<(String, usize)>) -> MessageData {
		MessageData { name, fields }
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn fields(&self) -> &Vec<(String, usize)> {
		&self.fields
	}
}

#[derive(Debug)]
pub struct DebugMessageTree {
	name: String,
	variants: Option<Vec<DebugMessageTree>>,
	message_handler: Option<MessageData>,
	message_handler_data: Option<MessageData>,
	path: &'static str,
}

impl DebugMessageTree {
	pub fn new(name: &str) -> DebugMessageTree {
		DebugMessageTree {
			name: name.to_string(),
			variants: None,
			message_handler: None,
			message_handler_data: None,
			path: "",
		}
	}

	pub fn add_path(&mut self, path: &'static str) {
		self.path = path;
	}

	pub fn add_variant(&mut self, variant: DebugMessageTree) {
		if let Some(variants) = &mut self.variants {
			variants.push(variant);
		} else {
			self.variants = Some(vec![variant]);
		}
	}

	pub fn add_message_handler_data_field(&mut self, message_handler_data: MessageData) {
		self.message_handler_data = Some(message_handler_data);
	}

	pub fn add_message_handler_field(&mut self, message_handler: MessageData) {
		self.message_handler = Some(message_handler);
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn path(&self) -> &'static str {
		self.path
	}

	pub fn variants(&self) -> Option<&Vec<DebugMessageTree>> {
		self.variants.as_ref()
	}

	pub fn message_handler_data_fields(&self) -> Option<&MessageData> {
		self.message_handler_data.as_ref()
	}

	pub fn message_handler_fields(&self) -> Option<&MessageData> {
		self.message_handler.as_ref()
	}

	pub fn has_message_handler_data_fields(&self) -> bool {
		match self.message_handler_data_fields() {
			Some(_) => true,
			None => false,
		}
	}

	pub fn has_message_handler_fields(&self) -> bool {
		match self.message_handler_fields() {
			Some(_) => true,
			None => false,
		}
	}
}
