use crate::cstr_to_string;
use crate::*;
use bytes::Bytes;
use modular_core::error::*;
use modular_core::response::ModuleResponse;
use modular_rs::core::modules::{ModuleRequest, ModuleResponse};
use modular_sys::*;
use parking_lot::RwLock;
use std::ffi::CString;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};

#[repr(transparent)]
pub struct NativeCModule(pub CModule);

impl Drop for NativeCModule {
    fn drop(&mut self) {
        unsafe { (self.0.on_drop)(self.0.ptr) }
    }
}

impl tower::Service<ModuleRequest> for NativeCModule {
    type Response = ModuleResponse;
    type Error = ModuleError;
    type Future = CModuleFuture;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: ModuleRequest) -> Self::Future {
        let user_data = self.0.ptr;
        let f = self.0.on_invoke;

        let f = Box::new(move |state: CModuleFutureState| {
            let method = req.action;
            let action = CString::new(method).unwrap();

            let buf = CBuf {
                data: req.body.as_ptr(),
                len: req.body.len(),
            };

            let state = Box::into_raw(Box::new(state));

            unsafe extern "system" fn on_success(ptr: Obj, data: CBuf) {
                let state = Box::from_raw(ptr.0 as *mut CModuleFutureState);

                if let Some(v) = state.data.upgrade() {
                    let slice = std::slice::from_raw_parts(data.data, data.len);
                    let data = Bytes::copy_from_slice(slice);

                    *v.write() = Some(Ok(data));
                    state.waker.wake();
                }
            }

            unsafe extern "system" fn on_error(ptr: Obj, err: CModuleError) {
                let state = Box::from_raw(ptr.0 as *mut CModuleFutureState);

                if let Some(v) = state.data.upgrade() {
                    let err = ModuleError::Custom(CustomModuleError {
                        code: err.code,
                        name: cstr_to_string!(err.name),
                        message: cstr_to_string!(err.message),
                    });

                    *v.write() = Some(Err(err));
                    state.waker.wake();
                }
            }

            unsafe extern "system" fn on_unknown_method(ptr: Obj) {
                let state = Box::from_raw(ptr.0 as *mut CModuleFutureState);

                if let Some(v) = state.data.upgrade() {
                    *v.write() = Some(Err(ModuleError::UnknownMethod));
                    state.waker.wake();
                }
            }

            unsafe extern "system" fn on_destroyed(ptr: Obj) {
                let state = Box::from_raw(ptr.0 as *mut CModuleFutureState);

                if let Some(v) = state.data.upgrade() {
                    *v.write() = Some(Err(ModuleError::Destroyed));
                    state.waker.wake();
                }
            }

            let c_callback = CCallback {
                ptr: Obj(state.cast()),
                success: on_success,
                error: on_error,
                unknown_method: on_unknown_method,
                destroyed: on_destroyed,
            };

            unsafe { f(user_data, action.as_ptr(), buf, c_callback) }
        });

        CModuleFuture {
            f: Some(f),
            data: Default::default(),
        }
    }
}

pub struct CModuleFuture {
    f: Option<Box<dyn FnOnce(CModuleFutureState)>>,
    data: Arc<RwLock<Option<Result<Bytes, ModuleError>>>>,
}

unsafe impl Send for CModuleFuture {}
unsafe impl Sync for CModuleFuture {}

impl Future for CModuleFuture {
    type Output = Result<ModuleResponse, ModuleError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(f) = self.f.take() {
            let state = CModuleFutureState {
                waker: cx.waker().clone(),
                data: Arc::downgrade(&self.data),
            };

            f(state);
        }

        if let Some(v) = self.data.write().take() {
            Poll::Ready(v.map(ModuleResponse::new))
        } else {
            Poll::Pending
        }
    }
}

struct CModuleFutureState {
    waker: Waker,
    data: Weak<RwLock<Option<Result<Bytes, ModuleError>>>>,
}
