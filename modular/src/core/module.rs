use crate::core::modules::BoxModuleService;
use futures_util::future::BoxFuture;
use futures_util::TryFutureExt;
use std::marker::PhantomData;
use std::sync::Weak;
use std::task::{Context, Poll};
use tokio::sync::Mutex;
use tower::Service;
use modular_core::core::error::*;
use modular_core::core::module;
use modular_core::core::request::ModuleRequest;
use modular_core::core::response::ModuleResponse;

#[derive(Clone)]
pub struct Module<Request, Response>(pub(crate) Weak<Mutex<BoxModuleService<Request, Response>>>);

impl<Request, Response> module::Module<Request, Response> for Module<Request, Response> {
    type Future = Result<ModuleResponse<Response>, ModuleError>;

    async fn invoke(&self, request: ModuleRequest<Request>) -> Self::Future {
        let module = match self.0.upgrade() {
            Some(v) => v,
            None => {
                return Err(ModuleError::Destroyed);
            }
        };

        let mut v = module.lock().await;
        v.call(request).await
    }
}


// impl<Request, Response> Module<Request, Response> {
//     pub async fn invoke(
//         &self,
//         req: ModuleRequest<Request>,
//     ) -> Result<ModuleResponse<Response>, ModuleError> {
//         let module = match self.0.upgrade() {
//             Some(v) => v,
//             None => {
//                 return Err(ModuleError::Destroyed);
//             }
//         };
//
//         let mut v = module.lock().await;
//         v.call(req).await
//     }
// }

#[repr(transparent)]
pub(crate) struct ModuleService<S, Req, Request, Response>(
    pub S,
    pub PhantomData<(Req, Request, Response)>,
)
where
    S: Service<Req> + Send + 'static,
    Req: From<ModuleRequest<Request>> + Send,
    S::Error: Into<ModuleError> + Send + 'static,
    S::Response: Into<ModuleResponse<Response>> + Send + 'static,
    S::Future: Send + Sync;

impl<S, Req, Request, Response> Service<ModuleRequest<Request>>
    for ModuleService<S, Req, Request, Response>
where
    S: Service<Req> + Send + 'static,
    Req: From<ModuleRequest<Request>> + Send,
    S::Error: Into<ModuleError> + Send + 'static,
    S::Response: Into<ModuleResponse<Response>> + Send + 'static,
    S::Future: Send + Sync + 'static,
    Response: 'static,
{
    type Response = ModuleResponse<Response>;
    type Error = ModuleError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline(always)]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx).map_err(Into::into)
    }

    #[inline(always)]
    fn call(&mut self, req: ModuleRequest<Request>) -> Self::Future {
        Box::pin(
            self.0
                .call(req.into())
                .map_ok(Into::into)
                .map_err(Into::into),
        )
    }
}
