pub mod input_mapper;
pub mod input_preprocessor;
pub mod keyboard;
pub mod mouse;

mod input_mapper_macros;
mod input_mapper_message;
mod input_mapper_message_handler;
mod input_preprocessor_message;
mod input_preprocessor_message_handler;

#[doc(inline)]
pub use input_mapper_message::{InputMapperMessage, InputMapperMessageDiscriminant};
#[doc(inline)]
pub use input_mapper_message_handler::InputMapperMessageHandler;

#[doc(inline)]
pub use input_preprocessor_message::{InputPreprocessorMessage, InputPreprocessorMessageDiscriminant};
#[doc(inline)]
pub use input_preprocessor_message_handler::InputPreprocessorMessageHandler;
