use core_types::transform::Footprint;
use graphene_cache::{Cache, GenerationalEviction};

use crate::basic_brush::render::State;

pub(super) type BrushCache = Cache<Footprint, State, GenerationalEviction<2, 3>>;
