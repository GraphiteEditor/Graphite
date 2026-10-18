pub use graphene_std::runtime::{Spawn, Task};
use std::sync::Arc;

pub type Spawner = Arc<dyn Spawn + Send + Sync>;
