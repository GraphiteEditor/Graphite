//! Finds the relationships a moving point can take with the geometry around it.
//!
//! Geometry is solved in a work plane's coordinates and ranked by nearness on screen, so nothing here assumes the plane is the screen or the document.

pub mod bounds;
mod bounds_tree;
pub mod candidate;
pub mod curve;
pub mod frame;
pub mod grid;
pub mod locus;
pub mod scene;
pub mod solve;
pub mod tangency;
pub mod tunables;
pub mod view;

pub use bounds::Bounds;
pub use candidate::{Candidate, CandidateKey, Highlight, Kind, Shape};
pub use curve::{Curve, NearestOnCurve};
pub use frame::Frame;
pub use grid::Grid;
pub use locus::Locus;
pub use scene::{AnchorId, Exclusions, GeometryRole, LayerId, LayerKind, Scene, SceneAnchor, SceneCurve, SceneLayer};
pub use solve::{CrossingCache, Engagement, EngagementKey, FeatureMap, Incumbent, Snap, Tier};
pub use tunables::Tunables;
pub use view::ViewMapping;
