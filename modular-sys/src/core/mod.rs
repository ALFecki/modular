#![cfg(feature = "dll")]

use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use modular_core::error::*;
use modular_core::module::Module;
use modular_core::modules::ModuleResponse;

pub type BoxModule =
    Box<dyn Module<Future = BoxFuture<'static, Result<ModuleResponse, ModuleError>>>>;

pub trait Modular: Send + Sync {
    fn subscribe(&self, topic: &str) -> anyhow::Result<BoxStream<'static, (String, Bytes)>>;
    fn publish(&self, topic: &str, data: Bytes);

    fn register_module<S>(&self, name: &str, service: S)
    where
        S: tower::Service<(String, Bytes), Response = Bytes, Error = ModuleError>
            + 'static
            + Send
            + Sync,
        S::Future: Send + Sync + 'static;

    fn get_module(&self, name: &str) -> Option<BoxModule>;
    fn deregister_module(&self, name: &str);
}
