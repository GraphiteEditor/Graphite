mod editor_wrapper;
pub use editor_wrapper::{DesktopWrapper, NodeGraphExecutionResult};

pub mod messages;
pub use wgpu_executor::Context as WgpuContext;
