use graphene_std::text::Blob;
use std::sync::{Arc, LazyLock};

const FALLBACK_FONT_BYTES: &[u8] = include_bytes!("source-sans-pro-regular.ttf");
pub static FALLBACK_FONT_BLOB: LazyLock<Blob<u8>> = LazyLock::new(|| Blob::new(Arc::new(FALLBACK_FONT_BYTES.to_vec())));
