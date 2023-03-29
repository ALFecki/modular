#![cfg(feature = "dll")]

use crate::dll::LibraryError;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use std::future::Future;

pub type BoxModule = Box<dyn Module<Future = BoxFuture<'static, Result<Bytes, LibraryError>>>>;

pub trait Modular: Send + Sync {
    fn subscribe(&self, topic: &str) -> anyhow::Result<BoxStream<'static, (String, Bytes)>>;
    fn publish(&self, topic: &str, data: Bytes);

    fn register_module<S>(&self, name: &str, service: S)
    where
        S: tower::Service<(String, Bytes), Response = Bytes, Error = LibraryError>
            + 'static
            + Send
            + Sync,
        S::Future: Send + Sync + 'static;

    fn get_module(&self, name: &str) -> Option<BoxModule>;
    fn deregister_module(&self, name: &str);
}

pub trait Module {
    type Future: Future<Output = Result<Bytes, LibraryError>> + Send + 'static;

    fn invoke(&self, method: &str, data: Bytes) -> Self::Future;
}
