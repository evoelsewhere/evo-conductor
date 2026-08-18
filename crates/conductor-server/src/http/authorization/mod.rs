mod boundary;
mod catalog;
mod classified_router;

pub use boundary::{
    authorize_current_browser_target, authorize_current_browser_target_with_aggregate_fact,
    authorize_current_connection_target, RouteAuthorization,
};
pub use catalog::{
    BrowserRouteAlternative, BrowserRoutePolicy, ConnectionRoutePolicy, ManifestError,
    RouteAuthentication, RouteDefinition, RouteManifest, RouteMethod, RouteSpec,
    RouteTargetSelector, RouteTransport, EXPECTED_ROUTE_ACTIONS, MAX_LOGO_BYTES,
};

pub use crate::core::request_context::RequestContext;

pub(crate) use classified_router::{ClassifiedRouter, ManifestCollector, RouteRegistrar};

pub fn route_manifest() -> RouteManifest {
    crate::http::routes::manifest()
}
