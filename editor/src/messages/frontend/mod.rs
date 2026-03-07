mod frontend_message;

pub mod utility_types;

#[doc(inline)]
pub use frontend_message::{FrontendMessage, FrontendMessageDiscriminant};

// TODO: Make this an enum with the actual icon names, somehow derived from or tied to the frontend icon set
pub type IconName = String;
