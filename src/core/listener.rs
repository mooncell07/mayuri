use super::context::Context;
use linkme::distributed_slice;
use std::pin::Pin;

pub type ListenerFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct ListenerFutureInfo {
    pub listener_future_callback: fn(Context) -> ListenerFuture,
    pub belongs_to: &'static str,
}

#[distributed_slice]
pub static LISTENER_FUTURE_INFO_SLICE: [ListenerFutureInfo] = [..];
