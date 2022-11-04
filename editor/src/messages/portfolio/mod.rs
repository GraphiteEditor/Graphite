mod portfolio_message;
mod portfolio_message_handler;

pub mod document;
pub mod menu_bar;
pub mod utility_types;

#[doc(inline)]
pub use portfolio_message::{PortfolioMessage, PortfolioMessageDiscriminant};
#[doc(inline)]
pub use portfolio_message_handler::PortfolioMessageHandler;
