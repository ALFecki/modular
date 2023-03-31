use crate::core::error::ModuleError;
use crate::core::request::ModuleRequest;
use crate::core::response::ModuleResponse;
use async_trait::async_trait;
use bytes::Bytes;
use std::future::Future;

#[async_trait]
pub trait Module<Request = Bytes, Response = Bytes> {
    type Future: Future<Output = Result<ModuleResponse<Response>, ModuleError>> + Send + 'static;

    async fn invoke(&self, request: ModuleRequest<Request>) -> Self::Future;
}
