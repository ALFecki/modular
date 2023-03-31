use crate::core::module::{Module, ModuleService};

use modular_core::error::{ModuleError, RegistryError};
use modular_core::request::ModuleRequest;
use modular_core::response::ModuleResponse;
use parking_lot::RwLock;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::Service;

pub(crate) type BoxModuleService<Req, Resp> =
    tower::util::BoxService<ModuleRequest<Req>, ModuleResponse<Resp>, ModuleError>;

pub struct ModulesRegistry<Req, Resp> {
    modules: RwLock<HashMap<String, Arc<Mutex<BoxModuleService<Req, Resp>>>>>,
}

impl<Req, Resp> Default for ModulesRegistry<Req, Resp> {
    fn default() -> Self {
        Self {
            modules: Default::default(),
        }
    }
}

impl<Request, Response> ModulesRegistry<Request, Response>
where
    Request: Send + Sync + 'static,
    Response: Send + Sync + 'static,
{
    pub fn register<S, Req>(&self, name: &str, svc: S) -> Result<(), RegistryError>
    where
        S: Service<Req> + Send + 'static,
        Req: From<ModuleRequest<Request>> + Send + 'static,
        S::Response: Into<ModuleResponse<Response>> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Send + Sync + 'static,
    {
        let mut modules = self.modules.write();
        let svc = BoxModuleService::new(ModuleService(svc, Default::default()));

        match modules.entry(name.to_string()) {
            Entry::Occupied(_) => {
                return Err(RegistryError::AlreadyExists);
            }
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(Mutex::new(svc)));
            }
        }

        Ok(())
    }

    pub fn register_or_replace<S, Req>(&self, name: &str, svc: S)
    where
        S: Service<Req> + Send + 'static,
        Req: From<ModuleRequest<Request>> + Send + 'static,
        S::Response: Into<ModuleResponse<Response>> + Send + 'static,
        S::Error: Into<ModuleError> + Send + 'static,
        S::Future: Send + Sync + 'static,
    {
        let svc = BoxModuleService::new(ModuleService(svc, Default::default()));

        let mut modules = self.modules.write();

        let entry = modules.entry(name.to_string());

        let mut existing = match entry {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(Mutex::new(svc)));
                return;
            }
        };

        let existing = existing.get_mut();
        debug_assert!(Arc::strong_count(existing) == 1);

        *Arc::get_mut(existing).unwrap() = Mutex::new(svc)
    }

    pub fn get(&self, name: &str) -> Option<Module<Request, Response>> {
        let modules = self.modules.read();
        modules.get(name).map(|m| Module(Arc::downgrade(m)))
    }

    pub fn remove(&self, name: &str) {
        let mut modules = self.modules.write();
        modules.remove(name);
    }
}
