mod portfolio_message;
mod portfolio_message_handler;

pub mod document;
pub mod menu_bar;

#[doc(inline)]
pub use portfolio_message::{PortfolioMessage, PortfolioMessageDiscriminant};
#[doc(inline)]
pub use portfolio_message_handler::PortfolioMessageHandler;
