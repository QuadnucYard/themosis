//! Versioned profile configuration for the Themosis Godot addon.
//!
//! The model is pure JSON. The Godot plugin maps engine dictionaries to and
//! from [`serde_json::Value`], so validation, normalization, legacy migration,
//! and serialization have a single implementation that unit tests exercise
//! without a running engine.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

/// Schema version accepted by [`validate_config`].
pub const VERSION: u32 = 1;

/// Project setting naming the profile configuration file.
pub const CONFIG_SETTING: &str = "themosis/profile_config";

/// Default project-relative configuration path.
pub const DEFAULT_CONFIG_PATH: &str = "res://themosis.godot.json";

/// Legacy project setting holding the single migrated source root.
pub const LEGACY_SOURCE_SETTING: &str = "themosis/theme_source";

/// Legacy project setting holding the single migrated output path.
pub const LEGACY_OUTPUT_SETTING: &str = "themosis/generated_theme";

/// Legacy project setting holding the former refresh flag.
pub const LEGACY_AUTO_SETTING: &str = "themosis/auto_refresh";

/// Preview mode that never renders a preview.
pub const PREVIEW_NONE: &str = "none";

/// Preview mode rendering in the edited scene.
pub const PREVIEW_EDITED_SCENE: &str = "edited_scene";

/// Output used when legacy settings carry no destination.
pub const LEGACY_DEFAULT_OUTPUT: &str = "res://.themosis/generated_theme.tres";

/// One materialization profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    /// Profile name; letters, digits, `_`, and `-` only.
    pub name: String,
    /// Confined `res://` theme root (`.tms` or `.kdl`).
    pub source: String,
    /// Confined `res://` `.tres` destination.
    pub output: String,
    /// Whether the editor refreshes this profile on source changes.
    pub auto_refresh: bool,
    /// Whether headless builds include this profile by default.
    pub build_on_start: bool,
    /// Preview mode: `none` or `edited_scene`.
    pub preview: String,
    /// Whether tooling builds this profile.
    pub enabled: bool,
}

/// Versioned profile configuration document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProfileConfig {
    /// Schema version; currently always [`VERSION`].
    pub version: u32,
    /// Name of the profile tooling selects by default.
    pub active_profile: String,
    /// All declared profiles.
    pub profiles: Vec<Profile>,
}

impl ProfileConfig {
    /// Serializes like Godot's `JSON.stringify(config, "  ", true)` plus a
    /// trailing newline: two-space indentation with sorted object keys.
    pub fn to_json_string(&self) -> String {
        let value = serde_json::to_value(self).expect("profile configuration serializes");
        let mut text =
            serde_json::to_string_pretty(&value).expect("profile configuration serializes");
        text.push('\n');
        text
    }

    /// Returns the profile named `name`, if any.
    pub fn find(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.name == name)
    }

    /// Returns whether the configuration defines at least one profile.
    pub fn is_configured(&self) -> bool {
        !self.profiles.is_empty()
    }
}

/// Failure while parsing a configuration document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// The document is not valid JSON.
    Syntax {
        /// One-based line where parsing failed.
        line: usize,
        /// Parser message.
        message: String,
    },
    /// The document parsed but violates the profile schema.
    Invalid(String),
}

impl ConfigError {
    /// Formats the failure exactly as the addon's configuration loader
    /// reports it, including the configuration path.
    pub fn load_message(&self, path: &str) -> String {
        match self {
            ConfigError::Syntax { line, message } => {
                format!("cannot parse profile configuration '{path}' at line {line}: {message}")
            }
            ConfigError::Invalid(error) => {
                format!("invalid profile configuration '{path}': {error}")
            }
        }
    }
}

/// Creates the first-run configuration with no profiles.
pub fn empty_config() -> ProfileConfig {
    ProfileConfig {
        version: VERSION,
        active_profile: String::new(),
        profiles: Vec::new(),
    }
}

/// Creates an enabled profile with auto-refresh and no preview.
pub fn new_profile(
    name: impl Into<String>,
    source: impl Into<String>,
    output: impl Into<String>,
) -> Profile {
    Profile {
        name: name.into(),
        source: source.into(),
        output: output.into(),
        auto_refresh: true,
        build_on_start: false,
        preview: PREVIEW_NONE.to_owned(),
        enabled: true,
    }
}

/// Parses and validates a configuration document.
pub fn parse_config(text: &str) -> Result<ProfileConfig, ConfigError> {
    let value: Value = serde_json::from_str(text).map_err(|error| ConfigError::Syntax {
        line: error.line(),
        message: error.to_string(),
    })?;
    validate_config(&value).map_err(ConfigError::Invalid)
}

/// Validates a JSON value and returns the normalized configuration.
pub fn validate_config(value: &Value) -> Result<ProfileConfig, String> {
    let Value::Object(config) = value else {
        return Err("root must be an object".to_owned());
    };
    let version_ok = matches!(
        config.get("version"),
        Some(Value::Number(number)) if number.as_f64() == Some(f64::from(VERSION))
    );
    if !version_ok {
        return Err(format!("version must be {VERSION}"));
    }
    let Some(active_profile) = config.get("active_profile").and_then(Value::as_str) else {
        return Err("active_profile must be a string".to_owned());
    };
    let Some(profiles) = config.get("profiles").and_then(Value::as_array) else {
        return Err("profiles must be an array".to_owned());
    };

    let mut normalized = Vec::with_capacity(profiles.len());
    let mut names = BTreeSet::new();
    let mut outputs = BTreeMap::new();
    for (index, value) in profiles.iter().enumerate() {
        let Value::Object(profile) = value else {
            return Err(format!("profiles[{index}] must be an object"));
        };
        let name = profile_string(profile, "name", index)?;
        let source = profile_string(profile, "source", index)?;
        let output = profile_string(profile, "output", index)?;
        let auto_refresh = profile_bool(profile, "auto_refresh", index)?;
        let build_on_start = profile_bool(profile, "build_on_start", index)?;
        let preview = profile_string(profile, "preview", index)?;
        let enabled = profile_bool(profile, "enabled", index)?;

        if !valid_profile_name(name) {
            return Err(format!(
                "profiles[{index}].name must use letters, numbers, '_' or '-'"
            ));
        }
        if !names.insert(name.to_owned()) {
            return Err(format!("duplicate profile name '{name}'"));
        }
        let source = validate_source_path(source)
            .map_err(|error| format!("profile '{name}' source {error}"))?;
        let output = validate_output_path(output)
            .map_err(|error| format!("profile '{name}' output {error}"))?;
        if let Some(first) = outputs.insert(output.clone(), name.to_owned()) {
            return Err(format!(
                "profiles '{first}' and '{name}' share output '{output}'"
            ));
        }
        if preview != PREVIEW_NONE && preview != PREVIEW_EDITED_SCENE {
            return Err(format!(
                "profile '{name}' preview must be '{PREVIEW_NONE}' or '{PREVIEW_EDITED_SCENE}'"
            ));
        }
        normalized.push(Profile {
            name: name.to_owned(),
            source,
            output,
            auto_refresh,
            build_on_start,
            preview: preview.to_owned(),
            enabled,
        });
    }

    let active = active_profile.to_owned();
    if normalized.is_empty() {
        if !active.is_empty() {
            return Err("active_profile must be empty when no profiles exist".to_owned());
        }
    } else if active.is_empty() || !names.contains(&active) {
        return Err("active_profile must name an existing profile".to_owned());
    }
    Ok(ProfileConfig {
        version: VERSION,
        active_profile: active,
        profiles: normalized,
    })
}

/// Validates and normalizes an in-memory configuration.
///
/// Legacy migration derives a configuration from project settings that may no
/// longer validate; callers persist only the validated result, so a rejected
/// migration never reaches disk.
pub fn validate_profile_config(config: &ProfileConfig) -> Result<ProfileConfig, String> {
    let value = serde_json::to_value(config).expect("profile configuration serializes");
    validate_config(&value)
}

/// Outcome of migrating a legacy single-source configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migration {
    /// Whether legacy settings produced a profile.
    pub migrated: bool,
    /// The migrated configuration, or an empty one when nothing migrated.
    pub config: ProfileConfig,
}

/// Converts the former single-source settings into a default profile.
pub fn migrate_legacy_values(
    source: &str,
    output: &str,
    auto_refresh: bool,
    source_exists: bool,
) -> Migration {
    if source.is_empty() || !source_exists {
        return Migration {
            migrated: false,
            config: empty_config(),
        };
    }
    let output = if output.is_empty() {
        LEGACY_DEFAULT_OUTPUT
    } else {
        output
    };
    let mut profile = new_profile("default", source, output);
    profile.auto_refresh = auto_refresh;
    profile.build_on_start = true;
    Migration {
        migrated: true,
        config: ProfileConfig {
            version: VERSION,
            active_profile: "default".to_owned(),
            profiles: vec![profile],
        },
    }
}

/// Validates a `res://` JSON configuration path.
pub fn validate_config_path(path: &str) -> Result<String, String> {
    validate_resource_path(path, &["json"], "must be a res:// JSON path")
}

/// Validates a confined `res://` theme root path.
pub fn validate_source_path(path: &str) -> Result<String, String> {
    validate_resource_path(
        path,
        &["tms", "kdl"],
        "must be a confined res:// .tms or .kdl path",
    )
}

/// Validates a confined `res://` theme output path.
pub fn validate_output_path(path: &str) -> Result<String, String> {
    validate_resource_path(path, &["tres"], "must be a confined res:// .tres path")
}

/// Validates a confined `res://` directory path.
pub fn validate_output_directory(path: &str) -> Result<String, String> {
    validate_resource_path(path, &[], "must be a confined res:// directory")
}

fn validate_resource_path(
    path: &str,
    extensions: &[&str],
    kind_error: &str,
) -> Result<String, String> {
    let Some(relative) = path.strip_prefix("res://") else {
        return Err(kind_error.to_owned());
    };
    if path.contains('\\') || relative.is_empty() {
        return Err(kind_error.to_owned());
    }
    if relative
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err("must not contain empty, '.' or '..' path segments".to_owned());
    }
    if !extensions.is_empty() && !extensions.contains(&extension_of(path).as_str()) {
        return Err(kind_error.to_owned());
    }
    Ok(simplify_path(path))
}

/// Mirrors Godot's `String.get_extension`: the text after the final dot, with
/// the empty string when the final dot lies in a directory name.
fn extension_of(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or_default();
    match file.rsplit_once('.') {
        Some((_, extension)) => extension.to_ascii_lowercase(),
        None => String::new(),
    }
}

/// Mirrors Godot's `String.simplify_path` for resource paths.
fn simplify_path(path: &str) -> String {
    let (prefix, rest) = match path.split_once("://") {
        Some((scheme, rest)) => (format!("{scheme}://"), rest),
        None => (String::new(), path),
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in rest.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }
    format!("{prefix}{}", segments.join("/"))
}

fn valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_digit()
                || byte.is_ascii_uppercase()
                || byte.is_ascii_lowercase()
                || byte == b'-'
                || byte == b'_'
        })
}

fn profile_string<'a>(
    profile: &'a serde_json::Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<&'a str, String> {
    profile
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("profiles[{index}].{key} has the wrong type"))
}

fn profile_bool(
    profile: &serde_json::Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<bool, String> {
    profile
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("profiles[{index}].{key} has the wrong type"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn config_with(profiles: Value, active: Value) -> Value {
        json!({ "version": 1, "active_profile": active, "profiles": profiles })
    }

    fn profile(name: &str, output: &str) -> Value {
        json!({
            "name": name,
            "source": format!("res://theme/{name}.tms"),
            "output": output,
            "auto_refresh": true,
            "build_on_start": false,
            "preview": "none",
            "enabled": true,
        })
    }

    #[test]
    fn accepts_integral_float_version_but_rejects_other_version_values() {
        for version in [json!(1), json!(1.0)] {
            let mut config = config_with(json!([profile("light", "res://a.tres")]), json!("light"));
            config["version"] = version;
            assert!(validate_config(&config).is_ok(), "version must be accepted");
        }
        for version in [
            json!(1.5),
            json!(2),
            json!(2.0),
            json!("1"),
            json!(true),
            Value::Null,
        ] {
            let mut config = config_with(json!([profile("light", "res://a.tres")]), json!("light"));
            config["version"] = version.clone();
            assert!(
                validate_config(&config).is_err(),
                "version {version} must be rejected"
            );
        }
        let mut missing = config_with(json!([profile("light", "res://a.tres")]), json!("light"));
        missing.as_object_mut().expect("object").remove("version");
        assert!(validate_config(&missing).is_err());
    }

    #[test]
    fn normalizes_paths_and_rejects_duplicate_names_and_outputs() {
        let normalized = validate_config(&config_with(
            json!([
                profile("light", "res://.themosis/light.tres"),
                profile("dark", "res://.themosis/dark.tres"),
            ]),
            json!("dark"),
        ))
        .expect("configuration is valid");
        assert_eq!(normalized.active_profile, "dark");
        assert_eq!(normalized.profiles[0].source, "res://theme/light.tms");

        let duplicate_name = config_with(
            json!([
                profile("light", "res://a.tres"),
                profile("light", "res://b.tres"),
            ]),
            json!("light"),
        );
        assert_eq!(
            validate_config(&duplicate_name).expect_err("rejected"),
            "duplicate profile name 'light'"
        );

        let duplicate_output = config_with(
            json!([
                profile("light", "res://same.tres"),
                profile("dark", "res://same.tres"),
            ]),
            json!("light"),
        );
        assert_eq!(
            validate_config(&duplicate_output).expect_err("rejected"),
            "profiles 'light' and 'dark' share output 'res://same.tres'"
        );

        let bad_name = config_with(
            json!([profile("bad name", "res://a.tres")]),
            json!("bad name"),
        );
        assert_eq!(
            validate_config(&bad_name).expect_err("rejected"),
            "profiles[0].name must use letters, numbers, '_' or '-'"
        );

        let mismatched_type = config_with(
            json!([{
                "name": "light",
                "source": "res://a.tms",
                "output": "res://a.tres",
                "auto_refresh": "yes",
                "build_on_start": false,
                "preview": "none",
                "enabled": true,
            }]),
            json!("light"),
        );
        assert_eq!(
            validate_config(&mismatched_type).expect_err("rejected"),
            "profiles[0].auto_refresh has the wrong type"
        );
    }

    #[test]
    fn enforces_active_profile_and_preview_rules() {
        assert_eq!(
            validate_config(&config_with(json!([]), json!("light"))).expect_err("rejected"),
            "active_profile must be empty when no profiles exist"
        );
        assert_eq!(
            validate_config(&config_with(
                json!([profile("light", "res://a.tres")]),
                json!("")
            ))
            .expect_err("rejected"),
            "active_profile must name an existing profile"
        );
        let mut bad_preview =
            config_with(json!([profile("light", "res://a.tres")]), json!("light"));
        bad_preview["profiles"][0]["preview"] = json!("scene");
        assert_eq!(
            validate_config(&bad_preview).expect_err("rejected"),
            "profile 'light' preview must be 'none' or 'edited_scene'"
        );
    }

    #[test]
    fn rejects_escaping_or_ambiguous_resource_paths() {
        for path in [
            "res://theme/../light.tms",
            "res://theme\\light.tms",
            "res://theme/./light.tms",
            "res://theme//light.tms",
            "res://",
            "theme/light.tms",
            "/light.tms",
        ] {
            assert!(
                validate_source_path(path).is_err(),
                "'{path}' must be rejected"
            );
        }
        assert!(validate_output_path("res://.themosis/../escape.tres").is_err());
        assert!(validate_output_directory("res://theme/./generated").is_err());
        assert!(validate_output_directory("res://theme/generated").is_ok());
        assert!(validate_source_path("res://theme/light.kdl").is_ok());
        assert!(validate_config_path("res://themosis.godot.json").is_ok());
        assert_eq!(
            validate_source_path("res://theme/light.tms").expect("valid"),
            "res://theme/light.tms"
        );
    }

    #[test]
    fn migrates_legacy_values_only_for_an_existing_source() {
        let skipped = migrate_legacy_values("res://theme/light.tms", "", true, false);
        assert!(!skipped.migrated);
        assert!(!skipped.config.is_configured());

        let migrated = migrate_legacy_values("res://theme/light.tms", "", false, true);
        assert!(migrated.migrated);
        assert_eq!(migrated.config.active_profile, "default");
        let profile = &migrated.config.profiles[0];
        assert_eq!(profile.name, "default");
        assert_eq!(profile.output, LEGACY_DEFAULT_OUTPUT);
        assert!(!profile.auto_refresh);
        assert!(profile.build_on_start);
        assert_eq!(profile.preview, PREVIEW_NONE);
    }

    #[test]
    fn validates_migrated_configurations_before_persisting() {
        let escaping =
            migrate_legacy_values("res://theme/light.tms", "res://../outside.tres", true, true);
        assert!(escaping.migrated);
        assert_eq!(
            validate_profile_config(&escaping.config).expect_err("rejected"),
            "profile 'default' output must not contain empty, '.' or '..' path segments"
        );

        let valid = migrate_legacy_values(
            "res://theme/light.tms",
            "res://.themosis/light.tres",
            true,
            true,
        );
        assert_eq!(
            validate_profile_config(&valid.config).expect("valid"),
            valid.config
        );
    }

    #[test]
    fn serializes_sorted_keys_and_round_trips() {
        let config = validate_config(&config_with(
            json!([profile("light", "res://.themosis/light.tres")]),
            json!("light"),
        ))
        .expect("configuration is valid");
        let text = config.to_json_string();
        assert!(text.ends_with("}\n"), "trailing newline is preserved");
        assert!(
            text.starts_with(
                "{\n  \"active_profile\": \"light\",\n  \"profiles\": [\n    {\n      \"auto_refresh\": true,\n      \"build_on_start\": false,\n"
            ),
            "sorted keys with two-space indentation:\n{text}"
        );
        assert_eq!(parse_config(&text).expect("round trip"), config);
    }

    #[test]
    fn reports_syntax_and_schema_failures_separately() {
        let syntax = parse_config("{invalid").expect_err("syntax error");
        assert!(matches!(syntax, ConfigError::Syntax { .. }));
        assert!(
            syntax
                .load_message("res://themosis.godot.json")
                .starts_with(
                    "cannot parse profile configuration 'res://themosis.godot.json' at line 1:"
                )
        );
        let invalid = parse_config("{\"version\": 2}").expect_err("schema error");
        assert_eq!(
            invalid.load_message("res://themosis.godot.json"),
            "invalid profile configuration 'res://themosis.godot.json': version must be 1"
        );
    }
}
