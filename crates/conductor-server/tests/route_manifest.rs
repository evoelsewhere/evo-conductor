use std::collections::{BTreeMap, BTreeSet};

use conductor_server::http::authorization::{
    route_manifest, RouteAuthentication, RouteMethod, EXPECTED_ROUTE_ACTIONS, MAX_LOGO_BYTES,
};

#[test]
fn manifest_is_complete_unique_and_matches_the_reviewed_baseline() {
    let manifest = route_manifest();
    manifest.validate().expect("valid route manifest");

    assert_eq!(manifest.routes.len(), EXPECTED_ROUTE_ACTIONS);
    assert_eq!(
        manifest
            .routes
            .iter()
            .map(|route| route.path)
            .collect::<BTreeSet<_>>()
            .len(),
        75
    );

    let mut methods = BTreeMap::new();
    let mut classes = BTreeMap::new();
    for route in &manifest.routes {
        *methods.entry(route.method.as_str()).or_insert(0usize) += 1;
        let class = match route.authentication {
            RouteAuthentication::ExplicitPublic => "public",
            RouteAuthentication::Bootstrap => "bootstrap",
            RouteAuthentication::Browser(_) => "browser",
            RouteAuthentication::Connection(_) => "connection",
        };
        *classes.entry(class).or_insert(0usize) += 1;
    }

    assert_eq!(
        methods,
        BTreeMap::from([
            ("DELETE", 5),
            ("GET", 41),
            ("PATCH", 6),
            ("POST", 32),
            ("PUT", 11),
        ])
    );
    assert_eq!(
        classes,
        BTreeMap::from([
            ("bootstrap", 1),
            ("browser", 77),
            ("connection", 11),
            ("public", 6),
        ])
    );
}

#[test]
fn only_the_explicit_safe_allowlist_is_public_or_bootstrap() {
    let manifest = route_manifest();
    let public = manifest
        .routes
        .iter()
        .filter_map(|route| {
            matches!(route.authentication, RouteAuthentication::ExplicitPublic)
                .then_some(route.route_id)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public,
        BTreeSet::from([
            "auth.login",
            "auth.sso.callback",
            "auth.sso.start",
            "health.read",
            "project.logo.read",
            "setup.status.read",
        ])
    );

    let bootstrap = manifest
        .routes
        .iter()
        .filter_map(|route| {
            matches!(route.authentication, RouteAuthentication::Bootstrap).then_some(route.route_id)
        })
        .collect::<Vec<_>>();
    assert_eq!(bootstrap, vec!["setup.complete"]);
}

#[test]
fn each_protected_action_has_typed_permission_or_scope_and_selector() {
    let manifest = route_manifest();
    for route in &manifest.routes {
        match &route.authentication {
            RouteAuthentication::Browser(policy) => {
                assert!(!policy.alternatives.is_empty(), "{}", route.route_id);
                assert_eq!(policy.requirement_id, route.route_id);
                assert!(policy
                    .alternatives
                    .iter()
                    .all(|alternative| alternative.permission.as_str().contains('.')));
            }
            RouteAuthentication::Connection(policy) => {
                assert_eq!(policy.requirement_id, route.route_id);
                assert!(!policy.required_scope.as_str().is_empty());
            }
            RouteAuthentication::ExplicitPublic | RouteAuthentication::Bootstrap => {}
        }
    }
}

#[test]
fn route_local_body_limits_are_attached_to_the_exact_actions() {
    let manifest = route_manifest();
    let limits = manifest
        .routes
        .iter()
        .filter_map(|route| {
            route
                .transport
                .body_limit_bytes
                .map(|limit| (route.route_id, route.method, limit))
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        limits,
        BTreeSet::from([
            ("project.logo.upload", RouteMethod::Put, MAX_LOGO_BYTES),
            (
                "resource.archive.import",
                RouteMethod::Post,
                conductor_server::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            ),
            (
                "resource.archive.inspect",
                RouteMethod::Post,
                conductor_server::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            ),
            (
                "resource.draft_archive.import",
                RouteMethod::Post,
                conductor_server::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            ),
            (
                "resource.plugin_archive.import",
                RouteMethod::Post,
                conductor_server::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            ),
            (
                "resource.plugin_archive.inspect",
                RouteMethod::Post,
                conductor_server::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES,
            ),
        ])
    );
}
