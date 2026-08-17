use axum::extract::DefaultBodyLimit;
use axum::handler::Handler;
use axum::middleware;
use axum::routing::{delete, get, patch, post, put, MethodRouter};
use axum::Router;

use crate::core::state::AppState;

use super::boundary::{enforce_route_policy, BoundaryState};
use super::catalog::{RouteDefinition, RouteManifest, RouteMethod, RouteSpec};

pub(crate) trait RouteRegistrar {
    fn get<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static;

    fn post<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static;

    fn put<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static;

    fn patch<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static;

    fn delete<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static;
}

pub(crate) struct ClassifiedRouter {
    state: AppState,
    router: Router<AppState>,
    specs: Vec<RouteSpec>,
}

impl ClassifiedRouter {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            router: Router::new(),
            specs: vec![],
        }
    }

    fn register(
        &mut self,
        method: RouteMethod,
        path: &'static str,
        definition: RouteDefinition,
        mut method_router: MethodRouter<AppState>,
    ) {
        let spec = RouteSpec::new(method, path, definition);
        if let Some(limit) = spec.transport.body_limit_bytes {
            method_router = method_router.layer(DefaultBodyLimit::max(limit));
        }
        method_router = method_router.layer(middleware::from_fn_with_state(
            BoundaryState {
                app: self.state.clone(),
                spec: spec.clone(),
            },
            enforce_route_policy,
        ));

        let router = std::mem::replace(&mut self.router, Router::new());
        self.router = router.route(path, method_router);
        self.specs.push(spec);
    }

    pub fn finish(self) -> Router {
        RouteManifest::new(self.specs)
            .validate()
            .unwrap_or_else(|error| panic!("invalid classified route catalog: {error}"));
        self.router.with_state(self.state)
    }
}

impl RouteRegistrar for ClassifiedRouter {
    fn get<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Get, path, definition, get(handler));
    }

    fn post<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Post, path, definition, post(handler));
    }

    fn put<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Put, path, definition, put(handler));
    }

    fn patch<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Patch, path, definition, patch(handler));
    }

    fn delete<H, T>(&mut self, path: &'static str, definition: RouteDefinition, handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Delete, path, definition, delete(handler));
    }
}

#[derive(Default)]
pub(crate) struct ManifestCollector {
    specs: Vec<RouteSpec>,
}

impl ManifestCollector {
    fn register(&mut self, method: RouteMethod, path: &'static str, definition: RouteDefinition) {
        self.specs.push(RouteSpec::new(method, path, definition));
    }

    pub fn finish(self) -> RouteManifest {
        RouteManifest::new(self.specs)
    }
}

impl RouteRegistrar for ManifestCollector {
    fn get<H, T>(&mut self, path: &'static str, definition: RouteDefinition, _handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Get, path, definition);
    }

    fn post<H, T>(&mut self, path: &'static str, definition: RouteDefinition, _handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Post, path, definition);
    }

    fn put<H, T>(&mut self, path: &'static str, definition: RouteDefinition, _handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Put, path, definition);
    }

    fn patch<H, T>(&mut self, path: &'static str, definition: RouteDefinition, _handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Patch, path, definition);
    }

    fn delete<H, T>(&mut self, path: &'static str, definition: RouteDefinition, _handler: H)
    where
        H: Handler<T, AppState>,
        T: 'static,
    {
        self.register(RouteMethod::Delete, path, definition);
    }
}
