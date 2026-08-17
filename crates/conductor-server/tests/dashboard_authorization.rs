mod support;

use axum::http::StatusCode;
use conductor_domain::PrimaryRole;
use support::test_app;

#[tokio::test]
async fn dashboard_allows_admin_and_contributor_but_denies_user() {
    let app = test_app().await;
    app.seed_project_identity().await;

    assert_eq!(
        app.get("/api/dashboard", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    for role in [PrimaryRole::Admin, PrimaryRole::Contribute] {
        let token = app.token_for_role(role).await;
        let (status, body) = app.get("/api/dashboard", Some(&token)).await;
        assert_eq!(status, StatusCode::OK, "{role:?}: {body}");
    }

    let user_token = app.token_for_role(PrimaryRole::User).await;
    assert_eq!(
        app.get("/api/dashboard", Some(&user_token)).await.0,
        StatusCode::FORBIDDEN
    );
}
