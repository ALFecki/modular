use crate::modules::ModuleRequest;
use bytes::Bytes;
use std::future::Future;


pub trait Module<Request = Bytes, Response = Bytes>
where
    Response: Send + 'static,
    Request: Send,
{
    type Future: Future + Send + 'static;

    fn invoke(&self, req: ModuleRequest<Request>) -> Self::Future;
}
