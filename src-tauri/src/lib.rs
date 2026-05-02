use chrono::{DateTime, Datelike, Timelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager};

static DEBUG_LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

macro_rules! log_debug {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let line = format!("[{}] {}", chrono::Utc::now().format("%H:%M:%S%.3f"), msg);
        if let Ok(mut logs) = DEBUG_LOGS.lock() {
            logs.push(line.clone());
            if logs.len() > 500 {
                logs.remove(0);
            }
        }
        eprintln!("{}", line);
    }};
}

const DASHBOARD_STATE_KEY: &str = "dashboard_state";
const FIVE_HOURS_MS: i64 = 5 * 60 * 60 * 1000;
const SEVEN_DAYS_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const THIRTY_DAYS_MS: i64 = 30 * 24 * 60 * 60 * 1000;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardState {
    accounts: Vec<UsageAccount>,
    settings: AppSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OpenCodeResetConfig {
    day: u32,
    hour: u32,
    minute: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    locale: String,
    theme: String,
    opencode_weekly_reset: Option<OpenCodeResetConfig>,
    opencode_monthly_reset: Option<OpenCodeResetConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UsageAccount {
    id: String,
    provider: String,
    account_name: String,
    plan_name: String,
    status: String,
    accuracy: String,
    last_updated: String,
    windows: Vec<QuotaWindow>,
    notes: String,
    order: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct QuotaWindow {
    id: String,
    label: String,
    kind: String,
    used: f64,
    limit: f64,
    reset_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderEnvironment {
    provider: String,
    label: String,
    detected: bool,
    source: String,
    detail: String,
}

#[derive(Debug, Clone)]
struct ProviderContext {
    order: i64,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsageResponse {
    #[serde(rename = "five_hour")]
    five_hour: Option<ClaudeUsageWindow>,
    #[serde(rename = "seven_day")]
    seven_day: Option<ClaudeUsageWindow>,
    #[serde(rename = "extra_usage")]
    extra_usage: Option<ClaudeExtraUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsageWindow {
    utilization: f64,
    #[serde(rename = "resets_at")]
    resets_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeExtraUsage {
    is_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct GeminiQuotaResponse {
    buckets: Vec<GeminiQuotaBucket>,
}

#[derive(Debug, Deserialize)]
struct GeminiLoadCodeAssistResponse {
    #[serde(rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiQuotaBucket {
    #[serde(rename = "resetTime")]
    reset_time: String,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: f64,
    #[serde(rename = "modelId")]
    model_id: String,
}

#[tauri::command]
fn load_dashboard_state(app: AppHandle) -> Result<DashboardState, String> {
    let connection = open_connection(&app)?;
    initialize_database(&connection)?;
    load_state_from_connection(&connection)
}

#[tauri::command]
fn save_dashboard_state(app: AppHandle, state: DashboardState) -> Result<(), String> {
    let connection = open_connection(&app)?;
    initialize_database(&connection)?;
    save_state_to_connection(&connection, &state)
}

#[tauri::command]
fn sync_dashboard_state(app: AppHandle) -> Result<DashboardState, String> {
    let connection = open_connection(&app)?;
    initialize_database(&connection)?;
    let state = load_state_from_connection(&connection)?;
    let refreshed = refresh_dashboard_state(state);
    save_state_to_connection(&connection, &refreshed)?;
    Ok(refreshed)
}

#[tauri::command]
fn scan_provider_environment() -> Vec<ProviderEnvironment> {
    let home = home_dir();

    vec![
        inspect_path("claude-code", "Claude Code", home.join(".claude")),
        inspect_path("codex", "Codex", home.join(".codex")),
        inspect_path("gemini-cli", "Gemini CLI", home.join(".gemini")),
        inspect_opencode(home),
    ]
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    let connection = open_connection(&app)?;
    initialize_database(&connection)?;
    load_settings_from_connection(&connection)
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let connection = open_connection(&app)?;
    initialize_database(&connection)?;
    save_settings_to_connection(&connection, &settings)
}

fn refresh_dashboard_state(existing: DashboardState) -> DashboardState {
    let now = Utc::now();
    let home = home_dir();
    let default_state = default_dashboard_state();

    let contexts = existing
        .accounts
        .iter()
        .map(|account| {
            (
                account.provider.clone(),
                ProviderContext {
                    order: account.order,
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();

    let accounts = vec![
        merge_account_state(
            refresh_claude_account(
                home.join(".claude"),
                provider_context(&contexts, &default_state, "claude-code"),
                now,
            ),
            existing.accounts.iter().find(|a| a.provider == "claude-code"),
            now,
        ),
        merge_account_state(
            refresh_codex_account(
                home.join(".codex"),
                provider_context(&contexts, &default_state, "codex"),
                now,
            ),
            existing.accounts.iter().find(|a| a.provider == "codex"),
            now,
        ),
        merge_account_state(
            refresh_gemini_account(
                home.join(".gemini"),
                provider_context(&contexts, &default_state, "gemini-cli"),
                now,
            ),
            existing.accounts.iter().find(|a| a.provider == "gemini-cli"),
            now,
        ),
        merge_account_state(
            refresh_opencode_account(
                &home,
                provider_context(&contexts, &default_state, "opencode-go"),
                &existing.settings,
                now,
            ),
            existing.accounts.iter().find(|a| a.provider == "opencode-go"),
            now,
        ),
    ];

    DashboardState {
        accounts,
        settings: existing.settings.clone(),
    }
}

fn provider_context(
    contexts: &std::collections::HashMap<String, ProviderContext>,
    defaults: &DashboardState,
    provider: &str,
) -> ProviderContext {
    contexts
        .get(provider)
        .cloned()
        .unwrap_or_else(|| ProviderContext {
            order: defaults
                .accounts
                .iter()
                .find(|account| account.provider == provider)
                .map(|account| account.order)
                .unwrap_or_default(),
        })
}

/// When an OAuth/quota API temporarily fails, preserve the previous windows
/// so the account doesn't "disappear" from the dashboard.
fn merge_account_state(new: UsageAccount, previous: Option<&UsageAccount>, now: DateTime<Utc>) -> UsageAccount {
    let mut merged = new;
    let Some(prev) = previous else {
        return merged;
    };

    // If new state downgraded to "connected" with no windows, but previous had windows,
    // preserve previous windows for up to 360 minutes to avoid flickering during rate limits.
    if merged.status == "connected" && merged.windows.is_empty() && !prev.windows.is_empty() {
        let last_updated = DateTime::parse_from_rfc3339(&prev.last_updated).ok();
        let is_recent = last_updated
            .map(|dt| (now - dt.with_timezone(&Utc)).num_minutes() < 360)
            .unwrap_or(false);

        if is_recent {
            merged.windows = prev.windows.clone();
            merged.status = prev.status.clone();
            merged.accuracy = "estimated".to_string();
            if merged.notes.is_empty() {
                merged.notes = "額度 API 暫時無法讀取，顯示為上次成功取得的資料。".to_string();
            }
        }
    }

    merged
}

fn refresh_claude_account(
    claude_dir: PathBuf,
    context: ProviderContext,
    now: DateTime<Utc>,
) -> UsageAccount {
    let mut account = base_account(
        "claude-main",
        "claude-code",
        "Claude Code",
        "Claude Code",
        context.order,
        now,
    );

    if !claude_dir.exists() {
        account.status = "disconnected".to_string();
        account.notes = "找不到 Claude Code 本機設定目錄。".to_string();
        return account;
    }

    let credentials_path = claude_dir.join(".credentials.json");
    let oauth_token = read_claude_oauth_token(&credentials_path);

    if oauth_token.is_none() {
        account.status = "disconnected".to_string();
        account.notes = "Claude Code 已安裝，但目前未偵測到 OAuth 登入憑證。".to_string();
        return account;
    }

    let token = oauth_token.unwrap();
    log_debug!("claude: token len={}", token.len());

    match fetch_claude_usage(&token) {
        Ok(usage) => apply_claude_usage(&mut account, usage, now),
        Err(error) => {
            log_debug!("claude: API FAILED: {}", error);
            account.status = "connected".to_string();
            account.accuracy = "estimated".to_string();
            account.notes = format!("Anthropic API 暫時失敗：{}；額度視窗將在 API 恢復後顯示。", error);
        }
    }

    account
}

fn apply_claude_usage(account: &mut UsageAccount, usage: ClaudeUsageResponse, now: DateTime<Utc>) {
    log_debug!("claude: API OK, windows={}",
        [usage.five_hour.as_ref().map(|_| "5h"), usage.seven_day.as_ref().map(|_| "7d")]
            .into_iter().flatten().collect::<Vec<_>>().join(", "));
    account.status = "available".to_string();
    account.accuracy = "official".to_string();
    account.plan_name = infer_claude_plan(&usage);
    account.notes = "已從 Anthropic OAuth API 讀取真實額度。".to_string();

    if let Some(fh) = usage.five_hour {
        let reset_at = fh.resets_at.unwrap_or_else(|| now.to_rfc3339());
        account.windows.push(window(
            "claude-5h",
            "5 小時滾動",
            "rolling-5h",
            fh.utilization,
            100.0,
            &reset_at,
        ));
    }

    if let Some(sd) = usage.seven_day {
        let reset_at = sd.resets_at.unwrap_or_else(|| now.to_rfc3339());
        account.windows.push(window(
            "claude-weekly",
            "每週用量",
            "weekly",
            sd.utilization,
            100.0,
            &reset_at,
        ));
    }
}

#[derive(Debug, Deserialize)]
struct CodexApiRateLimitWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<i64>,
    reset_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CodexApiRateLimit {
    primary_window: Option<CodexApiRateLimitWindow>,
    secondary_window: Option<CodexApiRateLimitWindow>,
}

#[derive(Debug, Deserialize)]
struct CodexApiUsageResponse {
    rate_limit: Option<CodexApiRateLimit>,
}

fn read_codex_access_token(auth_path: &Path) -> Option<String> {
    let raw = fs::read_to_string(auth_path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    json.get("tokens")
        .and_then(|t| t.get("access_token"))
        .and_then(Value::as_str)
        .map(String::from)
}

fn fetch_codex_usage_from_api(token: &str) -> Result<CodexApiUsageResponse, String> {
    let mut last_error = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt as u64));
        }
        match ureq::get("https://chatgpt.com/backend-api/wham/usage")
            .set("Authorization", &format!("Bearer {token}"))
            .set("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(15))
            .call()
        {
            Ok(response) => return response.into_json().map_err(|e| e.to_string()),
            Err(ureq::Error::Status(code, _)) => {
                last_error = format!("HTTP {code}");
                if code == 401 {
                    break;
                }
            }
            Err(e) => {
                last_error = e.to_string();
            }
        }
    }
    Err(last_error)
}

fn codex_window_seconds_to_label(secs: i64) -> String {
    match secs {
        18000 => "5 小時滾動".to_string(),
        604800 => "每週用量".to_string(),
        s => {
            let hours = s / 3600;
            if hours >= 24 {
                format!("{} 天", hours / 24)
            } else {
                format!("{} 小時", hours)
            }
        }
    }
}

fn refresh_codex_account(
    codex_dir: PathBuf,
    context: ProviderContext,
    now: DateTime<Utc>,
) -> UsageAccount {
    let mut account = base_account(
        "codex-chatgpt",
        "codex",
        "Codex",
        "Codex",
        context.order,
        now,
    );

    if !codex_dir.exists() {
        account.status = "disconnected".to_string();
        account.notes = "找不到 Codex 本機設定目錄。".to_string();
        return account;
    }

    let auth_path = codex_dir.join("auth.json");
    let Some(token) = read_codex_access_token(&auth_path) else {
        account.status = "disconnected".to_string();
        account.notes = "Codex 已安裝，但目前未登入 ChatGPT。".to_string();
        return account;
    };

    log_debug!("codex: token len={}", token.len());

    match fetch_codex_usage_from_api(&token) {
        Ok(usage) => apply_codex_usage(&mut account, usage, now, &auth_path),
        Err(error) => {
            log_debug!("codex: API FAILED: {}", error);
            account.notes = format!("ChatGPT API 失敗：{}；改以登入狀態顯示。", error);
            account.status = "connected".to_string();
        }
    }

    account
}

fn apply_codex_usage(
    account: &mut UsageAccount,
    usage: CodexApiUsageResponse,
    now: DateTime<Utc>,
    auth_path: &Path,
) {
    account.accuracy = "official".to_string();
    account.status = "available".to_string();
    account.notes = "已從 ChatGPT API 讀取真實額度。".to_string();

    if let Some(rate_limit) = usage.rate_limit {
        for (idx, win) in [rate_limit.primary_window, rate_limit.secondary_window]
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(used) = win.used_percent else { continue };
            let label = win
                .limit_window_seconds
                .map(codex_window_seconds_to_label)
                .unwrap_or_else(|| "unknown".to_string());
            let id = if idx == 0 { "codex-5h" } else { "codex-weekly" };
            let reset_at = win
                .reset_at
                .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
                .unwrap_or(now)
                .to_rfc3339();
            log_debug!("codex: window={} used={:.1}% reset_at={}", id, used, reset_at);
            account.windows.push(window(id, &label, "rolling-5h", used, 100.0, &reset_at));
        }
    } else {
        log_debug!("codex: API OK but no rate_limit in response");
        account.notes = "ChatGPT API 回傳成功，但無額度視窗資料。".to_string();
    }

    if let Ok(raw) = fs::read_to_string(auth_path) {
        if let Ok(json) = serde_json::from_str::<Value>(&raw) {
            if let Some(plan) = json.get("plan_type").and_then(Value::as_str) {
                account.plan_name = format!("ChatGPT {}", title_case(plan));
            }
        }
    }
}

fn refresh_gemini_account(
    gemini_dir: PathBuf,
    context: ProviderContext,
    now: DateTime<Utc>,
) -> UsageAccount {
    let mut account = base_account(
        "gemini-default",
        "gemini-cli",
        "Gemini CLI",
        "Gemini CLI",
        context.order,
        now,
    );

    if !gemini_dir.exists() {
        account.status = "disconnected".to_string();
        account.notes = "找不到 Gemini CLI 本機設定目錄。".to_string();
        return account;
    }

    // Try OAuth-backed quota API first
    let oauth_token = read_gemini_oauth_token(&gemini_dir.join("oauth_creds.json"));

    if let Some(token) = oauth_token {
        log_debug!("[DIAG] gemini: attempting API with token len={}", token.len());
        
        // Step 1: loadCodeAssist to get project ID
        let project_id = match fetch_gemini_load_code_assist(&token) {
            Ok(pid) => pid,
            Err(e) => {
                log_debug!("[DIAG] gemini: loadCodeAssist failed: {}", e);
                None
            }
        };
        
        // Step 2: retrieveUserQuota with project ID
        match fetch_gemini_quota(&token, project_id.as_deref()) {
            Ok(quota) => {
                apply_gemini_quota(&mut account, quota);
                return account;
            }
            Err(error) => {
                log_debug!("[DIAG] gemini: API FAILED: {}", error);
                account.notes = format!("Google Quota API 失敗：{error}；改以設定檔偵測。");
            }
        }
    }

    apply_gemini_fallback(&mut account, &gemini_dir);
    account
}

fn apply_gemini_fallback(account: &mut UsageAccount, gemini_dir: &Path) {
    let active_email = read_active_gemini_email(&gemini_dir.join("google_accounts.json"));
    let auth_type = read_gemini_auth_type(&gemini_dir.join("settings.json"));

    if active_email.is_none() && auth_type.is_none() {
        account.status = "disconnected".to_string();
        account.notes = "Gemini CLI 已安裝，但目前未偵測到可用登入狀態。".to_string();
        return;
    }

    account.status = "connected".to_string();
    account.accuracy = "local".to_string();
    account.plan_name = auth_type
        .as_deref()
        .map(title_case)
        .unwrap_or_else(|| "Gemini CLI".to_string());
    if account.notes.is_empty() {
        account.notes = "已讀取 Gemini CLI 真實登入與本機設定；尚無額度視窗。".to_string();
    }
}

fn apply_gemini_quota(account: &mut UsageAccount, quota: GeminiQuotaResponse) {
    log_debug!("[DIAG] gemini: API OK, {} buckets", quota.buckets.len());
    account.status = "available".to_string();
    account.accuracy = "official".to_string();
    account.plan_name = "Gemini".to_string();
    account.notes = "已從 Google cloudcode-pa API 讀取真實額度。".to_string();

    // Group buckets by model category (Pro/Flash/Flash Lite) and take
    // the minimum remainingFraction for each category. This matches the
    // behavior of CC-Switch and correctly aggregates multi-version buckets.
    let mut category_map: HashMap<String, (f64, String)> = HashMap::new();
    
    for bucket in &quota.buckets {
        let category = classify_gemini_model(&bucket.model_id).to_string();
        let remaining = bucket.remaining_fraction.clamp(0.0, 1.0);
        
        let entry = category_map
            .entry(category)
            .or_insert((remaining, bucket.reset_time.clone()));
        if remaining < entry.0 {
            entry.0 = remaining;
            entry.1 = bucket.reset_time.clone();
        }
    }
    
    log_debug!("[DIAG] gemini: aggregated into {} categories", category_map.len());
    
    // Convert to tiers (remainingFraction → utilization)
    let mut tiers: Vec<_> = category_map
        .into_iter()
        .map(|(category, (remaining, reset_time))| {
            let used = (1.0 - remaining) * 100.0;
            let id = format!("gemini-{}", category.to_lowercase().replace(" ", "-"));
            log_debug!(
                "[DIAG] gemini: aggregated window={} class={} used={:.1}% remaining={:.2} reset={}",
                id, category, used, remaining, reset_time
            );
            (id, category, used, reset_time)
        })
        .collect();

    // Fixed order: Pro → Flash → Flash Lite
    tiers.sort_by_key(|(_, category, _, _)| match category.as_str() {
        "Pro" => 0,
        "Flash" => 1,
        "Flash Lite" => 2,
        _ => 3,
    });

    for (id, category, used, reset_time) in tiers {
        account.windows.push(window(&id, &category, "daily", used, 100.0, &reset_time));
    }
}

fn refresh_opencode_account(
    home: &Path,
    context: ProviderContext,
    settings: &AppSettings,
    now: DateTime<Utc>,
) -> UsageAccount {
    let mut account = base_account(
        "opencode-go",
        "opencode-go",
        "OpenCode",
        "OpenCode Go",
        context.order,
        now,
    );

    let auth_path = home
        .join(".local")
        .join("share")
        .join("opencode")
        .join("auth.json");
    let db_path = home
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db");

    if !auth_path.exists() && !db_path.exists() {
        account.status = "disconnected".to_string();
        account.notes = "找不到 OpenCode auth 或資料庫。".to_string();
        return account;
    }

    account.status = "available".to_string();
    account.accuracy = "local".to_string();
    account.notes =
        "已從 OpenCode 本機資料庫彙總真實成本；視窗為 trailing usage，非固定週期重置。"
            .to_string();

    if let Ok(connection) = Connection::open(db_path) {
        apply_opencode_windows(&mut account, &connection, settings, now);
    } else {
        account.notes = "已找到 OpenCode auth，但目前無法開啟本機資料庫。".to_string();
    }

    account
}

fn apply_opencode_windows(account: &mut UsageAccount, connection: &Connection, settings: &AppSettings, now: DateTime<Utc>) {
    let now_ms = now.timestamp_millis();
    log_debug!("[DIAG] opencode: db opened, now_ms={}", now_ms);
    let windows = [
        ("opencode-5h", "5 小時滾動", "rolling-5h", 12.0, FIVE_HOURS_MS),
        ("opencode-weekly", "每週使用量", "weekly", 30.0, SEVEN_DAYS_MS),
        ("opencode-monthly", "每月使用量", "monthly", 60.0, THIRTY_DAYS_MS),
    ];

    for (id, label, kind, limit, width_ms) in windows {
        let since_ms = now_ms - width_ms;
        let used = match query_opencode_cost(connection, since_ms) {
            Ok(cost) => cost,
            Err(error) => {
                log_debug!("[DIAG] opencode: query cost failed for {}: {}", id, error);
                account.notes = format!("{} 視窗讀取失敗：{}；其餘視窗可能仍可正常顯示。", label, error);
                continue;
            }
        };

        let (final_used, reset_at) = if kind == "rolling-5h" {
            let reset_at = query_trailing_reset_at(connection, since_ms, width_ms)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| {
                    log_debug!("[DIAG] opencode: no records in {} window, reset_at=now", id);
                    now.to_rfc3339()
                });
            
            // If reset time has passed, the rolling window is empty
            let final_used = if let Ok(reset_dt) = reset_at.parse::<DateTime<Utc>>() {
                if now > reset_dt {
                    0.0
                } else {
                    used
                }
            } else {
                used
            };
            
            (final_used, reset_at)
        } else if kind == "weekly" {
            let reset_at = settings.opencode_weekly_reset.as_ref()
                .and_then(|config| calculate_next_weekly_reset(config, now))
                .unwrap_or_default();
            (used, reset_at)
        } else {
            let reset_at = settings.opencode_monthly_reset.as_ref()
                .and_then(|config| calculate_next_monthly_reset(config, now))
                .unwrap_or_default();
            (used, reset_at)
        };

        log_debug!("[DIAG] opencode: window={} used={:.2} reset_at={}", id, final_used, reset_at);
        account.windows.push(window(id, label, kind, final_used, limit, &reset_at));
    }
}

fn open_connection(app: &AppHandle) -> Result<Connection, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&app_dir).map_err(|error| error.to_string())?;
    Connection::open(app_dir.join("token-anxiety-dashboard.sqlite3"))
        .map_err(|error| error.to_string())
}

const SETTINGS_KEY: &str = "app_settings";

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "create table if not exists app_state (
                key text primary key,
                value text not null,
                updated_at text not null default current_timestamp
            )",
            [],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn load_state_from_connection(connection: &Connection) -> Result<DashboardState, String> {
    let stored: Result<String, rusqlite::Error> = connection.query_row(
        "select value from app_state where key = ?1",
        params![DASHBOARD_STATE_KEY],
        |row| row.get(0),
    );

    match stored {
        Ok(value) => serde_json::from_str(&value).map_err(|error| error.to_string()),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_dashboard_state()),
        Err(error) => Err(error.to_string()),
    }
}

fn save_state_to_connection(connection: &Connection, state: &DashboardState) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(state).map_err(|error| error.to_string())?;
    connection
        .execute(
            "insert into app_state(key, value, updated_at) values (?1, ?2, current_timestamp)
             on conflict(key) do update set value = excluded.value, updated_at = current_timestamp",
            params![DASHBOARD_STATE_KEY, serialized],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn load_settings_from_connection(connection: &Connection) -> Result<AppSettings, String> {
    let stored: Result<String, rusqlite::Error> = connection.query_row(
        "select value from app_state where key = ?1",
        params![SETTINGS_KEY],
        |row| row.get(0),
    );

    match stored {
        Ok(value) => serde_json::from_str(&value).map_err(|error| error.to_string()),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_app_settings()),
        Err(error) => Err(error.to_string()),
    }
}

fn save_settings_to_connection(connection: &Connection, settings: &AppSettings) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    connection
        .execute(
            "insert into app_state(key, value, updated_at) values (?1, ?2, current_timestamp)
             on conflict(key) do update set value = excluded.value, updated_at = current_timestamp",
            params![SETTINGS_KEY, serialized],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn home_dir() -> PathBuf {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn inspect_path(provider: &str, label: &str, path: PathBuf) -> ProviderEnvironment {
    let detected = path.exists();
    ProviderEnvironment {
        provider: provider.to_string(),
        label: label.to_string(),
        source: path.display().to_string(),
        detected,
        detail: if detected {
            "已找到本機設定目錄。".to_string()
        } else {
            "尚未找到本機設定目錄。".to_string()
        },
    }
}

fn inspect_opencode(home: PathBuf) -> ProviderEnvironment {
    let auth_path = home
        .join(".local")
        .join("share")
        .join("opencode")
        .join("auth.json");
    let config_path = home.join(".config").join("opencode").join("opencode.json");
    let detected = auth_path.exists() || config_path.exists();
    let source = format!("{} | {}", auth_path.display(), config_path.display());

    ProviderEnvironment {
        provider: "opencode-go".to_string(),
        label: "OpenCode".to_string(),
        source,
        detected,
        detail: if detected {
            "已找到 OpenCode auth 或 config。".to_string()
        } else {
            "尚未找到 OpenCode auth/config。".to_string()
        },
    }
}

fn default_app_settings() -> AppSettings {
    AppSettings {
        locale: "zh-TW".to_string(),
        theme: "aurora".to_string(),
        opencode_weekly_reset: Some(OpenCodeResetConfig {
            day: 1,    // Monday
            hour: 7,
            minute: 0,
        }),
        opencode_monthly_reset: Some(OpenCodeResetConfig {
            day: 29,
            hour: 0,
            minute: 0,
        }),
    }
}

fn default_dashboard_state() -> DashboardState {
    DashboardState {
        settings: default_app_settings(),
        accounts: vec![
            base_account(
                "claude-main",
                "claude-code",
                "Claude Code",
                "Claude Code",
                0,
                Utc::now(),
            ),
            base_account("codex-chatgpt", "codex", "Codex", "Codex", 1, Utc::now()),
            base_account(
                "gemini-default",
                "gemini-cli",
                "Gemini CLI",
                "Gemini CLI",
                2,
                Utc::now(),
            ),
            base_account(
                "opencode-go",
                "opencode-go",
                "OpenCode",
                "OpenCode Go",
                3,
                Utc::now(),
            ),
        ],
    }
}

fn base_account(
    id: &str,
    provider: &str,
    name: &str,
    plan: &str,
    order: i64,
    now: DateTime<Utc>,
) -> UsageAccount {
    UsageAccount {
        id: id.to_string(),
        provider: provider.to_string(),
        account_name: name.to_string(),
        plan_name: plan.to_string(),
        status: "available".to_string(),
        accuracy: "estimated".to_string(),
        last_updated: now.to_rfc3339(),
        windows: Vec::new(),
        notes: String::new(),
        order,
    }
}

fn window(id: &str, label: &str, kind: &str, used: f64, limit: f64, reset_at: &str) -> QuotaWindow {
    QuotaWindow {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        used,
        limit,
        reset_at: reset_at.to_string(),
    }
}

fn read_active_gemini_email(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let json = serde_json::from_str::<Value>(&raw).ok()?;
    json.get("active")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn read_gemini_auth_type(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let json = serde_json::from_str::<Value>(&raw).ok()?;
    json.get("security")?
        .get("auth")?
        .get("selectedType")?
        .as_str()
        .map(str::to_string)
}

fn query_opencode_cost(connection: &Connection, since_ms: i64) -> Result<f64, String> {
    // OpenCode's `part` table stores *incremental* cost per `step-finish` row.
    // Each row's `$.cost` is the cost of that individual step, not a running
    // total. Therefore SUM is correct for all window types (5h rolling,
    // weekly, monthly).
    let result = connection
        .query_row(
            "select coalesce(sum(json_extract(data, '$.cost')), 0)
             from part
             where json_extract(data, '$.type') = 'step-finish'
               and time_created >= ?1",
            params![since_ms],
            |row| row.get::<_, f64>(0),
        )
        .map_err(|error| error.to_string());
    log_debug!("[DIAG] opencode: query cost since {} = {:?}", since_ms, result);
    result
}

fn query_trailing_reset_at(
    connection: &Connection,
    since_ms: i64,
    width_ms: i64,
) -> Option<DateTime<Utc>> {
    // OpenCode's official logic: the 5h rolling window resets 5 hours after
    // the *latest* usage. When that time passes, the window is empty and
    // used% automatically drops to 0.
    let latest: Option<i64> = connection
        .query_row(
            "select max(time_created)
             from part
             where json_extract(data, '$.type') = 'step-finish'
               and time_created >= ?1",
            params![since_ms],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .ok()
        .flatten()
        .flatten();

    log_debug!("[DIAG] opencode: latest record since {} = {:?}", since_ms, latest);
    latest.and_then(|timestamp| {
        let reset = if timestamp > 1_000_000_000_000i64 {
            DateTime::<Utc>::from_timestamp_millis(timestamp + width_ms)
        } else {
            DateTime::<Utc>::from_timestamp(timestamp + (width_ms / 1000), 0)
        };
        log_debug!("[DIAG] opencode: calculated reset_at = {:?}", reset);
        reset
    })
}

fn calculate_next_weekly_reset(config: &OpenCodeResetConfig, now: DateTime<Utc>) -> Option<String> {
    let target_dow = config.day as u8; // 0=Sunday, 1=Monday, ..., 6=Saturday
    let current_dow = now.weekday().num_days_from_sunday() as u8;
    
    let days_until = if target_dow > current_dow {
        target_dow - current_dow
    } else if target_dow < current_dow {
        7 - (current_dow - target_dow)
    } else {
        // Same day: if the reset time has already passed today, move to next week
        let reset_today = now.date_naive().and_hms_opt(config.hour as u32, config.minute as u32, 0)?;
        let reset_today_dt = DateTime::<Utc>::from_naive_utc_and_offset(reset_today, Utc);
        if now >= reset_today_dt {
            7
        } else {
            0
        }
    };

    let next_reset = now + chrono::Duration::days(days_until as i64);
    let reset_naive = next_reset.date_naive().and_hms_opt(config.hour as u32, config.minute as u32, 0)?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(reset_naive, Utc).to_rfc3339())
}

fn calculate_next_monthly_reset(config: &OpenCodeResetConfig, now: DateTime<Utc>) -> Option<String> {
    let target_dom = config.day as u32;
    let current_year = now.year();
    let current_month = now.month();
    let current_day = now.day();
    let current_hour = now.hour();
    let current_minute = now.minute();

    let (target_year, target_month) = if target_dom > current_day || 
        (target_dom == current_day && (config.hour as u32 > current_hour || 
         (config.hour as u32 == current_hour && config.minute as u32 > current_minute))) {
        (current_year, current_month)
    } else {
        // Move to next month
        let next_month = current_month + 1;
        if next_month > 12 {
            (current_year + 1, 1)
        } else {
            (current_year, next_month)
        }
    };

    // Handle months with fewer days
    let last_day = chrono::NaiveDate::from_ymd_opt(target_year, target_month, 1)
        .and_then(|d| d.checked_add_months(chrono::Months::new(1)))
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28);

    let actual_dom = target_dom.min(last_day);
    let reset_naive = chrono::NaiveDate::from_ymd_opt(target_year, target_month, actual_dom)?
        .and_hms_opt(config.hour as u32, config.minute as u32, 0)?;
    
    Some(DateTime::<Utc>::from_naive_utc_and_offset(reset_naive, Utc).to_rfc3339())
}

fn read_claude_oauth_token(credentials_path: &Path) -> Option<String> {
    let raw = fs::read_to_string(credentials_path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    json.get("claudeAiOauth")?
        .get("accessToken")?
        .as_str()
        .map(String::from)
}

fn fetch_claude_usage(token: &str) -> Result<ClaudeUsageResponse, String> {
    let mut last_error = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt as u64));
        }
        match ureq::get("https://api.anthropic.com/api/oauth/usage")
            .set("Authorization", &format!("Bearer {token}"))
            .set("anthropic-beta", "oauth-2025-04-20")
            .set("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(15))
            .call()
        {
            Ok(response) => return response.into_json().map_err(|e| e.to_string()),
            Err(ureq::Error::Status(code, _)) => {
                last_error = format!("HTTP {code}");
                if code == 401 {
                    break; // Token invalid, don't retry
                }
            }
            Err(e) => {
                last_error = e.to_string();
            }
        }
    }
    Err(last_error)
}

fn infer_claude_plan(usage: &ClaudeUsageResponse) -> String {
    if usage.extra_usage.as_ref().map(|e| e.is_enabled).unwrap_or(false) {
        "Claude Pro / Max".to_string()
    } else {
        "Claude".to_string()
    }
}

fn read_gemini_oauth_token(oauth_path: &Path) -> Option<String> {
    let raw = fs::read_to_string(oauth_path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    json.get("access_token")?
        .as_str()
        .map(String::from)
}

fn fetch_gemini_load_code_assist(token: &str) -> Result<Option<String>, String> {
    match ureq::post("https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(15))
        .send_json(ureq::json!({
            "metadata": {
                "ideType": "GEMINI_CLI",
                "pluginType": "GEMINI"
            }
        }))
    {
        Ok(response) => {
            let data: GeminiLoadCodeAssistResponse = response.into_json().map_err(|e| e.to_string())?;
            let project_id = data.cloudaicompanion_project.as_ref()
                .and_then(|s| s.split('/').last())
                .map(String::from);
            log_debug!("[DIAG] gemini: loadCodeAssist project_id={:?}", project_id);
            Ok(project_id)
        }
        Err(ureq::Error::Status(code, response)) => {
            log_debug!("[DIAG] gemini: loadCodeAssist HTTP {code}, body={}", response.into_string().unwrap_or_default());
            Ok(None) // Continue without project ID
        }
        Err(e) => {
            log_debug!("[DIAG] gemini: loadCodeAssist error: {}", e);
            Ok(None) // Continue without project ID
        }
    }
}

fn fetch_gemini_quota(token: &str, project_id: Option<&str>) -> Result<GeminiQuotaResponse, String> {
    let mut last_error = String::new();
    let mut body = serde_json::Map::new();
    if let Some(pid) = project_id {
        body.insert("project".to_string(), serde_json::Value::String(pid.to_string()));
    }
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt as u64));
        }
        match ureq::post("https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota")
            .set("Authorization", &format!("Bearer {token}"))
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(15))
            .send_json(ureq::json!(body))
        {
            Ok(response) => return response.into_json().map_err(|e| e.to_string()),
            Err(ureq::Error::Status(code, _)) => {
                last_error = format!("HTTP {code}");
                if code == 401 {
                    break; // Token invalid, don't retry
                }
            }
            Err(e) => {
                last_error = e.to_string();
            }
        }
    }
    Err(last_error)
}

fn classify_gemini_model(model_id: &str) -> &str {
    let lower = model_id.to_lowercase();
    if lower.contains("flash-lite") {
        "Flash Lite"
    } else if lower.contains("flash") {
        "Flash"
    } else if lower.contains("pro") {
        "Pro"
    } else {
        model_id
    }
}

fn title_case(value: &str) -> String {
    value
        .split(['-', '_', ' '])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
fn toggle_devtools(window: tauri::WebviewWindow) {
    if window.is_devtools_open() {
        window.close_devtools();
    } else {
        window.open_devtools();
    }
}

#[tauri::command]
fn get_debug_logs() -> Vec<String> {
    DEBUG_LOGS.lock().map(|logs| logs.clone()).unwrap_or_default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_dashboard_state,
            save_dashboard_state,
            sync_dashboard_state,
            scan_provider_environment,
            toggle_devtools,
            get_debug_logs,
            get_settings,
            set_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_account_state_preserves_windows() {
        let prev = UsageAccount {
            id: "test".to_string(),
            provider: "claude-code".to_string(),
            account_name: "Test".to_string(),
            plan_name: "Test Plan".to_string(),
            status: "available".to_string(),
            accuracy: "official".to_string(),
            last_updated: Utc::now().to_rfc3339(),
            windows: vec![QuotaWindow {
                id: "w1".to_string(),
                label: "Window".to_string(),
                kind: "daily".to_string(),
                used: 50.0,
                limit: 100.0,
                reset_at: Utc::now().to_rfc3339(),
            }],
            notes: "prev note".to_string(),
            order: 0,
        };

        let mut next = prev.clone();
        next.status = "connected".to_string();
        next.windows.clear();
        next.notes = String::new();

        let merged = merge_account_state(next, Some(&prev), Utc::now());
        assert_eq!(merged.status, "available"); // preserved
        assert_eq!(merged.windows.len(), 1); // preserved
        assert_eq!(merged.accuracy, "estimated");
    }

    #[test]
    fn test_merge_account_state_expires_after_360_minutes() {
        let now = Utc::now();
        let stale = now - chrono::Duration::minutes(361);

        let prev = UsageAccount {
            id: "test".to_string(),
            provider: "claude-code".to_string(),
            account_name: "Test".to_string(),
            plan_name: "Test Plan".to_string(),
            status: "available".to_string(),
            accuracy: "official".to_string(),
            last_updated: stale.to_rfc3339(),
            windows: vec![QuotaWindow {
                id: "w1".to_string(),
                label: "Window".to_string(),
                kind: "daily".to_string(),
                used: 50.0,
                limit: 100.0,
                reset_at: stale.to_rfc3339(),
            }],
            notes: "prev note".to_string(),
            order: 0,
        };

        let mut next = prev.clone();
        next.status = "connected".to_string();
        next.windows.clear();
        next.notes = String::new();

        let merged = merge_account_state(next, Some(&prev), now);
        assert_eq!(merged.status, "connected"); // not preserved, too old
        assert_eq!(merged.windows.len(), 0);
    }
}
