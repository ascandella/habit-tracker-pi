pub fn router(access: db::AccessLayer) -> axum::Router {
    axum::Router::new()
        .route("/api/current", axum::routing::get(current_streak))
        .with_state(access)
}

#[derive(serde::Deserialize, Debug)]
struct StreakArgs {
    timezone: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct StreakResponse {
    days: Option<u32>,
    active: bool,
    end: Option<String>,
}

enum StreakFetchError {
    InvalidTimezone,
    DataAccessError(db::DataAccessError),
}

impl axum::response::IntoResponse for StreakFetchError {
    fn into_response(self) -> axum::response::Response {
        let (status_code, error) = match self {
            Self::InvalidTimezone => (
                axum::http::StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "invalid timezone"}),
            ),
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

#[tracing::instrument(skip(access))]
async fn current_streak(
    axum::extract::State(access): axum::extract::State<db::AccessLayer>,
    options: axum::extract::Query<StreakArgs>,
) -> axum::response::Result<axum::Json<StreakResponse>> {
    let timezone: chrono_tz::Tz = options
        .timezone
        .parse()
        .map_err(|_| StreakFetchError::InvalidTimezone)?;

    let current_streak = access
        .current_streak(&timezone)
        .map_err(StreakFetchError::DataAccessError)?;

    let response = match current_streak {
        db::StreakData::NoData => StreakResponse {
            days: None,
            active: false,
            end: None,
        },
        db::StreakData::Streak(ref streak) => StreakResponse {
            days: Some(streak.days(&timezone) as u32),
            active: true,
            end: Some(streak.end().to_rfc3339()),
        },
    };
    Ok(axum::Json(response))
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
        router(db::in_memory().expect("in memory create"))
    }

    #[tokio::test]
    async fn current_no_timezone() {
        let app = create_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/current")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn current_invalid_timezone() {
        let app = create_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/current?timezone=invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
    async fn current_valid_timezone_no_data() {
        let app = create_router();
        let response = response_for_query(app, "/api/current?timezone=US/Pacific").await;

        assert!(!response.active);
        assert_eq!(response.days, None);
    }

    #[tokio::test]
    async fn current_valid_timezone_with_data() {
        let access = db::in_memory().expect("in memory create");
        access.record_event().unwrap();
        let app = router(access);
        let response = response_for_query(app, "/api/current?timezone=US/Pacific").await;

        assert!(response.active);
        assert_eq!(response.days, Some(1));
    }
}
