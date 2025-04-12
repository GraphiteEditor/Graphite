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

	#[test]
	fn generate_message_tree() {
		let res = Message::build_message_tree();
		println!("{}", res.name());
		if let Some(variants) = res.variants() {
			for (i, variant) in variants.iter().enumerate() {
				let is_last = i == variants.len() - 1;
				print_tree_node(variant, "", is_last);
			}
		}
	}

	fn print_tree_node(tree: &DebugMessageTree, prefix: &str, is_last: bool) {
		// Print the current node
		let branch = if is_last { "└── " } else { "├── " };
		println!("{}{}{}", prefix, branch, tree.name());

		// Prepare prefix for children
		let child_prefix = if is_last { format!("{}    ", prefix) } else { format!("{}│   ", prefix) };

		// Print children if any
		if let Some(variants) = tree.variants() {
			let len = variants.len();
			for (i, variant) in variants.iter().enumerate() {
				let is_last_child = i == len - 1;
				print_tree_node(variant, &child_prefix, is_last_child);
			}
		}
	}
}
