//! Finds the relationships a moving point can take with the geometry around it.
//!
//! Geometry is solved in a work plane's coordinates and ranked by nearness on screen, so nothing here assumes the plane is the screen or the document.

pub mod bounds;
pub mod curve;
pub mod frame;
pub mod locus;
pub mod tangency;
pub mod view;

pub use bounds::Bounds;
pub use curve::{Curve, NearestOnCurve};
pub use frame::Frame;
pub use locus::Locus;
pub use view::ViewMapping;
