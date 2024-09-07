pub fn router(access: db::AccessLayer, timezone: chrono_tz::Tz) -> axum::Router {
    axum::Router::new()
        .route("/api/current", axum::routing::get(current_streak))
        .with_state(AppState { access, timezone })
}

#[derive(Clone, Debug)]
struct AppState {
    access: db::AccessLayer,
    timezone: chrono_tz::Tz,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct StreakResponse {
    days: Option<u32>,
    active: bool,
    end: Option<String>,
    active_today: bool,
}

impl StreakResponse {
    fn from_timezone(streak: db::StreakData, timezone: &impl chrono::TimeZone) -> Self {
        match streak {
            db::StreakData::NoData => StreakResponse {
                days: None,
                active: false,
                end: None,
                active_today: false,
            },
            db::StreakData::Streak(ref streak) => StreakResponse {
                days: Some(streak.days(timezone) as u32),
                active: true,
                end: Some(streak.end().to_rfc3339()),
                active_today: streak.active_today(timezone),
            },
        }
    }
}

enum StreakFetchError {
    DataAccessError(db::DataAccessError),
}

impl axum::response::IntoResponse for StreakFetchError {
    fn into_response(self) -> axum::response::Response {
        let (status_code, error) = match self {
            Self::DataAccessError(err) => {
                tracing::error!(%err, "Data access error in API fetch");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    serde_json::json!({"error": format!("data fetch error: {}", err)}),
                )
            }
        };
        (status_code, axum::Json(error)).into_response()
    }
}

#[tracing::instrument(skip(app_state))]
async fn current_streak(
    axum::extract::State(app_state): axum::extract::State<AppState>,
) -> axum::response::Result<axum::Json<StreakResponse>> {
    let current_streak = app_state
        .access
        .current_streak(&app_state.timezone)
        .map_err(StreakFetchError::DataAccessError)?;

    Ok(axum::Json(StreakResponse::from_timezone(
        current_streak,
        &app_state.timezone,
    )))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    fn create_router() -> Router {
        router(db::in_memory().expect("in memory create"), chrono_tz::UTC)
    }

    async fn response_for_query(app: Router, uri: &str) -> StreakResponse {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let streak_response: StreakResponse = serde_json::from_slice(&body).unwrap();
        streak_response
    }

    #[tokio::test]
    async fn current_no_data() {
        let app = create_router();
        let response = response_for_query(app, "/api/current").await;

        assert!(!response.active);
        assert!(!response.active_today);
        assert_eq!(response.days, None);
    }

    #[tokio::test]
    async fn current_with_data() {
        let access = db::in_memory().expect("in memory create");
        access.record_event().unwrap();
        let app = router(access, chrono_tz::UTC);
        let response = response_for_query(app, "/api/current").await;

        assert!(response.active);
        assert_eq!(response.days, Some(1));
        assert!(response.end.is_some());
        assert!(response.active_today);
    }
}
