use editor::messages::message::Message;
use editor::utility_types::DebugMessageTree;
use std::io::Write;

fn main() {
	let result = Message::message_tree();
	std::fs::create_dir_all("../../website/generated").unwrap();
	let mut file = std::fs::File::create("../../website/generated/hierarchical_message_system_tree.txt").unwrap();
	file.write_all(format!("{} `{}`\n", result.name(), result.path()).as_bytes()).unwrap();
	if let Some(variants) = result.variants() {
		for (i, variant) in variants.iter().enumerate() {
			let is_last = i == variants.len() - 1;
			print_tree_node(variant, "", is_last, &mut file);
		}
	}
}

fn print_tree_node(tree: &DebugMessageTree, prefix: &str, is_last: bool, file: &mut std::fs::File) {
	// Print the current node
	let (branch, child_prefix) = if tree.message_handler_data_fields().is_some() || tree.message_handler_fields().is_some() {
		("├── ", format!("{prefix}│   "))
	} else if is_last {
		("└── ", format!("{prefix}    "))
	} else {
		("├── ", format!("{prefix}│   "))
	};

	if tree.path().is_empty() {
		file.write_all(format!("{}{}{}\n", prefix, branch, tree.name()).as_bytes()).unwrap();
	} else {
		file.write_all(format!("{}{}{} `{}`\n", prefix, branch, tree.name(), tree.path()).as_bytes()).unwrap();
	}

	// Print children if any
	if let Some(variants) = tree.variants() {
		let len = variants.len();
		for (i, variant) in variants.iter().enumerate() {
			let is_last_child = i == len - 1;
			print_tree_node(variant, &child_prefix, is_last_child, file);
		}
	}

	// Print message field if any
	if let Some(fields) = tree.fields() {
		let len = fields.len();
		for (i, field) in fields.iter().enumerate() {
			let is_last_field = i == len - 1;
			let branch = if is_last_field { "└── " } else { "├── " };

			file.write_all(format!("{child_prefix}{branch}{field}\n").as_bytes()).unwrap();
		}
	}

	// Print handler field if any
	if let Some(data) = tree.message_handler_fields() {
		let len = data.fields().len();
		let (branch, child_prefix) = if tree.message_handler_data_fields().is_some() {
			("├── ", format!("{prefix}│   "))
		} else {
			("└── ", format!("{prefix}    "))
		};

		const FRONTEND_MESSAGE_STR: &str = "FrontendMessage";
		if data.name().is_empty() && tree.name() != FRONTEND_MESSAGE_STR {
			panic!("{}'s MessageHandler is missing #[message_handler_data]", tree.name());
		} else if tree.name() != FRONTEND_MESSAGE_STR {
			file.write_all(format!("{}{}{} `{}`\n", prefix, branch, data.name(), data.path()).as_bytes()).unwrap();

			for (i, field) in data.fields().iter().enumerate() {
				let is_last_field = i == len - 1;
				let branch = if is_last_field { "└── " } else { "├── " };

				file.write_all(format!("{}{}{}\n", child_prefix, branch, field.0).as_bytes()).unwrap();
			}
		}
	}

	// Print data field if any
	if let Some(data) = tree.message_handler_data_fields() {
		let len = data.fields().len();
		if data.path().is_empty() {
			file.write_all(format!("{}{}{}\n", prefix, "└── ", data.name()).as_bytes()).unwrap();
		} else {
			file.write_all(format!("{}{}{} `{}`\n", prefix, "└── ", data.name(), data.path()).as_bytes()).unwrap();
		}
		for (i, field) in data.fields().iter().enumerate() {
			let is_last_field = i == len - 1;
			let branch = if is_last_field { "└── " } else { "├── " };
			let field = &field.0;
			file.write_all(format!("{prefix}    {branch}{field}\n").as_bytes()).unwrap();
		}
	}
}
