//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

// Version 2 moves these nodes' parameters ahead of their scope inputs, and only the compiler inserts them, so no stored node has version 1's order
rename!("graphene_std::platform_application_io::ResourceNode"@1 => @2);
rename!("graphene_std::render_background::RenderBackgroundNode"@1 => @2);
rename!("graphene_std::render_cache::RenderOutputCacheNode"@1 => @2);
rename!("graphene_std::render_node::RenderNode"@1 => @2);
rename!("graphene_std::render_pixel_preview::RenderPixelPreviewNode"@1 => @2);
