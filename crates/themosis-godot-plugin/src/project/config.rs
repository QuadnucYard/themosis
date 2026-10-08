//! Resolving, loading, and saving the project's profile configuration.
//!
//! Validation, normalization, migration, and serialization live in
//! [`themosis_godot::profiles`]; this module adds engine-side file and
//! project-setting access. Every result is a Rust type: no Godot dictionary
//! crosses a module boundary.

use godot::{
    classes::{FileAccess, ProjectSettings, file_access::ModeFlags},
    obj::{EngineEnum, Singleton},
    prelude::*,
    register::info::PropertyHint,
};
use themosis_godot::profiles::{self, ProfileConfig};

use crate::native::materialize::save_profile_config;

/// Loaded configuration and where it came from.
pub(crate) struct LoadedConfig {
    /// `res://` path the configuration was resolved from.
    pub(crate) path: String,
    /// The normalized configuration, or an empty one when nothing is configured.
    pub(crate) config: ProfileConfig,
    /// Whether the configuration defines at least one profile.
    pub(crate) configured: bool,
}

/// Failure to resolve, read, or parse the configuration.
pub(crate) struct ConfigFailure {
    /// Human-readable failure message.
    pub(crate) message: String,
}

/// Registers the `themosis/profile_config` project setting.
pub(crate) fn ensure_project_setting() {
    let mut settings = ProjectSettings::singleton();
    let default = profiles::DEFAULT_CONFIG_PATH.to_variant();
    if !settings.has_setting(profiles::CONFIG_SETTING) {
        settings.set_setting(profiles::CONFIG_SETTING, &default);
    }
    settings.set_initial_value(profiles::CONFIG_SETTING, &default);
    let mut info = VarDictionary::new();
    info.set("name", profiles::CONFIG_SETTING);
    info.set("type", VariantType::STRING.ord() as i64);
    info.set("hint", &PropertyHint::FILE.to_variant());
    info.set("hint_string", "*.json");
    settings.add_property_info(&info);
}

/// Resolves, loads, and migrates the profile configuration.
pub(crate) fn load_config(migrate_legacy: bool) -> Result<LoadedConfig, ConfigFailure> {
    ensure_project_setting();
    let path = resolve_path()?;
    if FileAccess::file_exists(&path) {
        let Some(text) = read_text(&path) else {
            return Err(ConfigFailure {
                message: format!("cannot open profile configuration '{path}'"),
            });
        };
        return match profiles::parse_config(&text) {
            Ok(config) => Ok(LoadedConfig {
                configured: config.is_configured(),
                path,
                config,
            }),
            Err(error) => Err(ConfigFailure {
                message: error.load_message(&path),
            }),
        };
    }
    if migrate_legacy {
        let migration = profiles::migrate_legacy_values(
            &legacy_setting(profiles::LEGACY_SOURCE_SETTING),
            &legacy_setting(profiles::LEGACY_OUTPUT_SETTING),
            legacy_flag(profiles::LEGACY_AUTO_SETTING),
            legacy_source_exists(),
        );
        if migration.migrated {
            // A legacy output can no longer validate; persist only a valid
            // configuration so a rejected migration leaves no file behind.
            let config = profiles::validate_profile_config(&migration.config).map_err(|error| {
                ConfigFailure {
                    message: format!("invalid migrated profile configuration '{path}': {error}"),
                }
            })?;
            save_config(&config)?;
            return Ok(LoadedConfig {
                configured: config.is_configured(),
                path,
                config,
            });
        }
    }
    Ok(LoadedConfig {
        configured: false,
        path,
        config: profiles::empty_config(),
    })
}

/// Validates and atomically saves a configuration document.
pub(crate) fn save_config(config: &ProfileConfig) -> Result<String, ConfigFailure> {
    ensure_project_setting();
    let path = resolve_path()?;
    let text = config.to_json_string();
    save_profile_config(&text, &path).map_err(|diagnostic| ConfigFailure {
        message: diagnostic.render(),
    })?;
    Ok(path)
}

/// Resolves and validates the configured path.
fn resolve_path() -> Result<String, ConfigFailure> {
    let path = configured_path();
    profiles::validate_config_path(&path).map_err(|message| ConfigFailure {
        message: format!("invalid profile configuration path '{path}': {message}"),
    })
}

/// Returns the configured path, falling back to the default.
fn configured_path() -> String {
    let path = ProjectSettings::singleton()
        .get_setting(profiles::CONFIG_SETTING)
        .try_to::<GString>()
        .map(|path| path.to_string())
        .unwrap_or_default();
    if path.is_empty() {
        profiles::DEFAULT_CONFIG_PATH.to_owned()
    } else {
        path
    }
}

/// Reads an existing file's text, or `None` when it cannot be opened.
fn read_text(path: &str) -> Option<String> {
    let file = FileAccess::open(path, ModeFlags::READ)?;
    Some(file.get_as_text().to_string())
}

/// Returns a string setting, or the empty string when unset.
fn legacy_setting(setting: &str) -> String {
    ProjectSettings::singleton()
        .get_setting(setting)
        .try_to::<GString>()
        .map(|value| value.to_string())
        .unwrap_or_default()
}

/// Returns the legacy auto-refresh flag, defaulting to enabled.
fn legacy_flag(setting: &str) -> bool {
    ProjectSettings::singleton()
        .get_setting(setting)
        .try_to::<bool>()
        .unwrap_or(true)
}

/// Returns whether the legacy source setting names an existing file.
fn legacy_source_exists() -> bool {
    let source = legacy_setting(profiles::LEGACY_SOURCE_SETTING);
    !source.is_empty() && FileAccess::file_exists(&source)
}
