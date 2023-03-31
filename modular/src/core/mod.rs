use bytes::Bytes;
use futures::Sink;
use modular_core::error::*;
use modular_core::request::ModuleRequest;
use modular_core::response::ModuleResponse;
use modular_core::BoxModule;
use std::sync::Arc;
use tower::Service;

pub mod events;
mod module;
mod modules_registry;
pub mod pattern;
// mod req;
// mod response;

pub mod modules {
    pub use super::module::*;
    pub use super::modules_registry::*;
}

use crate::core::pattern::Pattern;
use modules::*;

#[derive(Default)]
pub struct Modular {
    modules: Arc<ModulesRegistry<Bytes, Bytes>>,
    events: Arc<events::EventsManager<Bytes>>,
}

impl modular_core::core::Modular for Modular {
    type Module = Option<Box<Module<Bytes, Bytes>>>;
    type Subscribe = Result<(), SubscribeError>;

    fn subscribe<S, Err>(&self, topic: &str, sink: S) -> Self::Subscribe
    where
        S: Sink<(String, Bytes), Error = Err> + Send + Sync + 'static,
    {
        let pattern = Pattern::parse(topic).map_err(SubscribeError::InvalidPattern)?;
        self.events.subscribe(pattern, sink);
        Ok(())
    }

    fn publish(&self, topic: &str, data: Bytes) {
        if topic.starts_with("$.sys.") {
            return;
        }

        self.events.publish(topic, data);
    }

    fn register_module<S, Request>(&self, name: &str, service: S) -> Result<(), RegistryError>
    where
        S: Service<Request> + Send + 'static,
        Request: From<ModuleRequest<Bytes>> + Send + 'static,
        S::Response: Into<ModuleResponse<Bytes>> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Send + Sync + 'static,
    {
        self.modules.register(name, service)?;
        Ok(())
    }

    fn get_module(&self, name: &str) -> Self::Module {
        self.modules.get(name).map(|f| Box::new(f))
    }

    fn deregister_module(&self, name: &str) {
        self.modules.remove(name);
    }
}

impl Modular {
    pub fn register_or_replace_module<S, Request>(&self, name: &str, svc: S)
    where
        S: Service<Request> + Send + 'static,
        Request: From<ModuleRequest<Bytes>> + Send + 'static,
        S::Response: Into<ModuleResponse<Bytes>> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Send + Sync + 'static,
    {
        self.modules.register_or_replace(name, svc);
    }
}
