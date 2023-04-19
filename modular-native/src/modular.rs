use crate::VTable;
use bytes::Bytes;
use futures::Sink;
use modular_core::error::{ModuleError, RegistryError, SubscribeError};
use modular_core::modular::Modular;
use modular_core::modules::{ModuleRequest, ModuleResponse};
use std::future::Future;
use tower::Service;

pub struct CModularVTable<V> {
    register_modular: unsafe extern "C" fn(this: *mut V, name: &str, service: S),
}

pub struct CModular<V> {
    pub ptr: *mut V,
    pub vtable: CModularVTable<V>,
}

impl<V: 'static + Sized> Modular for CModular<V> {
    type Stream = ();
    type Module = ();

    fn register_module<S>(&self, name: &str, service: S) -> Result<(), RegistryError>
    where
        S: Service<ModuleRequest> + 'static + Send + Sync,
        tower_service::Response: Into<ModuleResponse> + Send + 'static,
        tower_service::Error: Into<ModuleError> + Send + 'static,
        tower_service::Future:
            Future<Output = Result<ModuleResponse, ModuleError>> + Send + Sync + 'static,
    {
        (self.vtable.register_modular)(self.ptr, name, service)
    }

    fn subscribe<S, Err>(
        &self,
        topic: &str,
        sink: Option<S>,
    ) -> Result<Self::Stream, SubscribeError>
    where
        S: Sink<(String, Bytes), Error = Err> + Send + Sync + 'static,
    {
        todo!()
    }

    fn publish<Request>(&self, event: Request)
    where
        Request: Into<ModuleRequest<Bytes>>,
    {
        todo!()
    }

    fn get_module(&self, name: &str) -> Option<Self::Module> {
        todo!()
    }

    fn deregister_module(&self, name: &str) {
        todo!()
    }
}

// impl<V: Modular> CModularVTable<V> {
//     pub fn new() -> Self {
//         Self {
//             register_modular:  CModularVTable::<V>::register_modular
//         }
//     }
//
//     unsafe extern "C" fn register_modular<S>(this: *mut V, name: &str, service: S) -> Result<(), RegistryError> {
//         let this = &mut *this;
//         this.register_module(name, service)
//     }
// }

pub trait ModularNativeExt {}
