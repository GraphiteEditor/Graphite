//! Overview:
//!
//!                         ShapeEditor
//!                         /          \
//!                 VectorShape ... VectorShape  <- ShapeEditor contains many VectorShapes
//!                     /                 \
//!                VectorAnchor ...  VectorAnchor <- VectorShape contains many VectorAnchors
//!
//!
//!                     VectorAnchor <- Container for the anchor metadata and optional VectorControlPoints
//!                           /
//!             [Option<VectorControlPoint>; 3] <- [0] is the anchor's draggable point (but not metadata), [1] is the handle1's draggable point, [2] is the handle2's draggable point
//!              /              |                      \
//!         "Anchor"        "Handle1"          "Handle2" <- These are VectorControlPoints and the only editable / draggable "primitive"

pub mod constants;
pub mod shape_editor;
pub mod vector_anchor;
pub mod vector_control_point;
pub mod vector_shape;
