#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum MessageLoggingVerbosity {
	#[default]
	Off,
	Names,
	Contents,
}

pub struct DebugMessageTree {
	name: String,
	variants: Option<Vec<DebugMessageTree>>,
}

impl DebugMessageTree {
	pub fn new(name: &str) -> DebugMessageTree {
		DebugMessageTree {
			name: name.to_string(),
			variants: None,
		}
	}

	pub fn add_variant(&mut self, variant: DebugMessageTree) {
		if let Some(variants) = &mut self.variants {
			variants.push(variant);
		} else {
			self.variants = Some(vec![variant]);
		}
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn variants(&self) -> Option<&Vec<DebugMessageTree>> {
		self.variants.as_ref()
	}
}
