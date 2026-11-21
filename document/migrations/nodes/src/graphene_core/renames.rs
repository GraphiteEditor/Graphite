//! Nodes that only moved to a new identifier, or changed version with stored nodes staying as they are.

use migration_core::rename;

// Version 2 is where old Upload Texture nodes join the lineage, and stored Passthrough nodes already have its layout
rename!("graphene_core::ops::PassthroughNode"@1 => @2);
// The removed Monitor node passes its one input on, as a Passthrough does
rename!("graphene_core::memo::MonitorNode"@1 => "graphene_core::ops::PassthroughNode"@2);
// Version 2 only adds `InjectIndex` to the context bounds, so stored nodes keep their layout
rename!("graphene_core::vector::MapPointsNode"@1 => @2);
