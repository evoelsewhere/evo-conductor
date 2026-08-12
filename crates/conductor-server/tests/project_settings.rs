mod support;

use axum::http::StatusCode;
use conductor_domain::{PrimaryRole, SetupRequest};
use support::test_app;

#[tokio::test]
async fn admin_manages_project_description_and_branding_exposes_it() {
    let app = test_app().await;
    let (_, admin) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "project-settings-test".into(),
                display_name: Some("Project settings test".into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "settings-admin@example.test".into(),
                admin_display_name: "Settings Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure project");
    let admin_token = app.token_for(&admin).await;
    let user_token = app.token_for_role(PrimaryRole::User).await;

    let (status, _) = app
        .patch(
            "/api/settings",
            Some(&user_token),
            serde_json::json!({"description": "not allowed"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, updated) = app
        .patch(
            "/api/settings",
            Some(&admin_token),
            serde_json::json!({
                "description": "  Governed agents, skills, and plugins for delivery teams.  "
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        updated["description"],
        "Governed agents, skills, and plugins for delivery teams."
    );

    let (status, branding) = app.get("/api/project", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{branding}");
    assert_eq!(branding["description"], updated["description"]);

    let (status, too_long) = app
        .patch(
            "/api/settings",
            Some(&admin_token),
            serde_json::json!({"description": "x".repeat(501)}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{too_long}");

    let (status, cleared) = app
        .patch(
            "/api/settings",
            Some(&admin_token),
            serde_json::json!({"description": "   "}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["description"].is_null());
}
