#![allow(clippy::missing_safety_doc)]

mod module;

use crate::module::NativeCModule;
use bytes::Bytes;
use futures::Sink;
use modular_core::modular::Modular;
use modular_core::module::Module;
use modular_core::modules::*;
use modular_sys::*;
use parking_lot::RwLock;
use std::ffi::{CStr, CString};
use std::future::Future;
use std::os::raw::c_char;
use std::panic::catch_unwind;
use std::pin::Pin;
use std::ptr::{drop_in_place, null, null_mut};
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};
use tokio::runtime::Runtime;

#[macro_export]
macro_rules! cstr_to_string {
    ($arg:expr) => {
        unsafe { cstr_to_str!($arg).map(|i| i.to_string()) }
    };
}

#[macro_export]
macro_rules! cstr_to_str {
    ($arg:expr) => {
        if $arg.is_null() {
            None
        } else {
            Some(CStr::from_ptr($arg).to_string_lossy())
        }
    };
}

pub struct NativeModular {
    tokio_runtime: Arc<Runtime>,
    modular: modular_rs::core::Modular,
}

#[repr(C)]
pub struct VTable<M> {
    create_instance: unsafe extern "system" fn(threads: u32) -> *mut M,
    destroy_instance: unsafe extern "system" fn(modular: *mut M),
    subscribe: unsafe extern "system" fn(
        modular: &M,
        subscribe: CSubscribe,
        subscription: *mut CSubscriptionRef,
    ) -> i32,
    publish: unsafe extern "system" fn(modular: &M, topic: *const c_char, data: CBuf),
    register_module: unsafe extern "system" fn(
        modular: &M,
        name: *const c_char,
        module: CModule,
        replace: bool,
    ) -> i32,
    remove_module: unsafe extern "system" fn(modular: &M, name: *const c_char),
    get_module_ref: unsafe extern "system" fn(modular: &M, name: *const c_char) -> CModuleRef,
}

#[no_mangle]
pub unsafe extern "system" fn __modular_vtable() -> *const NativeModularVTable {
    static VTABLE: &VTable<NativeModular> = &VTable {
        create_instance: __modular_create,
        destroy_instance: __modular_destroy,
        subscribe: __modular_events_subscribe,
        publish: __modular_events_publish,
        register_module: __modular_register_module,
        remove_module: __modular_remove_module,
        get_module_ref: __modular_get_module_ref,
    };

    VTABLE as *const VTable<_> as _
}

pub unsafe extern "system" fn __modular_create(threads: u32) -> *mut NativeModular {
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

    let modular = modular_rs::core::Modular::default();

    Box::into_raw(Box::new(NativeModular {
        tokio_runtime: Arc::new(runtime),
        modular,
    }))
}

pub unsafe extern "system" fn __modular_destroy(modular: *mut NativeModular) {
    let _ = Box::from_raw(modular);
}

struct Subscribe {
    close_flag: Arc<RwLock<Option<()>>>,
    on_event: OnEvent,
    subscription: CSubscriptionRef,
    is_closed: bool,
}

impl Subscribe {
    #[allow(clippy::complexity)]
    fn poll_state(
        mut self: Pin<&mut Self>,
    ) -> Poll<Result<(), <Subscribe as Sink<(String, Bytes)>>::Error>> {
        if self.is_closed {
            return Poll::Ready(Err(()));
        }

        if self.close_flag.read().is_none() {
            self.is_closed = true;
            return Poll::Ready(Err(()));
        }

        Poll::Ready(Ok(()))
    }
}

impl Sink<(String, Bytes)> for Subscribe {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_state()
    }

    fn start_send(self: Pin<&mut Self>, item: (String, Bytes)) -> Result<(), Self::Error> {
        let _lock = self.close_flag.read();
        if _lock.is_none() {
            return Err(());
        }

        let topic = CString::new(item.0).expect("`null` in topic");
        let data = CBuf {
            data: item.1.as_ptr(),
            len: item.1.len(),
        };

        drop(_lock);
        unsafe { (self.on_event)(self.subscription, topic.as_ptr(), data) };

        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_state()
    }

    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_state()
    }
}

unsafe impl Send for Subscribe {}
unsafe impl Sync for Subscribe {}

pub struct Subscription {
    pub user_data: Obj,
    pub close_flag: Weak<RwLock<Option<()>>>,
    pub on_unsubscribe: Option<Cleanup>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(v) = self.close_flag.upgrade() {
            let mut guard = v.write();
            if guard.take().is_some() {
                if let Some(v) = self.on_unsubscribe {
                    unsafe { v(self.user_data) }
                }
            }
        }
    }
}

pub unsafe extern "system" fn __modular_events_subscribe(
    modular: &NativeModular,
    subscribe: CSubscribe,
    subscription: *mut CSubscriptionRef,
) -> i32 {
    let Some(topic) = cstr_to_str!(subscribe.topic) else {
        return 1
    };

    let flag = Arc::new(RwLock::new(Some(())));
    let weak_flag = Arc::downgrade(&flag);
    let subscription_ptr = Box::into_raw(Box::new(Subscription {
        user_data: subscribe.user_data,
        close_flag: weak_flag,
        on_unsubscribe: subscribe.on_unsubscribe,
    }));

    let subscription_ref = CSubscriptionRef {
        user_data: subscribe.user_data,
        subscription_ref: Obj(subscription_ptr.cast()),
        unsubscribe: __modular_events_unsubscribe,
    };

    let subscribe = Subscribe {
        close_flag: flag,
        on_event: subscribe.on_event,
        subscription: subscription_ref,
        is_closed: false,
    };

    let handle = modular.tokio_runtime.handle();
    let _guard = handle.enter();

    match modular.modular.subscribe(&topic, Some(subscribe)) {
        Ok(_) => {
            *subscription = subscription_ref;

            0
        }
        Err(err) => {
            drop_in_place(subscription_ptr);

            match err {
                SubscribeError::InvalidPattern(_) => -1,
            }
        }
    }
}

pub unsafe extern "system" fn __modular_events_publish(
    modular: &NativeModular,
    topic: *const c_char,
    buf: CBuf,
) {
    let topic = cstr_to_str!(topic).expect("topic must not be null");
    let bytes = Bytes::copy_from_slice(std::slice::from_raw_parts(buf.data, buf.len));

    modular.modular.publish(ModuleRequest::new(&topic, bytes))
}

pub unsafe extern "system" fn __modular_events_unsubscribe(subscription: Obj) {
    let _ = catch_unwind(|| {
        let _ = Box::from_raw(subscription.0 as *mut Subscription);
    })
    .map_err(|e| eprintln!("failed to drop subscription: {:?}", e));
}

pub unsafe extern "system" fn __modular_register_module(
    modular: &NativeModular,
    name: *const c_char,
    module: CModule,
    replace: bool,
) -> i32 {
    let name = cstr_to_str!(name).expect("module name can't be null");

    let module = NativeCModule(module);

    let result = if replace {
        modular.modular.register_or_replace_module(&name, module);

        Ok(())
    } else {
        modular.modular.register_module(&name, module)
    };

    match result {
        Ok(_) => 0,
        Err(err) => match err {
            RegistryError::AlreadyExists => -1,
        },
    }
}

pub unsafe extern "system" fn __modular_remove_module(
    modular: &NativeModular,
    name: *const c_char,
) {
    if let Some(v) = cstr_to_str!(name) {
        modular.modular.deregister_module(&v)
    }
}

pub unsafe extern "system" fn __modular_get_module_ref(
    modular: &NativeModular,
    name: *const c_char,
) -> CModuleRef {
    static C_MODULE_REF_VTABLE: CModuleRefVTable = CModuleRefVTable {
        clone,
        drop,
        invoke,
    };

    #[derive(Clone)]
    pub struct RtModule {
        runtime: Weak<Runtime>,
        module: modular_rs::core::modules::Module<Bytes, Bytes>,
    }

    let name = cstr_to_str!(name).expect("name can't be empty");
    let Some(module) = modular.modular.get_module(&name) else {
        return CModuleRef {
            ptr: Obj(null_mut()),
            vtable: C_MODULE_REF_VTABLE,
        };
    };

    let module = RtModule {
        runtime: Arc::downgrade(&modular.tokio_runtime),
        module,
    };

    unsafe extern "system" fn clone(ptr: Obj) -> CModuleRef {
        let v = &*(ptr.0 as *mut RtModule);
        let new_module = v.clone();

        CModuleRef {
            ptr: Obj(Box::into_raw(Box::new(new_module)).cast()),
            vtable: C_MODULE_REF_VTABLE,
        }
    }

    unsafe extern "system" fn drop(ptr: Obj) {
        let _ = Box::from_raw(ptr.0 as *mut RtModule);
    }

    unsafe extern "system" fn invoke(
        ptr: Obj,
        action: *const c_char,
        data: CBuf,
        callback: CCallback,
    ) {
        let RtModule { runtime, module } = (*(ptr.0 as *mut RtModule)).clone();

        let action = CStr::from_ptr(action).to_string_lossy().to_string();
        let data = Bytes::copy_from_slice(std::slice::from_raw_parts(data.data, data.len));

        if let Some(v) = runtime.upgrade() {
            let task = ModuleTask {
                task: Box::pin(async move {
                    match module.invoke(ModuleRequest::new(&action, data)).await {
                        Ok(response) => {
                            if let Ok(response) = response.await {
                                let buf = CBuf {
                                    data: response.data.as_ptr(),
                                    len: response.data.len(),
                                };
                                (callback.success)(callback.ptr, buf)
                            }
                        }
                        Err(error) => match error {
                            ModuleError::UnknownMethod => (callback.unknown_method)(callback.ptr),
                            ModuleError::Custom(v) => {
                                let name = v.name.map(|v| CString::new(v).unwrap());
                                let message = v.message.map(|v| CString::new(v).unwrap());

                                let module_error = CModuleError {
                                    code: v.code,
                                    name: name.as_ref().map(|i| i.as_ptr()).unwrap_or(null()),
                                    message: message.as_ref().map(|i| i.as_ptr()).unwrap_or(null()),
                                };

                                (callback.error)(callback.ptr, module_error)
                            }
                            ModuleError::Destroyed => (callback.destroyed)(callback.ptr),
                        },
                    };
                }),
                on_drop: Some(Box::new(move || (callback.destroyed)(callback.ptr))),
            };

            v.spawn(task);
        } else {
            (callback.destroyed)(callback.ptr)
        }
    }

    CModuleRef {
        ptr: Obj(Box::into_raw(Box::new(module)).cast()),
        vtable: C_MODULE_REF_VTABLE,
    }
}

struct ModuleTask<D, F>
where
    F: Future<Output = ()> + Send + Unpin,
    D: FnOnce() + Send + Unpin,
{
    task: F,
    on_drop: Option<D>,
}

impl<D, F> Drop for ModuleTask<D, F>
where
    F: Future<Output = ()> + Send + Unpin,
    D: FnOnce() + Send + Unpin,
{
    fn drop(&mut self) {
        if let Some(f) = self.on_drop.take() {
            f()
        }
    }
}

impl<D, F> Future for ModuleTask<D, F>
where
    F: Future<Output = ()> + Send + Unpin,
    D: FnOnce() + Send + Unpin,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.on_drop.take();

        Pin::new(&mut self.task).poll(cx)
    }
}

#[test]
fn a() {}
