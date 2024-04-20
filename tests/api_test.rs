#[cfg(test)]
mod api_test {
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::Router;
    use mintpool::api;
    use mintpool::api::{with_admin_routes, AppState};

    use mintpool::config::Config;
    use mintpool::rules::RulesEngine;
    use mintpool::run::start_p2p_services;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_routes() {
        let mut config = Config::test_default();
        config.api_port = 1111;

        let router = make_test_router(&config).await;

        let res = router
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    async fn make_test_router(config: &Config) -> Router {
        let mut rules = RulesEngine::new(config);
        rules.add_default_rules();
        let ctl = start_p2p_services(config, rules).await.unwrap();

        let router = api::router_with_defaults(config);
        let state = AppState::from(config, ctl.clone()).await;

        with_admin_routes(state.clone(), router).with_state(state)
    }
}
