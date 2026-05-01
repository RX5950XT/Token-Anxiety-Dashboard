use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{AppHandle, Manager};

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    locale: String,
    theme: String,
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
#[serde(rename_all = "camelCase")]
struct ClaudeAuthStatus {
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    subscription_type: Option<String>,
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
        refresh_claude_account(
            home.join(".claude"),
            provider_context(&contexts, &default_state, "claude-code"),
            now,
        ),
        refresh_codex_account(
            home.join(".codex"),
            provider_context(&contexts, &default_state, "codex"),
            now,
        ),
        refresh_gemini_account(
            home.join(".gemini"),
            provider_context(&contexts, &default_state, "gemini-cli"),
            now,
        ),
        refresh_opencode_account(
            &home,
            provider_context(&contexts, &default_state, "opencode-go"),
            now,
        ),
    ];

    DashboardState {
        accounts,
        settings: AppSettings {
            locale: existing.settings.locale,
            theme: existing.settings.theme,
        },
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

    let auth = run_command_json(&claude_executable_path(), &["auth", "status", "--json"])
        .and_then(|value| serde_json::from_value::<ClaudeAuthStatus>(value).ok());

    match auth {
        Some(status) if status.logged_in => {
            account.status = "available".to_string();
            account.accuracy = "local".to_string();
            account.plan_name = status
                .subscription_type
                .map(|plan| format!("Claude {}", title_case(&plan)))
                .unwrap_or_else(|| "Claude Code".to_string());
            account.notes =
                "已讀取 Claude Code 真實登入與方案狀態；本機尚無不耗額度的用量視窗來源。"
                    .to_string();
        }
        _ => {
            account.status = "disconnected".to_string();
            account.notes = "Claude Code 已安裝，但目前未登入。".to_string();
        }
    }

    account
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

    if !codex_is_logged_in(&codex_dir.join("auth.json")) {
        account.status = "disconnected".to_string();
        account.notes = "Codex 已安裝，但目前未登入 ChatGPT。".to_string();
        return account;
    }

    account.accuracy = "local".to_string();
    account.status = "available".to_string();
    account.notes = "已從 Codex 本機 session log 讀取真實 5 小時與每週額度。".to_string();

    if let Some(rate_limits) = read_latest_codex_rate_limits(&codex_dir.join("sessions")) {
        account.plan_name = rate_limits
            .plan_type
            .as_deref()
            .map(title_case)
            .map(|plan| format!("ChatGPT {plan}"))
            .unwrap_or_else(|| "ChatGPT".to_string());

        if let Some(primary) = rate_limits.primary {
            account.windows.push(window(
                "codex-5h",
                "5 小時滾動",
                "rolling-5h",
                primary.used_percent,
                100.0,
                &epoch_seconds_to_rfc3339(primary.resets_at, now),
            ));
        }

        if let Some(secondary) = rate_limits.secondary {
            account.windows.push(window(
                "codex-weekly",
                "每週用量",
                "weekly",
                secondary.used_percent,
                100.0,
                &epoch_seconds_to_rfc3339(secondary.resets_at, now),
            ));
        }
    } else {
        account.notes = "已確認 Codex 登入，但近期 session log 尚未出現 rate_limits。".to_string();
    }

    account
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

    let active_email = read_active_gemini_email(&gemini_dir.join("google_accounts.json"));
    let auth_type = read_gemini_auth_type(&gemini_dir.join("settings.json"));

    if active_email.is_none() && auth_type.is_none() {
        account.status = "disconnected".to_string();
        account.notes = "Gemini CLI 已安裝，但目前未偵測到可用登入狀態。".to_string();
        return account;
    }

    account.status = "available".to_string();
    account.accuracy = "local".to_string();
    account.plan_name = auth_type
        .as_deref()
        .map(title_case)
        .unwrap_or_else(|| "Gemini CLI".to_string());
    account.notes =
        "已讀取 Gemini CLI 真實登入與本機設定；官方每日額度尚未提供可離線讀取來源。".to_string();
    account
}

fn refresh_opencode_account(
    home: &Path,
    context: ProviderContext,
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
        "已從 OpenCode 本機資料庫彙總真實成本；週/月視窗採最近 7 天與 30 天 trailing usage。"
            .to_string();

    if let Ok(connection) = Connection::open(db_path) {
        let now_ms = now.timestamp_millis();
        let windows = [
            (
                "opencode-5h",
                "5 小時滾動",
                "rolling-5h",
                12.0,
                FIVE_HOURS_MS,
            ),
            ("opencode-weekly", "每週用量", "weekly", 30.0, SEVEN_DAYS_MS),
            (
                "opencode-monthly",
                "每月用量",
                "monthly",
                60.0,
                THIRTY_DAYS_MS,
            ),
        ];

        for (id, label, kind, limit, width_ms) in windows {
            let since_ms = now_ms - width_ms;
            let used = query_opencode_cost(&connection, since_ms).unwrap_or(0.0);
            let reset_at = query_trailing_reset_at(&connection, since_ms, width_ms, now);
            account
                .windows
                .push(window(id, label, kind, used, limit, &reset_at.to_rfc3339()));
        }
    } else {
        account.notes = "已找到 OpenCode auth，但目前無法開啟本機資料庫。".to_string();
    }

    account
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

fn default_dashboard_state() -> DashboardState {
    DashboardState {
        settings: AppSettings {
            locale: "zh-TW".to_string(),
            theme: "aurora".to_string(),
        },
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

fn run_command_json(program: &Path, args: &[&str]) -> Option<Value> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    serde_json::from_slice::<Value>(&output.stdout).ok()
}

fn claude_executable_path() -> PathBuf {
    home_dir().join(".local").join("bin").join("claude.exe")
}

fn codex_is_logged_in(auth_path: &Path) -> bool {
    let Ok(raw) = fs::read_to_string(auth_path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };

    let tokens = match json.get("tokens") {
        Some(Value::Object(tokens)) => tokens,
        _ => return false,
    };

    tokens.get("access_token").and_then(Value::as_str).is_some()
        && tokens.get("account_id").and_then(Value::as_str).is_some()
}

#[derive(Debug)]
struct CodexRateLimitWindow {
    used_percent: f64,
    resets_at: i64,
}

#[derive(Debug)]
struct CodexRateLimits {
    primary: Option<CodexRateLimitWindow>,
    secondary: Option<CodexRateLimitWindow>,
    plan_type: Option<String>,
}

fn read_latest_codex_rate_limits(sessions_dir: &Path) -> Option<CodexRateLimits> {
    let newest_files = collect_recent_files(sessions_dir);

    for path in newest_files {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        for line in content.lines().rev() {
            let Ok(json) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(rate_limits) = json
                .get("payload")
                .and_then(|payload| payload.get("rate_limits"))
            else {
                continue;
            };
            let primary = parse_codex_window(rate_limits.get("primary"));
            let secondary = parse_codex_window(rate_limits.get("secondary"));
            if primary.is_none() && secondary.is_none() {
                continue;
            }

            return Some(CodexRateLimits {
                primary,
                secondary,
                plan_type: rate_limits
                    .get("plan_type")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
    }

    None
}

fn collect_recent_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(root, &mut files);
    files.sort_by_key(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok());
    files.reverse();
    files
}

fn collect_files_recursive(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}

fn parse_codex_window(value: Option<&Value>) -> Option<CodexRateLimitWindow> {
    let value = value?;
    Some(CodexRateLimitWindow {
        used_percent: value.get("used_percent")?.as_f64()?,
        resets_at: value.get("resets_at")?.as_i64()?,
    })
}

fn epoch_seconds_to_rfc3339(epoch: i64, fallback: DateTime<Utc>) -> String {
    Utc.timestamp_opt(epoch, 0)
        .single()
        .unwrap_or(fallback)
        .to_rfc3339()
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
    connection
        .query_row(
            "select coalesce(sum(json_extract(data, '$.cost')), 0)
             from part
             where json_extract(data, '$.type') = 'step-finish'
               and time_created >= ?1",
            params![since_ms],
            |row| row.get::<_, f64>(0),
        )
        .map_err(|error| error.to_string())
}

fn query_trailing_reset_at(
    connection: &Connection,
    since_ms: i64,
    width_ms: i64,
    fallback: DateTime<Utc>,
) -> DateTime<Utc> {
    let oldest = connection
        .query_row(
            "select min(time_created)
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

    oldest
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp_millis(timestamp + width_ms))
        .unwrap_or(fallback)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_dashboard_state,
            save_dashboard_state,
            sync_dashboard_state,
            scan_provider_environment
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
