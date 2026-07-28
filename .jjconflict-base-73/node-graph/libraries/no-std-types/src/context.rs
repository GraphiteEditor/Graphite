pub trait Ctx: Clone + Send {}

impl<T: Ctx> Ctx for Option<T> {}
impl<T: Ctx + Sync> Ctx for &T {}
impl Ctx for () {}

pub trait ArcCtx: Send + Sync {}
#[cfg(feature = "std")]
impl<T: ArcCtx> Ctx for std::sync::Arc<T> {}

// The cache-hash bound record kernels place on their element generics; the
// shader build compiles the same signatures without the hashing machinery.
#[cfg(feature = "std")]
pub use graphene_hash::CacheHash;
#[cfg(not(feature = "std"))]
pub trait CacheHash {}
#[cfg(not(feature = "std"))]
impl<T> CacheHash for T {}
