use crate::{cstr_to_str, cstr_to_string, CStr, Subscribe, Subscription, VTable};
use bytes::Bytes;
use futures::Sink;
use modular_core::error::{ModuleError, RegistryError, SubscribeError};
use modular_core::modular::Modular;
use modular_core::modules::{ModuleRequest, ModuleResponse};
use modular_sys::{CBuf, CModule, CModuleRef, CSubscribe, CSubscriptionRef};
use std::ffi::{c_char, c_void};
use std::future::Future;
use std::io::Read;
use std::ops::Deref;
use tower::Service;

pub struct CObj<T>(pub *mut T);

unsafe impl<V> Send for CObj<V> {}
unsafe impl<V> Sync for CObj<V> {}

pub struct CModularVTable<V> {
    create_instance: unsafe extern "system" fn(threads: u32) -> CObj<V>,
    destroy_instance: unsafe extern "system" fn(modular: CObj<V>),
    subscribe: unsafe extern "system" fn(
        modular: &CObj<V>,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32,
    publish: unsafe extern "system" fn(modular: &CObj<V>, topic: *const c_char, data: CBuf),
    register_module: unsafe extern "system" fn(
        modular: &CObj<V>,
        name: *const c_char,
        module: CModule,
        replace: bool,
    ) -> i32,
    /*    remove_module: unsafe extern "system" fn(modular: &CObj<V>, name: *const c_char),
    get_module_ref: unsafe extern "system" fn(modular: &CObj<V>, name: *const c_char) -> CModuleRef,*/
}

impl<V: Modular + Default> CModularVTable<V> {
    pub fn new() -> Self {
        Self {
            create_instance: Self::create_instance,
            destroy_instance: Self::destroy_instance,
            subscribe: Self::subscribe,
            publish: Self::publish,
            register_module: Self::register_module,
            /*            remove_module: ,
            get_module_ref: (),*/
        }
    }

    unsafe extern "system" fn create_instance(threads: u32) -> CObj<V> {
        #[cfg(not(target_family = "wasm"))]
        let runtime = {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(threads as usize)
                .build()
                .unwrap()
        };

        #[cfg(target_family = "wasm")]
        let runtime = {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
        };
        let modular = V::default();
        CObj::<V>(Box::into_raw(Box::new(modular)))
    }

    unsafe extern "system" fn destroy_instance(modular: CObj<V>) {
        let _ = Box::from_raw(modular.0);
    }

    unsafe extern "system" fn subscribe(
        modular: &CObj<V>,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32 {
        let res = modular
            .0
            .as_mut()
            .unwrap()
            .subscribe::<Subscribe, ()>(&cstr_to_str!(subscribe.topic).unwrap(), None);
        match res {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }

    unsafe extern "system" fn publish(modular: &CObj<V>, topic: *const c_char, data: CBuf) {
        if let Some(action) = cstr_to_string!(topic) {
            modular.0.as_mut().unwrap().publish(ModuleRequest::<Bytes> {
                action,
                body: Bytes::copy_from_slice(std::slice::from_raw_parts(data.data, data.len)),
            })
        }
    }

    unsafe extern "system" fn register_module(
        modular: &CObj<V>,
        name: *const c_char,
        module: CModule,
        replace: bool,
    ) -> i32 {
        // if let Some(name) = cstr_to_str!(name) {
        //     match modular.0.as_mut().unwrap().register_module(&*name, module.ptr) {
        //         Ok(_) => 0,
        //         Err(_) => -1,
        //     }
        // }
        -1
    }
}

pub struct CModular<V> {
    pub ptr: CObj<V>,
    pub vtable: CModularVTable<V>,
}

impl<V: Modular + Default> CModular<V> {
    pub fn wrap(value: V) -> Self {
        let ptr = Box::into_raw(Box::new(value));
        Self {
            ptr: CObj::<V>(ptr),
            vtable: CModularVTable::<V>::new(),
        }
    }
}

unsafe impl<V> Send for CModular<V> {}
unsafe impl<V> Sync for CModular<V> {}

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

pub trait ModularNativeExt<M> {
    unsafe extern "C" fn __modular_create(threads: u32) -> *mut M;
    unsafe extern "system" fn __modular_destroy(modular: *mut M);
    unsafe extern "system" fn __modular_events_subscribe(
        modular: &M,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32;
    unsafe extern "system" fn __modular_events_publish(
        modular: &M,
        topic: *const c_char,
        buf: CBuf,
    );
    unsafe extern "system" fn __modular_events_unsubscribe(subscription: *mut Subscription);
    unsafe extern "system" fn __modular_register_module(
        modular: &M,
        name: *const c_char,
        module: CModule,
        replace: bool,
    ) -> i32;
    unsafe extern "system" fn __modular_remove_module(modular: &M, name: *const c_char);
    unsafe extern "system" fn __modular_get_module_ref(
        modular: &M,
        name: *const c_char,
    ) -> CModuleRef;
}

impl<V: 'static + Sized> ModularNativeExt<V> for CModularVTable<V> {
    unsafe extern "C" fn __modular_create(threads: u32) -> *mut V {

    }

    unsafe extern "system" fn __modular_destroy(modular: *mut V) {
        todo!()
    }

    unsafe extern "system" fn __modular_events_subscribe(modular: &V, subscribe: CSubscribe, subscription: *mut CSubscriptionRef) -> i32 {
        todo!()
    }

    unsafe extern "system" fn __modular_events_publish(modular: &V, topic: *const c_char, buf: CBuf) {
        todo!()
    }

    unsafe extern "system" fn __modular_events_unsubscribe(subscription: *mut Subscription) {
        todo!()
    }

    unsafe extern "system" fn __modular_register_module(modular: &V, name: *const c_char, module: CModule, replace: bool) -> i32 {
        todo!()
    }

    unsafe extern "system" fn __modular_remove_module(modular: &V, name: *const c_char) {
        todo!()
    }

    unsafe extern "system" fn __modular_get_module_ref(modular: &V, name: *const c_char) -> CModuleRef {
        todo!()
    }
}