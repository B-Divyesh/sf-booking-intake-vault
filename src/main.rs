mod auth;

use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Duration, NaiveDate, NaiveTime, Utc};
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    FromRow, SqlitePool,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{Duration as StdDuration, Instant},
};
use tokio::{io::AsyncWriteExt, sync::Mutex};
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};
use tracing::{error, info, warn};

const FREE_FIELD_LIMIT: usize = 8;
const FREE_LINK_HOURS: i64 = 48;
const LICENSE_HEADER: &str = "x-route-license";
const RATE_WINDOW: StdDuration = StdDuration::from_secs(60);
const API_RATE_LIMIT: usize = 120;
const AUTH_RATE_LIMIT: usize = 20;
const WRITE_RATE_LIMIT: usize = 60;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum RateClass {
    Api,
    Auth,
    Write,
}

struct AppState {
    db: SqlitePool,
    database_file: Option<PathBuf>,
    backup_file: Option<PathBuf>,
    build_sha: String,
    static_dir: PathBuf,
    rate_windows: Mutex<HashMap<(IpAddr, RateClass), VecDeque<Instant>>>,
    license_cache: Mutex<HashMap<String, CachedLicense>>,
    billing_base: String,
    http: reqwest::Client,
    entra: auth::EntraValidator,
    demos: Mutex<HashMap<String, DemoWorkspace>>,
}

#[derive(Clone)]
struct CachedLicense {
    valid: bool,
    checked_at: Instant,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("not signed in")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Validation(String),
    #[error("service error")]
    Internal,
    #[error("too many requests")]
    RateLimited(u64),
    #[error("route pass required")]
    PaymentRequired,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, retry_after) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Sign in with your Sociobot account to continue.".to_string(),
                None,
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "This vault belongs to another Sociobot account.".to_string(),
                None,
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "That item was not found or has expired.".to_string(),
                None,
            ),
            Self::Validation(message) => (StatusCode::UNPROCESSABLE_ENTITY, message, None),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Private Intake could not complete that request. Try again.".to_string(),
                None,
            ),
            Self::RateLimited(seconds) => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many requests from this connection. Wait a minute and try again.".to_string(),
                Some(seconds),
            ),
            Self::PaymentRequired => (
                StatusCode::PAYMENT_REQUIRED,
                "A valid Route pass is required for more than 8 questions or worker links longer than 48 hours.".to_string(),
                None,
            ),
        };
        let mut response = (status, Json(json!({ "error": message }))).into_response();
        if let Some(seconds) = retry_after {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_str(&seconds.to_string())
                    .expect("retry delay is a valid header value"),
            );
        }
        response
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

struct BootstrapConfig {
    business_name: String,
    timezone: String,
    region: String,
    deletion_days: i64,
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
    available: bool,
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

#[derive(Clone, Serialize)]
struct DemoWorkspace {
    id: String,
    created_at: String,
    expires_at: String,
    delete_at: String,
    worker_name: String,
    status: String,
    manager_responses: Vec<ResponseRow>,
    worker_responses: Vec<ResponseRow>,
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
    let db_supplied = env::var_os("DATABASE_URL").is_some();
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://private-intake.db?mode=rwc".to_string());
    let database_file = sqlite_file_path(&db_url);
    let backup_file = env::var_os("DATABASE_BACKUP_PATH").map(PathBuf::from);
    restore_database_from_backup(database_file.as_deref(), backup_file.as_deref())
        .await
        .expect("database snapshot restore");
    let options = SqliteConnectOptions::from_str(&db_url)
        .expect("valid DATABASE_URL")
        .create_if_missing(true)
        .busy_timeout(StdDuration::from_secs(30))
        .journal_mode(SqliteJournalMode::Delete)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        // This is a single-tenant SQLite vault. One writer connection avoids
        // competing startup/schema locks on durable network-backed storage.
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("database connection");
    run_migrations(&db).await.expect("database migrations");

    let bootstrap_overridden = [
        "INITIAL_BUSINESS_NAME",
        "INITIAL_TIMEZONE",
        "INITIAL_REGION",
        "INITIAL_DELETION_DAYS",
    ]
    .iter()
    .any(|key| env::var_os(key).is_some());
    initialize_default_workspace(&db)
        .await
        .expect("workspace initialization");
    if let Err(error) =
        persist_database_file(database_file.as_deref(), backup_file.as_deref()).await
    {
        warn!(error = %error, "could not persist initial vault snapshot");
    }
    info!(
        database = if db_supplied {
            "supplied"
        } else {
            "generated-default"
        },
        workspace_defaults = if bootstrap_overridden {
            "overridden"
        } else {
            "generated-default"
        },
        identity = "Sociobot Entra defaults",
        "runtime configuration ready"
    );

    let http = reqwest::Client::new();
    let state = Arc::new(AppState {
        db,
        database_file,
        backup_file,
        build_sha: env::var("BUILD_SHA").unwrap_or_else(|_| "development".to_string()),
        static_dir: PathBuf::from("frontend/dist"),
        rate_windows: Mutex::new(HashMap::new()),
        license_cache: Mutex::new(HashMap::new()),
        billing_base: env::var("BILLING_BASE")
            .unwrap_or_else(|_| "https://api.sociobot.in/api/v1".to_string()),
        entra: auth::EntraValidator::from_environment(http.clone()),
        http,
        demos: Mutex::new(HashMap::new()),
    });
    info!(
        identity_authority = state.entra.authority(),
        "Sociobot Entra identity validation ready"
    );
    purge_expired(&state.db).await.ok();

    let app = app(state);
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
        .route("/api/demo/workspaces", post(create_demo_workspace))
        .route("/api/demo/workspaces/{id}", get(get_demo_workspace))
        .route(
            "/api/demo/workspaces/{id}/reset",
            post(reset_demo_workspace),
        )
        .route(
            "/api/demo/workspaces/{id}/export.csv",
            get(export_demo_workspace),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_api,
        ));

    // Serve the application shell only for client routes that actually exist.
    // `ServeDir::not_found_service` keeps a 404 status even when it sends the
    // index body, which breaks direct visits and refreshes of SPA routes.
    let client_routes = Router::new()
        .route("/", get(spa_index))
        .route("/book", get(spa_index))
        .route("/demo", get(spa_index))
        .route("/admin", get(spa_index))
        .route("/auth/callback", get(spa_index))
        .route("/privacy", get(spa_index))
        .route("/terms", get(spa_index))
        .route("/worker/{token}", get(spa_index));

    let static_dir = state.static_dir.clone();
    Router::new()
        .route("/health", get(health))
        .merge(api)
        .merge(client_routes)
        .nest_service("/assets", ServeDir::new(static_dir.join("assets")))
        .route_service(
            "/favicon.svg",
            ServeFile::new(static_dir.join("favicon.svg")),
        )
        .route_service(
            "/apple-touch-icon.png",
            ServeFile::new(static_dir.join("apple-touch-icon.png")),
        )
        .route_service("/sw.js", ServeFile::new(static_dir.join("sw.js")))
        .route_service("/robots.txt", ServeFile::new(static_dir.join("robots.txt")))
        .route_service(
            "/sitemap.xml",
            ServeFile::new(static_dir.join("sitemap.xml")),
        )
        .fallback(spa_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            snapshot_after_api_request,
        ))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(64 * 1024))
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

/// Apply a shared per-client ceiling to every API route, with stricter
/// buckets for authentication and writes. Azure Container Apps forwards the
/// originating address in the first `X-Forwarded-For` entry, so the limiter
/// must not collapse every visitor into the ingress proxy's socket address.
async fn rate_limit_api(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let ip = client_ip(&request);
    if let Err(error) = enforce_rate(&state, ip, RateClass::Api, API_RATE_LIMIT).await {
        return error.into_response();
    }

    let path = request.uri().path();
    let scoped_limit = if path == "/api/session" {
        Some((RateClass::Auth, AUTH_RATE_LIMIT))
    } else if request.method() != axum::http::Method::GET
        && request.method() != axum::http::Method::HEAD
        && request.method() != axum::http::Method::OPTIONS
    {
        Some((RateClass::Write, WRITE_RATE_LIMIT))
    } else {
        None
    };
    if let Some((class, limit)) = scoped_limit {
        if let Err(error) = enforce_rate(&state, ip, class, limit).await {
            return error.into_response();
        }
    }
    next.run(request).await
}

fn client_ip(request: &Request<Body>) -> IpAddr {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .and_then(|value| value.parse().ok())
        .or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(peer)| peer.ip())
        })
        .unwrap_or(IpAddr::from([0, 0, 0, 0]))
}

/// Persist the local SQLite snapshot only after a successful API response. The
/// deployed single-tenant vault uses local SQLite for correct locking and an
/// Azure Files copy for revision-to-revision durability.
async fn snapshot_after_api_request(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let snapshot_needed = request.uri().path().starts_with("/api/");
    let response = next.run(request).await;
    if snapshot_needed && response.status().is_success() {
        if let Err(error) =
            persist_database_file(state.database_file.as_deref(), state.backup_file.as_deref())
                .await
        {
            error!(kind = "database_snapshot", error = %error, "could not persist vault snapshot");
        }
    }
    response
}

fn sqlite_file_path(url: &str) -> Option<PathBuf> {
    let path = url.strip_prefix("sqlite://")?.split('?').next()?;
    if path == ":memory:" || path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

async fn restore_database_from_backup(
    database_file: Option<&std::path::Path>,
    backup_file: Option<&std::path::Path>,
) -> Result<(), std::io::Error> {
    let (Some(database_file), Some(backup_file)) = (database_file, backup_file) else {
        return Ok(());
    };
    if !backup_file.exists() {
        return Ok(());
    }
    if let Some(parent) = database_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(backup_file, database_file).await?;
    Ok(())
}

async fn persist_database_file(
    database_file: Option<&std::path::Path>,
    backup_file: Option<&std::path::Path>,
) -> Result<(), std::io::Error> {
    let (Some(database_file), Some(backup_file)) = (database_file, backup_file) else {
        return Ok(());
    };
    if !database_file.exists() {
        return Ok(());
    }
    if let Some(parent) = backup_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    // Azure Files does not support the atomic rename operation SQLite expects.
    // The database commits locally first; a direct replacement here retains the
    // last complete local file rather than sharing SQLite's lock files.
    let mut source = tokio::fs::File::open(database_file).await?;
    let mut destination = tokio::fs::File::create(backup_file).await?;
    tokio::io::copy(&mut source, &mut destination).await?;
    destination.flush().await
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

async fn spa_not_found(State(state): State<Arc<AppState>>, request: Request<Body>) -> Response {
    if request.uri().path().starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "That API route does not exist." })),
        )
            .into_response();
    }
    let mut response = spa_index(State(state)).await;
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
}

/// Azure Files may retain SQLite's migration lock briefly as a container
/// revision starts. Retry the idempotent migration before declaring startup
/// unhealthy rather than turning that transient storage condition into a
/// crash loop.
async fn run_migrations(db: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    const ATTEMPTS: u8 = 5;
    for attempt in 1..=ATTEMPTS {
        match sqlx::migrate!().run(db).await {
            Ok(()) => return Ok(()),
            Err(error) if attempt < ATTEMPTS => {
                warn!(attempt, error = %error, "database migration blocked; retrying");
                tokio::time::sleep(StdDuration::from_secs(u64::from(attempt) * 2)).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("migration retry loop always returns")
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
    headers.insert("content-security-policy", HeaderValue::from_static("default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' https://api.sociobot.in https://sociobotcustomers.ciamlogin.com https://login.microsoftonline.com; frame-src https://sociobotcustomers.ciamlogin.com; object-src 'none'; base-uri 'self'; form-action 'self' https://api.sociobot.in https://sociobotcustomers.ciamlogin.com; frame-ancestors 'none'"));
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

async fn session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    purge_expired(&state.db).await?;
    let configured: bool = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workspaces")
        .fetch_one(&state.db)
        .await?
        > 0;
    let authenticated = require_admin(&state, &headers).await.is_ok();
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
        json!({ "configured": configured, "authenticated": authenticated, "identity_provider": "Sociobot Microsoft Entra External ID", "workspace": workspace }),
    ))
}

async fn admin_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers).await?;
    let fields = load_fields(&state.db).await?;
    Ok(Json(json!({ "fields": serialize_admin_fields(fields) })))
}

async fn update_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<FormInput>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers).await?;
    if !(2..=12).contains(&input.fields.len()) {
        return Err(AppError::Validation(
            "Keep the form between 2 and 12 fields.".into(),
        ));
    }
    if input.fields.len() > FREE_FIELD_LIMIT {
        require_route_pass(&state, &headers).await?;
    }
    let mut ids = HashSet::new();
    let mut prepared = Vec::with_capacity(input.fields.len());
    for (index, field) in input.fields.iter().enumerate() {
        validate_field(field)?;
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
        if !ids.insert(id.clone()) {
            return Err(AppError::Validation(
                "Every form field must have a unique ID.".into(),
            ));
        }
        prepared.push((id, field));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM form_fields")
        .execute(&mut *tx)
        .await?;
    for (index, (id, field)) in prepared.into_iter().enumerate() {
        let label = field.label.trim();
        let options = field.options.clone().unwrap_or_default();
        sqlx::query("INSERT INTO form_fields (id, label, field_type, required, visibility, sort_order, options_json) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(id).bind(label).bind(&field.field_type).bind(field.required).bind(&field.visibility).bind(index as i64)
            .bind(serde_json::to_string(&options).map_err(|_| AppError::Validation("A field choice is invalid.".into()))?)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

async fn public_form(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    purge_expired(&state.db).await?;
    let workspace: Option<Workspace> = sqlx::query_as(
        "SELECT business_name, timezone, region, deletion_days FROM workspaces WHERE id = 1",
    )
    .fetch_optional(&state.db)
    .await?;
    let Some(workspace) = workspace else {
        return Ok(Json(json!({
            "available": false,
            "error": "The booking desk has not been configured yet."
        })));
    };
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
    Ok(Json(
        serde_json::to_value(PublicForm {
            available: true,
            business_name: workspace.business_name,
            region: workspace.region,
            deletion_days: workspace.deletion_days,
            fields,
        })
        .map_err(|_| AppError::Internal)?,
    ))
}

async fn create_booking(
    State(state): State<Arc<AppState>>,
    Json(input): Json<BookingInput>,
) -> Result<(StatusCode, Json<Value>), AppError> {
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
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers).await?;
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
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<BookingDetail>, AppError> {
    require_admin(&state, &headers).await?;
    Ok(Json(load_booking(&state.db, &id, false).await?))
}

async fn worker_preview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<BookingDetail>, AppError> {
    require_admin(&state, &headers).await?;
    Ok(Json(load_booking(&state.db, &id, true).await?))
}

async fn assign_worker(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<AssignmentInput>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers).await?;
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
    if input.expires_hours > FREE_LINK_HOURS {
        require_route_pass(&state, &headers).await?;
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
) -> Result<Json<Value>, AppError> {
    purge_expired(&state.db).await?;
    let booking_id: Option<String> = sqlx::query_scalar(
        "SELECT booking_id FROM worker_tokens WHERE token_hash = ? AND expires_at > ?",
    )
    .bind(token_hash(&token))
    .bind(Utc::now().to_rfc3339())
    .fetch_optional(&state.db)
    .await?;
    let Some(id) = booking_id else {
        return Ok(Json(json!({
            "available": false,
            "error": "That worker ticket was not found or has expired."
        })));
    };
    let brief = load_booking(&state.db, &id, true).await?;
    let mut value = serde_json::to_value(brief).map_err(|_| AppError::Internal)?;
    value["available"] = Value::Bool(true);
    Ok(Json(value))
}

async fn update_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<StatusInput>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers).await?;
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
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_admin(&state, &headers).await?;
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
    headers: HeaderMap,
) -> Result<Response, AppError> {
    require_admin(&state, &headers).await?;
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
            .write_record([
                csv_safe_cell(&row.0),
                csv_safe_cell(&row.1),
                csv_safe_cell(&row.2),
                csv_safe_cell(&row.3),
                csv_safe_cell(&row.4),
                csv_safe_cell(&row.5),
            ])
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

fn sample_demo_workspace() -> DemoWorkspace {
    let created = Utc::now();
    let responses = vec![
        ResponseRow {
            field_id: "client_name".into(),
            label_snapshot: "Client name".into(),
            visibility_snapshot: "admin".into(),
            value: "Nadia Patel".into(),
            sort_order: 0,
        },
        ResponseRow {
            field_id: "contact_number".into(),
            label_snapshot: "Contact number".into(),
            visibility_snapshot: "admin".into(),
            value: "+1 555 0142".into(),
            sort_order: 1,
        },
        ResponseRow {
            field_id: "service_address".into(),
            label_snapshot: "Service address".into(),
            visibility_snapshot: "worker".into(),
            value: "18 Juniper Lane, side entrance".into(),
            sort_order: 2,
        },
        ResponseRow {
            field_id: "appointment_date".into(),
            label_snapshot: "Preferred date".into(),
            visibility_snapshot: "worker".into(),
            value: "2026-09-04".into(),
            sort_order: 3,
        },
        ResponseRow {
            field_id: "arrival_window".into(),
            label_snapshot: "Preferred arrival window".into(),
            visibility_snapshot: "worker".into(),
            value: "Morning · 8–12".into(),
            sort_order: 4,
        },
        ResponseRow {
            field_id: "job_details".into(),
            label_snapshot: "What needs attention?".into(),
            visibility_snapshot: "worker".into(),
            value: "Replace the leaking garden tap.".into(),
            sort_order: 5,
        },
        ResponseRow {
            field_id: "access_notes".into(),
            label_snapshot: "Access or safety notes".into(),
            visibility_snapshot: "worker".into(),
            value: "Call from the gate. Dog stays indoors.".into(),
            sort_order: 6,
        },
        ResponseRow {
            field_id: "billing_context".into(),
            label_snapshot: "Billing or account notes".into(),
            visibility_snapshot: "admin".into(),
            value: "Warranty account NW-204. Do not charge on site.".into(),
            sort_order: 7,
        },
    ];
    DemoWorkspace {
        id: random_token(32),
        created_at: created.to_rfc3339(),
        expires_at: (created + Duration::hours(24)).to_rfc3339(),
        delete_at: (created + Duration::days(14)).to_rfc3339(),
        worker_name: "Morgan Lee".into(),
        status: "assigned".into(),
        worker_responses: responses
            .iter()
            .filter(|response| response.visibility_snapshot == "worker")
            .cloned()
            .collect(),
        manager_responses: responses,
    }
}

async fn prune_demo_workspaces(state: &AppState) {
    let now = Utc::now();
    state.demos.lock().await.retain(|_, workspace| {
        chrono::DateTime::parse_from_rfc3339(&workspace.expires_at)
            .map(|expires| expires > now)
            .unwrap_or(false)
    });
}

async fn create_demo_workspace(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<DemoWorkspace>) {
    prune_demo_workspaces(&state).await;
    let workspace = sample_demo_workspace();
    state
        .demos
        .lock()
        .await
        .insert(workspace.id.clone(), workspace.clone());
    (StatusCode::CREATED, Json(workspace))
}

async fn get_demo_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DemoWorkspace>, AppError> {
    prune_demo_workspaces(&state).await;
    state
        .demos
        .lock()
        .await
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn reset_demo_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DemoWorkspace>, AppError> {
    prune_demo_workspaces(&state).await;
    let mut demos = state.demos.lock().await;
    if demos.remove(&id).is_none() {
        return Err(AppError::NotFound);
    }
    let workspace = sample_demo_workspace();
    demos.insert(workspace.id.clone(), workspace.clone());
    Ok(Json(workspace))
}

async fn export_demo_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let workspace = get_demo_workspace(State(state), Path(id)).await?.0;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record(["field", "visibility", "value"])
        .map_err(|_| AppError::Internal)?;
    for response in workspace.manager_responses {
        writer
            .write_record([
                csv_safe_cell(&response.label_snapshot),
                csv_safe_cell(&response.visibility_snapshot),
                csv_safe_cell(&response.value),
            ])
            .map_err(|_| AppError::Internal)?;
    }
    let bytes = writer.into_inner().map_err(|_| AppError::Internal)?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=private-intake-demo.csv",
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

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let oid = state
        .entra
        .owner_oid(headers)
        .await
        .map_err(|_| AppError::Unauthorized)?;
    let owner: Option<String> = sqlx::query_scalar("SELECT owner_oid FROM workspaces WHERE id = 1")
        .fetch_optional(&state.db)
        .await?
        .flatten();
    match owner {
        Some(owner) if owner == oid => Ok(()),
        Some(_) => Err(AppError::Forbidden),
        None => {
            let result = sqlx::query(
                "UPDATE workspaces SET owner_oid = ? WHERE id = 1 AND owner_oid IS NULL",
            )
            .bind(&oid)
            .execute(&state.db)
            .await?;
            if result.rows_affected() == 1 {
                info!("vault ownership assigned to an authenticated Sociobot Entra identity");
                return Ok(());
            }
            let claimed: Option<String> =
                sqlx::query_scalar("SELECT owner_oid FROM workspaces WHERE id = 1")
                    .fetch_optional(&state.db)
                    .await?
                    .flatten();
            if claimed.as_deref() == Some(&oid) {
                Ok(())
            } else {
                Err(AppError::Forbidden)
            }
        }
    }
}

fn csv_safe_cell(value: &str) -> String {
    if value
        .chars()
        .next()
        .is_some_and(|first| matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r'))
    {
        format!("'{value}")
    } else {
        value.to_owned()
    }
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

fn validate_field(field: &FieldInput) -> Result<(), AppError> {
    let label = field.label.trim();
    if label.len() < 2 || label.len() > 60 {
        return Err(AppError::Validation(
            "Every field label must be 2–60 characters.".into(),
        ));
    }
    if !["text", "email", "tel", "textarea", "date", "time", "select"]
        .contains(&field.field_type.as_str())
    {
        return Err(AppError::Validation(
            "Choose a supported field type.".into(),
        ));
    }
    if field.visibility != "worker" && field.visibility != "admin" {
        return Err(AppError::Validation(
            "Choose who can see every field.".into(),
        ));
    }
    let options = field.options.as_deref().unwrap_or_default();
    if field.field_type == "select" && options.len() < 2 {
        return Err(AppError::Validation(format!(
            "Add at least two choices for {label}."
        )));
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
    if field.field_type == "date"
        && !value.is_empty()
        && NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err()
    {
        return Err(AppError::Validation(format!(
            "Enter a valid {}.",
            field.label.to_lowercase()
        )));
    }
    if field.field_type == "time"
        && !value.is_empty()
        && NaiveTime::parse_from_str(value, "%H:%M").is_err()
        && NaiveTime::parse_from_str(value, "%H:%M:%S").is_err()
    {
        return Err(AppError::Validation(format!(
            "Enter a valid {}.",
            field.label.to_lowercase()
        )));
    }
    if field.field_type == "tel" && !value.is_empty() {
        let digits = value
            .chars()
            .filter(|character| character.is_ascii_digit())
            .count();
        let phone_shape = Regex::new(r"^\+?[0-9][0-9 ()\-.]{5,24}$")
            .expect("phone expression is valid")
            .is_match(value);
        if !phone_shape || !(7..=15).contains(&digits) {
            return Err(AppError::Validation(format!(
                "Enter a valid {}.",
                field.label.to_lowercase()
            )));
        }
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

async fn initialize_default_workspace(db: &SqlitePool) -> Result<(), AppError> {
    initialize_workspace_from_bootstrap(
        db,
        BootstrapConfig {
            business_name: env::var("INITIAL_BUSINESS_NAME")
                .unwrap_or_else(|_| "Private Intake Field Team".into()),
            timezone: env::var("INITIAL_TIMEZONE").unwrap_or_else(|_| "UTC".into()),
            region: env::var("INITIAL_REGION").unwrap_or_else(|_| "United States".into()),
            deletion_days: env::var("INITIAL_DELETION_DAYS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(30),
        },
    )
    .await
}

async fn initialize_workspace_from_bootstrap(
    db: &SqlitePool,
    config: BootstrapConfig,
) -> Result<(), AppError> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(db)
        .await?;
    if exists > 0 {
        return Ok(());
    }
    if config.business_name.trim().len() < 2
        || config.business_name.trim().len() > 80
        || config.timezone.trim().is_empty()
        || config.timezone.len() > 64
        || !(1..=90).contains(&config.deletion_days)
    {
        return Err(AppError::Validation(
            "The initial workspace settings are invalid.".into(),
        ));
    }
    let mut tx = db.begin().await?;
    let result = sqlx::query("INSERT OR IGNORE INTO workspaces (id, business_name, timezone, region, deletion_days, created_at) VALUES (1, ?, ?, ?, ?, ?)")
        .bind(config.business_name.trim())
        .bind(config.timezone.trim())
        .bind(config.region)
        .bind(config.deletion_days)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 1 {
        seed_fields(&mut tx).await?;
        info!("workspace initialized with safe defaults; awaiting Sociobot Entra owner");
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Deserialize)]
struct LicenseResponse {
    valid: bool,
}

async fn require_route_pass(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let token = headers
        .get(LICENSE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or(AppError::PaymentRequired)?;
    let hash = token_hash(token);
    if let Some(cached) = state.license_cache.lock().await.get(&hash).cloned() {
        if cached.checked_at.elapsed() < StdDuration::from_secs(86_400) {
            return if cached.valid {
                Ok(())
            } else {
                Err(AppError::PaymentRequired)
            };
        }
    }
    let url = format!(
        "{}/products/booking-intake-vault/verify",
        state.billing_base.trim_end_matches('/')
    );
    let valid = match state
        .http
        .get(url)
        .query(&[("license", token)])
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response
            .json::<LicenseResponse>()
            .await
            .map(|result| result.valid)
            .unwrap_or(false),
        _ => false,
    };
    state.license_cache.lock().await.insert(
        hash,
        CachedLicense {
            valid,
            checked_at: Instant::now(),
        },
    );
    if valid {
        Ok(())
    } else {
        Err(AppError::PaymentRequired)
    }
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

async fn enforce_rate(
    state: &AppState,
    ip: IpAddr,
    class: RateClass,
    limit: usize,
) -> Result<(), AppError> {
    let now = Instant::now();
    let mut windows = state.rate_windows.lock().await;
    let attempts = windows.entry((ip, class)).or_default();
    while attempts
        .front()
        .is_some_and(|seen| now.duration_since(*seen) >= RATE_WINDOW)
    {
        attempts.pop_front();
    }
    if attempts.len() >= limit {
        let retry_after = attempts
            .front()
            .map(|seen| {
                RATE_WINDOW
                    .saturating_sub(now.duration_since(*seen))
                    .as_secs()
                    .max(1)
            })
            .unwrap_or(1);
        return Err(AppError::RateLimited(retry_after));
    }
    attempts.push_back(now);
    Ok(())
}

async fn purge_expired(db: &SqlitePool) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
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
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn test_state(static_dir: PathBuf) -> Arc<AppState> {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let http = reqwest::Client::new();
        Arc::new(AppState {
            db,
            database_file: None,
            backup_file: None,
            build_sha: "test-build".into(),
            static_dir,
            rate_windows: Mutex::new(HashMap::new()),
            license_cache: Mutex::new(HashMap::new()),
            billing_base: "http://127.0.0.1:1".into(),
            entra: auth::EntraValidator::from_environment(http.clone()),
            http,
            demos: Mutex::new(HashMap::new()),
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
    fn validation_rejects_malformed_typed_values() {
        let mut field = test_field("worker");
        field.field_type = "date".into();
        assert!(validate_value(&field, "not-a-date").is_err());
        assert!(validate_value(&field, "2026-09-02").is_ok());

        field.field_type = "time".into();
        assert!(validate_value(&field, "quarter-past-nine").is_err());
        assert!(validate_value(&field, "09:15").is_ok());

        field.field_type = "tel".into();
        assert!(validate_value(&field, "not a phone").is_err());
        assert!(validate_value(&field, "+1 (555) 019-9000").is_ok());
    }

    #[test]
    // @claim:token-hashing
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

    #[test]
    fn csv_cells_neutralize_every_spreadsheet_formula_prefix() {
        for value in ["=SUM(1,1)", "+cmd", "-2+3", "@SUM(A1:A2)", "\t=1", "\r=1"] {
            assert_eq!(csv_safe_cell(value), format!("'{value}"));
        }
        assert_eq!(csv_safe_cell("ordinary text"), "ordinary text");
    }

    #[tokio::test]
    async fn controlled_bootstrap_initializes_a_working_vault_only_once() {
        let state = test_state(PathBuf::new()).await;
        sqlx::migrate!().run(&state.db).await.unwrap();
        initialize_workspace_from_bootstrap(
            &state.db,
            BootstrapConfig {
                business_name: "Durable Route Repairs".into(),
                timezone: "UTC".into(),
                region: "European Union".into(),
                deletion_days: 30,
            },
        )
        .await
        .unwrap();

        let workspace: (String, i64) =
            sqlx::query_as("SELECT business_name, deletion_days FROM workspaces WHERE id = 1")
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert_eq!(workspace, ("Durable Route Repairs".into(), 30));
        assert_eq!(load_fields(&state.db).await.unwrap().len(), 8);

        initialize_workspace_from_bootstrap(
            &state.db,
            BootstrapConfig {
                business_name: "Must not replace owner data".into(),
                timezone: "UTC".into(),
                region: "United States".into(),
                deletion_days: 1,
            },
        )
        .await
        .unwrap();
        let business_name: String =
            sqlx::query_scalar("SELECT business_name FROM workspaces WHERE id = 1")
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert_eq!(business_name, "Durable Route Repairs");
    }

    #[tokio::test]
    async fn zero_secret_bootstrap_makes_the_booking_form_available() {
        let state = test_state(PathBuf::new()).await;
        sqlx::migrate!().run(&state.db).await.unwrap();
        initialize_workspace_from_bootstrap(
            &state.db,
            BootstrapConfig {
                business_name: "Configured Field Team".into(),
                timezone: "UTC".into(),
                region: "European Union".into(),
                deletion_days: 14,
            },
        )
        .await
        .unwrap();
        let service = app(state);

        let session_response = service
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(session_response.status(), StatusCode::OK);
        let session_body = session_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let session_json: Value = serde_json::from_slice(&session_body).unwrap();
        assert_eq!(session_json["configured"], true);
        assert_eq!(session_json["authenticated"], false);
        assert_eq!(
            session_json["identity_provider"],
            "Sociobot Microsoft Entra External ID"
        );

        let form_response = service
            .oneshot(
                Request::builder()
                    .uri("/api/form/public")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(form_response.status(), StatusCode::OK);
        let form_body = form_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let form_json: Value = serde_json::from_slice(&form_body).unwrap();
        assert_eq!(form_json["available"], true);
        assert_eq!(form_json["fields"].as_array().unwrap().len(), 8);
    }

    #[tokio::test]
    async fn first_entra_identity_claims_the_vault_and_other_identities_are_denied() {
        let state = test_state(PathBuf::new()).await;
        sqlx::migrate!().run(&state.db).await.unwrap();
        initialize_workspace_from_bootstrap(
            &state.db,
            BootstrapConfig {
                business_name: "Identity Test Team".into(),
                timezone: "UTC".into(),
                region: "United States".into(),
                deletion_days: 30,
            },
        )
        .await
        .unwrap();
        let mut owner_headers = HeaderMap::new();
        owner_headers.insert("x-test-oid", HeaderValue::from_static("entra-owner-one"));
        require_admin(&state, &owner_headers).await.unwrap();
        let stored: String = sqlx::query_scalar("SELECT owner_oid FROM workspaces WHERE id = 1")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(stored, "entra-owner-one");

        let mut other_headers = HeaderMap::new();
        other_headers.insert("x-test-oid", HeaderValue::from_static("entra-owner-two"));
        assert!(matches!(
            require_admin(&state, &other_headers).await,
            Err(AppError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn startup_migrations_are_safe_to_repeat_for_a_persisted_vault() {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        run_migrations(&db).await.unwrap();
        run_migrations(&db).await.unwrap();
        let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(migration_count, 2);
    }

    #[tokio::test]
    // @claim:durable-snapshot
    async fn durable_snapshot_restores_the_last_local_vault_copy() {
        let directory = env::temp_dir().join(format!("piv-snapshot-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let database = directory.join("vault.db");
        let backup = directory.join("durable.db");
        std::fs::write(&database, b"configured vault").unwrap();
        persist_database_file(Some(&database), Some(&backup))
            .await
            .unwrap();
        std::fs::write(&database, b"replacement content").unwrap();
        restore_database_from_backup(Some(&database), Some(&backup))
            .await
            .unwrap();
        assert_eq!(std::fs::read(&database).unwrap(), b"configured vault");
    }

    #[tokio::test]
    async fn route_pass_requires_a_token_and_accepts_a_cached_verified_token() {
        let state = test_state(PathBuf::new()).await;
        assert!(matches!(
            require_route_pass(&state, &HeaderMap::new()).await,
            Err(AppError::PaymentRequired)
        ));

        let token = "verified-route-pass";
        state.license_cache.lock().await.insert(
            token_hash(token),
            CachedLicense {
                valid: true,
                checked_at: Instant::now(),
            },
        );
        let mut headers = HeaderMap::new();
        headers.insert(LICENSE_HEADER, HeaderValue::from_static(token));
        assert!(require_route_pass(&state, &headers).await.is_ok());
    }

    #[test]
    fn client_identity_uses_the_first_forwarded_hop() {
        let mut request = Request::builder()
            .uri("/api/session")
            .header("x-forwarded-for", "203.0.113.9, 10.0.0.4")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "127.0.0.1:45678".parse::<SocketAddr>().unwrap(),
        ));
        assert_eq!(
            client_ip(&request),
            "203.0.113.9".parse::<IpAddr>().unwrap()
        );
    }

    #[tokio::test]
    async fn identity_rate_limit_is_forwarded_ip_aware_and_returns_retry_after() {
        let state = test_state(PathBuf::new()).await;
        sqlx::migrate!().run(&state.db).await.unwrap();
        let service = app(state);

        for attempt in 1..=AUTH_RATE_LIMIT {
            let response = service
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/session")
                        .header("x-forwarded-for", "203.0.113.20, 10.0.0.4")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "attempt {attempt} should reach the identity handler"
            );
        }

        let limited = service
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/session")
                    .header("x-forwarded-for", "203.0.113.20, 10.0.0.4")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(limited.headers().get(header::RETRY_AFTER).is_some());

        let separate_client = service
            .oneshot(
                Request::builder()
                    .uri("/api/session")
                    .header("x-forwarded-for", "203.0.113.21, 10.0.0.4")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(separate_client.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn every_api_route_has_a_general_rate_limit() {
        let state = test_state(PathBuf::new()).await;
        sqlx::migrate!().run(&state.db).await.unwrap();
        let service = app(state);

        for _ in 0..API_RATE_LIMIT {
            let response = service
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/form/public")
                        .header("x-forwarded-for", "198.51.100.30")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let limited = service
            .oneshot(
                Request::builder()
                    .uri("/api/form/public")
                    .header("x-forwarded-for", "198.51.100.30")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(limited.headers().contains_key(header::RETRY_AFTER));
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
    // @claim:automatic-deletion
    async fn claim_automatic_deletion() {
        let state = test_state(PathBuf::new()).await;
        sqlx::migrate!().run(&state.db).await.unwrap();
        let now = Utc::now();
        for (id, delete_at) in [
            ("expired", now - Duration::minutes(1)),
            ("current", now + Duration::days(1)),
        ] {
            sqlx::query("INSERT INTO bookings (id, created_at, delete_at) VALUES (?, ?, ?)")
                .bind(id)
                .bind((now - Duration::days(2)).to_rfc3339())
                .bind(delete_at.to_rfc3339())
                .execute(&state.db)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO worker_tokens (token_hash, booking_id, expires_at, created_at) VALUES ('expired-token', 'expired', ?, ?)")
            .bind((now + Duration::hours(2)).to_rfc3339())
            .bind(now.to_rfc3339())
            .execute(&state.db)
            .await
            .unwrap();
        purge_expired(&state.db).await.unwrap();
        let bookings: Vec<String> = sqlx::query_scalar("SELECT id FROM bookings ORDER BY id")
            .fetch_all(&state.db)
            .await
            .unwrap();
        let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM worker_tokens")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(bookings, vec!["current"]);
        assert_eq!(tokens, 0);
    }

    #[tokio::test]
    async fn recognized_spa_routes_return_the_shell_with_ok_status() {
        let static_dir = env::temp_dir().join(format!("piv-spa-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&static_dir).unwrap();
        std::fs::write(static_dir.join("index.html"), "<main>Private Intake</main>").unwrap();
        let state = test_state(static_dir.clone()).await;

        for path in [
            "/",
            "/demo",
            "/book",
            "/admin",
            "/auth/callback",
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

    #[tokio::test]
    async fn oversized_requests_keep_the_security_policy_headers() {
        let state = test_state(PathBuf::new()).await;
        sqlx::migrate!().run(&state.db).await.unwrap();
        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/bookings")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        "{{\"values\":{{\"oversized\":\"{}\"}},\"website\":\"\"}}",
                        "x".repeat(70_000)
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(response.headers().contains_key("content-security-policy"));
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }
}
