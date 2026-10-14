use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("No se pudo leer o guardar la configuración: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuración TOML inválida: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("No se pudo codificar la configuración: {0}")]
    Encode(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub commercial_name: String,
    pub store_logo_text: String,
    pub store_address: String,
    pub store_phone: String,
    pub instance_id: String,
    pub currency: String,
    pub database_file: String,
    pub printer: PrinterConfig,
    pub connection: ConnectionConfig,
    pub sync_interval_seconds: u64,
    pub interface: InterfaceConfig,
    pub update_mode: UpdateMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            commercial_name: "MiniPOS Demo".into(),
            store_logo_text: "[ MINIPOS ]".into(),
            store_address: String::new(),
            store_phone: String::new(),
            instance_id: "CAJA01".into(),
            currency: "PEN".into(),
            database_file: "minipos.sqlite3".into(),
            printer: PrinterConfig::default(),
            connection: ConnectionConfig::default(),
            sync_interval_seconds: 60,
            interface: InterfaceConfig::default(),
            update_mode: UpdateMode::Manual,
        }
    }
}

impl AppConfig {
    pub fn load_or_create(path: impl AsRef<Path>, instance_id: &str) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let mut config = if path.exists() {
            toml::from_str(&fs::read_to_string(path)?)?
        } else {
            Self::default()
        };
        config.instance_id = instance_id.to_owned();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, toml::to_string_pretty(&config)?)?;
        }
        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrinterConfig {
    pub kind: PrinterKind,
    pub name: Option<String>,
}
impl Default for PrinterConfig {
    fn default() -> Self {
        Self {
            kind: PrinterKind::Virtual,
            name: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrinterKind {
    #[default]
    Virtual,
    WindowsQueue,
    EscPos,
    Zpl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionConfig {
    pub kind: ConnectionKind,
    pub server_address: Option<String>,
}
impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            kind: ConnectionKind::Standalone,
            server_address: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    #[default]
    Standalone,
    LocalService,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceConfig {
    pub columns: u16,
    pub rows: u16,
    pub font_size: u8,
    pub header_color: String,
    pub text_color: String,
    pub background_color: String,
}
impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 25,
            font_size: 16,
            header_color: "blue".into(),
            text_color: "green".into(),
            background_color: "black".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    #[default]
    Manual,
    NotifyOnly,
    Disabled,
}
