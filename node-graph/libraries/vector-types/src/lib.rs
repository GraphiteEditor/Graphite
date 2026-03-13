#[macro_use]
extern crate log;

pub mod gradient;
pub mod math;
pub mod subpath;
pub mod vector;

// Re-export commonly used types at the crate root
pub use core_types as gcore;
pub use gradient::{GradientStop, GradientStops, GradientType};
pub use math::{QuadExt, RectExt};
pub use subpath::Subpath;
pub use vector::Vector;
pub use vector::reference_point::ReferencePoint;

// Re-export dependencies that users of this crate will need
pub use dyn_any;
pub use glam;
pub use kurbo;
