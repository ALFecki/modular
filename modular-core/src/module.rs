use std::future::Future;
use crate::error::ModuleError;
use crate::response::ModuleResponse;

pub trait Module<Request, Response> {
    type Future: Future<Output = Result<ModuleResponse<Response>, ModuleError>> + Send + 'static;

}