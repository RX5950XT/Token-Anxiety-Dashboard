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
    #[serde(default)]
    visible_providers: Option<Vec<String>>,
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

#[derive(Clone, Debug, Deserialize)]
struct AntigravityTier {
    #[serde(rename = "isDefault")]
    is_default: Option<bool>,
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AntigravityLoadResponse {
    #[serde(rename = "cloudaicompanionProject")]
    cloudaicompanion_project: Option<String>,
    #[serde(rename = "paidTier")]
    paid_tier: Option<AntigravityTier>,
    #[serde(rename = "currentTier")]
    current_tier: Option<AntigravityTier>,
    #[serde(rename = "allowedTiers")]
    allowed_tiers: Option<Vec<AntigravityTier>>,
    #[serde(rename = "ineligibleTiers")]
    ineligible_tiers: Option<Vec<AntigravityIneligibleTier>>,
}

#[derive(Debug, Deserialize)]
struct AntigravityIneligibleTier {
    #[allow(dead_code)]
    #[serde(rename = "reasonCode")]
    reason_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AntigravityModelsResponse {
    #[serde(default)]
    models: HashMap<String, AntigravityModelInfo>,
}

#[derive(Debug, Deserialize)]
struct AntigravityModelInfo {
    #[serde(rename = "quotaInfo")]
    quota_info: Option<AntigravityQuotaInfo>,
}

#[derive(Debug, Deserialize)]
struct AntigravityQuotaInfo {
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AntigravityQuotaSummaryResponse {
    #[serde(default)]
    groups: Vec<AntigravityQuotaSummaryGroup>,
}

#[derive(Debug, Deserialize)]
struct AntigravityQuotaSummaryGroup {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(default)]
    buckets: Vec<AntigravityQuotaSummaryBucket>,
}

#[derive(Debug, Deserialize)]
struct AntigravityQuotaSummaryBucket {
    #[serde(rename = "bucketId")]
    bucket_id: Option<String>,
    window: Option<String>,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum AntigravityFamily {
    Claude,
    Gemini,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum AntigravityWindowSlot {
    FiveHour,
    Weekly,
}

struct AntigravitySlotValue {
    remaining: f64,
    reset: String,
}

struct AntigravityCreds {
    access_token: String,
    refresh_token: String,
    expiry: Option<String>,
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
        inspect_antigravity(),
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
            refresh_antigravity_account(
                provider_context(&contexts, &default_state, "antigravity"),
                now,
            ),
            existing.accounts.iter().find(|a| a.provider == "antigravity"),
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

    if merged.provider == "antigravity" {
        merge_antigravity_windows(&mut merged, prev);
    }

    merged
}

/// Keep Antigravity Claude/Gemini bars visible when the API omits exhausted pools.
fn merge_antigravity_windows(merged: &mut UsageAccount, prev: &UsageAccount) {
    const EXPECTED: [(&str, &str, &str); 4] = [
        ("antigravity-claude-5h", "Claude", "rolling-5h"),
        ("antigravity-claude-weekly", "Claude", "weekly"),
        ("antigravity-gemini-5h", "Gemini", "rolling-5h"),
        ("antigravity-gemini-weekly", "Gemini", "weekly"),
    ];

    let mut restored = false;
    for (id, label, kind) in EXPECTED {
        if merged.windows.iter().any(|window| window.id == id) {
            continue;
        }

        if let Some(prev_window) = find_antigravity_previous_window(prev, id) {
            merged.windows.push(prev_window);
            restored = true;
            log_debug!("[DIAG] antigravity: preserved missing window {id}");
            continue;
        }

        merged.windows.push(window(id, label, kind, 100.0, 100.0, ""));
        restored = true;
        log_debug!("[DIAG] antigravity: placeholder window {id} at 100% used");
    }

    sort_antigravity_windows(&mut merged.windows);

    if restored && merged.accuracy == "official" {
        merged.accuracy = "estimated".to_string();
        if merged.notes == "已從 Google cloudcode-pa API（Antigravity）讀取真實額度。" {
            merged.notes =
                "部分額度視窗 API 未回傳，已保留上次資料或顯示為已用盡。".to_string();
        }
    }
}

fn find_antigravity_previous_window(prev: &UsageAccount, id: &str) -> Option<QuotaWindow> {
    if let Some(prev_window) = prev.windows.iter().find(|window| window.id == id) {
        return Some(prev_window.clone());
    }

    let legacy_id = match id {
        "antigravity-claude-5h" => Some("antigravity-claude"),
        "antigravity-gemini-5h" => Some("antigravity-gemini"),
        _ => None,
    }?;
    let legacy = prev.windows.iter().find(|window| window.id == legacy_id)?;
    let kind = if id.ends_with("-weekly") {
        "weekly"
    } else {
        "rolling-5h"
    };
    Some(window(
        id,
        &legacy.label,
        kind,
        legacy.used,
        legacy.limit,
        &legacy.reset_at,
    ))
}

fn sort_antigravity_windows(windows: &mut Vec<QuotaWindow>) {
    let order = |id: &str| -> i32 {
        match id {
            "antigravity-claude-5h" => 0,
            "antigravity-claude-weekly" => 1,
            "antigravity-gemini-5h" => 2,
            "antigravity-gemini-weekly" => 3,
            "antigravity-claude" => 0,
            "antigravity-gemini" => 2,
            _ => 99,
        }
    };
    windows.sort_by_key(|window| order(&window.id));
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
            "",
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
            "",
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

fn codex_window_seconds_to_label(_secs: i64) -> String {
    // Labels are handled by frontend i18n formatWindowLabel based on window kind
    String::new()
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
            let (id, kind) = if idx == 0 {
                ("codex-5h", "rolling-5h")
            } else {
                ("codex-weekly", "weekly")
            };
            let reset_at = win
                .reset_at
                .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
                .unwrap_or(now)
                .to_rfc3339();
            log_debug!("codex: window={} used={:.1}% reset_at={}", id, used, reset_at);
            account.windows.push(window(id, &label, kind, used, 100.0, &reset_at));
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

fn refresh_antigravity_account(context: ProviderContext, now: DateTime<Utc>) -> UsageAccount {
    let mut account = base_account(
        "antigravity-default",
        "antigravity",
        "Antigravity",
        "Antigravity",
        context.order,
        now,
    );

    let Some(creds) = read_antigravity_credentials() else {
        account.status = "disconnected".to_string();
        account.notes = "找不到 Antigravity 登入憑證（請先在 Antigravity 登入）。".to_string();
        return account;
    };

    log_debug!(
        "[DIAG] antigravity: creds read, access_token len={}, expiry={:?}",
        creds.access_token.len(),
        creds.expiry
    );

    // Use stored access_token; refresh proactively if expired / near expiry / unparseable.
    let mut token = creds.access_token.clone();
    if antigravity_token_expired(&creds.expiry, now) {
        log_debug!("[DIAG] antigravity: token expired/near expiry, refreshing");
        match refresh_antigravity_token(&creds.refresh_token) {
            Ok(fresh) => {
                log_debug!("[DIAG] antigravity: refresh OK, new token len={}", fresh.len());
                token = fresh;
            }
            Err(e) => log_debug!("[DIAG] antigravity: refresh FAILED: {e} (keeping stored token)"),
        }
    }

    let (project_id, subscription_tier) = fetch_antigravity_code_assist(&token);
    if let Err(error) = sync_antigravity_quota(
        &mut account,
        &token,
        project_id.as_deref(),
        subscription_tier.as_deref(),
    ) {
        if error == "HTTP 401" {
            log_debug!("[DIAG] antigravity: models 401, forcing refresh + retry");
            match refresh_antigravity_token(&creds.refresh_token) {
                Ok(fresh) => {
                    let (pid, tier) = fetch_antigravity_code_assist(&fresh);
                    if let Err(e2) =
                        sync_antigravity_quota(&mut account, &fresh, pid.as_deref(), tier.as_deref())
                    {
                        log_debug!("[DIAG] antigravity: retry FAILED: {e2}");
                        account.status = "connected".to_string();
                        account.notes = format!("Antigravity 額度 API 失敗：{e2}。");
                    }
                }
                Err(e) => {
                    log_debug!("[DIAG] antigravity: forced refresh FAILED: {e}");
                    account.status = "connected".to_string();
                    account.notes = "Antigravity token 更新失敗，請重新登入 Antigravity。".to_string();
                }
            }
        } else {
            log_debug!("[DIAG] antigravity: models FAILED: {error}");
            account.status = "connected".to_string();
            account.notes = format!("Antigravity 額度 API 失敗：{error}。");
        }
    }

    account
}

fn sync_antigravity_quota(
    account: &mut UsageAccount,
    token: &str,
    project_id: Option<&str>,
    subscription_tier: Option<&str>,
) -> Result<(), String> {
    let summary = fetch_antigravity_quota_summary(token, project_id);
    let models = fetch_antigravity_models(token, project_id)?;
    apply_antigravity_quota(account, models, summary, subscription_tier);
    Ok(())
}

/// Align with Antigravity-Manager: group displayName is authoritative; bucketId is fallback.
fn antigravity_family_for_group(group_name: &str, bucket_id: &str) -> Option<AntigravityFamily> {
    let group = group_name.to_lowercase();
    if group.contains("gemini") {
        return Some(AntigravityFamily::Gemini);
    }
    if group.contains("claude") || group.contains("gpt") || group.contains("third") {
        return Some(AntigravityFamily::Claude);
    }

    let bucket = bucket_id.to_lowercase();
    if bucket.contains("gemini") {
        return Some(AntigravityFamily::Gemini);
    }
    if bucket.contains("claude") || bucket.contains("3p") {
        return Some(AntigravityFamily::Claude);
    }
    None
}

fn antigravity_window_slot(window: &str) -> Option<AntigravityWindowSlot> {
    match window.trim().to_lowercase().as_str() {
        "5h" => Some(AntigravityWindowSlot::FiveHour),
        "weekly" => Some(AntigravityWindowSlot::Weekly),
        _ => None,
    }
}

fn upsert_antigravity_slot(
    slots: &mut std::collections::HashMap<(AntigravityFamily, AntigravityWindowSlot), AntigravitySlotValue>,
    family: AntigravityFamily,
    slot: AntigravityWindowSlot,
    remaining: f64,
    reset: String,
) {
    let remaining = remaining.clamp(0.0, 1.0);
    match slots.get_mut(&(family, slot)) {
        Some(entry) if remaining < entry.remaining => {
            entry.remaining = remaining;
            if !reset.is_empty() {
                entry.reset = reset;
            }
        }
        Some(_) => {}
        None => {
            slots.insert(
                (family, slot),
                AntigravitySlotValue {
                    remaining,
                    reset,
                },
            );
        }
    }
}

fn populate_antigravity_slots_from_summary(
    slots: &mut std::collections::HashMap<(AntigravityFamily, AntigravityWindowSlot), AntigravitySlotValue>,
    summary: &AntigravityQuotaSummaryResponse,
) {
    for group in &summary.groups {
        let group_name = group.display_name.clone().unwrap_or_default();
        for bucket in &group.buckets {
            let bucket_id = bucket.bucket_id.clone().unwrap_or_default();
            let Some(family) = antigravity_family_for_group(&group_name, &bucket_id) else {
                log_debug!(
                    "[DIAG] antigravity: skip unknown quota group={group_name} bucket={bucket_id}"
                );
                continue;
            };
            let window_name = bucket.window.clone().unwrap_or_default();
            let Some(slot) = antigravity_window_slot(&window_name) else {
                continue;
            };
            let remaining = bucket.remaining_fraction.unwrap_or(0.0);
            let reset = bucket.reset_time.clone().unwrap_or_default();
            upsert_antigravity_slot(slots, family, slot, remaining, reset);
        }
    }
}

fn populate_antigravity_5h_from_models_if_missing(
    slots: &mut std::collections::HashMap<(AntigravityFamily, AntigravityWindowSlot), AntigravitySlotValue>,
    resp: &AntigravityModelsResponse,
) {
    let mut claude: Option<(f64, String)> = None;
    let mut gemini: Option<(f64, String)> = None;

    for (name, info) in &resp.models {
        let bucket = if name.starts_with("claude") {
            &mut claude
        } else if name.starts_with("gemini") {
            &mut gemini
        } else {
            continue;
        };

        let (remaining, reset) = match &info.quota_info {
            Some(qi) => {
                let remaining = qi.remaining_fraction.unwrap_or(0.0).clamp(0.0, 1.0);
                let reset = qi.reset_time.clone().unwrap_or_default();
                (remaining, reset)
            }
            None => (0.0, String::new()),
        };

        match bucket {
            Some(entry) if remaining < entry.0 => {
                entry.0 = remaining;
                if !reset.is_empty() {
                    entry.1 = reset;
                }
            }
            Some(_) => {}
            None => *bucket = Some((remaining, reset)),
        }
    }

    if !slots.contains_key(&(AntigravityFamily::Claude, AntigravityWindowSlot::FiveHour)) {
        if let Some((remaining, reset)) = claude {
            upsert_antigravity_slot(
                slots,
                AntigravityFamily::Claude,
                AntigravityWindowSlot::FiveHour,
                remaining,
                reset,
            );
        }
    }

    if !slots.contains_key(&(AntigravityFamily::Gemini, AntigravityWindowSlot::FiveHour)) {
        if let Some((remaining, reset)) = gemini {
            upsert_antigravity_slot(
                slots,
                AntigravityFamily::Gemini,
                AntigravityWindowSlot::FiveHour,
                remaining,
                reset,
            );
        }
    }
}

fn push_antigravity_windows(
    account: &mut UsageAccount,
    slots: &std::collections::HashMap<(AntigravityFamily, AntigravityWindowSlot), AntigravitySlotValue>,
) {
    const ORDER: [(AntigravityFamily, AntigravityWindowSlot, &str, &str, &str); 4] = [
        (
            AntigravityFamily::Claude,
            AntigravityWindowSlot::FiveHour,
            "antigravity-claude-5h",
            "Claude",
            "rolling-5h",
        ),
        (
            AntigravityFamily::Claude,
            AntigravityWindowSlot::Weekly,
            "antigravity-claude-weekly",
            "Claude",
            "weekly",
        ),
        (
            AntigravityFamily::Gemini,
            AntigravityWindowSlot::FiveHour,
            "antigravity-gemini-5h",
            "Gemini",
            "rolling-5h",
        ),
        (
            AntigravityFamily::Gemini,
            AntigravityWindowSlot::Weekly,
            "antigravity-gemini-weekly",
            "Gemini",
            "weekly",
        ),
    ];

    for (family, slot, id, label, kind) in ORDER {
        let Some(value) = slots.get(&(family, slot)) else {
            continue;
        };
        let used = (1.0 - value.remaining) * 100.0;
        let family_name = match family {
            AntigravityFamily::Claude => "Claude",
            AntigravityFamily::Gemini => "Gemini",
        };
        let window_name = match slot {
            AntigravityWindowSlot::FiveHour => "5h",
            AntigravityWindowSlot::Weekly => "weekly",
        };
        log_debug!(
            "[DIAG] antigravity: {family_name} {window_name} used={used:.1}% remaining={:.2} reset={}",
            value.remaining,
            value.reset
        );
        account
            .windows
            .push(window(id, label, kind, used, 100.0, &value.reset));
    }
}

fn apply_antigravity_quota(
    account: &mut UsageAccount,
    resp: AntigravityModelsResponse,
    summary: Option<AntigravityQuotaSummaryResponse>,
    subscription_tier: Option<&str>,
) {
    log_debug!(
        "[DIAG] antigravity: models OK ({}), summary groups={}",
        resp.models.len(),
        summary
            .as_ref()
            .map(|value| value.groups.len())
            .unwrap_or(0)
    );
    account.status = "available".to_string();
    account.accuracy = "official".to_string();
    account.plan_name = subscription_tier
        .filter(|tier| !tier.is_empty())
        .map(|tier| format!("Antigravity {tier}"))
        .unwrap_or_else(|| "Antigravity".to_string());
    account.notes = "已從 Google cloudcode-pa API（Antigravity）讀取真實額度。".to_string();
    account.windows.clear();

    let mut slots: std::collections::HashMap<
        (AntigravityFamily, AntigravityWindowSlot),
        AntigravitySlotValue,
    > = std::collections::HashMap::new();

    if let Some(summary) = summary {
        populate_antigravity_slots_from_summary(&mut slots, &summary);
    }
    populate_antigravity_5h_from_models_if_missing(&mut slots, &resp);
    push_antigravity_windows(account, &slots);
    sort_antigravity_windows(&mut account.windows);

    if account.windows.is_empty() {
        account.notes = "Antigravity 已連線，但未取得 Claude/Gemini 額度資料。".to_string();
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
        ("opencode-5h", "", "rolling-5h", 12.0, FIVE_HOURS_MS),
        ("opencode-weekly", "", "weekly", 30.0, SEVEN_DAYS_MS),
        ("opencode-monthly", "", "monthly", 60.0, THIRTY_DAYS_MS),
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

fn inspect_antigravity() -> ProviderEnvironment {
    let detected = read_antigravity_credentials().is_some();
    let source = if cfg!(target_os = "windows") {
        "Windows 憑證管理員: gemini:antigravity".to_string()
    } else {
        "（目前僅支援 Windows 讀取 Antigravity 憑證）".to_string()
    };
    ProviderEnvironment {
        provider: "antigravity".to_string(),
        label: "Antigravity".to_string(),
        source,
        detected,
        detail: if detected {
            "已找到 Antigravity 登入憑證。".to_string()
        } else {
            "尚未找到 Antigravity 登入憑證。".to_string()
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
        visible_providers: None,
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
                "antigravity-default",
                "antigravity",
                "Antigravity",
                "Antigravity",
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

// Antigravity uses Google's internal cloudcode-pa API (same family as the
// retired Gemini CLI) but with ideType=ANTIGRAVITY and the fetchAvailableModels
// endpoint. Antigravity's own built-in OAuth client (used only to refresh the
// locally stored token) is injected at build time via environment variables so
// the literals are never committed — see README「建置」. When unset, refresh is
// skipped and a still-valid stored access_token is used as-is.
const ANTIGRAVITY_CLIENT_ID: Option<&str> = option_env!("ANTIGRAVITY_CLIENT_ID");
const ANTIGRAVITY_CLIENT_SECRET: Option<&str> = option_env!("ANTIGRAVITY_CLIENT_SECRET");
const ANTIGRAVITY_USER_AGENT: &str = "vscode/1.X.X (Antigravity/4.2.1)";
const ANTIGRAVITY_BASES: [&str; 3] = [
    "https://daily-cloudcode-pa.sandbox.googleapis.com",
    "https://daily-cloudcode-pa.googleapis.com",
    "https://cloudcode-pa.googleapis.com",
];

/// Read the locally logged-in Antigravity token from the Windows Credential
/// Manager (target `gemini:antigravity`). Antigravity 4.x stores its OAuth
/// token there as a JSON blob: `{ "token": { access_token, refresh_token,
/// expiry, .. }, "auth_method": .. }`.
#[cfg(target_os = "windows")]
fn read_antigravity_credentials() -> Option<AntigravityCreds> {
    use std::os::windows::ffi::OsStrExt;

    #[repr(C)]
    struct Filetime {
        _low: u32,
        _high: u32,
    }

    #[repr(C)]
    struct CredentialW {
        _flags: u32,
        _cred_type: u32,
        _target_name: *const u16,
        _comment: *const u16,
        _last_written: Filetime,
        credential_blob_size: u32,
        credential_blob: *const u8,
        _persist: u32,
        _attribute_count: u32,
        _attributes: *const std::ffi::c_void,
        _target_alias: *const u16,
        _user_name: *const u16,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn CredReadW(
            target_name: *const u16,
            cred_type: u32,
            flags: u32,
            credential: *mut *mut CredentialW,
        ) -> i32;
        fn CredFree(buffer: *const std::ffi::c_void);
    }

    let target_wide: Vec<u16> = std::ffi::OsStr::new("gemini:antigravity")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut cred_ptr: *mut CredentialW = std::ptr::null_mut();
        // 1 = CRED_TYPE_GENERIC
        if CredReadW(target_wide.as_ptr(), 1, 0, &mut cred_ptr) == 0 || cred_ptr.is_null() {
            log_debug!("[DIAG] antigravity: CredReadW found no gemini:antigravity entry");
            return None;
        }
        let cred = &*cred_ptr;
        let bytes =
            std::slice::from_raw_parts(cred.credential_blob, cred.credential_blob_size as usize)
                .to_vec();
        CredFree(cred_ptr as *const std::ffi::c_void);

        let text = String::from_utf8(bytes).ok()?;
        parse_antigravity_creds(&text)
    }
}

#[cfg(not(target_os = "windows"))]
fn read_antigravity_credentials() -> Option<AntigravityCreds> {
    None
}

fn parse_antigravity_creds(text: &str) -> Option<AntigravityCreds> {
    let json: Value = serde_json::from_str(text).ok()?;
    let token = json.get("token")?;
    Some(AntigravityCreds {
        access_token: token.get("access_token")?.as_str()?.to_string(),
        refresh_token: token.get("refresh_token")?.as_str()?.to_string(),
        expiry: token.get("expiry").and_then(Value::as_str).map(String::from),
    })
}

/// Token is considered stale when expiry is missing, unparseable, or within
/// 15 minutes of now. RFC3339 with offset/`Z` are both accepted.
fn antigravity_token_expired(expiry: &Option<String>, now: DateTime<Utc>) -> bool {
    match expiry {
        Some(raw) => match DateTime::parse_from_rfc3339(raw) {
            Ok(dt) => now + chrono::Duration::minutes(15) >= dt.with_timezone(&Utc),
            Err(_) => true,
        },
        None => true,
    }
}

fn refresh_antigravity_token(refresh_token: &str) -> Result<String, String> {
    let (Some(client_id), Some(client_secret)) =
        (ANTIGRAVITY_CLIENT_ID, ANTIGRAVITY_CLIENT_SECRET)
    else {
        return Err(
            "未設定 ANTIGRAVITY_CLIENT_ID/SECRET build 環境變數，無法 refresh token".to_string(),
        );
    };
    match ureq::post("https://oauth2.googleapis.com/token")
        .set("User-Agent", ANTIGRAVITY_USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .send_form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ]) {
        Ok(response) => {
            let json: Value = response.into_json().map_err(|e| e.to_string())?;
            json.get("access_token")
                .and_then(Value::as_str)
                .map(String::from)
                .ok_or_else(|| "refresh response missing access_token".to_string())
        }
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code}")),
        Err(e) => Err(e.to_string()),
    }
}

fn extract_antigravity_subscription_tier(data: &AntigravityLoadResponse) -> Option<String> {
    let mut tier = data
        .paid_tier
        .as_ref()
        .and_then(|value| value.name.clone().or_else(|| value.id.clone()));

    let ineligible = data
        .ineligible_tiers
        .as_ref()
        .is_some_and(|values| !values.is_empty());

    if tier.is_none() && !ineligible {
        tier = data
            .current_tier
            .as_ref()
            .and_then(|value| value.name.clone().or_else(|| value.id.clone()));
    }

    if tier.is_none() {
        if let Some(mut allowed) = data.allowed_tiers.clone() {
            if let Some(default_tier) = allowed.iter_mut().find(|value| value.is_default == Some(true))
            {
                tier = default_tier
                    .name
                    .clone()
                    .or_else(|| default_tier.id.clone())
                    .map(|value| format!("{value} (Restricted)"));
            }
        }
    }

    tier
}

fn fetch_antigravity_code_assist(token: &str) -> (Option<String>, Option<String>) {
    for base in ANTIGRAVITY_BASES {
        let url = format!("{base}/v1internal:loadCodeAssist");
        match ureq::post(&url)
            .set("Authorization", &format!("Bearer {token}"))
            .set("Content-Type", "application/json")
            .set("Accept", "application/json")
            .set("User-Agent", ANTIGRAVITY_USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .send_json(ureq::json!({ "metadata": { "ideType": "ANTIGRAVITY" } }))
        {
            Ok(response) => {
                if let Ok(data) = response.into_json::<AntigravityLoadResponse>() {
                    let project_id = data
                        .cloudaicompanion_project
                        .as_ref()
                        .and_then(|s| s.split('/').last())
                        .map(String::from);
                    let subscription_tier = extract_antigravity_subscription_tier(&data);
                    log_debug!(
                        "[DIAG] antigravity: loadCodeAssist OK via {base}, project={:?}, tier={:?}",
                        project_id,
                        subscription_tier
                    );
                    return (project_id, subscription_tier);
                }
            }
            Err(ureq::Error::Status(code, _)) => {
                log_debug!("[DIAG] antigravity: loadCodeAssist {base} HTTP {code}");
            }
            Err(e) => {
                log_debug!("[DIAG] antigravity: loadCodeAssist {base} error: {e}");
            }
        }
    }
    (None, None)
}

fn fetch_antigravity_models(
    token: &str,
    project_id: Option<&str>,
) -> Result<AntigravityModelsResponse, String> {
    let mut last_error = String::new();
    for base in ANTIGRAVITY_BASES {
        let url = format!("{base}/v1internal:fetchAvailableModels");
        // Try with project id first; on 403 retry the same base with empty body.
        let bodies: Vec<Value> = match project_id {
            Some(pid) => vec![ureq::json!({ "project": pid }), ureq::json!({})],
            None => vec![ureq::json!({})],
        };
        for body in bodies {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {token}"))
                .set("Content-Type", "application/json")
                .set("Accept", "application/json")
                .set("User-Agent", ANTIGRAVITY_USER_AGENT)
                .timeout(std::time::Duration::from_secs(15))
                .send_json(body)
            {
                Ok(response) => return response.into_json().map_err(|e| e.to_string()),
                Err(ureq::Error::Status(code, _)) => {
                    last_error = format!("HTTP {code}");
                    if code == 401 {
                        return Err(last_error); // let caller refresh + retry
                    }
                    if code != 403 {
                        break; // non-403: move to next base
                    }
                    // 403: fall through to retry this base with empty body
                }
                Err(e) => {
                    last_error = e.to_string();
                    break; // network error: move to next base
                }
            }
        }
    }
    Err(last_error)
}

/// Best-effort grouped quota (5h + weekly per model family). Failure must not block sync.
fn fetch_antigravity_quota_summary(
    token: &str,
    project_id: Option<&str>,
) -> Option<AntigravityQuotaSummaryResponse> {
    let mut last_error = String::new();
    for base in ANTIGRAVITY_BASES {
        let url = format!("{base}/v1internal:retrieveUserQuotaSummary");
        let bodies: Vec<Value> = match project_id {
            Some(pid) => vec![ureq::json!({ "project": pid }), ureq::json!({})],
            None => vec![ureq::json!({})],
        };
        for body in bodies {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {token}"))
                .set("Content-Type", "application/json")
                .set("Accept", "application/json")
                .set("User-Agent", ANTIGRAVITY_USER_AGENT)
                .timeout(std::time::Duration::from_secs(15))
                .send_json(body)
            {
                Ok(response) => {
                    match response.into_json::<AntigravityQuotaSummaryResponse>() {
                        Ok(summary) => {
                            log_debug!(
                                "[DIAG] antigravity: quotaSummary OK via {base}, groups={}",
                                summary.groups.len()
                            );
                            return Some(summary);
                        }
                        Err(error) => {
                            log_debug!(
                                "[DIAG] antigravity: quotaSummary parse failed via {base}: {error}"
                            );
                            return None;
                        }
                    }
                }
                Err(ureq::Error::Status(code, _)) => {
                    last_error = format!("HTTP {code}");
                    if code == 401 {
                        log_debug!("[DIAG] antigravity: quotaSummary {base} HTTP 401");
                        return None;
                    }
                    // Match Antigravity-Manager: 429/5xx fall through endpoints; other 4xx stop.
                    if (400..500).contains(&code) && code != 403 && code != 429 {
                        log_debug!("[DIAG] antigravity: quotaSummary {base} HTTP {code} (no retry)");
                        return None;
                    }
                    if code != 403 {
                        break;
                    }
                }
                Err(error) => {
                    last_error = error.to_string();
                    break;
                }
            }
        }
    }
    if !last_error.is_empty() {
        log_debug!("[DIAG] antigravity: quotaSummary unavailable: {last_error}");
    }
    None
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

    #[test]
    fn test_parse_antigravity_creds() {
        let blob = r#"{"token":{"access_token":"AT","token_type":"Bearer","refresh_token":"RT","expiry":"2026-05-23T07:07:24.94+08:00"},"auth_method":"consumer"}"#;
        let creds = parse_antigravity_creds(blob).expect("should parse");
        assert_eq!(creds.access_token, "AT");
        assert_eq!(creds.refresh_token, "RT");
        assert_eq!(creds.expiry.as_deref(), Some("2026-05-23T07:07:24.94+08:00"));
    }

    #[test]
    fn test_parse_antigravity_creds_rejects_missing_fields() {
        assert!(parse_antigravity_creds(r#"{"auth_method":"consumer"}"#).is_none());
        assert!(parse_antigravity_creds("not json").is_none());
    }

    #[test]
    fn test_antigravity_token_expired() {
        let now = Utc::now();
        let fresh = (now + chrono::Duration::hours(1)).to_rfc3339();
        let stale = (now - chrono::Duration::hours(1)).to_rfc3339();
        assert!(!antigravity_token_expired(&Some(fresh), now));
        assert!(antigravity_token_expired(&Some(stale), now));
        assert!(antigravity_token_expired(&None, now)); // missing → refresh
        assert!(antigravity_token_expired(&Some("garbage".to_string()), now)); // unparseable → refresh
    }

    #[test]
    fn test_apply_antigravity_quota_summary_four_windows() {
        let summary_json = r#"{
            "groups": [
                {
                    "displayName": "Claude and GPT models",
                    "buckets": [
                        {"bucketId": "3p-5h", "window": "5h", "remainingFraction": 0.6, "resetTime": "2026-05-24T08:54:55Z"},
                        {"bucketId": "3p-weekly", "window": "weekly", "remainingFraction": 0.75, "resetTime": "2026-05-27T00:00:00Z"}
                    ]
                },
                {
                    "displayName": "Gemini Models",
                    "buckets": [
                        {"bucketId": "gemini-5h", "window": "5h", "remainingFraction": 0.4, "resetTime": "2026-05-24T09:00:00Z"},
                        {"bucketId": "gemini-weekly", "window": "weekly", "remainingFraction": 0.9, "resetTime": "2026-05-30T00:00:00Z"}
                    ]
                }
            ]
        }"#;
        let summary: AntigravityQuotaSummaryResponse = serde_json::from_str(summary_json).unwrap();
        let models: AntigravityModelsResponse = serde_json::from_str(r#"{"models": {}}"#).unwrap();
        let mut account =
            base_account("antigravity-default", "antigravity", "Antigravity", "Antigravity", 2, Utc::now());
        apply_antigravity_quota(&mut account, models, Some(summary), Some("Pro"));

        assert_eq!(account.windows.len(), 4);
        assert_eq!(account.windows[0].id, "antigravity-claude-5h");
        assert_eq!(account.windows[0].kind, "rolling-5h");
        assert!((account.windows[0].used - 40.0).abs() < 0.001);
        assert_eq!(account.windows[1].id, "antigravity-claude-weekly");
        assert_eq!(account.windows[1].kind, "weekly");
        assert!((account.windows[1].used - 25.0).abs() < 0.001);
        assert_eq!(account.windows[2].id, "antigravity-gemini-5h");
        assert!((account.windows[2].used - 60.0).abs() < 0.001);
        assert_eq!(account.windows[3].id, "antigravity-gemini-weekly");
        assert!((account.windows[3].used - 10.0).abs() < 0.001);
        assert_eq!(account.plan_name, "Antigravity Pro");
    }

    #[test]
    fn test_apply_antigravity_quota_models_fallback_for_5h() {
        let json = r#"{
            "models": {
                "claude-sonnet-4-6": {"quotaInfo": {"remainingFraction": 0.8, "resetTime": "2026-05-24T08:54:55Z"}},
                "claude-opus-4-6-thinking": {"quotaInfo": {"remainingFraction": 0.4, "resetTime": "2026-05-24T08:54:55Z"}},
                "gemini-2.5-pro": {"quotaInfo": {"remainingFraction": 0.9, "resetTime": "2026-05-24T08:54:55Z"}},
                "gemini-3-flash": {"quotaInfo": {"remainingFraction": 0.6, "resetTime": "2026-05-24T08:54:55Z"}},
                "gpt-oss-120b-medium": {"quotaInfo": {"remainingFraction": 0.1, "resetTime": ""}}
            }
        }"#;
        let resp: AntigravityModelsResponse = serde_json::from_str(json).unwrap();
        let mut account =
            base_account("antigravity-default", "antigravity", "Antigravity", "Antigravity", 2, Utc::now());
        apply_antigravity_quota(&mut account, resp, None, None);

        assert_eq!(account.windows.len(), 2);
        assert_eq!(account.windows[0].id, "antigravity-claude-5h");
        assert_eq!(account.windows[0].kind, "rolling-5h");
        assert!((account.windows[0].used - 60.0).abs() < 0.001);
        assert_eq!(account.windows[1].id, "antigravity-gemini-5h");
        assert!((account.windows[1].used - 40.0).abs() < 0.001);
    }

    #[test]
    fn test_apply_antigravity_quota_exhausted_pools_stay_visible() {
        let json = r#"{
            "models": {
                "claude-sonnet-4-6": {"quotaInfo": {"remainingFraction": 0, "resetTime": "2026-05-24T08:54:55Z"}},
                "claude-opus-4-6-thinking": {"quotaInfo": {"resetTime": "2026-05-24T09:00:00Z"}},
                "gemini-2.5-pro": {},
                "gemini-3-flash": {"quotaInfo": {"remainingFraction": 0.0, "resetTime": "2026-05-24T10:00:00Z"}}
            }
        }"#;
        let resp: AntigravityModelsResponse = serde_json::from_str(json).unwrap();
        let mut account =
            base_account("antigravity-default", "antigravity", "Antigravity", "Antigravity", 2, Utc::now());
        apply_antigravity_quota(&mut account, resp, None, None);

        assert_eq!(account.windows.len(), 2);
        assert_eq!(account.windows[0].id, "antigravity-claude-5h");
        assert!((account.windows[0].used - 100.0).abs() < 0.001);
        assert_eq!(account.windows[1].id, "antigravity-gemini-5h");
        assert!((account.windows[1].used - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_apply_antigravity_quota_uses_subscription_tier() {
        let models: AntigravityModelsResponse = serde_json::from_str(r#"{"models": {}}"#).unwrap();
        let mut account =
            base_account("antigravity-default", "antigravity", "Antigravity", "Antigravity", 2, Utc::now());
        apply_antigravity_quota(&mut account, models, None, Some("Google AI Pro"));
        assert_eq!(account.plan_name, "Antigravity Google AI Pro");
    }

    #[test]
    fn test_merge_antigravity_windows_preserves_missing_pool() {
        let prev = UsageAccount {
            id: "antigravity-default".to_string(),
            provider: "antigravity".to_string(),
            account_name: "Antigravity".to_string(),
            plan_name: "Antigravity".to_string(),
            status: "available".to_string(),
            accuracy: "official".to_string(),
            last_updated: Utc::now().to_rfc3339(),
            windows: vec![
                window("antigravity-claude-5h", "Claude", "rolling-5h", 100.0, 100.0, "2026-05-24T08:54:55Z"),
                window("antigravity-claude-weekly", "Claude", "weekly", 80.0, 100.0, "2026-05-27T00:00:00Z"),
                window("antigravity-gemini-5h", "Gemini", "rolling-5h", 40.0, 100.0, "2026-05-24T09:00:00Z"),
                window("antigravity-gemini-weekly", "Gemini", "weekly", 20.0, 100.0, "2026-05-30T00:00:00Z"),
            ],
            notes: "prev".to_string(),
            order: 2,
        };

        let mut next = prev.clone();
        next.windows = vec![window(
            "antigravity-gemini-5h",
            "Gemini",
            "rolling-5h",
            40.0,
            100.0,
            "2026-05-24T09:00:00Z",
        )];
        next.accuracy = "official".to_string();
        next.notes = "已從 Google cloudcode-pa API（Antigravity）讀取真實額度。".to_string();

        merge_antigravity_windows(&mut next, &prev);

        assert_eq!(next.windows.len(), 4);
        assert_eq!(next.windows[0].id, "antigravity-claude-5h");
        assert!((next.windows[0].used - 100.0).abs() < 0.001);
        assert_eq!(next.windows[1].id, "antigravity-claude-weekly");
        assert_eq!(next.windows[2].id, "antigravity-gemini-5h");
        assert_eq!(next.accuracy, "estimated");
    }
}
