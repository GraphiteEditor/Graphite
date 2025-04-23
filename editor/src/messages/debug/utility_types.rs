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
	data: Option<MessageData>,
}

impl DebugMessageTree {
	pub fn new(name: &str) -> DebugMessageTree {
		DebugMessageTree {
			name: name.to_string(),
			variants: None,
			data: None,
		}
	}

	pub fn add_variant(&mut self, variant: DebugMessageTree) {
		if let Some(variants) = &mut self.variants {
			variants.push(variant);
		} else {
			self.variants = Some(vec![variant]);
		}
	}

	pub fn add_data_field(&mut self, name: String, fields: Vec<String>) {
		self.data = Some(MessageData { name, fields });
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn variants(&self) -> Option<&Vec<DebugMessageTree>> {
		self.variants.as_ref()
	}

	pub fn data_fields(&self) -> Option<&MessageData> {
		self.data.as_ref()
	}
}
