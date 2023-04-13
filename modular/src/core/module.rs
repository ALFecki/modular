use crate::core::modules::BoxModuleService;
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, TryFutureExt};
use modular_core::error::ModuleError;
use modular_core::modules::{ModuleRequest, ModuleResponse};
use std::marker::PhantomData;
use std::sync::Weak;
use std::task::{Context, Poll};
use tokio::sync::Mutex;
use tower::Service;

#[derive(Clone)]
pub struct Module<Request, Response>(pub(crate) Weak<Mutex<BoxModuleService<Request, Response>>>);

impl<Request, Response> modular_core::module::Module<Request, Response>
    for Module<Request, Response>
where
    Response: Send + 'static,
    Request: Send + 'static,
{
    type Future = BoxFuture<
        'static,
        Result<BoxFuture<'static, Result<ModuleResponse<Response>, ModuleError>>, ModuleError>,
    >;

    fn invoke(&self, req: ModuleRequest<Request>) -> Self::Future {
        let module = match self.0.upgrade() {
            Some(v) => v,
            None => {
                return futures::future::err(ModuleError::Destroyed).boxed();
            }
        };
        async move {
            let mut v = module.lock().await;
            Ok(v.call(req))
        }
        .boxed()
    }
}

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
