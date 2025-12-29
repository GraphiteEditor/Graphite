#[derive(Debug)]
pub struct MessageData {
	name: String,
	fields: Vec<(String, usize)>,
	path: &'static str,
	line_number: usize,
}

impl MessageData {
	pub fn new(name: String, fields: Vec<(String, usize)>, path: &'static str, line_number: usize) -> MessageData {
		MessageData { name, fields, path, line_number }
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn fields(&self) -> &Vec<(String, usize)> {
		&self.fields
	}

	pub fn path(&self) -> &'static str {
		self.path
	}

	pub fn line_number(&self) -> usize {
		self.line_number
	}
}

#[derive(Debug)]
pub struct DebugMessageTree {
	name: String,
	fields: Option<Vec<String>>,
	variants: Option<Vec<DebugMessageTree>>,
	message_handler: Option<MessageData>,
	message_handler_data: Option<MessageData>,
	path: &'static str,
	line_number: usize,
}

impl DebugMessageTree {
	pub fn new(name: &str) -> DebugMessageTree {
		DebugMessageTree {
			name: name.to_string(),
			fields: None,
			variants: None,
			message_handler: None,
			message_handler_data: None,
			path: "",
			line_number: 0,
		}
	}

	pub fn add_fields(&mut self, fields: Vec<String>) {
		self.fields = Some(fields);
	}

	pub fn set_path(&mut self, path: &'static str) {
		self.path = path;
	}

	pub fn set_line_number(&mut self, line_number: usize) {
		self.line_number = line_number
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

	pub fn fields(&self) -> Option<&Vec<String>> {
		self.fields.as_ref()
	}

	pub fn path(&self) -> &'static str {
		self.path
	}

	pub fn line_number(&self) -> usize {
		self.line_number
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
}
