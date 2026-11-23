//! Lays out a node graph on a grid from the placements its nodes store, fitting each node's upstream together so nothing overlaps.

#![warn(clippy::pedantic)]

mod chains;
mod connections;
mod contour;
mod drag;
mod fitting;
mod forest;
mod graph;
mod groups;
mod layout;
mod per_node;
mod rect;

pub use chains::snaps_into_chain;
pub use drag::Drag;
pub use graph::{CHAIN_WIDTH, Graph, Node, NodeIndex, NodeKind, Placement, Wire};
pub use layout::{Layout, NodeLayout};
pub use rect::Rect;
