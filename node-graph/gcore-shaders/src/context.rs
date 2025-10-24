pub trait Ctx: Clone + Send {}

impl<T: Ctx> Ctx for Option<T> {}
impl<T: Ctx + Sync> Ctx for &T {}
impl Ctx for () {}

pub trait ArcCtx: Send + Sync {}
#[cfg(feature = "std")]
impl<T: ArcCtx> Ctx for std::sync::Arc<T> {}
