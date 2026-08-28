use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Path, State},
    http::{header, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    FromRow, SqlitePool,
};
use std::{
    collections::{HashMap, VecDeque},
    env,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};
use tokio::sync::Mutex;
use tower_http::{limit::RequestBodyLimitLayer, services::ServeDir};
use tracing::{error, info};

const SESSION_COOKIE: &str = "piv_session";
const SESSION_DAYS: i64 = 14;

struct AppState {
    db: SqlitePool,
    production: bool,
    build_sha: String,
    static_dir: PathBuf,
    rate_windows: Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("not signed in")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Validation(String),
    #[error("service error")]
    Internal,
    #[error("too many requests")]
    RateLimited,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "Sign in to continue.".to_string()),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "That item was not found or has expired.".to_string(),
            ),
            Self::Validation(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Private Intake could not complete that request. Try again.".to_string(),
            ),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many requests from this connection. Wait a minute and try again.".to_string(),
            ),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        error!(kind = "database", error = %error, "request failed");
        Self::Internal
    }
}

#[derive(Serialize, FromRow)]
struct Workspace {
    business_name: String,
    timezone: String,
    region: String,
    deletion_days: i64,
}

#[derive(Deserialize)]
struct SetupInput {
    business_name: String,
    passphrase: String,
    timezone: String,
    region: String,
    deletion_days: i64,
}

#[derive(Deserialize)]
struct LoginInput {
    passphrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct FormField {
    id: String,
    label: String,
    field_type: String,
    required: bool,
    visibility: String,
    sort_order: i64,
    options_json: String,
}

#[derive(Serialize)]
struct PublicField {
    id: String,
    label: String,
    field_type: String,
    required: bool,
    options: Vec<String>,
}

#[derive(Serialize)]
struct PublicForm {
    business_name: String,
    region: String,
    deletion_days: i64,
    fields: Vec<PublicField>,
}

#[derive(Deserialize)]
struct FieldInput {
    id: Option<String>,
    label: String,
    field_type: String,
    required: bool,
    visibility: String,
    options: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct FormInput {
    fields: Vec<FieldInput>,
}

#[derive(Deserialize)]
struct BookingInput {
    values: std::collections::HashMap<String, String>,
    #[serde(default)]
    website: String,
}

#[derive(Serialize, FromRow)]
struct BookingRow {
    id: String,
    created_at: String,
    delete_at: String,
    status: String,
    worker_name: Option<String>,
}

#[derive(Serialize, FromRow, Clone)]
struct ResponseRow {
    field_id: String,
    label_snapshot: String,
    visibility_snapshot: String,
    value: String,
    sort_order: i64,
}

#[derive(Serialize)]
struct BookingDetail {
    id: String,
    created_at: String,
    delete_at: String,
    status: String,
    worker_name: Option<String>,
    responses: Vec<ResponseRow>,
}

#[derive(Deserialize)]
struct AssignmentInput {
    worker_name: String,
    expires_hours: i64,
}

#[derive(Deserialize)]
struct StatusInput {
    status: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://private-intake.db?mode=rwc".to_string());
    let options = SqliteConnectOptions::from_str(&db_url)
        .expect("valid DATABASE_URL")
        .create_if_missing(true)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .expect("database connection");
    sqlx::migrate!()
        .run(&db)
        .await
        .expect("database migrations");

    let state = Arc::new(AppState {
        db,
        production: env::var("APP_ENV")
            .map(|v| v == "production")
            .unwrap_or(false),
        build_sha: env::var("BUILD_SHA").unwrap_or_else(|_| "development".to_string()),
        static_dir: PathBuf::from("frontend/dist"),
        rate_windows: Mutex::new(HashMap::new()),
    });
    purge_expired(&state.db).await.ok();

    let app = app(state)
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(64 * 1024));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("port is available");
    info!(port, "Private Intake ready");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("server exits cleanly");
}

fn app(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/api/setup", post(setup))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/session", get(session))
        .route("/api/form", get(admin_form).put(update_form))
        .route("/api/form/public", get(public_form))
        .route("/api/bookings", get(list_bookings).post(create_booking))
        .route("/api/bookings/export.csv", get(export_bookings))
        .route(
            "/api/bookings/{id}",
            get(get_booking).delete(delete_booking),
        )
        .route("/api/bookings/{id}/preview", get(worker_preview))
        .route("/api/bookings/{id}/assign", post(assign_worker))
        .route("/api/bookings/{id}/status", put(update_status))
        .route("/api/worker/{token}", get(worker_brief))
        .with_state(state.clone());

    // Serve the application shell only for client routes that actually exist.
    // `ServeDir::not_found_service` keeps a 404 status even when it sends the
    // index body, which breaks direct visits and refreshes of SPA routes.
    let client_routes = Router::new()
        .route("/", get(spa_index))
        .route("/book", get(spa_index))
        .route("/admin", get(spa_index))
        .route("/privacy", get(spa_index))
        .route("/terms", get(spa_index))
        .route("/worker/{token}", get(spa_index))
        .with_state(state.clone());

    api.merge(client_routes)
        .fallback_service(ServeDir::new(state.static_dir.clone()))
        .layer(middleware::from_fn(security_headers))
}

async fn spa_index(State(state): State<Arc<AppState>>) -> Response {
    match tokio::fs::read(state.static_dir.join("index.html")).await {
        Ok(body) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response(),
        Err(error) => {
            error!(kind = "static", error = %error, "could not read application shell");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn security_headers(request: Request<Body>, next: Next) -> Response {
    let asset = request.uri().path().starts_with("/assets/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert("content-security-policy", HeaderValue::from_static("default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' https://api.sociobot.in; base-uri 'self'; form-action 'self' https://api.sociobot.in; frame-ancestors 'none'"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if asset {
            "public, max-age=31536000, immutable"
        } else {
            "no-store"
        }),
    );
    response
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "status": "ok", "build": state.build_sha }))
}

async fn setup(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(input): Json<SetupInput>,
) -> Result<Response, AppError> {
    enforce_rate(&state, peer.ip(), 20).await?;
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(&state.db)
        .await?;
    if exists > 0 {
        return Err(AppError::Validation(
            "This vault is already configured. Sign in instead.".into(),
        ));
    }
    validate_setup(&input)?;
    let passphrase_hash = hash_passphrase(&input.passphrase)?;
    let now = Utc::now().to_rfc3339();
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT INTO workspaces (id, business_name, passphrase_hash, timezone, region, deletion_days, created_at) VALUES (1, ?, ?, ?, ?, ?, ?)")
        .bind(input.business_name.trim()).bind(passphrase_hash).bind(input.timezone.trim()).bind(input.region.trim()).bind(input.deletion_days).bind(now)
        .execute(&mut *tx).await?;
    seed_fields(&mut tx).await?;
    tx.commit().await?;
    create_session_response(&state).await
}

async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(input): Json<LoginInput>,
) -> Result<Response, AppError> {
    enforce_rate(&state, peer.ip(), 20).await?;
    let stored: Option<String> =
        sqlx::query_scalar("SELECT passphrase_hash FROM workspaces WHERE id = 1")
            .fetch_optional(&state.db)
            .await?;
    let stored =
        stored.ok_or_else(|| AppError::Validation("Set up the vault before signing in.".into()))?;
    let parsed = PasswordHash::new(&stored).map_err(|_| AppError::Internal)?;
    if Argon2::default()
        .verify_password(input.passphrase.as_bytes(), &parsed)
        .is_err()
    {
        return Err(AppError::Unauthorized);
    }
    create_session_response(&state).await
}

async fn logout(State(state): State<Arc<AppState>>, jar: CookieJar) -> Result<Response, AppError> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash(cookie.value()))
            .execute(&state.db)
            .await?;
    }
    let value = format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}",
        if state.production { "; Secure" } else { "" }
    );
    let mut response = Json(json!({ "ok": true })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).map_err(|_| AppError::Internal)?,
    );
    Ok(response)
}

async fn session(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    purge_expired(&state.db).await?;
    let configured: bool = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workspaces")
        .fetch_one(&state.db)
        .await?
        > 0;
    let authenticated = require_admin(&state, &jar).await.is_ok();
    let workspace: Option<Workspace> = if authenticated {
        sqlx::query_as(
            "SELECT business_name, timezone, region, deletion_days FROM workspaces WHERE id = 1",
        )
        .fetch_optional(&state.db)
        .await?
    } else {
        None
    };
    Ok(Json(
        json!({ "configured": configured, "authenticated": authenticated, "workspace": workspace }),
    ))
}

async fn admin_form(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &jar).await?;
    let fields = load_fields(&state.db).await?;
    Ok(Json(json!({ "fields": serialize_admin_fields(fields) })))
}

async fn update_form(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(input): Json<FormInput>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &jar).await?;
    if !(2..=12).contains(&input.fields.len()) {
        return Err(AppError::Validation(
            "Keep the form between 2 and 12 fields.".into(),
        ));
    }
    let allowed = ["text", "email", "tel", "textarea", "date", "time", "select"];
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM form_fields")
        .execute(&mut *tx)
        .await?;
    for (index, field) in input.fields.iter().enumerate() {
        let label = field.label.trim();
        if label.len() < 2 || label.len() > 60 {
            return Err(AppError::Validation(
                "Every field label must be 2–60 characters.".into(),
            ));
        }
        if !allowed.contains(&field.field_type.as_str()) {
            return Err(AppError::Validation(
                "Choose a supported field type.".into(),
            ));
        }
        if field.visibility != "worker" && field.visibility != "admin" {
            return Err(AppError::Validation(
                "Choose who can see every field.".into(),
            ));
        }
        let options = field.options.clone().unwrap_or_default();
        if field.field_type == "select" && options.len() < 2 {
            return Err(AppError::Validation(format!(
                "Add at least two choices for {label}."
            )));
        }
        let id = field
            .id
            .clone()
            .filter(|id| valid_id(id))
            .unwrap_or_else(|| {
                format!(
                    "field_{}",
                    Utc::now().timestamp_nanos_opt().unwrap_or_default() + index as i64
                )
            });
        sqlx::query("INSERT INTO form_fields (id, label, field_type, required, visibility, sort_order, options_json) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(id).bind(label).bind(&field.field_type).bind(field.required).bind(&field.visibility).bind(index as i64)
            .bind(serde_json::to_string(&options).map_err(|_| AppError::Validation("A field choice is invalid.".into()))?)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

async fn public_form(State(state): State<Arc<AppState>>) -> Result<Json<PublicForm>, AppError> {
    purge_expired(&state.db).await?;
    let workspace: Workspace = sqlx::query_as(
        "SELECT business_name, timezone, region, deletion_days FROM workspaces WHERE id = 1",
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let fields = load_fields(&state.db)
        .await?
        .into_iter()
        .map(|field| PublicField {
            id: field.id,
            label: field.label,
            field_type: field.field_type,
            required: field.required,
            options: serde_json::from_str(&field.options_json).unwrap_or_default(),
        })
        .collect();
    Ok(Json(PublicForm {
        business_name: workspace.business_name,
        region: workspace.region,
        deletion_days: workspace.deletion_days,
        fields,
    }))
}

async fn create_booking(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(input): Json<BookingInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    enforce_rate(&state, peer.ip(), 60).await?;
    if !input.website.is_empty() {
        return Ok((StatusCode::CREATED, Json(json!({ "received": true }))));
    }
    purge_expired(&state.db).await?;
    let fields = load_fields(&state.db).await?;
    if fields.is_empty() {
        return Err(AppError::Validation(
            "This intake form is not open yet.".into(),
        ));
    }
    let deletion_days: i64 =
        sqlx::query_scalar("SELECT deletion_days FROM workspaces WHERE id = 1")
            .fetch_one(&state.db)
            .await?;
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = Utc::now();
    let delete_at = created_at + Duration::days(deletion_days);
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT INTO bookings (id, created_at, delete_at) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(created_at.to_rfc3339())
        .bind(delete_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;
    for field in fields {
        let value = input
            .values
            .get(&field.id)
            .map(|value| value.trim())
            .unwrap_or("");
        validate_value(&field, value)?;
        if !value.is_empty() {
            sqlx::query("INSERT INTO responses (booking_id, field_id, label_snapshot, visibility_snapshot, value, sort_order) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(&id).bind(field.id).bind(field.label).bind(field.visibility).bind(value).bind(field.sort_order).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "received": true, "delete_at": delete_at.to_rfc3339() })),
    ))
}

async fn list_bookings(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &jar).await?;
    purge_expired(&state.db).await?;
    let rows: Vec<BookingRow> = sqlx::query_as("SELECT id, created_at, delete_at, status, worker_name FROM bookings ORDER BY created_at DESC").fetch_all(&state.db).await?;
    let mut results = Vec::new();
    for row in rows {
        let summary: Option<String> = sqlx::query_scalar(
            "SELECT value FROM responses WHERE booking_id = ? ORDER BY sort_order LIMIT 1",
        )
        .bind(&row.id)
        .fetch_optional(&state.db)
        .await?;
        results.push(json!({ "id": row.id, "created_at": row.created_at, "delete_at": row.delete_at, "status": row.status, "worker_name": row.worker_name, "summary": summary.unwrap_or_else(|| "Untitled request".into()) }));
    }
    Ok(Json(json!({ "bookings": results })))
}

async fn get_booking(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<BookingDetail>, AppError> {
    require_admin(&state, &jar).await?;
    Ok(Json(load_booking(&state.db, &id, false).await?))
}

async fn worker_preview(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<BookingDetail>, AppError> {
    require_admin(&state, &jar).await?;
    Ok(Json(load_booking(&state.db, &id, true).await?))
}

async fn assign_worker(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(input): Json<AssignmentInput>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &jar).await?;
    let name = input.worker_name.trim();
    if name.len() < 2 || name.len() > 60 {
        return Err(AppError::Validation(
            "Enter a worker name between 2 and 60 characters.".into(),
        ));
    }
    if !(1..=336).contains(&input.expires_hours) {
        return Err(AppError::Validation(
            "Worker links can last from 1 hour to 14 days.".into(),
        ));
    }
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bookings WHERE id = ?")
        .bind(&id)
        .fetch_one(&state.db)
        .await?;
    if exists == 0 {
        return Err(AppError::NotFound);
    }
    let token = random_token(42);
    let expires_at = Utc::now() + Duration::hours(input.expires_hours);
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM worker_tokens WHERE booking_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO worker_tokens (token_hash, booking_id, expires_at, created_at) VALUES (?, ?, ?, ?)")
        .bind(token_hash(&token)).bind(&id).bind(expires_at.to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&mut *tx).await?;
    sqlx::query("UPDATE bookings SET worker_name = ?, status = 'assigned' WHERE id = ?")
        .bind(name)
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({ "worker_path": format!("/worker/{token}"), "expires_at": expires_at.to_rfc3339() }),
    ))
}

async fn worker_brief(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Json<BookingDetail>, AppError> {
    purge_expired(&state.db).await?;
    let booking_id: Option<String> = sqlx::query_scalar(
        "SELECT booking_id FROM worker_tokens WHERE token_hash = ? AND expires_at > ?",
    )
    .bind(token_hash(&token))
    .bind(Utc::now().to_rfc3339())
    .fetch_optional(&state.db)
    .await?;
    let id = booking_id.ok_or(AppError::NotFound)?;
    load_booking(&state.db, &id, true).await.map(Json)
}

async fn update_status(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(input): Json<StatusInput>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &jar).await?;
    if !["new", "assigned", "complete"].contains(&input.status.as_str()) {
        return Err(AppError::Validation("Choose a valid job status.".into()));
    }
    let result = sqlx::query("UPDATE bookings SET status = ? WHERE id = ?")
        .bind(input.status)
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn delete_booking(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &jar).await?;
    let result = sqlx::query("DELETE FROM bookings WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "deleted": true })))
}

async fn export_bookings(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    require_admin(&state, &jar).await?;
    purge_expired(&state.db).await?;
    let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as("SELECT b.created_at, b.delete_at, b.status, COALESCE(b.worker_name, ''), r.label_snapshot, r.value FROM bookings b JOIN responses r ON r.booking_id = b.id ORDER BY b.created_at DESC, r.sort_order").fetch_all(&state.db).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "created_at",
            "delete_at",
            "status",
            "worker",
            "field",
            "value",
        ])
        .map_err(|_| AppError::Internal)?;
    for row in rows {
        writer
            .write_record([row.0, row.1, row.2, row.3, row.4, row.5])
            .map_err(|_| AppError::Internal)?;
    }
    let bytes = writer.into_inner().map_err(|_| AppError::Internal)?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=private-intake-export.csv",
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn load_booking(
    db: &SqlitePool,
    id: &str,
    worker_only: bool,
) -> Result<BookingDetail, AppError> {
    let row: BookingRow = sqlx::query_as(
        "SELECT id, created_at, delete_at, status, worker_name FROM bookings WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    let query = if worker_only {
        "SELECT field_id, label_snapshot, visibility_snapshot, value, sort_order FROM responses WHERE booking_id = ? AND visibility_snapshot = 'worker' ORDER BY sort_order"
    } else {
        "SELECT field_id, label_snapshot, visibility_snapshot, value, sort_order FROM responses WHERE booking_id = ? ORDER BY sort_order"
    };
    let responses = sqlx::query_as(query).bind(id).fetch_all(db).await?;
    Ok(BookingDetail {
        id: row.id,
        created_at: row.created_at,
        delete_at: row.delete_at,
        status: row.status,
        worker_name: row.worker_name,
        responses,
    })
}

async fn require_admin(state: &AppState, jar: &CookieJar) -> Result<(), AppError> {
    let token = jar
        .get(SESSION_COOKIE)
        .ok_or(AppError::Unauthorized)?
        .value();
    let valid: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE token_hash = ? AND expires_at > ?")
            .bind(token_hash(token))
            .bind(Utc::now().to_rfc3339())
            .fetch_one(&state.db)
            .await?;
    if valid == 1 {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

async fn create_session_response(state: &AppState) -> Result<Response, AppError> {
    let token = random_token(48);
    let expires = Utc::now() + Duration::days(SESSION_DAYS);
    sqlx::query("INSERT INTO sessions (token_hash, expires_at) VALUES (?, ?)")
        .bind(token_hash(&token))
        .bind(expires.to_rfc3339())
        .execute(&state.db)
        .await?;
    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        SESSION_DAYS * 86400,
        if state.production { "; Secure" } else { "" }
    );
    let mut response = Json(json!({ "authenticated": true })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| AppError::Internal)?,
    );
    Ok(response)
}

async fn seed_fields(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<(), AppError> {
    let defaults = [
        ("client_name", "Client name", "text", true, "admin", "[]"),
        (
            "contact_number",
            "Contact number",
            "tel",
            true,
            "admin",
            "[]",
        ),
        (
            "service_address",
            "Service address",
            "textarea",
            true,
            "worker",
            "[]",
        ),
        (
            "appointment_date",
            "Preferred date",
            "date",
            true,
            "worker",
            "[]",
        ),
        (
            "arrival_window",
            "Preferred arrival window",
            "select",
            true,
            "worker",
            "[\"Morning · 8–12\",\"Afternoon · 12–5\"]",
        ),
        (
            "job_details",
            "What needs attention?",
            "textarea",
            true,
            "worker",
            "[]",
        ),
        (
            "access_notes",
            "Access or safety notes",
            "textarea",
            false,
            "worker",
            "[]",
        ),
        (
            "billing_context",
            "Billing or account notes",
            "textarea",
            false,
            "admin",
            "[]",
        ),
    ];
    for (index, item) in defaults.iter().enumerate() {
        sqlx::query("INSERT INTO form_fields (id, label, field_type, required, visibility, sort_order, options_json) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(item.0).bind(item.1).bind(item.2).bind(item.3).bind(item.4).bind(index as i64).bind(item.5).execute(&mut **tx).await?;
    }
    Ok(())
}

async fn load_fields(db: &SqlitePool) -> Result<Vec<FormField>, AppError> {
    Ok(sqlx::query_as("SELECT id, label, field_type, required, visibility, sort_order, options_json FROM form_fields ORDER BY sort_order").fetch_all(db).await?)
}

fn serialize_admin_fields(fields: Vec<FormField>) -> Vec<Value> {
    fields.into_iter().map(|field| json!({
        "id": field.id, "label": field.label, "field_type": field.field_type, "required": field.required,
        "visibility": field.visibility, "options": serde_json::from_str::<Vec<String>>(&field.options_json).unwrap_or_default()
    })).collect()
}

fn validate_setup(input: &SetupInput) -> Result<(), AppError> {
    if input.business_name.trim().len() < 2 || input.business_name.trim().len() > 80 {
        return Err(AppError::Validation(
            "Business name must be 2–80 characters.".into(),
        ));
    }
    if input.passphrase.len() < 12 {
        return Err(AppError::Validation(
            "Use a passphrase with at least 12 characters.".into(),
        ));
    }
    if input.timezone.trim().is_empty() || input.timezone.len() > 64 {
        return Err(AppError::Validation("Enter a valid timezone.".into()));
    }
    if ![
        "United States",
        "United Kingdom",
        "European Union",
        "Canada",
        "Australia",
        "Other",
    ]
    .contains(&input.region.as_str())
    {
        return Err(AppError::Validation("Choose a privacy region.".into()));
    }
    if !(1..=90).contains(&input.deletion_days) {
        return Err(AppError::Validation(
            "Choose automatic deletion between 1 and 90 days.".into(),
        ));
    }
    Ok(())
}

fn validate_value(field: &FormField, value: &str) -> Result<(), AppError> {
    if field.required && value.is_empty() {
        return Err(AppError::Validation(format!(
            "{} is required.",
            field.label
        )));
    }
    if value.len() > 2000 {
        return Err(AppError::Validation(format!(
            "{} is too long.",
            field.label
        )));
    }
    if field.field_type == "email"
        && !value.is_empty()
        && !Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$")
            .unwrap()
            .is_match(value)
    {
        return Err(AppError::Validation(format!(
            "Enter a valid {}.",
            field.label.to_lowercase()
        )));
    }
    if field.field_type == "select" && !value.is_empty() {
        let options: Vec<String> = serde_json::from_str(&field.options_json).unwrap_or_default();
        if !options.iter().any(|item| item == value) {
            return Err(AppError::Validation(format!(
                "Choose a listed option for {}.",
                field.label
            )));
        }
    }
    Ok(())
}

fn hash_passphrase(passphrase: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(passphrase.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AppError::Internal)
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
fn random_token(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}
fn valid_id(id: &str) -> bool {
    id.len() <= 80
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

async fn enforce_rate(state: &AppState, ip: IpAddr, limit: usize) -> Result<(), AppError> {
    let now = Instant::now();
    let mut windows = state.rate_windows.lock().await;
    let attempts = windows.entry(ip).or_default();
    while attempts
        .front()
        .is_some_and(|seen| now.duration_since(*seen) > StdDuration::from_secs(60))
    {
        attempts.pop_front();
    }
    if attempts.len() >= limit {
        return Err(AppError::RateLimited);
    }
    attempts.push_back(now);
    Ok(())
}

async fn purge_expired(db: &SqlitePool) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(&now)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM worker_tokens WHERE expires_at <= ?")
        .bind(&now)
        .execute(db)
        .await?;
    sqlx::query("DELETE FROM bookings WHERE delete_at <= ?")
        .bind(&now)
        .execute(db)
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("terminate handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    info!("shutdown requested");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    async fn test_state(static_dir: PathBuf) -> Arc<AppState> {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        Arc::new(AppState {
            db,
            production: false,
            build_sha: "test-build".into(),
            static_dir,
            rate_windows: Mutex::new(HashMap::new()),
        })
    }

    fn test_field(visibility: &str) -> FormField {
        FormField {
            id: "email".into(),
            label: "Email".into(),
            field_type: "email".into(),
            required: true,
            visibility: visibility.into(),
            sort_order: 0,
            options_json: "[]".into(),
        }
    }

    #[test]
    fn validation_rejects_bad_email() {
        assert!(validate_value(&test_field("admin"), "not-an-email").is_err());
        assert!(validate_value(&test_field("admin"), "client@example.com").is_ok());
    }

    #[test]
    fn tokens_are_never_stored_verbatim() {
        let token = random_token(42);
        assert_ne!(token, token_hash(&token));
        assert_eq!(token_hash(&token).len(), 64);
    }

    #[test]
    fn ids_allow_only_safe_characters() {
        assert!(valid_id("access_notes-2"));
        assert!(!valid_id("../../token"));
    }

    #[tokio::test]
    async fn worker_query_excludes_manager_only_responses() {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        let now = Utc::now();
        sqlx::query("INSERT INTO bookings (id, created_at, delete_at) VALUES ('job', ?, ?)")
            .bind(now.to_rfc3339())
            .bind((now + Duration::days(1)).to_rfc3339())
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO responses (booking_id, field_id, label_snapshot, visibility_snapshot, value, sort_order) VALUES ('job','address','Address','worker','12 Route Road',0), ('job','billing','Billing notes','admin','PRIVATE-CONTEXT',1)").execute(&db).await.unwrap();
        let brief = load_booking(&db, "job", true).await.unwrap();
        assert_eq!(brief.responses.len(), 1);
        assert_eq!(brief.responses[0].value, "12 Route Road");
        assert!(!serde_json::to_string(&brief)
            .unwrap()
            .contains("PRIVATE-CONTEXT"));
    }

    #[tokio::test]
    async fn recognized_spa_routes_return_the_shell_with_ok_status() {
        let static_dir = env::temp_dir().join(format!("piv-spa-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&static_dir).unwrap();
        std::fs::write(static_dir.join("index.html"), "<main>Private Intake</main>").unwrap();
        let state = test_state(static_dir.clone()).await;

        for path in [
            "/",
            "/book",
            "/admin",
            "/privacy",
            "/terms",
            "/worker/a-live-token",
        ] {
            let response = app(state.clone())
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{path} should be a 200 shell route"
            );
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                "text/html; charset=utf-8"
            );
            assert_eq!(
                response.headers().get("strict-transport-security").unwrap(),
                "max-age=31536000; includeSubDomains"
            );
        }

        let missing_asset = app(state)
            .oneshot(
                Request::builder()
                    .uri("/assets/does-not-exist.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_asset.status(), StatusCode::NOT_FOUND);
        std::fs::remove_dir_all(static_dir).unwrap();
    }
}
