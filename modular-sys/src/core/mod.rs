#![cfg(feature = "dll")]

use crate::dll::ModuleError;
use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use modular_core::error::ModuleError;
use modular_core::module::Module;
use std::future::Future;
use tower::Service;

// pub type BoxModule = Box<dyn Module<Future = BoxFuture<'static, Result<Bytes, ModuleError>>>>;
//
// pub trait Modular: Send + Sync {
//     fn subscribe(&self, topic: &str) -> anyhow::Result<BoxStream<'static, (String, Bytes)>>;
//     fn publish(&self, topic: &str, data: Bytes);
//
//     fn register_module<S>(&self, name: &str, service: S)
//     where
//         S: Service<(String, Bytes), Response = Bytes, Error = ModuleError>
//             + 'static
//             + Send
//             + Sync,
//         S::Future: Send + Sync + 'static;
//
//     fn get_module(&self, name: &str) -> Option<BoxModule>;
//     fn deregister_module(&self, name: &str);
// }
