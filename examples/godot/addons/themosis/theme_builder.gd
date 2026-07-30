@tool
extends RefCounted

const ProfileStore := preload("res://addons/themosis/profile_store.gd")

const SOURCE_SETTING := ProfileStore.LEGACY_SOURCE_SETTING
const OUTPUT_SETTING := ProfileStore.LEGACY_OUTPUT_SETTING
const AUTO_SETTING := ProfileStore.LEGACY_AUTO_SETTING

static func ensure_project_settings() -> void:
    ProfileStore.ensure_project_setting()

static func load_profiles() -> Dictionary:
    return ProfileStore.load_config()

static func build_from_project_settings() -> Dictionary:
    var loaded := load_profiles()
    if not bool(loaded["ok"]):
        return _failure("", "", "", str(loaded["error"]), 0, PackedStringArray())
    var config: Dictionary = loaded["config"]
    if config["profiles"].is_empty():
        return _failure("", "", "", "no Themosis profiles are configured", 0, PackedStringArray())
    return build_named(config, str(config["active_profile"]))

static func build_named(config: Dictionary, profile_name: String) -> Dictionary:
    var validation := ProfileStore.validate_config(config)
    if not bool(validation["ok"]):
        return _failure(profile_name, "", "", str(validation["error"]), 0, PackedStringArray())
    var profile := ProfileStore.find_profile(validation["config"], profile_name)
    if profile.is_empty():
        return _failure(
            profile_name,
            "",
            "",
            "profile '%s' does not exist" % profile_name,
            0,
            PackedStringArray(),
        )
    if not bool(profile["enabled"]):
        return _failure(
            profile_name,
            str(profile["source"]),
            str(profile["output"]),
            "profile '%s' is disabled" % profile_name,
            0,
            PackedStringArray([str(profile["source"])]),
        )
    return build_profile(profile)

static func validate_named(config: Dictionary, profile_name: String) -> Dictionary:
    var validation := ProfileStore.validate_config(config)
    if not bool(validation["ok"]):
        return _failure(profile_name, "", "", str(validation["error"]), 0, PackedStringArray())
    var profile := ProfileStore.find_profile(validation["config"], profile_name)
    if profile.is_empty():
        return _failure(
            profile_name,
            "",
            "",
            "profile '%s' does not exist" % profile_name,
            0,
            PackedStringArray(),
        )
    return validate_profile(profile)

static func build_all(config: Dictionary) -> Dictionary:
    var validation := ProfileStore.validate_config(config)
    if not bool(validation["ok"]):
        return {
            "ok": false,
            "error": str(validation["error"]),
            "results": [],
            "outputs": PackedStringArray(),
        }
    var results: Array = []
    var outputs := PackedStringArray()
    var all_ok := true
    for profile in validation["config"]["profiles"]:
        if not bool(profile["enabled"]):
            continue
        var result := build_profile(profile)
        results.append(result)
        if bool(result["ok"]):
            outputs.append(str(result["output"]))
        else:
            all_ok = false
    return {
        "ok": all_ok,
        "error": "" if all_ok else "one or more profiles failed",
        "results": results,
        "outputs": outputs,
    }

static func build_profile(profile: Dictionary) -> Dictionary:
    return _run(profile, true)

static func validate_profile(profile: Dictionary) -> Dictionary:
    return _run(profile, false)

static func materialize_source(source: String, output: String) -> Dictionary:
    var profile := ProfileStore.new_profile(source.get_file().get_basename(), source, output)
    return _run(profile, true)

static func _run(profile: Dictionary, save_output: bool) -> Dictionary:
    var profile_name := str(profile.get("name", ""))
    var source := str(profile.get("source", ""))
    var output := str(profile.get("output", ""))
    var started := Time.get_ticks_msec()
    var source_error := ProfileStore.validate_source_path(source)
    if not source_error.is_empty():
        return _failure(
            profile_name,
            source,
            output,
            "source %s" % source_error,
            0,
            PackedStringArray([source]),
        )
    var output_error := ProfileStore.validate_output_path(output)
    if not output_error.is_empty():
        return _failure(
            profile_name,
            source,
            output,
            "output %s" % output_error,
            0,
            PackedStringArray([source]),
        )
    var previous_valid := FileAccess.file_exists(output)
    var generator := ThemosisThemeGenerator.new()
    var generation: Dictionary = generator.generate_result(source)
    var elapsed := Time.get_ticks_msec() - started
    var dependencies: PackedStringArray = generation.get(
        "dependencies",
        PackedStringArray([source]),
    )
    if not bool(generation.get("ok", false)):
        return _preserving_failure(
            profile_name,
            source,
            output,
            str(generation.get("error", "generation failed")),
            elapsed,
            dependencies,
            previous_valid,
            generation.get("diagnostics", []),
        )
    var generated := generation.get("theme") as Theme
    if generated == null:
        return _preserving_failure(
            profile_name,
            source,
            output,
            "Themosis extension returned no Theme",
            elapsed,
            dependencies,
            previous_valid,
        )
    if not save_output:
        return _success(
            profile_name,
            source,
            output,
            generated,
            "validated",
            elapsed,
            dependencies,
            previous_valid,
        )

    var directory_error := DirAccess.make_dir_recursive_absolute(
        ProjectSettings.globalize_path(output.get_base_dir())
    )
    if directory_error != OK:
        return _preserving_failure(
            profile_name,
            source,
            output,
            "cannot create generated theme directory '%s' (%s)" % [
                output.get_base_dir(),
                error_string(directory_error),
            ],
            elapsed,
            dependencies,
            previous_valid,
        )
    var temporary := output.get_basename() + ".themosis-tmp.tres"
    var save_error := ResourceSaver.save(generated, temporary)
    if save_error != OK:
        return _preserving_failure(
            profile_name,
            source,
            output,
            "cannot save temporary generated theme '%s' (%s)" % [
                temporary,
                error_string(save_error),
            ],
            elapsed,
            dependencies,
            previous_valid,
        )
    var replacement := _replace_file(temporary, output)
    if replacement != OK:
        DirAccess.remove_absolute(ProjectSettings.globalize_path(temporary))
        return _preserving_failure(
            profile_name,
            source,
            output,
            "cannot replace generated theme '%s' (%s)" % [
                output,
                error_string(replacement),
            ],
            elapsed,
            dependencies,
            previous_valid,
        )
    return _success(
        profile_name,
        source,
        output,
        generated,
        "success",
        Time.get_ticks_msec() - started,
        dependencies,
        true,
    )

static func _replace_file(temporary: String, output: String) -> Error:
    var temporary_absolute := ProjectSettings.globalize_path(temporary)
    var output_absolute := ProjectSettings.globalize_path(output)
    if not FileAccess.file_exists(output):
        return DirAccess.rename_absolute(temporary_absolute, output_absolute)
    var backup := output.get_basename() + ".themosis-backup.tres"
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

static func _success(
    profile: String,
    source: String,
    output: String,
    theme: Theme,
    status: String,
    elapsed_ms: int,
    dependencies: PackedStringArray,
    output_valid: bool,
) -> Dictionary:
    return {
        "ok": true,
        "status": status,
        "profile": profile,
        "source": source,
        "output": output,
        "theme": theme,
        "error": "",
        "diagnostics": [],
        "dependencies": dependencies,
        "elapsed_ms": elapsed_ms,
        "previous_output_valid": output_valid,
        "previous_output_remains_valid": output_valid,
        "preview_stale": false,
    }

static func _failure(
    profile: String,
    source: String,
    output: String,
    message: String,
    elapsed_ms: int,
    dependencies: PackedStringArray,
    diagnostics: Array = [],
) -> Dictionary:
    return {
        "ok": false,
        "status": "failure",
        "profile": profile,
        "source": source,
        "output": output,
        "theme": null,
        "error": message,
        "diagnostics": diagnostics if not diagnostics.is_empty() else _diagnostics(message),
        "dependencies": dependencies,
        "elapsed_ms": elapsed_ms,
        "previous_output_valid": false,
        "previous_output_remains_valid": false,
        "preview_stale": false,
    }

static func _preserving_failure(
    profile: String,
    source: String,
    output: String,
    message: String,
    elapsed_ms: int,
    dependencies: PackedStringArray,
    previous_valid: bool,
    diagnostics: Array = [],
) -> Dictionary:
    var result := _failure(
        profile,
        source,
        output,
        message,
        elapsed_ms,
        dependencies,
        diagnostics,
    )
    result["previous_output_valid"] = previous_valid
    result["previous_output_remains_valid"] = previous_valid
    return result

static func _diagnostics(message: String) -> Array:
    var result: Array = []
    for line in message.split("\n", false):
        var span_start := -1
        var span_end := -1
        var bytes_marker := line.rfind(" at bytes ")
        if bytes_marker >= 0:
            var byte_range := line.substr(bytes_marker + 10).split("..", false, 1)
            if byte_range.size() == 2 and byte_range[0].is_valid_int() and byte_range[1].is_valid_int():
                span_start = int(byte_range[0])
                span_end = int(byte_range[1])
        var source_line := -1
        var column := -1
        var line_marker := line.rfind(" at line ")
        if line_marker >= 0:
            var location := line.substr(line_marker + 9).split(", column ", false, 1)
            if location.size() == 2 and location[0].is_valid_int() and location[1].is_valid_int():
                source_line = int(location[0])
                column = int(location[1])
        var code := ""
        var code_start := line.find("error[")
        if code_start >= 0:
            var code_end := line.find("]", code_start)
            if code_end > code_start:
                code = line.substr(code_start + 6, code_end - code_start - 6)
        result.append({
            "severity": "error",
            "code": code,
            "message": line,
            "path": _diagnostic_path(line),
            "span_start": span_start,
            "span_end": span_end,
            "line": source_line,
            "column": column,
        })
    return result

static func _diagnostic_path(line: String) -> String:
    for extension in [".kdl", ".json"]:
        var end := line.find(extension)
        if end < 0:
            continue
        var start := line.rfind("'", end)
        if start >= 0:
            return line.substr(start + 1, end + extension.length() - start - 1)
        start = line.rfind(" ", end)
        return line.substr(start + 1, end + extension.length() - start - 1)
    return ""
