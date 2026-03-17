mod frontend_message;

pub mod utility_types;

#[doc(inline)]
pub use frontend_message::{FrontendMessage, FrontendMessageDiscriminant};

// TODO: Make this an enum with the actual icon names, somehow derived from or tied to the frontend icon set.
// TODO: Then remove `#[widget_builder(string)]` from all icon fields.
pub type IconName = String;
