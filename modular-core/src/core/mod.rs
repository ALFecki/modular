use crate::core::error::ModuleError;
use crate::core::module::Module;
use crate::core::request::ModuleRequest;
use crate::core::response::ModuleResponse;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use tower::Service;


pub mod error;
pub mod module;
pub mod request;
pub mod response;

pub type BoxModule = Box<dyn Module<Future = BoxFuture<'static, Result<Bytes, ModuleError>>>>;

pub trait Modular: Send + Sync {
    fn subscribe(&self, topic: &str) -> anyhow::Result<BoxStream<'static, ModuleResponse<Bytes>>>;
    fn publish(&self, topic: &str, data: Bytes);

    fn register_module<S, Request>(&self, name: &str, service: S) -> Result<(), RegistryError>
    where
        S: Service<Request> + Send + 'static,
        Request: From<ModuleRequest<Bytes>> + Send + 'static,
        S::Response: Into<ModuleResponse<Bytes>> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Send + Sync + 'static;

    fn get_module(&self, name: &str) -> Option<BoxModule>;
    fn deregister_module(&self, name: &str);
}
