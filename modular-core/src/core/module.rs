use crate::core::error::ModuleError;
use std::future::Future;
use async_trait::async_trait;
use bytes::Bytes;
use crate::core::request::ModuleRequest;
use crate::core::response::ModuleResponse;

#[async_trait]
pub trait Module<Request = Bytes, Response = Bytes> {
    type Future: Future<Output = Result<ModuleResponse<Response>, ModuleError>> + Send + 'static;

    async fn invoke(&self, request: ModuleRequest<Request>) -> Self::Future;
}



