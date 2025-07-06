use crate::messages::prelude::*;
use graphite_proc_macros::*;

#[impl_message]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Message {
	NoOp,
	Init,
	Batched(Box<[Message]>),
	StartBuffer,
	EndBuffer(graphene_std::renderer::RenderMetadata),

	#[child]
	Animation(AnimationMessage),
	#[child]
	Broadcast(BroadcastMessage),
	#[child]
	Debug(DebugMessage),
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
	#[child]
	Workspace(WorkspaceMessage),
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
		let (branch, child_prefix) = if tree.has_message_handler_data_fields() || tree.has_message_handler_fields() {
			("├── ", format!("{}│   ", prefix))
		} else {
			if is_last {
				("└── ", format!("{}    ", prefix))
			} else {
				("├── ", format!("{}│   ", prefix))
			}
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

		// Print handler field if any
		if let Some(data) = tree.message_handler_fields() {
			let len = data.fields().len();
			let (branch, child_prefix) = if tree.has_message_handler_data_fields() {
				("├── ", format!("{}│   ", prefix))
			} else {
				("└── ", format!("{}    ", prefix))
			};
			if data.path().is_empty() {
				file.write_all(format!("{}{}{}\n", prefix, branch, data.name()).as_bytes()).unwrap();
			} else {
				file.write_all(format!("{}{}{} `{}`\n", prefix, branch, data.name(), data.path()).as_bytes()).unwrap();
			}
			for (i, field) in data.fields().iter().enumerate() {
				let is_last_field = i == len - 1;
				let branch = if is_last_field { "└── " } else { "├── " };

				file.write_all(format!("{}{}{}\n", child_prefix, branch, field.0).as_bytes()).unwrap();
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
