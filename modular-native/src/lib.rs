#![allow(clippy::missing_safety_doc)]

mod module;
mod modular;
mod vtable;

use crate::module::NativeCModule;
use bytes::Bytes;
use futures::Sink;
use modular_core::request::ModuleRequest;
use modular_rs::core::Modular;
use modular_sys::*;
use parking_lot::RwLock;
use std::ffi::{CStr, CString};
use std::future::Future;
use std::pin::Pin;
use std::ptr::{null, null_mut};
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
    modular: Modular,
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
