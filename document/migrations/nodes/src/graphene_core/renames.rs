//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

// Version 2 is where old Upload Texture nodes join the lineage, and stored Passthrough nodes already have its layout
rename!("graphene_core::ops::PassthroughNode"@1 => @2);
