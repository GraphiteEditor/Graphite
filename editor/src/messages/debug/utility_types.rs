#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum MessageLoggingVerbosity {
	#[default]
	Off,
	Names,
	Contents,
}

#[derive(Debug)]
pub struct MessageData {
	name: String,
	fields: Vec<String>,
}

impl MessageData {
	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn fields(&self) -> &Vec<String> {
		&self.fields
	}
}

#[derive(Debug)]
pub struct DebugMessageTree {
	name: String,
	variants: Option<Vec<DebugMessageTree>>,
	message_handler: Option<MessageData>,
	message_handler_data: Option<MessageData>,
}

impl DebugMessageTree {
	pub fn new(name: &str) -> DebugMessageTree {
		DebugMessageTree {
			name: name.to_string(),
			variants: None,
			message_handler: None,
			message_handler_data: None,
		}
	}

	pub fn add_variant(&mut self, variant: DebugMessageTree) {
		if let Some(variants) = &mut self.variants {
			variants.push(variant);
		} else {
			self.variants = Some(vec![variant]);
		}
	}

	pub fn add_message_handler_data_field(&mut self, name: String, fields: Vec<String>) {
		self.message_handler_data = Some(MessageData { name, fields });
	}

	pub fn add_message_handler_field(&mut self, name: String, fields: Vec<String>) {
		self.message_handler = Some(MessageData { name, fields });
	}

	pub fn name(&self) -> &str {
		&self.name
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
