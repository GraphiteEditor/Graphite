use graph_craft::application_io::resource::Resource;
use std::sync::LazyLock;

const FALLBACK_FONT_BYTES: &[u8] = include_bytes!("source-sans-pro-regular.ttf");
pub static FALLBACK_FONT_RESOURCE: LazyLock<Resource> = LazyLock::new(|| Resource::new(FALLBACK_FONT_BYTES));
