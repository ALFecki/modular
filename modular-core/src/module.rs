use crate::request::ModuleRequest;
use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait Module<Request = Bytes, Response = Bytes> {
    type Result: Send + 'static;

    async fn invoke(&self, request: ModuleRequest<Request>) -> Self::Result;
}
