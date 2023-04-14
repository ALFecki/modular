use bytes::Bytes;
use futures::Sink;
use modular_core::modules::*;
use std::future::Future;
use std::sync::Arc;
use tower::Service;

pub mod events;
mod module;
mod modules_registry;
pub mod pattern;

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

impl modular_core::modular::Modular for Modular {
    type Stream = ();
    type Module = Module<Bytes, Bytes>;

    fn register_module<S>(&self, name: &str, service: S) -> Result<(), RegistryError>
    where
        S: Service<ModuleRequest> + 'static + Send + Sync,
        S::Response: Into<ModuleResponse> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Future<Output = Result<ModuleResponse, ModuleError>> + Send + Sync + 'static,
    {
        self.modules.register(name, service)?;
        Ok(())
    }

    fn subscribe<S, Err>(
        &self,
        topic: &str,
        sink: Option<S>,
    ) -> Result<Self::Stream, SubscribeError>
    where
        S: Sink<(String, Bytes), Error = Err> + Send + Sync + 'static,
    {
        if let Some(sink) = sink {
            let pattern = Pattern::parse(topic).map_err(SubscribeError::InvalidPattern)?;
            self.events.subscribe(pattern, sink);
        }
        Ok(())
    }

    fn publish<Request>(&self, event: Request)
    where
        Request: Into<ModuleRequest<Bytes>>,
    {
        let event = event.into();
        if event.action.starts_with("$.sys.") {
            return;
        }
        self.publish_event_inner(event.action(), event.body.clone());
    }

    fn get_module(&self, name: &str) -> Option<Self::Module> {
        self.modules.get(name)
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

    fn publish_event_inner(&self, path: &str, event: Bytes) {
        self.events.publish(path, event);
    }
}
