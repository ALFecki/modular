use futures::Sink;
use futures_util::SinkExt;
use parking_lot::Mutex;
use std::marker::PhantomData;
use bytes::Bytes;

use crate::core::pattern::Pattern;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use modular_core::request::ModuleRequest;

type EventsHandler<T> = (Pattern, UnboundedSender<(String, T)>);

pub struct EventsManager<T> {
    handlers: Mutex<Vec<EventsHandler<T>>>,
    _pd: PhantomData<T>,
}

impl<T> Default for EventsManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> EventsManager<T> {
    pub fn new() -> Self {
        Self {
            handlers: Default::default(),
            _pd: Default::default(),
        }
    }

    pub fn publish(&self, dest: &str, data: T)
    where
        T: Clone + Send + Sync + 'static,
    {
        let mut guard = self.handlers.lock();

        guard.retain(|(pattern, handler)| {
            if pattern.matches(dest) {
                let data = data.clone();
                let dest = dest.to_owned();

                handler.send((dest, data)).is_ok()
            } else {
                true
            }
        });
    }

    pub fn subscribe<L, E>(&self, pattern: Pattern, listener: L)
    where
        L: Sink<(String, T), Error = E> + Send + Sync + 'static,
        T: Send + Sync + 'static,
    {
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.handlers.lock().push((pattern, tx));

        tokio::spawn(async move {
            let mut listener = Box::pin(listener);
            while let Some((dest, data)) = rx.recv().await {
                if listener.send((dest, data)).await.is_err() {
                    break;
                }
            }
        });
    }
}
