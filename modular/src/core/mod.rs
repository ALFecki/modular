use bytes::Bytes;
use futures::Sink;
use std::sync::Arc;
use tower::Service;
use modular_core::error::*;

pub mod events;
mod module;
mod modules_registry;
pub mod pattern;
mod req;
mod response;

pub mod modules {
    pub use super::module::*;
    pub use super::modules_registry::*;
    pub use super::req::*;
    pub use super::response::*;
}

use crate::core::pattern::Pattern;
use modules::*;

#[derive(Default)]
pub struct Modular {
    modules: Arc<ModulesRegistry<Bytes, Bytes>>,
    events: Arc<events::EventsManager<Bytes>>,
}

impl Modular {
    pub fn register_module<S, Request>(&self, name: &str, svc: S) -> Result<(), RegistryError>
    where
        S: Service<Request> + Send + 'static,
        Request: From<ModuleRequest<Bytes>> + Send + 'static,
        S::Response: Into<ModuleResponse<Bytes>> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Send + Sync + 'static,
    {
        self.modules.register(name, svc)?;

        Ok(())
    }

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

    pub fn remove_module(&self, name: &str) {
        self.modules.remove(name);
    }

    pub fn get_module(&self, name: &str) -> Option<Module<Bytes, Bytes>> {
        self.modules.get(name)
    }

    pub fn subscribe<S, Err>(&self, name: &str, sink: S) -> Result<(), SubscribeError>
    where
        S: Sink<(String, Bytes), Error = Err> + Send + Sync + 'static,
    {
        let pattern = Pattern::parse(name).map_err(SubscribeError::InvalidPattern)?;
        self.events.subscribe(pattern, sink);

        Ok(())
    }

    pub fn publish_event<E: Into<Bytes>>(&self, path: &str, event: E) {
        if path.starts_with("$.sys.") {
            return;
        }

        self.publish_event_inner(path, event.into());
    }

    fn publish_event_inner(&self, path: &str, event: Bytes) {
        self.events.publish(path, event);
    }
}
