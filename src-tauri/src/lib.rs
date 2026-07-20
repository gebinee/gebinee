use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tauri::{Manager, path::BaseDirectory};

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct AppConfig {
    #[serde(default)]
    use_custom_db: bool,
    #[serde(default)]
    db_path: String,
}

#[derive(serde::Serialize)]
struct Segment {
    text: String,
    is_word: bool,
    found: bool,
}

#[derive(serde::Serialize)]
struct DatabaseInfo {
    word_count: i64,
    db_path: String,
    mode: String,
    connected: bool,
    error: String,
}

struct AppState {
    conn: std::sync::Mutex<Option<rusqlite::Connection>>,
    config: std::sync::Mutex<AppConfig>,
}

fn load_config(app_data_dir: &Path) -> AppConfig {
    let config_path = app_data_dir.join("config.json");
    match std::fs::read_to_string(&config_path) {
        Ok(content) => serde_json::from_str::<AppConfig>(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

fn save_config(app_data_dir: &Path, config: &AppConfig) -> Result<(), String> {
    let config_path = app_data_dir.join("config.json");
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, content).map_err(|e| e.to_string())
}

fn resolve_db_path(config: &AppConfig, builtin_db_path: &Path) -> PathBuf {
    if config.use_custom_db && !config.db_path.is_empty() {
        PathBuf::from(&config.db_path)
    } else {
        builtin_db_path.to_path_buf()
    }
}

// 解析内置数据库路径（打包后在 $RESOURCE/resources/word.sqlite，dev 模式下从源文件读取）
fn builtin_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve("resources/word.sqlite", BaseDirectory::Resource)
        .map_err(|e| e.to_string())
}

fn open_connection(path: &Path) -> Result<rusqlite::Connection, String> {
    if !path.exists() {
        return Err(format!("数据库文件不存在: {}", path.display()));
    }
    rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn transform(state: tauri::State<AppState>, input: String) -> Result<Vec<Segment>, String> {
    let conn_guard = state.conn.lock().map_err(|e| e.to_string())?;
    let conn = match conn_guard.as_ref() {
        Some(c) => c,
        None => return Err("数据库未连接".to_string()),
    };

    // 解析输入：字母字符组成单词，非字母字符原样收集
    // 先按顺序构建段序列（保持原始大小写用于输出，同时收集小写形式用于查询）
    enum Piece {
        Word(String),        // 原始大小写单词
        NonWord(String),     // 非字母字符片段
    }

    let mut pieces: Vec<Piece> = Vec::new();
    let mut current_word = String::new();
    let mut current_nonword = String::new();

    for ch in input.chars() {
        if ch.is_alphabetic() {
            if !current_nonword.is_empty() {
                pieces.push(Piece::NonWord(std::mem::take(&mut current_nonword)));
            }
            current_word.push(ch);
        } else {
            if !current_word.is_empty() {
                pieces.push(Piece::Word(std::mem::take(&mut current_word)));
            }
            current_nonword.push(ch);
        }
    }
    if !current_word.is_empty() {
        pieces.push(Piece::Word(current_word));
    }
    if !current_nonword.is_empty() {
        pieces.push(Piece::NonWord(current_nonword));
    }

    // 收集所有唯一单词（精确匹配，与原 Java 逻辑一致）
    let mut unique_words: HashSet<String> = HashSet::new();
    for piece in &pieces {
        if let Piece::Word(w) = piece {
            unique_words.insert(w.clone());
        }
    }

    // 分块批量查询数据库（每批 500 个）
    let mut word_map: HashMap<String, String> = HashMap::new();
    let words: Vec<String> = unique_words.into_iter().collect();
    for chunk in words.chunks(500) {
        if chunk.is_empty() {
            continue;
        }
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT key, value FROM kv_store WHERE key IN ({})", placeholders);
        let params = rusqlite::params_from_iter(chunk.iter());
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params, |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (key, value) = row.map_err(|e| e.to_string())?;
            word_map.insert(key, value);
        }
    }

    // 构建 Vec<Segment>
    let mut segments: Vec<Segment> = Vec::new();
    for piece in pieces {
        match piece {
            Piece::NonWord(text) => {
                segments.push(Segment {
                    text,
                    is_word: false,
                    found: true,
                });
            }
            Piece::Word(original) => {
                match word_map.get(&original) {
                    Some(value) => segments.push(Segment {
                        text: value.clone(),
                        is_word: true,
                        found: true,
                    }),
                    None => segments.push(Segment {
                        text: original,
                        is_word: true,
                        found: false,
                    }),
                }
            }
        }
    }

    Ok(segments)
}

#[tauri::command]
fn get_database_info(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<DatabaseInfo, String> {
    let builtin_path = builtin_db_path(&app)?;
    let config = state.config.lock().map_err(|e| e.to_string())?;
    let db_path = resolve_db_path(&config, &builtin_path);
    let db_path_str = db_path.display().to_string();

    let conn_guard = state.conn.lock().map_err(|e| e.to_string())?;
    match conn_guard.as_ref() {
        Some(conn) => {
            let word_count: i64 = match conn.query_row("SELECT COUNT(*) FROM kv_store", [], |row| {
                row.get(0)
            }) {
                Ok(c) => c,
                Err(e) => {
                    return Ok(DatabaseInfo {
                        word_count: 0,
                        db_path: db_path_str,
                        mode: "read-only".to_string(),
                        connected: true,
                        error: e.to_string(),
                    });
                }
            };
            Ok(DatabaseInfo {
                word_count,
                db_path: db_path_str,
                mode: "read-only".to_string(),
                connected: true,
                error: String::new(),
            })
        }
        None => Ok(DatabaseInfo {
            word_count: 0,
            db_path: db_path_str,
            mode: "read-only".to_string(),
            connected: false,
            error: "数据库未连接".to_string(),
        }),
    }
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> AppConfig {
    let config = state.config.lock().expect("config lock poisoned");
    config.clone()
}

#[tauri::command]
fn set_config(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    use_custom_db: bool,
    db_path: String,
) -> Result<(), String> {
    if use_custom_db && db_path.is_empty() {
        return Err("数据库文件路径不能为空".to_string());
    }

    let new_config = AppConfig {
        use_custom_db,
        db_path: db_path.clone(),
    };

    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let builtin_path = builtin_db_path(&app)?;
    let target_path = resolve_db_path(&new_config, &builtin_path);

    // 尝试打开新连接
    let new_conn = open_connection(&target_path)?;

    // 成功：替换旧连接，更新 config，保存配置
    let mut conn_guard = state.conn.lock().map_err(|e| e.to_string())?;
    *conn_guard = Some(new_conn);

    {
        let mut config_guard = state.config.lock().map_err(|e| e.to_string())?;
        *config_guard = new_config.clone();
    }

    save_config(&app_data_dir, &new_config)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let app_data_dir = app.path().app_data_dir()?;

            // 确保 app_data_dir 存在（仅用于 config.json）
            std::fs::create_dir_all(&app_data_dir)?;

            // 加载配置
            let config = load_config(&app_data_dir);

            // 解析内置数据库路径并打开连接
            // 内置数据库直接从 resource_dir 只读访问，不复制到 app_data_dir
            // 这样软件更新时数据库也会更新
            let builtin_path = builtin_db_path(&app_handle)?;
            let db_path = resolve_db_path(&config, &builtin_path);
            let conn = match open_connection(&db_path) {
                Ok(c) => Some(c),
                Err(e) => {
                    eprintln!("Database connection failed: {}", e);
                    None
                }
            };

            let state = AppState {
                conn: std::sync::Mutex::new(conn),
                config: std::sync::Mutex::new(config),
            };

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            transform,
            get_database_info,
            get_config,
            set_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    if let Ok(mut guard) = state.conn.lock() {
                        *guard = None;
                    }
                }
            }
        });
}
