use serde::Serialize;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::env;

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

use std::process::Command;
use std::process::Child;
use std::sync::Mutex;
use std::os::windows::process::CommandExt;
use std::fs::File;
use tauri::Manager;

#[derive(Serialize)]
pub struct ZapretResult {
    message: String,
    success: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Config {
    last_version: String,
    last_preset: String,
    game_filter: bool,
    auto_start: bool,
}

struct AppState {
    config: Mutex<Config>,
    child_process: Mutex<Option<Child>>,
}

#[tauri::command]
fn get_presets(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    let mut presets = Vec::new();
    let config = state.config.lock().unwrap();
    let folder_path = &config.last_version;
    let dir_path = Path::new("downloads/").join(folder_path);

    if dir_path.exists() && dir_path.is_dir() {
        let entries = fs::read_dir(dir_path).map_err(|e| e.to_string())?;

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let file_name = entry.file_name().to_string_lossy().into_owned();

            if file_name.starts_with("general") && file_name.ends_with(".bat") {
                presets.push(file_name);
            }
        }
    }
    
    presets.sort();
    Ok(presets)
}

#[tauri::command]
fn read_sites_list() -> Result<String, String> {
    let path = Path::new("resources/list-general.txt");
    
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, "").map_err(|e| e.to_string())?;
    }

    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_sites_list(content: String, state: tauri::State<AppState>) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    let path = Path::new("resources/list-general.txt");
    let download_dir = Path::new("downloads/").join(&config.last_version).join("lists/list-general.txt");
    println!("Saving sites list to: {}", download_dir.display());
    match fs::write(path, &content) {
        Ok(_) => {
            match fs::write(&download_dir, &content) {
                Ok(_) => println!("Successfully saved sites list to download directory."),
                Err(e) => eprintln!("Failed to save sites list to download directory: {}", e),
            }
        },
        Err(e) => eprintln!("Failed to save sites list to main directory: {}", e),
    }
    Ok(())
}

fn update_autostart(enabled: bool) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        let app_path = env::current_exe().map_err(|e| e.to_string())?;
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
        let (key, _) = hkcu.create_subkey(path).map_err(|e| e.to_string())?;

        if enabled {
            key.set_value("ZapretUIApp", &app_path.to_str().unwrap()).map_err(|e| e.to_string())?;
        } else {
            let _ = key.delete_value("ZapretUIApp");
        }
    }
    Ok(())
}

fn update_zapret_config(target_dir: &String, filename: String, status: bool) {
    let target_path = target_dir.to_owned() + &filename + ".enabled";
    
    if status {
        let file = File::create(&target_path);
        fs::write(target_path, "ENABLED");
    } else {
        if Path::new(&target_path).exists() {
            fs::remove_file(target_path).ok();
        }
    } 
}

fn save_config_internal(config: &Config) -> Result<(), String> {
    let base_path = "downloads/".to_owned() + &config.last_version + "/utils/";

    update_zapret_config(&base_path, "game_filter".to_string(), config.game_filter);
    update_autostart(config.auto_start).expect("Failed to update autostart setting");

    let data = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Не удалось сериализовать конфиг: {}", e))?;
    fs::write("resources/config.json", data)
        .map_err(|e| format!("Не удалось сохранить конфиг: {}", e))
}

#[tauri::command]
fn save_last_preset(preset: String, state: tauri::State<AppState>) {
    println!("Saving last preset: {}", preset);
    let mut config = state.config.lock().unwrap();
    config.last_preset = preset;
}

#[tauri::command]
fn save_config(config: Config, state: tauri::State<AppState>) {
    println!("Saving config: {:?}", config);
    let mut internal_config = state.config.lock().unwrap();
    internal_config.last_preset = config.last_preset.clone();

    if internal_config.auto_start != config.auto_start {
        update_autostart(config.auto_start).expect("Failed to update autostart setting");
        internal_config.auto_start = config.auto_start;
    }
    if internal_config.game_filter != config.game_filter {
        let base_path = "downloads/".to_owned() + &internal_config.last_version + "/utils/";
        update_zapret_config(&base_path, "game_filter".to_string(), config.game_filter);
        internal_config.game_filter = config.game_filter;
    }
    println!("Config saved successfully: {:?}", internal_config);
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> Result<Config, String> {
    let config = state.config.lock().unwrap();
    Ok(config.clone())
}

#[tauri::command]
#[allow(non_snake_case)]
fn enable_zapret(selectedPreset: String, state: tauri::State<AppState>) -> Result<ZapretResult, String> {
    let mut lock = state.child_process.lock().unwrap();
    if let Some(child) = lock.as_mut() {
        match child.kill() {
            Ok(_) => println!("Previous zapret process killed successfully."),
            Err(e) => eprintln!("Failed to kill previous zapret process: {}", e),
        }
    }

    let config = state.config.lock().unwrap();
    let base_path = Path::new("downloads/").join(&config.last_version);
    let preset_path = base_path.join(&selectedPreset);
    println!("Starting zapret process with preset: {}", preset_path.display());

    let cmd = Command::new("cmd")
        .args(["/C", &selectedPreset])
        .current_dir(&base_path)
        .env("DIR", &base_path)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| e.to_string())?;
    
    *lock = Some(cmd);
    Ok(ZapretResult {
        message: "Процесс запущен успешно".to_string(),
        success: true,
    })
}

#[tauri::command]
fn disable_zapret(state: tauri::State<AppState>) -> Result<ZapretResult, String> {
    Command::new("taskkill")
        .args(["/F", "/IM", "winws.exe", "/T"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut lock = state.child_process.lock().unwrap();
    if let Some(child) = lock.as_mut() {
        match child.kill() {
            Ok(_) => {
                *lock = None;
                println!("Zapret process killed successfully.");
                Ok(ZapretResult {
                    message: "Процесс остановлен успешно".to_string(),
                    success: true,
                })
            },
            Err(e) => {
                eprintln!("Failed to kill zapret process: {}", e);
                Err(format!("Не удалось остановить процесс: {}", e))
            },
        }
    } else {
        Ok(ZapretResult {
            message: "Процесс не запущен".to_string(),
            success: false,
        })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_data = fs::read_to_string("resources/config.json")
        .expect("Не удалось найти resources/config.json");
    
    let config: Config = serde_json::from_str(&config_data)
        .expect("Ошибка в формате config.json");

    let app = tauri::Builder::default()
        .manage(AppState { config: Mutex::new(config), child_process: Mutex::new(None) })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![enable_zapret, disable_zapret, read_sites_list, save_sites_list, get_presets, save_config, get_config, save_last_preset]) 
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|_app_handle, _event| {
        match _event {
            tauri::RunEvent::ExitRequested { .. } => {
                save_config_internal(&_app_handle.state::<AppState>().config.lock().unwrap()).expect("Failed to save config on exit");

                Command::new("taskkill")
                    .args(["/F", "/IM", "winws.exe", "/T"])
                    .creation_flags(0x08000000) // CREATE_NO_WINDOW
                    .output();
            }
            _ => {}
        }
    });
}