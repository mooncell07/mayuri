use super::context::Context;
use linkme::distributed_slice;
use std::pin::Pin;
use std::sync::Arc;

pub type ListenerFuture = Pin<Box<dyn Future<Output = ()>>>;

pub struct ListenerFutureInfo {
    pub listener_future_callback: fn(Arc<Context>) -> ListenerFuture,
    pub belongs_to: &'static str,
}

#[distributed_slice]
pub static LISTENER_FUTURE_INFO_SLICE: [ListenerFutureInfo] = [..];
