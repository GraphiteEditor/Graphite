pub use core_types::runtime::*;

use crate::platform_application_io::editor_api;
use core_types::Ctx;
use graph_craft::application_io::PlatformEditorApi;
use std::sync::Arc;

#[node_macro::node(category(""), inject_scope)]
pub fn runtime(_: impl Ctx, #[scope(editor_api::IDENTIFIER)] editor_api: Arc<PlatformEditorApi>) -> RuntimeHandle {
	editor_api.runtime.clone()
}
