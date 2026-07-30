@tool
extends RefCounted

const VERSION := 1
const CONFIG_SETTING := "themosis/profile_config"
const DEFAULT_CONFIG_PATH := "res://themosis.godot.json"
const LEGACY_SOURCE_SETTING := "themosis/theme_source"
const LEGACY_OUTPUT_SETTING := "themosis/generated_theme"
const LEGACY_AUTO_SETTING := "themosis/auto_refresh"
const PREVIEW_NONE := "none"
const PREVIEW_EDITED_SCENE := "edited_scene"

static func ensure_project_setting() -> void:
    if not ProjectSettings.has_setting(CONFIG_SETTING):
        ProjectSettings.set_setting(CONFIG_SETTING, DEFAULT_CONFIG_PATH)
    ProjectSettings.set_initial_value(CONFIG_SETTING, DEFAULT_CONFIG_PATH)
    ProjectSettings.add_property_info({
        "name": CONFIG_SETTING,
        "type": TYPE_STRING,
        "hint": PROPERTY_HINT_FILE,
        "hint_string": "*.json",
    })

static func empty_config() -> Dictionary:
    return {
        "version": VERSION,
        "active_profile": "",
        "profiles": [],
    }

static func new_profile(name: String, source: String, output: String) -> Dictionary:
    return {
        "name": name,
        "source": source,
        "output": output,
        "auto_refresh": true,
        "build_on_start": false,
        "preview": PREVIEW_NONE,
        "enabled": true,
    }

static func load_config(path_override := "", migrate_legacy := true) -> Dictionary:
    ensure_project_setting()
    var path := str(ProjectSettings.get_setting(CONFIG_SETTING, DEFAULT_CONFIG_PATH))
    if not str(path_override).is_empty():
        path = str(path_override)
    var path_error := _validate_config_path(path)
    if not path_error.is_empty():
        return _failure(path_error, path)

    if FileAccess.file_exists(path):
        var file := FileAccess.open(path, FileAccess.READ)
        if file == null:
            return _failure("cannot open profile configuration '%s'" % path, path)
        var parser := JSON.new()
        var parse_error := parser.parse(file.get_as_text())
        if parse_error != OK:
            return _failure(
                "cannot parse profile configuration '%s' at line %d: %s" % [
                    path,
                    parser.get_error_line(),
                    parser.get_error_message(),
                ],
                path,
            )
        var validation := validate_config(parser.data)
        if not bool(validation["ok"]):
            return _failure(
                "invalid profile configuration '%s': %s" % [path, validation["error"]],
                path,
            )
        return _success(validation["config"], path, false)

    if migrate_legacy:
        var source := str(ProjectSettings.get_setting(LEGACY_SOURCE_SETTING, ""))
        var migration := migrate_legacy_values({
            "source": source,
            "output": str(ProjectSettings.get_setting(LEGACY_OUTPUT_SETTING, "")),
            "auto_refresh": bool(ProjectSettings.get_setting(LEGACY_AUTO_SETTING, true)),
        }, not source.is_empty() and FileAccess.file_exists(source))
        if bool(migration["migrated"]):
            var saved := save_config(migration["config"], path)
            if not bool(saved["ok"]):
                return saved
            return _success(saved["config"], path, true)

    return _success(empty_config(), path, false)

static func save_config(config: Variant, path_override := "") -> Dictionary:
    ensure_project_setting()
    var path := str(ProjectSettings.get_setting(CONFIG_SETTING, DEFAULT_CONFIG_PATH))
    if not str(path_override).is_empty():
        path = str(path_override)
    var path_error := _validate_config_path(path)
    if not path_error.is_empty():
        return _failure(path_error, path)
    var validation := validate_config(config)
    if not bool(validation["ok"]):
        return _failure(str(validation["error"]), path)
    var normalized: Dictionary = validation["config"]
    var directory_error := DirAccess.make_dir_recursive_absolute(
        ProjectSettings.globalize_path(path.get_base_dir())
    )
    if directory_error != OK:
        return _failure(
            "cannot create profile configuration directory '%s' (%s)" % [
                path.get_base_dir(),
                error_string(directory_error),
            ],
            path,
        )
    var temporary := path.get_basename() + ".themosis-tmp.json"
    var file := FileAccess.open(temporary, FileAccess.WRITE)
    if file == null:
        return _failure("cannot write temporary profile configuration '%s'" % temporary, path)
    file.store_string(JSON.stringify(normalized, "  ", true) + "\n")
    file.close()
    var replacement := _replace_file(temporary, path)
    if replacement != OK:
        DirAccess.remove_absolute(ProjectSettings.globalize_path(temporary))
        return _failure(
            "cannot replace profile configuration '%s' (%s)" % [
                path,
                error_string(replacement),
            ],
            path,
        )
    return _success(normalized, path, false)

static func migrate_legacy_values(values: Dictionary, source_exists: bool) -> Dictionary:
    var source := str(values.get("source", ""))
    if source.is_empty() or not source_exists:
        return {"migrated": false, "config": empty_config()}
    var output := str(values.get("output", ""))
    if output.is_empty():
        output = "res://.themosis/generated_theme.tres"
    var profile := new_profile("default", source, output)
    profile["auto_refresh"] = bool(values.get("auto_refresh", true))
    profile["build_on_start"] = true
    return {
        "migrated": true,
        "config": {
            "version": VERSION,
            "active_profile": "default",
            "profiles": [profile],
        },
    }

static func validate_config(value: Variant) -> Dictionary:
    if typeof(value) != TYPE_DICTIONARY:
        return _validation_failure("root must be an object")
    var config: Dictionary = value
    var version_type := typeof(config.get("version"))
    if (
        (version_type != TYPE_INT and version_type != TYPE_FLOAT)
        or int(config["version"]) != VERSION
    ):
        return _validation_failure("version must be %d" % VERSION)
    if typeof(config.get("active_profile")) != TYPE_STRING:
        return _validation_failure("active_profile must be a string")
    if typeof(config.get("profiles")) != TYPE_ARRAY:
        return _validation_failure("profiles must be an array")

    var normalized_profiles: Array = []
    var names := {}
    var outputs := {}
    for index in config["profiles"].size():
        var profile_value: Variant = config["profiles"][index]
        if typeof(profile_value) != TYPE_DICTIONARY:
            return _validation_failure("profiles[%d] must be an object" % index)
        var profile: Dictionary = profile_value
        var required := {
            "name": TYPE_STRING,
            "source": TYPE_STRING,
            "output": TYPE_STRING,
            "auto_refresh": TYPE_BOOL,
            "build_on_start": TYPE_BOOL,
            "preview": TYPE_STRING,
            "enabled": TYPE_BOOL,
        }
        for key in required:
            if not profile.has(key) or typeof(profile[key]) != required[key]:
                return _validation_failure("profiles[%d].%s has the wrong type" % [index, key])
        var name := str(profile["name"])
        if not _valid_profile_name(name):
            return _validation_failure(
                "profiles[%d].name must use letters, numbers, '_' or '-'" % index
            )
        if names.has(name):
            return _validation_failure("duplicate profile name '%s'" % name)
        names[name] = true
        var source := str(profile["source"])
        var source_error := validate_source_path(source)
        if not source_error.is_empty():
            return _validation_failure(
                "profile '%s' source %s" % [name, source_error]
            )
        source = source.simplify_path()
        var output := str(profile["output"])
        var output_error := validate_output_path(output)
        if not output_error.is_empty():
            return _validation_failure(
                "profile '%s' output %s" % [name, output_error]
            )
        output = output.simplify_path()
        if outputs.has(output):
            return _validation_failure(
                "profiles '%s' and '%s' share output '%s'" % [outputs[output], name, output]
            )
        outputs[output] = name
        var preview := str(profile["preview"])
        if preview != PREVIEW_NONE and preview != PREVIEW_EDITED_SCENE:
            return _validation_failure(
                "profile '%s' preview must be '%s' or '%s'" % [
                    name,
                    PREVIEW_NONE,
                    PREVIEW_EDITED_SCENE,
                ]
            )
        normalized_profiles.append({
            "name": name,
            "source": source,
            "output": output,
            "auto_refresh": bool(profile["auto_refresh"]),
            "build_on_start": bool(profile["build_on_start"]),
            "preview": preview,
            "enabled": bool(profile["enabled"]),
        })

    var active := str(config["active_profile"])
    if normalized_profiles.is_empty():
        if not active.is_empty():
            return _validation_failure("active_profile must be empty when no profiles exist")
    elif active.is_empty() or not names.has(active):
        return _validation_failure("active_profile must name an existing profile")
    return {
        "ok": true,
        "error": "",
        "config": {
            "version": VERSION,
            "active_profile": active,
            "profiles": normalized_profiles,
        },
    }

static func find_profile(config: Dictionary, name: String) -> Dictionary:
    for profile in config.get("profiles", []):
        if str(profile.get("name", "")) == name:
            return profile
    return {}

static func _valid_profile_name(value: String) -> bool:
    if value.is_empty():
        return false
    for index in value.length():
        var code := value.unicode_at(index)
        var valid := (
            (code >= 48 and code <= 57)
            or (code >= 65 and code <= 90)
            or (code >= 97 and code <= 122)
            or code == 45
            or code == 95
        )
        if not valid:
            return false
    return true

static func _validate_config_path(path: String) -> String:
    return _validate_resource_path(path, PackedStringArray(["json"]), "must be a res:// JSON path")

static func validate_source_path(path: String) -> String:
    return _validate_resource_path(
        path,
        PackedStringArray(["tms", "kdl"]),
        "must be a confined res:// .tms or .kdl path",
    )

static func validate_output_path(path: String) -> String:
    return _validate_resource_path(
        path,
        PackedStringArray(["tres"]),
        "must be a confined res:// .tres path",
    )

static func validate_output_directory(path: String) -> String:
    return _validate_resource_path(path, PackedStringArray(), "must be a confined res:// directory")

static func _validate_resource_path(
    path: String,
    extensions: PackedStringArray,
    kind_error: String,
) -> String:
    if not path.begins_with("res://") or path.contains("\\"):
        return kind_error
    var relative := path.substr("res://".length())
    if relative.is_empty():
        return kind_error
    var segments := relative.split("/", true)
    for segment in segments:
        if segment.is_empty() or segment == "." or segment == "..":
            return "must not contain empty, '.' or '..' path segments"
    if not extensions.is_empty() and not extensions.has(path.get_extension().to_lower()):
        return kind_error
    return ""

static func _replace_file(temporary: String, output: String) -> Error:
    var temporary_absolute := ProjectSettings.globalize_path(temporary)
    var output_absolute := ProjectSettings.globalize_path(output)
    if not FileAccess.file_exists(output):
        return DirAccess.rename_absolute(temporary_absolute, output_absolute)
    var backup := output.get_basename() + ".themosis-backup." + output.get_extension()
    if FileAccess.file_exists(backup):
        return ERR_ALREADY_EXISTS
    var backup_absolute := ProjectSettings.globalize_path(backup)
    var backup_error := DirAccess.rename_absolute(output_absolute, backup_absolute)
    if backup_error != OK:
        return backup_error
    var replace_error := DirAccess.rename_absolute(temporary_absolute, output_absolute)
    if replace_error != OK:
        DirAccess.rename_absolute(backup_absolute, output_absolute)
        return replace_error
    return DirAccess.remove_absolute(backup_absolute)

static func _success(config: Dictionary, path: String, migrated: bool) -> Dictionary:
    return {
        "ok": true,
        "error": "",
        "path": path,
        "config": config,
        "migrated": migrated,
        "configured": not config["profiles"].is_empty(),
    }

static func _failure(message: String, path: String) -> Dictionary:
    return {
        "ok": false,
        "error": message,
        "path": path,
        "config": empty_config(),
        "migrated": false,
        "configured": false,
    }

static func _validation_failure(message: String) -> Dictionary:
    return {"ok": false, "error": message, "config": empty_config()}
