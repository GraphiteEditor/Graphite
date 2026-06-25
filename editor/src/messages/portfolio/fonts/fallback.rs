// Re-export the fallback font resource from text-nodes, which is the authoritative location.
// This avoids duplicating the font bytes in the editor binary.
// This file can be removed after deciding where to place the authority of fallback_resource.
pub use graphene_std::text_nodes::FALLBACK_FONT_RESOURCE;
