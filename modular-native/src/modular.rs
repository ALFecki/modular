use std::os::raw::c_char;
use std::panic::catch_unwind;
use modular_sys::{CBuf, CModule, CModuleRef, CSubscribe, CSubscriptionRef, Obj};
use crate::Subscription;

pub trait NativeModularExt<T> {
    unsafe extern "system" fn __modular_create(threads: u32) -> *mut T;
    unsafe extern "system" fn __modular_destroy(modular: *mut T);
    unsafe extern "system" fn __modular_events_subscribe(
        modular: &T,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32;
    unsafe extern "system" fn __modular_events_unsubscribe(subscription: Obj) {
        let _ = catch_unwind(|| {
            let _ = Box::from_raw(subscription.0 as *mut Subscription);
        })
            .map_err(|e| eprintln!("failed to drop subscription: {:?}", e));
    }
    unsafe extern "system" fn __modular_events_publish(
        modular: &T,
        topic: *const c_char,
        buf: CBuf,
    );
    unsafe extern "system" fn __modular_register_module(
        modular: &T,
        name: *const c_char,
        module: CModule,
        replace: bool,
    ) -> i32;
    unsafe extern "system" fn __modular_remove_module(modular: &T, name: *const c_char);
    unsafe extern "system" fn __modular_get_module_ref(
        modular: &T,
        name: *const c_char,
    ) -> CModuleRef;
}