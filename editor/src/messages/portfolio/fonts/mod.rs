mod fallback;
mod fonts_message;
mod fonts_message_handler;

pub mod utility_types;

#[doc(inline)]
pub use fallback::FALLBACK_FONT_RESOURCE;
#[doc(inline)]
pub use fonts_message::{FontsMessage, FontsMessageDiscriminant};
#[doc(inline)]
pub use fonts_message_handler::{FontsMessageContext, FontsMessageHandler};
