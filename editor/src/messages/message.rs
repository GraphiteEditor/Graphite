use crate::messages::prelude::*;
use graphite_proc_macros::*;

#[impl_message]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Message {
	// Sub-messages
	#[child]
	Animation(AnimationMessage),
	#[child]
	AppWindow(AppWindowMessage),
	#[child]
	Broadcast(BroadcastMessage),
	#[child]
	Debug(DebugMessage),
	#[child]
	Defer(DeferMessage),
	#[child]
	Dialog(DialogMessage),
	#[child]
	Frontend(FrontendMessage),
	#[child]
	Globals(GlobalsMessage),
	#[child]
	InputPreprocessor(InputPreprocessorMessage),
	#[child]
	KeyMapping(KeyMappingMessage),
	#[child]
	Layout(LayoutMessage),
	#[child]
	Portfolio(PortfolioMessage),
	#[child]
	Preferences(PreferencesMessage),
	#[child]
	Tool(ToolMessage),

	// Messages
	Batched {
		messages: Box<[Message]>,
	},
	NoOp,
}

/// Provides an impl of `specta::Type` for `MessageDiscriminant`, the struct created by `impl_message`.
/// Specta isn't integrated with `impl_message`, so a remote impl must be provided using this struct.
impl specta::Type for MessageDiscriminant {
	fn inline(_type_map: &mut specta::TypeCollection, _generics: specta::Generics) -> specta::DataType {
		specta::DataType::Any
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use std::io::Write;

	#[test]
	fn generate_message_tree() {
		let result = Message::build_message_tree();
		let mut file = std::fs::File::create("../hierarchical_message_system_tree.txt").unwrap();
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
				file.write_all(format!("{}{}{}\n", format!("{}    ", prefix), branch, field.0).as_bytes()).unwrap();
			}
		}
	}
}
