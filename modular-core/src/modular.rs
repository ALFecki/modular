use crate::error::*;
use crate::module::Module;
use crate::request::ModuleRequest;
use crate::response::ModuleResponse;
use bytes::Bytes;
use futures::Sink;
use futures_util::future::BoxFuture;
use std::future::Future;
use tower::Service;

pub type BoxModule = Box<dyn Module<Future = BoxFuture<'static, Result<Bytes, ModuleError>>>>;

pub trait Modular<Sub = ()>: Send + Sync {
    type Module: Module;
    type Subscribe: Future<Output = Result<Sub, SubscribeError>> + Send + Sync + 'static;

    fn subscribe<S, Err>(&self, topic: &str, sink: S) -> Self::Subscribe
        where
            S: Sink<(String, Bytes), Error = Err> + Send + Sync + 'static;
    fn publish(&self, topic: &str, data: Bytes);

    fn register_module<S, Request>(&self, name: &str, service: S) -> Result<(), RegistryError>
        where
            S: Service<Request> + Send + 'static,
            Request: From<ModuleRequest<Bytes>> + Send + 'static,
            S::Response: Into<ModuleResponse<Bytes>> + Send + 'static,
            S::Error: Into<ModuleError> + Send + 'static,
            S::Future: Send + Sync + 'static;

    fn get_module(&self, name: &str) -> Self::Module;
    fn deregister_module(&self, name: &str);
}
