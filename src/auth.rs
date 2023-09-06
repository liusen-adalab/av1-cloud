use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;

use actix_session::SessionExt;
use actix_web::dev::{forward_ready, Service, Transform};
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error, HttpMessage};
// use futures::future::{ok, Future, Ready};

use actix_casbin_auth::CasbinVals;

pub struct RoleExtractor;

impl<S, B> Transform<S, ServiceRequest> for RoleExtractor
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RoleExtractorMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RoleExtractorMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct RoleExtractorMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RoleExtractorMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let session = req.get_session();
        let srv = Rc::clone(&self.service);

        Box::pin(async move {
            let role = session
                .get::<String>("role")?
                .unwrap_or("anonymous".to_string());

            let vals = CasbinVals {
                subject: role,
                domain: None,
            };

            req.extensions_mut().insert(vals);

            srv.call(req).await
        })
    }
}
