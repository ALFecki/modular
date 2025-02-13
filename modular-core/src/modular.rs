use crate::module::Module;
use crate::modules::*;
use bytes::Bytes;
use futures::future::BoxFuture;
use futures::Sink;
use std::future::Future;
use tower::Service;

pub type BoxModule =
    Box<dyn Module<Future = BoxFuture<'static, Result<ModuleResponse, ModuleError>>>>;

pub trait Modular: Send + Sync {
    type Stream: Send + 'static;
    type Module;

    fn register_module<S>(&self, name: &str, service: S) -> Result<(), RegistryError>
    where
        S: Service<ModuleRequest> + 'static + Send + Sync,
        S::Response: Into<ModuleResponse> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Future<Output = Result<ModuleResponse, ModuleError>> + Send + Sync + 'static;

    fn subscribe<S, Err>(
        &self,
        topic: &str,
        sink: Option<S>,
    ) -> Result<Self::Stream, SubscribeError>
    where
        S: Sink<(String, Bytes), Error = Err> + Send + Sync + 'static;

    fn publish<Request>(&self, event: Request)
    where
        Request: Into<ModuleRequest<Bytes>>;

    fn get_module(&self, name: &str) -> Option<Self::Module>;

    fn deregister_module(&self, name: &str);
}
