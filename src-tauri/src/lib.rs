use std::process::Command;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================
// FUENTES — gestión de archivos de fuentes y configuración en INI  
// ============================================================

fn install_fonts_if_needed(game_root: &str) -> Result<(), String> {
    let dest_dir = PathBuf::from(game_root).join("UserData").join("fonts");
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("No se pudo crear UserData/fonts: {}", e))?;

    let fonts = [
        "arialuni_sdf_u2019",
        "arabic_font",
        "devanagari_font",
        "georgian_font",
        "thai_font",
    ];

    for font in &fonts {
        let dest = dest_dir.join(font);
        if dest.exists() { continue; } // ya instalada, saltar

        // resource_path resuelve la ruta dentro del bundle de Tauri
        let src = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent().unwrap()
            .join("fonts")
            .join(font);

        if src.exists() {
            fs::copy(&src, &dest)
                .map_err(|e| format!("Error al copiar {}: {}", font, e))?;
        }
    }

    Ok(())
}

// ============================================================
// ESTRUCTURAS
// ============================================================

#[derive(Debug, Serialize, Deserialize)]
struct SetupConfig {
    #[serde(rename = "Size")]
    size: String,
    #[serde(rename = "Width")]
    width: i32,
    #[serde(rename = "Height")]
    height: i32,
    #[serde(rename = "Quality")]
    quality: i32,
    #[serde(rename = "FullScreen")]
    full_screen: bool,
    #[serde(rename = "Display")]
    display: i32,
    #[serde(rename = "Language")]
    language: i32,
}

// Preferencias personalizadas del launcher — guardadas en UserData/KoikatsuSunshineLauncher/settings.json
#[derive(Debug, Serialize, Deserialize)]
struct LauncherSettings {
    #[serde(default)]
    background: String,
    #[serde(default)]
    logo: String,
    #[serde(default)]
    hide_logo: bool,
}


// ============================================================
// HELPERS
// ============================================================

/// Devuelve (y crea si hace falta) la carpeta de datos del launcher.
fn launcher_user_data() -> Result<PathBuf, String> {
    let root = game_root()?;
    let dir = PathBuf::from(&root)
        .join("UserData")
        .join("KoikatsuSunshineLauncher");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("No se pudo crear la carpeta del launcher: {}", e))?;
    Ok(dir)
}

fn settings_path(root: &Path) -> PathBuf {
    root.join("settings.json")
}

fn load_settings(root: &Path) -> Option<LauncherSettings> {
    let data = fs::read_to_string(settings_path(root)).ok()?;
    serde_json::from_str::<LauncherSettings>(&data).ok()
}

/// Convierte una imagen a data URL. Soporta PNG, JPG, JPEG, WEBP y BMP.
fn image_to_data_url(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("No se pudo leer la imagen: {}", e))?;
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp"         => "image/webp",
        "bmp"          => "image/bmp",
        _              => "image/png",
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    Ok(format!("data:{};base64,{}", mime, STANDARD.encode(&bytes)))
}

/// Verifica que KoikatsuSunshine.exe exista junto al launcher y devuelve la ruta raíz.
fn game_root() -> Result<String, String> {
    let launcher_path = std::env::current_exe()
        .map_err(|e| format!("No se pudo obtener la ruta del launcher: {}", e))?;
    let launcher_dir = launcher_path
        .parent()
        .ok_or("No se pudo determinar la carpeta del launcher")?;

    let game_exe = launcher_dir.join("KoikatsuSunshine.exe");
    if game_exe.exists() {
        Ok(launcher_dir
            .to_str()
            .ok_or("La ruta contiene caracteres no válidos")?
            .to_string())
    } else {
        Err(format!(
            "No se encontró KoikatsuSunshine.exe en la carpeta del launcher.\nColocá el launcher dentro de la carpeta del juego.\nRuta actual: {}",
            launcher_dir.display()
        ))
    }
}


// ============================================================
// COMANDOS — Configuración del juego (setup.xml / INI)
// ============================================================

/// Lee el idioma activo desde la sección [General] de AutoTranslatorConfig.ini.
#[tauri::command]
fn get_current_language_from_ini() -> Result<String, String> {
    let root = game_root()?;
    let ini_path = PathBuf::from(&root).join("BepInEx/config/AutoTranslatorConfig.ini");
    if !ini_path.exists() {
        return Ok(String::new());
    }

    let content = fs::read_to_string(&ini_path)
        .map_err(|e| format!("No se pudo leer AutoTranslatorConfig.ini: {}", e))?;

    let mut in_general = false;
    for line in content.lines() {
        if line.trim() == "[General]" {
            in_general = true;
        } else if line.starts_with('[') && line.ends_with(']') {
            in_general = false;
        } else if in_general && line.trim_start().starts_with("Language=") {
            return Ok(line.trim_start().strip_prefix("Language=").unwrap_or("").to_string());
        }
    }
    Ok(String::new())
}

/// Lee setup.xml (codificado en UTF-16) y devuelve la configuración actual del juego.
#[tauri::command]
fn get_setup_xml() -> Result<SetupConfig, String> {
    let root = game_root()?;
    let setup_path = PathBuf::from(&root).join("UserData/setup.xml");
    let bytes = fs::read(&setup_path)
        .map_err(|e| format!("No se pudo leer setup.xml: {}", e))?;

    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let content = if u16s.first() == Some(&0xFEFF) {
        String::from_utf16(&u16s[1..])
    } else {
        String::from_utf16(&u16s)
    }
    .map_err(|e| format!("Error al decodificar UTF-16: {}", e))?;

    let content_fixed = content.replace("encoding=\"utf-16\"", "encoding=\"utf-8\"");

    serde_xml_rs::from_str(&content_fixed)
        .map_err(|e| format!("Error al parsear setup.xml: {}", e))
}

/// Guarda la configuración en setup.xml (UTF-16) y actualiza el idioma en AutoTranslatorConfig.ini.
#[tauri::command]
fn save_setup_xml(config: SetupConfig, language_code: String) -> Result<String, String> {
    let root = game_root()?;
    let setup_path = PathBuf::from(&root).join("UserData/setup.xml");

    let body = format!(
        "<Setting>\r\n  <Size>{}</Size>\r\n  <Width>{}</Width>\r\n  <Height>{}</Height>\r\n  <Quality>{}</Quality>\r\n  <FullScreen>{}</FullScreen>\r\n  <Display>{}</Display>\r\n  <Language>{}</Language>\r\n</Setting>",
        config.size, config.width, config.height, config.quality,
        config.full_screen, config.display, config.language,
    );
    let full_xml = format!("<?xml version=\"1.0\" encoding=\"utf-16\"?>\r\n{}", body);

    let mut encoded: Vec<u16> = vec![0xFEFF];
    encoded.extend(full_xml.encode_utf16());
    let bytes: Vec<u8> = encoded.iter().flat_map(|&c| c.to_le_bytes()).collect();
    fs::write(&setup_path, bytes)
        .map_err(|e| format!("Error al guardar setup.xml: {}", e))?;

    if !language_code.is_empty() {
        update_auto_translator_ini(&root, &language_code)?;
    }

    Ok("Configuración guardada correctamente.".to_string())
}

/// Actualiza el valor Language= en la sección [General] de AutoTranslatorConfig.ini.
fn update_auto_translator_ini(game_root: &str, language_code: &str) -> Result<(), String> {
    let ini_path = PathBuf::from(game_root).join("BepInEx").join("config").join("AutoTranslatorConfig.ini");
    if !ini_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&ini_path)
        .map_err(|e| format!("No se pudo leer AutoTranslatorConfig.ini: {}", e))?;
    let mut new_content = String::new();
    let mut in_general = false;

    for line in content.lines() {
        if line.trim() == "[General]" {
            in_general = true;
            new_content.push_str(line);
            new_content.push('\n');
        } else if line.starts_with('[') && line.ends_with(']') {
            in_general = false;
            new_content.push_str(line);
            new_content.push('\n');
        } else if in_general && line.trim_start().starts_with("Language=") {
            new_content.push_str(&format!("Language={}", language_code));
            new_content.push('\n');
        } else {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }

    fs::write(&ini_path, new_content)
        .map_err(|e| format!("Error al guardar AutoTranslatorConfig.ini: {}", e))
}

// ============================================================
// COMANDOS — Fuentes (TextMeshPro)
// ============================================================

// Tabla de idiomas que requieren fuente especial; el resto usa arialuni_sdf_u2019.
const SPECIAL_FONTS: &[(&str, &str)] = &[
    ("th", "thai_font"),
    ("hi", "devanagari_font"),
    ("ka", "georgian_font"),
    ("ar", "arabic_font"),
    ("fa", "arabic_font"),
    ("ur", "arabic_font"),
];

fn resolve_font(language_code: &str) -> String {
    SPECIAL_FONTS
        .iter()
        .find(|(lang, _)| *lang == language_code)
        .map(|(_, font)| format!("UserData\\fonts/{}", font))
        .unwrap_or_else(|| "UserData\\fonts/arialuni_sdf_u2019".to_string())
}

/// Aplica la fuente TMP correspondiente al idioma dado.
#[tauri::command]
fn set_fallback_font(language_code: String) -> Result<String, String> {
    let root = game_root()?;
    let font_path = resolve_font(&language_code);
    update_ini_font_lines(&root, &font_path)?;
    Ok(format!("Fuente configurada a '{}'", font_path))
}

/// Igual que set_fallback_font; se usa al inicializar el launcher.
#[tauri::command]
fn initialize_fonts(language_code: String) -> Result<String, String> {
    let root = game_root()?;
    let font_path = resolve_font(&language_code);
    update_ini_font_lines(&root, &font_path)?;
    Ok(format!("Fuentes configuradas a '{}'", font_path))
}

/// Actualiza OverrideFontTextMeshPro y FallbackFontTextMeshPro en la sección [Behaviour].
fn update_ini_font_lines(game_root: &str, font_bundle: &str) -> Result<(), String> {
    let ini_path = PathBuf::from(game_root).join("BepInEx").join("config").join("AutoTranslatorConfig.ini");
    if !ini_path.exists() {
        return Err("AutoTranslatorConfig.ini no encontrado".to_string());
    }

    let content = fs::read_to_string(&ini_path)
        .map_err(|e| format!("No se pudo leer el INI: {}", e))?;
    let mut new_content = String::new();
    let mut in_behaviour = false;
    let mut override_written = false;
    let mut fallback_written = false;

    for line in content.lines() {
        if line.trim() == "[Behaviour]" {
            in_behaviour = true;
            new_content.push_str(line);
            new_content.push('\n');
        } else if line.starts_with('[') && line.ends_with(']') {
            // Al salir de [Behaviour], insertar claves faltantes si no fueron escritas
            if in_behaviour {
                if !override_written {
                    new_content.push_str(&format!("OverrideFontTextMeshPro={}\n", font_bundle));
                }
                if !fallback_written {
                    new_content.push_str(&format!("FallbackFontTextMeshPro={}\n", font_bundle));
                }
            }
            in_behaviour = false;
            new_content.push_str(line);
            new_content.push('\n');
        } else if in_behaviour && line.trim_start().starts_with("OverrideFontTextMeshPro=") {
            new_content.push_str(&format!("OverrideFontTextMeshPro={}\n", font_bundle));
            override_written = true;
        } else if in_behaviour && line.trim_start().starts_with("FallbackFontTextMeshPro=") {
            new_content.push_str(&format!("FallbackFontTextMeshPro={}\n", font_bundle));
            fallback_written = true;
        } else {
            new_content.push_str(line);
            new_content.push('\n');
        }
    }
    // Caso: [Behaviour] era la última sección del archivo
    if in_behaviour {
        if !override_written {
            new_content.push_str(&format!("OverrideFontTextMeshPro={}\n", font_bundle));
        }
        if !fallback_written {
            new_content.push_str(&format!("FallbackFontTextMeshPro={}\n", font_bundle));
        }
    }

    fs::write(&ini_path, new_content)
        .map_err(|e| format!("Error al guardar el INI: {}", e))
}


// ============================================================
// COMANDOS — Plugins (activar / desactivar via .dll ↔ .dl_)
// ============================================================

/// Busca el archivo de un plugin en BepInEx/plugins (raíz + un nivel de subcarpetas).
fn find_plugin_file(root: &PathBuf, base_name: &str) -> Option<PathBuf> {
    let plugins_dir = root.join("BepInEx/plugins");
    if !plugins_dir.exists() {
        return None;
    }
    let dll_name = format!("{}.dll", base_name);
    let dl_name  = format!("{}.dl_", base_name);

    for ext in &[&dll_name, &dl_name] {
        let path = plugins_dir.join(ext);
        if path.exists() { return Some(path); }
    }
    if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
        for entry in entries.flatten() {
            let sub_dir = entry.path();
            if sub_dir.is_dir() {
                for ext in &[&dll_name, &dl_name] {
                    let path = sub_dir.join(ext);
                    if path.exists() { return Some(path); }
                }
            }
        }
    }
    None
}

/// Devuelve el estado (activo/inactivo) de cada plugin gestionado.
#[tauri::command]
fn get_plugin_states() -> Result<std::collections::HashMap<String, bool>, String> {
    let root = PathBuf::from(game_root()?);
    let mut states = std::collections::HashMap::new();

    let plugins = [
        ("RimRemover", "KKS_RimRemover"),
        ("AutoSave",   "KKS_Autosave"),
        ("Stiletto",   "KKS_Stiletto"),
    ];

    for (id, base_name) in &plugins {
        let is_active = find_plugin_file(&root, base_name)
            .as_ref()
            .map(|p| p.extension().unwrap_or_default() == "dll")
            .unwrap_or(false);
        states.insert(id.to_string(), is_active);
    }

    Ok(states)
}

/// Activa (.dll) o desactiva (.dl_) un plugin renombrando su archivo.
#[tauri::command]
fn toggle_plugin(plugin_id: String, enable: bool) -> Result<String, String> {
    let root = PathBuf::from(game_root()?);
    let base_name = match plugin_id.as_str() {
        "RimRemover" => "KKS_RimRemover",
        "AutoSave"   => "KKS_Autosave",
        "Stiletto"   => "KKS_Stiletto",
        _            => return Err("Plugin desconocido".to_string()),
    };

    let current_file = find_plugin_file(&root, base_name)
        .ok_or_else(|| format!("No se encontró el archivo de {}", plugin_id))?;

    let is_active = current_file.extension().unwrap_or_default() == "dll";
    if enable  &&  is_active { return Ok(format!("{} ya estaba activado",   plugin_id)); }
    if !enable && !is_active { return Ok(format!("{} ya estaba desactivado", plugin_id)); }

    let new_path = current_file.with_extension(if enable { "dll" } else { "dl_" });
    fs::rename(&current_file, &new_path)
        .map_err(|e| format!("Error al {} {}: {}", if enable { "activar" } else { "desactivar" }, plugin_id, e))?;

    Ok(format!("{} {}", plugin_id, if enable { "activado" } else { "desactivado" }))
}


// ============================================================
// COMANDOS — Consola de BepInEx
// ============================================================

/// Lee el valor de Enabled en [Logging.Console] de BepInEx.cfg.
#[tauri::command]
fn get_console_enabled() -> Result<bool, String> {
    let root = game_root()?;
    let config_path = PathBuf::from(&root).join("BepInEx/config/BepInEx.cfg");
    if !config_path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("No se pudo leer BepInEx.cfg: {}", e))?;
    let mut in_console_section = false;
    for line in content.lines() {
        if line.trim().eq_ignore_ascii_case("[Logging.Console]") {
            in_console_section = true;
        } else if line.starts_with('[') && line.ends_with(']') {
            in_console_section = false;
        } else if in_console_section && line.trim_start().starts_with("Enabled") {
            return Ok(line.contains("true"));
        }
    }
    Ok(false)
}

/// Escribe el valor de Enabled en [Logging.Console]; crea la sección si no existe.
#[tauri::command]
fn set_console_enabled(enable: bool) -> Result<String, String> {
    let root = game_root()?;
    let config_path = PathBuf::from(&root).join("BepInEx/config/BepInEx.cfg");
    let mut lines: Vec<String> = if config_path.exists() {
        fs::read_to_string(&config_path)
            .map_err(|e| format!("No se pudo leer BepInEx.cfg: {}", e))?
            .lines()
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };

    let val = if enable { "true" } else { "false" };
    let header_idx = lines.iter().position(|l| l.trim().eq_ignore_ascii_case("[Logging.Console]"));

    if let Some(idx) = header_idx {
        let mut found = false;
        for i in (idx + 1)..lines.len() {
            if lines[i].starts_with('[') { break; }
            if lines[i].trim_start().starts_with("Enabled") {
                lines[i] = format!("Enabled = {}", val);
                found = true;
                break;
            }
        }
        if !found {
            lines.insert(idx + 1, format!("Enabled = {}", val));
        }
    } else {
        lines.push(String::new());
        lines.push("[Logging.Console]".to_string());
        lines.push(format!("Enabled = {}", val));
    }

    fs::write(&config_path, lines.join("\n"))
        .map_err(|e| format!("Error al guardar BepInEx.cfg: {}", e))?;
    Ok("Configuración de consola actualizada.".to_string())
}


// ============================================================
// COMANDOS — Lanzar ejecutables
// ============================================================

#[tauri::command]
fn launch_game() -> Result<String, String> {
    let root = game_root()?;
    Command::new(PathBuf::from(&root).join("KoikatsuSunshine.exe"))
        .current_dir(&root)
        .spawn()
        .map_err(|e| format!("Error al ejecutar el juego: {}", e))?;
    Ok("Juego iniciado correctamente.".to_string())
}

#[tauri::command]
fn launch_studio() -> Result<String, String> {
    let root = game_root()?;
    Command::new(PathBuf::from(&root).join("CharaStudio.exe"))
        .current_dir(&root)
        .spawn()
        .map_err(|e| format!("Error al ejecutar el Studio: {}", e))?;
    Ok("Studio iniciado correctamente.".to_string())
}

/// Abre una carpeta (ruta relativa a la raíz del juego) en el explorador de Windows.
#[tauri::command]
fn open_folder(relative_path: String) -> Result<String, String> {
    let root = game_root()?;
    let full_path = PathBuf::from(&root).join(&relative_path);
    if !full_path.exists() {
        return Err(format!("La carpeta no existe: {}", full_path.display()));
    }
    Command::new("explorer")
        .arg(&full_path)
        .spawn()
        .map_err(|e| format!("Error al abrir la carpeta: {}", e))?;
    Ok(format!("Carpeta abierta: {}", full_path.display()))
}


// ============================================================
// COMANDOS — Apariencia personalizada (fondo y logo)
// ============================================================

// Helper compartido: abre un file picker de imágenes y devuelve la ruta elegida.
// Devuelve Ok(None) si el usuario cancela.
async fn pick_image(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
        .pick_file(move |f| { let _ = tx.send(f); });

    let file = rx.await.map_err(|_| "Dialog error".to_string())?;
    Ok(file.map(|p| PathBuf::from(p.to_string())))
}

// Helper compartido: valida extensión, elimina archivos previos del mismo nombre base y copia la imagen.
fn save_image_to_launcher(dir: &Path, base: &str, src: &Path) -> Result<(PathBuf, String), String> {
    let ext = src.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    if !["png", "jpg", "jpeg", "webp", "bmp"].contains(&ext.as_str()) {
        return Err("Formato no soportado".into());
    }
    for old_ext in &["png", "jpg", "jpeg", "webp", "bmp"] {
        let _ = fs::remove_file(dir.join(format!("{}.{}", base, old_ext)));
    }
    let dest = dir.join(format!("{}.{}", base, ext));
    fs::copy(src, &dest).map_err(|e| format!("Error al copiar imagen: {}", e))?;
    Ok((dest, format!("{}.{}", base, ext)))
}

/// Abre un diálogo, copia la imagen elegida como fondo y devuelve su data URL.
#[tauri::command]
async fn pick_and_set_background(app: tauri::AppHandle) -> Result<String, String> {
    let src = match pick_image(&app).await? {
        Some(p) => p,
        None    => return Ok(String::new()),
    };

    let dir = launcher_user_data()?;
    let (dest, filename) = save_image_to_launcher(&dir, "background", &src)?;

    let mut settings = load_settings(&dir).unwrap_or_else(|| LauncherSettings {
        background: String::new(), logo: String::new(), hide_logo: false,
    });
    settings.background = filename;
    fs::write(
        settings_path(&dir),
        serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Error al serializar settings: {}", e))?,
    ).map_err(|e| format!("No se pudo guardar settings: {}", e))?;

    image_to_data_url(&dest)
}

/// Devuelve el fondo personalizado como data URL, o None si no hay ninguno configurado.
#[tauri::command]
fn get_custom_background() -> Option<String> {
    let dir = launcher_user_data().ok()?;
    let settings = load_settings(&dir)?;
    let candidate = dir.join(&settings.background);
    if candidate.exists() { image_to_data_url(&candidate).ok() } else { None }
}

/// Abre un diálogo, copia la imagen elegida como logo y devuelve su data URL.
#[tauri::command]
async fn pick_and_set_logo(app: tauri::AppHandle) -> Result<String, String> {
    let src = match pick_image(&app).await? {
        Some(p) => p,
        None    => return Ok(String::new()),
    };

    let dir = launcher_user_data()?;
    let (dest, filename) = save_image_to_launcher(&dir, "logo", &src)?;

    let mut settings = load_settings(&dir).unwrap_or_else(|| LauncherSettings {
        background: String::new(), logo: String::new(), hide_logo: false,
    });
    settings.logo = filename;
    fs::write(
        settings_path(&dir),
        serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Error al serializar settings: {}", e))?,
    ).map_err(|e| format!("No se pudo guardar settings: {}", e))?;

    image_to_data_url(&dest)
}

/// Devuelve el logo personalizado como data URL, o None si no hay ninguno configurado.
#[tauri::command]
fn get_custom_logo() -> Option<String> {
    let dir = launcher_user_data().ok()?;
    let settings = load_settings(&dir)?;
    if settings.logo.is_empty() { return None; }
    let candidate = dir.join(&settings.logo);
    if candidate.exists() { image_to_data_url(&candidate).ok() } else { None }
}

#[tauri::command]
fn set_hide_logo(hide: bool) -> Result<(), String> {
    let dir = launcher_user_data()?;
    let mut settings = load_settings(&dir).unwrap_or_else(|| LauncherSettings {
        background: String::new(), logo: String::new(), hide_logo: false,
    });
    settings.hide_logo = hide;
    fs::write(
        settings_path(&dir),
        serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("Error al serializar settings: {}", e))?,
    ).map_err(|e| format!("No se pudo guardar settings: {}", e))
}

#[tauri::command]
fn get_hide_logo() -> bool {
    launcher_user_data()
        .ok()
        .and_then(|dir| load_settings(&dir))
        .map(|s| s.hide_logo)
        .unwrap_or(false)
}


// ============================================================
// ENTRY POINT
// ============================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|_app| {
            if let Ok(root) = game_root() {
                let _ = install_fonts_if_needed(&root);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_current_language_from_ini,
            launch_game,
            launch_studio,
            open_folder,
            get_setup_xml,
            save_setup_xml,
            set_fallback_font,
            initialize_fonts,
            get_plugin_states,
            toggle_plugin,
            get_console_enabled,
            set_console_enabled,
            pick_and_set_background,
            get_custom_background,
            pick_and_set_logo,
            get_custom_logo,
            set_hide_logo,
            get_hide_logo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Fin del lib.rs
// deadshark