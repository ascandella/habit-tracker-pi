use tracing::info;

pub fn router(
    access: db::AccessLayer,
    refresh_sender: crossbeam_channel::Sender<()>,
    timezone: chrono_tz::Tz,
) -> axum::Router {
    axum::Router::new()
        .route("/api/current", axum::routing::get(current_streak))
        .route("/api/record", axum::routing::post(record_event))
        .with_state(AppState {
            access,
            timezone,
            refresh_sender,
        })
}

#[derive(Clone, Debug)]
struct AppState {
    access: db::AccessLayer,
    timezone: chrono_tz::Tz,
    refresh_sender: crossbeam_channel::Sender<()>,
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

enum WebApiError {
    DataAccessError(db::DataAccessError),
    RefreshError(crossbeam_channel::SendError<()>),
}

impl axum::response::IntoResponse for WebApiError {
    fn into_response(self) -> axum::response::Response {
        let (status_code, error) = match self {
            Self::DataAccessError(err) => {
                tracing::error!(%err, "Data access error in API fetch");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    serde_json::json!({"error": format!("data fetch error: {}", err)}),
                )
            }
            Self::RefreshError(err) => {
                tracing::error!(%err, "Refresh error");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    serde_json::json!({"error": format!("refresh device error: {}", err)}),
                )
            }
        };
        (status_code, axum::Json(error)).into_response()
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct RecordEvent {
    name: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct RecordResponse {
    ok: bool,
}

#[tracing::instrument(skip(app_state))]
async fn record_event(
    axum::extract::State(app_state): axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<RecordEvent>,
) -> axum::response::Result<axum::Json<RecordResponse>> {
    info!("Recording event via API");
    app_state
        .access
        .record_event(&payload.name)
        .map_err(WebApiError::DataAccessError)?;

    app_state
        .refresh_sender
        .send(())
        .map_err(WebApiError::RefreshError)?;

    Ok(axum::Json(RecordResponse { ok: true }))
}

#[tracing::instrument(skip(app_state))]
async fn current_streak(
    axum::extract::State(app_state): axum::extract::State<AppState>,
) -> axum::response::Result<axum::Json<StreakResponse>> {
    info!("Fetching current streak via API");
    let current_streak = app_state
        .access
        .current_streak(&app_state.timezone)
        .map_err(WebApiError::DataAccessError)?;

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

    fn create_router() -> (Router, db::AccessLayer) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let db = db::in_memory().expect("in memory create");
        std::thread::spawn(move || {
            let _ = rx.recv();
        });
        (router(db.clone(), tx, chrono_tz::UTC), db)
    }

    async fn response_for_record(app: Router, name: &str) -> RecordResponse {
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/record")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&RecordEvent {
                            name: name.to_string(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let streak_response = serde_json::from_slice(&body).unwrap();
        streak_response
    }

    async fn response_for_query(app: Router) -> StreakResponse {
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/current")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let streak_response = serde_json::from_slice(&body).unwrap();
        streak_response
    }

    #[tokio::test]
    async fn current_no_data() {
        let (app, _) = create_router();
        let response = response_for_query(app).await;

        assert!(!response.active);
        assert!(!response.active_today);
        assert_eq!(response.days, None);
    }

    #[tokio::test]
    async fn current_with_data() {
        let (app, access) = create_router();
        access.record_event("test").unwrap();
        let response = response_for_query(app).await;

        assert!(response.active);
        assert_eq!(response.days, Some(1));
        assert!(response.end.is_some());
        assert!(response.active_today);
    }

    #[tokio::test]
    async fn record_event_and_fetch() {
        let (app, _) = create_router();
        let response = response_for_record(app.clone(), "test event").await;
        assert!(response.ok);
        let response = response_for_query(app).await;

        assert!(response.active);
        assert_eq!(response.days, Some(1));
        assert!(response.end.is_some());
        assert!(response.active_today);
    }
}
