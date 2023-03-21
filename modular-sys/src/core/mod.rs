#![cfg(feature = "dll")]

use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use std::future::Future;

pub type BoxModule = Box<dyn Module<Future = BoxFuture<'static, Result<Bytes, ()>>>>;

pub trait Modular: Send + Sync {
    fn subscribe(&self, topic: &str) -> anyhow::Result<BoxStream<'static, (String, Bytes)>>;
    fn publish(&self, topic: &str, data: Bytes);

    fn register_module<S>(&self, name: &str, service: S)
    where
        S: tower::Service<(String, Bytes), Response = Bytes, Error = ()> + 'static + Send + Sync,
        S::Future: Send + Sync + 'static;

    fn get_module(&self, name: &str) -> Option<BoxModule>;
    fn deregister_module(&self, name: &str);
}

pub trait Module {
    type Future: Future<Output = Result<Bytes, ()>> + Send + 'static;

    fn invoke(&self, method: &str, data: Bytes) -> Self::Future;
}

// #[cfg(feature = "dll")]
// mod dll;
//
// mod core;
//
// use std::ffi::c_void;
// use std::os::raw::c_char;
// use std::ptr::{null, null_mut};
//
// #[derive(Copy, Clone)]
// #[repr(transparent)]
// pub struct Obj(pub *mut c_void);
//
// unsafe impl Send for Obj {}
// unsafe impl Sync for Obj {}
//
// #[derive(Copy, Clone)]
// #[repr(C)]
// pub struct NativeModularVTable {
//     pub create: unsafe extern "system" fn(threads: u32) -> Obj,
//     pub destroy_instance: unsafe extern "system" fn(modular: Obj),
//     pub subscribe: unsafe extern "system" fn(
//         modular: Obj,
//         subscribe: CSubscribe,
//         *mut CSubscriptionRef,
//     ) -> i32,
//     pub publish: unsafe extern "system" fn(modular: Obj, topic: *const c_char, data: CBuf),
//     pub register_module: unsafe extern "system" fn(
//         modular: Obj,
//         name: *const c_char,
//         module: CModule,
//         replace: bool,
//     ) -> i32,
//     pub remove_module: unsafe extern "system" fn(modular: Obj, name: *const c_char),
//     pub get_module_ref: unsafe extern "system" fn(modular: Obj, name: *const c_char) -> CModuleRef,
// }
//
// #[derive(Copy, Clone)]
// #[repr(C)]
// pub struct CBuf {
//     pub data: *const u8,
//     pub len: usize,
// }
//
// impl Default for CBuf {
//     fn default() -> Self {
//         Self {
//             data: null(),
//             len: 0,
//         }
//     }
// }
//
// #[repr(C)]
// pub struct CSubscribe {
//     pub user_data: Obj,
//     pub topic: *const c_char,
//
//     pub on_event: OnEvent,
//     pub on_unsubscribe: Option<Cleanup>,
// }
//
// #[derive(Copy, Clone)]
// #[repr(C)]
// pub struct CSubscriptionRef {
//     pub user_data: Obj,
//
//     pub subscription_ref: Obj,
//     pub unsubscribe: Cleanup,
// }
//
// impl Default for CSubscriptionRef {
//     fn default() -> Self {
//         extern "system" fn dummy(_: Obj) {}
//
//         Self {
//             user_data: Obj(null_mut()),
//             subscription_ref: Obj(null_mut()),
//             unsubscribe: dummy,
//         }
//     }
// }
//
// #[repr(C)]
// pub struct CModule {
//     pub ptr: Obj,
//
//     pub on_invoke:
//         unsafe extern "system" fn(ptr: Obj, method: *const c_char, data: CBuf, callback: CCallback),
//     pub on_drop: unsafe extern "system" fn(ptr: Obj),
// }
//
// unsafe impl Send for CModule {}
// unsafe impl Sync for CModule {}
//
// #[repr(C)]
// pub struct CModuleError {
//     pub code: i32,
//     pub name: *const c_char,
//     pub message: *const c_char,
// }
//
// #[repr(C)]
// pub struct CCallback {
//     pub ptr: Obj,
//
//     pub success: unsafe extern "system" fn(ptr: Obj, data: CBuf),
//     pub error: unsafe extern "system" fn(ptr: Obj, error: CModuleError),
//     pub unknown_method: unsafe extern "system" fn(ptr: Obj),
//     pub destroyed: unsafe extern "system" fn(ptr: Obj),
// }
//
// unsafe impl Send for CCallback {}
// unsafe impl Sync for CCallback {}
//
// #[derive(Copy, Clone)]
// #[repr(C)]
// pub struct CModuleRef {
//     pub ptr: Obj,
//     pub vtable: CModuleRefVTable,
// }
//
// #[derive(Copy, Clone)]
// #[repr(C)]
// pub struct CModuleRefVTable {
//     pub clone: unsafe extern "system" fn(ptr: Obj) -> CModuleRef,
//     pub drop: unsafe extern "system" fn(ptr: Obj),
//     pub invoke:
//         unsafe extern "system" fn(ptr: Obj, action: *const c_char, data: CBuf, callback: CCallback),
// }
//
// pub type OnEvent =
//     unsafe extern "system" fn(subscription: CSubscriptionRef, topic: *const c_char, data: CBuf);
//
// pub type Cleanup = unsafe extern "system" fn(_: Obj);
