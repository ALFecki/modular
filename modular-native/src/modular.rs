use crate::{cstr_to_str, cstr_to_string, CStr, Subscribe, Subscription};
use bytes::Bytes;
use modular_core::modular::Modular;
use modular_core::modules::ModuleRequest;
use modular_sys::{CBuf, CModule, CModuleRef, CSubscribe, CSubscriptionRef};
use std::ffi::c_char;

pub struct CObj<T>(pub *mut T);

unsafe impl<V> Send for CObj<V> {}
unsafe impl<V> Sync for CObj<V> {}

pub struct CModularVTable<V> {
    create_instance: unsafe extern "system" fn(threads: u32) -> CObj<V>,
    destroy_instance: unsafe extern "system" fn(modular: CObj<V>),
    subscribe: unsafe extern "system" fn(
        modular: &V,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32,
    publish: unsafe extern "system" fn(modular: &V, topic: *const c_char, data: CBuf),
    register_module: unsafe extern "system" fn(
        modular: &V,
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
        modular: &V,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32 {
        let res = modular
            .subscribe::<Subscribe, ()>(&cstr_to_str!(subscribe.topic).unwrap(), None);
        match res {
            Ok(_) => 0,
            Err(_) => -1,
        }
    }

    unsafe extern "system" fn publish(modular: &V, topic: *const c_char, data: CBuf) {
        if let Some(action) = cstr_to_string!(topic) {
            modular.publish(ModuleRequest::<Bytes> {
                action,
                body: Bytes::copy_from_slice(std::slice::from_raw_parts(data.data, data.len)),
            })
        }
    }

    unsafe extern "system" fn register_module(
        modular: &V,
        name: *const c_char,
        module: CModule,
        replace: bool,
    ) -> i32 {
        // if let Some(name) = cstr_to_str!(name) {
        //     return match modular.register_module(&*name, module.ptr) {
        //         Ok(_) => 0,
        //         Err(_) => -1,
        //     };
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

pub trait ModularNativeExt<M> {
    unsafe extern "C" fn __modular_create(&self, threads: u32) -> *mut M;
    unsafe extern "system" fn __modular_destroy(&self, modular: *mut M);
    unsafe extern "system" fn __modular_events_subscribe(
        &self,
        modular: &M,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32;
    unsafe extern "system" fn __modular_events_publish(
        &self,
        modular: &M,
        topic: *const c_char,
        buf: CBuf,
    );
    unsafe extern "system" fn __modular_events_unsubscribe(&self, subscription: *mut Subscription);
    unsafe extern "system" fn __modular_register_module(
        &self,
        modular: &M,
        name: *const c_char,
        module: CModule,
        replace: bool,
    ) -> i32;
    unsafe extern "system" fn __modular_remove_module(&self, modular: &M, name: *const c_char);
    unsafe extern "system" fn __modular_get_module_ref(
        &self,
        modular: &M,
        name: *const c_char,
    ) -> CModuleRef;
}

impl<V: 'static + Sized> ModularNativeExt<V> for CModular<V> {
    unsafe extern "C" fn __modular_create(&self, threads: u32) -> *mut V {
        (self.vtable.create_instance)(threads).0
    }

    unsafe extern "system" fn __modular_destroy(&self, modular: *mut V) {
        (self.vtable.destroy_instance)(CObj(modular))
    }

    unsafe extern "system" fn __modular_events_subscribe(&self, modular: &V, subscribe: CSubscribe, subscription: *mut CSubscriptionRef) -> i32 {
        (self.vtable.subscribe)(modular, subscribe, subscription)
    }

    unsafe extern "system" fn __modular_events_publish(&self, modular: &V, topic: *const c_char, buf: CBuf) {
        (self.vtable.publish)(modular, topic, buf)
    }

    unsafe extern "system" fn __modular_events_unsubscribe(&self, subscription: *mut Subscription) {
        todo!()
    }

    unsafe extern "system" fn __modular_register_module(&self, modular: &V, name: *const c_char, module: CModule, replace: bool) -> i32 {
        (self.vtable.register_module)(modular, name, module, replace)
    }

    unsafe extern "system" fn __modular_remove_module(&self, modular: &V, name: *const c_char) {
        todo!()
    }

    unsafe extern "system" fn __modular_get_module_ref(&self, modular: &V, name: *const c_char) -> CModuleRef {
        todo!()
    }
}