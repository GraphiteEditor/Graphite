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

// ============================================================

#[cfg(test)]
mod tests {
	use super::*;

	// Helper: creates a standard MessageData for reuse
	fn sample_message_data() -> MessageData {
		MessageData::new("TestMessage".to_string(), vec![("field_one".to_string(), 1), ("field_two".to_string(), 2)], "src/messages.rs", 42)
	}

	// MessageData Tests

	#[test]
	fn test_message_data_new_stores_name() {
		let msg = sample_message_data();
		assert_eq!(msg.name(), "TestMessage");
	}

	#[test]
	fn test_message_data_new_stores_fields() {
		let msg = sample_message_data();
		let fields = msg.fields();
		assert_eq!(fields.len(), 2);
		assert_eq!(fields[0], ("field_one".to_string(), 1));
		assert_eq!(fields[1], ("field_two".to_string(), 2));
	}

	#[test]
	fn test_message_data_new_stores_path() {
		let msg = sample_message_data();
		assert_eq!(msg.path(), "src/messages.rs");
	}

	#[test]
	fn test_message_data_new_stores_line_number() {
		let msg = sample_message_data();
		assert_eq!(msg.line_number(), 42);
	}

	#[test]
	fn test_message_data_empty_fields() {
		let msg = MessageData::new("Empty".to_string(), vec![], "src/lib.rs", 0);
		assert!(msg.fields().is_empty());
	}

	#[test]
	fn test_message_data_zero_line_number() {
		let msg = MessageData::new("ZeroLine".to_string(), vec![], "src/lib.rs", 0);
		assert_eq!(msg.line_number(), 0);
	}

	#[test]
	fn test_message_data_large_line_number() {
		let msg = MessageData::new("BigLine".to_string(), vec![], "src/lib.rs", usize::MAX);
		assert_eq!(msg.line_number(), usize::MAX);
	}

	#[test]
	fn test_message_data_name_empty_string() {
		let msg = MessageData::new("".to_string(), vec![], "src/lib.rs", 1);
		assert_eq!(msg.name(), "");
	}

	#[test]
	fn test_message_data_fields_large_index() {
		let msg = MessageData::new("Big".to_string(), vec![("huge".to_string(), usize::MAX)], "src/lib.rs", 1);
		assert_eq!(msg.fields()[0].1, usize::MAX);
	}

	#[test]
	fn test_message_data_debug_format() {
		let msg = sample_message_data();
		let debug_str = format!("{:?}", msg);
		// Debug output should contain the name
		assert!(debug_str.contains("TestMessage"));
	}

	// DebugMessageTree Tests

	#[test]
	fn test_debug_message_tree_new_sets_name() {
		let tree = DebugMessageTree::new("RootNode");
		assert_eq!(tree.name(), "RootNode");
	}

	#[test]
	fn test_debug_message_tree_new_fields_is_none() {
		let tree = DebugMessageTree::new("RootNode");
		assert!(tree.fields().is_none());
	}

	#[test]
	fn test_debug_message_tree_new_variants_is_none() {
		let tree = DebugMessageTree::new("RootNode");
		assert!(tree.variants().is_none());
	}

	#[test]
	fn test_debug_message_tree_new_message_handler_is_none() {
		let tree = DebugMessageTree::new("RootNode");
		assert!(tree.message_handler_fields().is_none());
	}

	#[test]
	fn test_debug_message_tree_new_message_handler_data_is_none() {
		let tree = DebugMessageTree::new("RootNode");
		assert!(tree.message_handler_data_fields().is_none());
	}

	#[test]
	fn test_debug_message_tree_new_path_is_empty() {
		let tree = DebugMessageTree::new("RootNode");
		assert_eq!(tree.path(), "");
	}

	#[test]
	fn test_debug_message_tree_new_line_number_is_zero() {
		let tree = DebugMessageTree::new("RootNode");
		assert_eq!(tree.line_number(), 0);
	}

	// add_fields() Tests

	#[test]
	fn test_add_fields_sets_fields() {
		let mut tree = DebugMessageTree::new("Node");
		tree.add_fields(vec!["alpha".to_string(), "beta".to_string()]);
		let fields = tree.fields().expect("fields should be Some");
		assert_eq!(fields.len(), 2);
		assert_eq!(fields[0], "alpha");
		assert_eq!(fields[1], "beta");
	}

	#[test]
	fn test_add_fields_empty_vec() {
		let mut tree = DebugMessageTree::new("Node");
		tree.add_fields(vec![]);
		let fields = tree.fields().expect("fields should be Some even if empty");
		assert!(fields.is_empty());
	}

	#[test]
	fn test_add_fields_overwrites_previous() {
		let mut tree = DebugMessageTree::new("Node");
		tree.add_fields(vec!["first".to_string()]);
		tree.add_fields(vec!["second".to_string(), "third".to_string()]);
		let fields = tree.fields().expect("fields should be Some");
		// Second call overwrites the first
		assert_eq!(fields.len(), 2);
		assert_eq!(fields[0], "second");
	}

	// set_path() Tests

	#[test]
	fn test_set_path_updates_path() {
		let mut tree = DebugMessageTree::new("Node");
		tree.set_path("editor/src/dispatcher.rs");
		assert_eq!(tree.path(), "editor/src/dispatcher.rs");
	}

	#[test]
	fn test_set_path_empty_string() {
		let mut tree = DebugMessageTree::new("Node");
		tree.set_path("");
		assert_eq!(tree.path(), "");
	}

	#[test]
	fn test_set_path_overwrite() {
		let mut tree = DebugMessageTree::new("Node");
		tree.set_path("old/path.rs");
		tree.set_path("new/path.rs");
		assert_eq!(tree.path(), "new/path.rs");
	}

	// set_line_number() Tests

	#[test]
	fn test_set_line_number_updates_value() {
		let mut tree = DebugMessageTree::new("Node");
		tree.set_line_number(99);
		assert_eq!(tree.line_number(), 99);
	}

	#[test]
	fn test_set_line_number_zero() {
		let mut tree = DebugMessageTree::new("Node");
		tree.set_line_number(0);
		assert_eq!(tree.line_number(), 0);
	}

	#[test]
	fn test_set_line_number_overwrite() {
		let mut tree = DebugMessageTree::new("Node");
		tree.set_line_number(10);
		tree.set_line_number(200);
		assert_eq!(tree.line_number(), 200);
	}

	#[test]
	fn test_set_line_number_max_value() {
		let mut tree = DebugMessageTree::new("Node");
		tree.set_line_number(usize::MAX);
		assert_eq!(tree.line_number(), usize::MAX);
	}

	// add_variant() Tests

	#[test]
	fn test_add_variant_first_variant_creates_vec() {
		let mut tree = DebugMessageTree::new("Root");
		let child = DebugMessageTree::new("Child");
		tree.add_variant(child);
		let variants = tree.variants().expect("variants should be Some");
		assert_eq!(variants.len(), 1);
		assert_eq!(variants[0].name(), "Child");
	}

	#[test]
	fn test_add_variant_multiple_variants() {
		let mut tree = DebugMessageTree::new("Root");
		tree.add_variant(DebugMessageTree::new("Child1"));
		tree.add_variant(DebugMessageTree::new("Child2"));
		tree.add_variant(DebugMessageTree::new("Child3"));
		let variants = tree.variants().expect("variants should be Some");
		assert_eq!(variants.len(), 3);
		assert_eq!(variants[0].name(), "Child1");
		assert_eq!(variants[1].name(), "Child2");
		assert_eq!(variants[2].name(), "Child3");
	}

	#[test]
	fn test_add_variant_nested_children() {
		let mut root = DebugMessageTree::new("Root");
		let mut child = DebugMessageTree::new("Child");
		child.add_variant(DebugMessageTree::new("Grandchild"));
		root.add_variant(child);

		let child_ref = &root.variants().unwrap()[0];
		let grandchild = &child_ref.variants().unwrap()[0];
		assert_eq!(grandchild.name(), "Grandchild");
	}

	//  add_message_handler_field() Tests

	#[test]
	fn test_add_message_handler_field_sets_value() {
		let mut tree = DebugMessageTree::new("Node");
		let msg = sample_message_data();
		tree.add_message_handler_field(msg);
		let handler = tree.message_handler_fields().expect("message_handler should be Some");
		assert_eq!(handler.name(), "TestMessage");
	}

	#[test]
	fn test_add_message_handler_field_overwrites() {
		let mut tree = DebugMessageTree::new("Node");
		tree.add_message_handler_field(MessageData::new("First".to_string(), vec![], "a.rs", 1));
		tree.add_message_handler_field(MessageData::new("Second".to_string(), vec![], "b.rs", 2));
		assert_eq!(tree.message_handler_fields().unwrap().name(), "Second");
	}

	#[test]
	fn test_add_message_handler_field_path_and_line() {
		let mut tree = DebugMessageTree::new("Node");
		tree.add_message_handler_field(MessageData::new("Handler".to_string(), vec![], "src/handler.rs", 77));
		let handler = tree.message_handler_fields().unwrap();
		assert_eq!(handler.path(), "src/handler.rs");
		assert_eq!(handler.line_number(), 77);
	}

	//  add_message_handler_data_field() Tests

	#[test]
	fn test_add_message_handler_data_field_sets_value() {
		let mut tree = DebugMessageTree::new("Node");
		let msg = sample_message_data();
		tree.add_message_handler_data_field(msg);
		let data = tree.message_handler_data_fields().expect("message_handler_data should be Some");
		assert_eq!(data.name(), "TestMessage");
	}

	#[test]
	fn test_add_message_handler_data_field_overwrites() {
		let mut tree = DebugMessageTree::new("Node");
		tree.add_message_handler_data_field(MessageData::new("Old".to_string(), vec![], "x.rs", 5));
		tree.add_message_handler_data_field(MessageData::new("New".to_string(), vec![], "y.rs", 10));
		assert_eq!(tree.message_handler_data_fields().unwrap().name(), "New");
	}

	#[test]
	fn test_add_message_handler_data_field_stores_fields() {
		let mut tree = DebugMessageTree::new("Node");
		tree.add_message_handler_data_field(MessageData::new("DataMsg".to_string(), vec![("x".to_string(), 3), ("y".to_string(), 7)], "src/data.rs", 20));
		let data = tree.message_handler_data_fields().unwrap();
		assert_eq!(data.fields().len(), 2);
		assert_eq!(data.fields()[0].0, "x");
		assert_eq!(data.fields()[1].1, 7);
	}

	//  Debug format Tests

	#[test]
	fn test_debug_message_tree_debug_format_contains_name() {
		let tree = DebugMessageTree::new("DebugMe");
		let debug_str = format!("{:?}", tree);
		assert!(debug_str.contains("DebugMe"));
	}
}
