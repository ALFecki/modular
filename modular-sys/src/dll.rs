use crate::core::{BoxModule, Modular, Module};
use crate::{
    CBuf, CCallback, CModule, CModuleError, CModuleRef, CSubscribe, CSubscriptionRef,
    NativeModularVTable, Obj,
};
use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use futures_util::{FutureExt, Stream, StreamExt};
use modular_core::error::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::ffi::{c_char, CStr, CString};
use std::future::Future;
use std::pin::Pin;
use std::ptr::null;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use tokio::runtime::Handle;
use tokio::spawn;
use tower::Service;

pub struct LibraryModular {
    ptr: Obj,
    vtable: NativeModularVTable,
}

impl LibraryModular {
    pub fn new() -> anyhow::Result<Self> {
        static LIB: OnceCell<libloading::Library> = OnceCell::new();
        let library = LIB.get_or_try_init(|| {
            let name = libloading::library_filename("modular_native");
            let lib = unsafe { libloading::Library::new(name)? };

            Result::<_, libloading::Error>::Ok(lib)
        })?;

        let (ptr, vtable) = unsafe {
            let vtable = library
                .get::<extern "system" fn() -> *const NativeModularVTable>(b"__modular_vtable")?(
            );

            let ptr = ((*vtable).create)(1);

            (ptr, *vtable)
        };

        Ok(Self { ptr, vtable })
    }
}

impl Modular for LibraryModular {
    fn subscribe(&self, topic: &str) -> anyhow::Result<BoxStream<'static, (String, Bytes)>> {
        let topic = CString::new(topic.to_string()).unwrap();
        let sink = NativeSubscriberSink {
            state: Arc::new(Mutex::new(SubscriberState {
                inner: Some(VecDeque::new()),
                waker: None,
            })),
        };

        let state = sink.state.clone();

        let subscribe = CSubscribe {
            user_data: Obj(Box::into_raw(Box::new(sink)).cast()),
            topic: topic.as_ptr(),
            on_event: NativeSubscriberSink::on_event,
            on_unsubscribe: Some(NativeSubscriberSink::on_close),
        };

        let mut subscription = CSubscriptionRef::default();
        unsafe { (self.vtable.subscribe)(self.ptr, subscribe, &mut subscription) };

        let stream = SubscriberStream {
            buffer: Default::default(),
            state,
            subscription_ref: subscription,
        };

        Ok(stream.boxed())
    }

    fn publish(&self, topic: &str, data: Bytes) {
        let topic = CString::new(topic.to_string()).unwrap();
        let buf = CBuf {
            data: data.as_ptr(),
            len: data.len(),
        };

        unsafe { (self.vtable.publish)(self.ptr, topic.as_ptr(), buf) }
    }

    fn register_module<S>(&self, name: &str, service: S)
    where
        S: Service<(String, Bytes), Response = Bytes, Error =ModuleError> + 'static + Send + Sync,
        S::Future: Send + Sync + 'static,
    {
        let inner = NativeModuleInner { service };
        let module = NativeModule {
            inner: Arc::new(Mutex::new(inner)),
            handle: Handle::current(),
        };
        let module = Box::into_raw(Box::new(module));

        let module = CModule {
            ptr: Obj(module.cast()),
            on_invoke: NativeModule::<S>::on_invoke,
            on_drop: NativeModule::<S>::on_drop,
        };

        let name = CString::new(name.to_string()).unwrap();

        unsafe {
            (self.vtable.register_module)(self.ptr, name.as_ptr(), module, false);
        }
    }

    fn get_module(&self, name: &str) -> Option<BoxModule> {
        let name = CString::new(name.to_string()).unwrap();
        let module = unsafe { (self.vtable.get_module_ref)(self.ptr, name.as_ptr()) };

        if module.ptr.0.is_null() {
            return None;
        }

        Some(Box::new(ModuleRef(module)))
    }

    fn deregister_module(&self, name: &str) {
        let name = CString::new(name.to_string()).unwrap();
        unsafe { (self.vtable.remove_module)(self.ptr, name.as_ptr()) }
    }
}

#[derive(Clone)]
struct SubscriberState {
    inner: Option<VecDeque<(String, Bytes)>>,
    waker: Option<Waker>,
}

struct NativeSubscriberSink {
    state: Arc<Mutex<SubscriberState>>,
}

impl NativeSubscriberSink {
    unsafe extern "system" fn on_event(
        subscription: CSubscriptionRef,
        topic: *const c_char,
        data: CBuf,
    ) {
        let topic = CStr::from_ptr(topic).to_string_lossy().to_string();
        let data = Bytes::copy_from_slice(std::slice::from_raw_parts(data.data, data.len));

        let this = &*(subscription.user_data.0 as *const Self);
        let mut state = this.state.lock();

        if let Some(mut v) = state.inner.as_mut() {
            v.push_back((topic, data));
        } else {
            (subscription.unsubscribe)(subscription.subscription_ref)
        }

        if let Some(ref v) = state.waker {
            v.wake_by_ref()
        }
    }

    unsafe extern "system" fn on_close(this: Obj) {
        let this = Box::from_raw(this.0 as *mut Self);
        this.state.lock().inner.take();
    }
}

struct SubscriberStream {
    buffer: VecDeque<(String, Bytes)>,
    state: Arc<Mutex<SubscriberState>>,
    subscription_ref: CSubscriptionRef,
}

impl Drop for SubscriberStream {
    fn drop(&mut self) {
        unsafe { (self.subscription_ref.unsubscribe)(self.subscription_ref.subscription_ref) }
    }
}

impl Stream for SubscriberStream {
    type Item = (String, Bytes);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        {
            let state = self.state.clone();
            let mut guard = state.lock();

            match guard.inner.replace(VecDeque::new()) {
                Some(v) => self.buffer.extend(v),
                None => return Poll::Ready(None),
            };

            let register_waker = match guard.waker.as_ref() {
                Some(v) => !v.will_wake(cx.waker()),
                None => true,
            };

            if register_waker {
                guard.waker = Some(cx.waker().clone())
            }
        }

        if let Some(next) = self.buffer.pop_front() {
            return Poll::Ready(Some(next));
        }

        Poll::Pending
    }
}

struct NativeModule<S> {
    inner: Arc<Mutex<NativeModuleInner<S>>>,
    handle: Handle,
}

#[repr(transparent)]
struct NativeModuleInner<S> {
    service: S,
}

impl<S> NativeModule<S>
where
    S: Service<(String, Bytes), Response = Bytes, Error =ModuleError> + Send + Sync + 'static,
    S::Future: Future<Output = Result<Bytes, ModuleError>> + Send + Sync + 'static,
{
    unsafe extern "system" fn on_invoke(
        ptr: Obj,
        method: *const c_char,
        data: CBuf,
        callback: CCallback,
    ) {
        let this = &*(ptr.0 as *const Self);
        let _guard = this.handle.enter();

        let mut service = this.inner.clone();
        let method = CStr::from_ptr(method).to_string_lossy().to_string();
        let data = Bytes::copy_from_slice(std::slice::from_raw_parts(data.data, data.len));

        spawn(async move {
            let mut service = service.lock();
            let v = service.service.call((method, data)).await;
            match v {
                Ok(v) => unsafe {
                    let buf = CBuf {
                        data: v.as_ptr(),
                        len: v.len(),
                    };

                    (callback.success)(callback.ptr, buf)
                },
                Err(error) => match error {
                    ModuleError::UnknownMethod => (callback.unknown_method)(callback.ptr),
                    ModuleError::Custom(err) => {
                        let name = err.name.map(|v| CString::new(v).unwrap());
                        let message = err.message.map(|v| CString::new(v).unwrap());

                        (callback.error)(
                            callback.ptr,
                            CModuleError {
                                code: err.code,
                                name: name.as_ref().map(|i| i.as_ptr()).unwrap_or(null()),
                                message: message.as_ref().map(|i| i.as_ptr()).unwrap_or(null()),
                            },
                        )
                    }
                    ModuleError::Destroyed => (callback.destroyed)(callback.ptr),
                },
            }
        });
    }

    unsafe extern "system" fn on_drop(ptr: Obj) {
        let _ = unsafe { Box::from_raw(ptr.0 as *mut Self) };
    }
}

impl<S> Service<(String, Bytes)> for NativeModuleInner<S>
where
    S: Service<(String, Bytes), Response = Bytes, Error =ModuleError> + 'static,
    S::Future: Send + Sync + 'static,
{
    type Response = Bytes;
    type Error = ModuleError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: (String, Bytes)) -> Self::Future {
        let v = self.service.call(req);
        Box::pin(v)
    }
}

struct ModuleRef(CModuleRef);

impl Drop for ModuleRef {
    fn drop(&mut self) {
        unsafe { (self.0.vtable.drop)(self.0.ptr) }
    }
}

impl Clone for ModuleRef {
    fn clone(&self) -> Self {
        let new = unsafe { (self.0.vtable.clone)(self.0.ptr) };
        Self(new)
    }
}

impl Module for ModuleRef {
    type Future = BoxFuture<'static, Result<Bytes, ModuleError>>;

    fn invoke(&self, method: &str, data: Bytes) -> Self::Future {
        let method = CString::new(method).unwrap();

        let inner = self.0;

        ModuleCallbackFuture::new(move |state| {
            let state = Box::into_raw(Box::new(state));

            let callback = CCallback {
                ptr: Obj(state.cast()),
                success: ModuleCallbackFutureState::on_success,
                error: ModuleCallbackFutureState::error,
                unknown_method: ModuleCallbackFutureState::unknown_method,
                destroyed: ModuleCallbackFutureState::destroyed,
            };

            let buf = CBuf {
                data: data.as_ptr(),
                len: data.len(),
            };

            unsafe { (inner.vtable.invoke)(inner.ptr, method.as_ptr(), buf, callback) }
        })
        .boxed()
    }
}

struct ModuleCallbackFutureState {
    waker: Waker,
    data: Arc<Mutex<Option<Result<Bytes, ModuleError>>>>,
}

impl ModuleCallbackFutureState {
    unsafe extern "system" fn with<F: FnOnce(&Self) -> Result<Bytes, ModuleError>>(
        obj: Obj,
        f: F,
    ) {
        let this = Box::from_raw(obj.0 as *mut Self);
        {
            let this = &*this;
            *this.data.lock() = Some(f(this));
        }

        this.waker.wake()
    }

    unsafe extern "system" fn on_success(this: Obj, data: CBuf) {
        let data = Bytes::copy_from_slice(std::slice::from_raw_parts(data.data, data.len));

        Self::with(this, |state| Ok(data));
    }

    unsafe extern "system" fn unknown_method(this: Obj) {
        Self::with(this, |state| Err(ModuleError::UnknownMethod));
    }

    unsafe extern "system" fn error(this: Obj, error: CModuleError) {
        Self::with(this, |state| {
            Err(ModuleError::Custom(CustomModuleError {
                code: error.code,
                name: if !error.name.is_null() {
                    let temp_name = Some(CStr::from_ptr(error.name).to_string_lossy());
                    temp_name.map(|i| i.to_string())
                } else {
                    None
                },
                message: if !error.message.is_null() {
                    let temp_name = Some(CStr::from_ptr(error.message).to_string_lossy());
                    temp_name.map(|i| i.to_string())
                } else {
                    None
                },
            }))
        });
    }

    unsafe extern "system" fn destroyed(this: Obj) {
        Self::with(this, |state| Err(ModuleError::Destroyed));
    }
}

struct ModuleCallbackFuture<F>
where
    F: FnOnce(ModuleCallbackFutureState),
{
    f: Option<F>,
    state: Arc<Mutex<Option<Result<Bytes, ModuleError>>>>,
}

impl<F> ModuleCallbackFuture<F>
where
    F: FnOnce(ModuleCallbackFutureState),
{
    fn new(f: F) -> Self {
        Self {
            f: Some(f),
            state: Default::default(),
        }
    }
}

impl<F: FnOnce(ModuleCallbackFutureState) + Unpin> Future for ModuleCallbackFuture<F> {
    type Output = Result<Bytes, ModuleError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(v) = self.f.take() {
            let state = ModuleCallbackFutureState {
                waker: cx.waker().clone(),
                data: self.state.clone(),
            };

            v(state);

            return Poll::Pending;
        }

        if let Some(mut v) = self.state.try_lock() {
            match v.take() {
                Some(v) => Poll::Ready(v),
                None => Poll::Pending,
            }
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}