extends SceneTree

const NativeThemeBuilder = preload("native_theme_builder.gd")
const MINIMUM_GODOT_MAJOR := 4
const MINIMUM_GODOT_MINOR := 5
const MINIMUM_GODOT_PATCH := 0


func _initialize() -> void:
	var arguments: PackedStringArray = OS.get_cmdline_user_args()
	if arguments.size() != 2:
		printerr("Themosis builder expects REQUEST_JSON RESPONSE_JSON after --")
		quit(2)
		return
	var response: Dictionary = _run_request_file(str(arguments[0]))
	if not _write_response(str(arguments[1]), response):
		quit(2)
		return
	quit(0 if bool(response.get("ok", false)) else 1)


static func _run_request_file(request_path: String) -> Dictionary:
	var version: Dictionary = _godot_version()
	if not _version_at_least(
		version,
		MINIMUM_GODOT_MAJOR,
		MINIMUM_GODOT_MINOR,
		MINIMUM_GODOT_PATCH,
	):
		return _runner_failure(
			version,
			"unsupported_godot_version",
			"Themosis requires Godot %d.%d.%d or newer, got %s" % [
				MINIMUM_GODOT_MAJOR,
				MINIMUM_GODOT_MINOR,
				MINIMUM_GODOT_PATCH,
				str(version.get("display", "unknown")),
			],
		)
	var request_file := FileAccess.open(request_path, FileAccess.READ)
	if request_file == null:
		return _runner_failure(version, "request_read", "could not read build request")
	var parser := JSON.new()
	var parse_error := parser.parse(request_file.get_as_text())
	request_file.close()
	if parse_error != OK or parser.data is not Dictionary:
		return _runner_failure(version, "request_parse", "build request is not valid JSON")
	var request: Dictionary = parser.data
	var required_value: Variant = request.get("required_godot_version")
	if required_value != null:
		if required_value is not String:
			return _runner_failure(
				version,
				"invalid_required_version",
				"required Godot version must be numeric MAJOR.MINOR.PATCH",
			)
		var required_text := str(required_value)
		var required: Dictionary = _parse_numeric_version(required_text)
		if not bool(required.get("ok", false)):
			return _runner_failure(
				version,
				"invalid_required_version",
				"required Godot version '%s' must be numeric MAJOR.MINOR.PATCH" % required_text,
			)
		if (
			int(version.get("major", 0)) != int(required.get("major", -1))
			or int(version.get("minor", 0)) != int(required.get("minor", -1))
			or int(version.get("patch", 0)) != int(required.get("patch", -1))
		):
			return _runner_failure(
				version,
				"godot_version_mismatch",
				"required Godot %s, got %s" % [
					required_text,
					str(version.get("display", "unknown")),
				],
			)
	var plan_value: Variant = request.get("plan")
	if plan_value is not Dictionary:
		return _runner_failure(version, "invalid_plan", "build request has no Godot plan")
	var builder := NativeThemeBuilder.new()
	var result: Dictionary = builder.build_plan(plan_value)
	result["godot_version"] = version
	if not bool(result.get("ok", false)):
		result.erase("theme")
		return result
	var operation_value: Variant = request.get("operation")
	if operation_value is not String:
		return _runner_failure(version, "invalid_operation", "operation must be 'check' or 'build'")
	var operation := str(operation_value)
	if operation == "check":
		result.erase("theme")
		return result
	if operation != "build":
		return _runner_failure(version, "invalid_operation", "operation must be 'check' or 'build'")
	var output_value: Variant = request.get("output")
	if output_value is not String:
		return _runner_failure(version, "invalid_output", "build operation requires an output path")
	var output := str(output_value)
	var save_result: Dictionary = _save_theme(result["theme"] as Theme, output)
	result.erase("theme")
	if not bool(save_result.get("ok", false)):
		return _runner_failure(
			version,
			str(save_result.get("code", "save_failed")),
			str(save_result.get("message", "could not save Godot theme")),
		)
	result["output"] = output
	return result


static func _godot_version() -> Dictionary:
	var info: Dictionary = Engine.get_version_info()
	return {
		"display": str(info.get("string", "unknown")),
		"major": int(info.get("major", 0)),
		"minor": int(info.get("minor", 0)),
		"patch": int(info.get("patch", 0)),
		"status": str(info.get("status", "")),
		"build": str(info.get("build", "")),
		"hash": str(info.get("hash", "")),
	}


static func _version_at_least(version: Dictionary, major: int, minor: int, patch: int) -> bool:
	var actual: Array[int] = [
		int(version.get("major", 0)),
		int(version.get("minor", 0)),
		int(version.get("patch", 0)),
	]
	var required: Array[int] = [major, minor, patch]
	for index: int in range(3):
		if actual[index] != required[index]:
			return actual[index] > required[index]
	return true


static func _parse_numeric_version(value: String) -> Dictionary:
	var parts: PackedStringArray = value.split(".")
	if parts.size() != 3:
		return {"ok": false}
	for part: String in parts:
		if (
			not _ascii_digits(part)
			or part.length() > 10
			or (part.length() == 10 and part > "4294967295")
		):
			return {"ok": false}
	return {
		"ok": true,
		"major": int(parts[0]),
		"minor": int(parts[1]),
		"patch": int(parts[2]),
	}


static func _ascii_digits(value: String) -> bool:
	if value.is_empty():
		return false
	for index: int in range(value.length()):
		var codepoint := value.unicode_at(index)
		if codepoint < 48 or codepoint > 57:
			return false
	return true


static func _save_theme(theme: Theme, output: String) -> Dictionary:
	if not _valid_output_path(output):
		return _item_failure("invalid_output", "theme output must be a res:// path ending in .tres")
	var directory_error := DirAccess.make_dir_recursive_absolute(
		ProjectSettings.globalize_path(output.get_base_dir())
	)
	if directory_error != OK:
		return _item_failure(
			"output_directory",
			"could not create output directory '%s': %s" % [
				output.get_base_dir(),
				error_string(directory_error),
			],
		)
	var temporary := output.get_basename() + ".themosis-tmp.tres"
	var save_error := ResourceSaver.save(theme, temporary)
	if save_error != OK:
		return _item_failure(
			"save_failed",
			"could not save temporary theme '%s': %s" % [temporary, error_string(save_error)],
		)
	var replacement := _replace_file(temporary, output)
	if replacement != OK:
		DirAccess.remove_absolute(ProjectSettings.globalize_path(temporary))
		return _item_failure(
			"replace_failed",
			"could not replace theme '%s': %s" % [output, error_string(replacement)],
		)
	return {"ok": true}


static func _valid_output_path(output: String) -> bool:
	if not output.begins_with("res://") or output.get_extension() != "tres":
		return false
	var relative := output.substr("res://".length())
	if relative.is_empty() or relative.contains("\\"):
		return false
	var segments: PackedStringArray = relative.split("/", true)
	for segment: String in segments:
		if segment.is_empty() or segment == "." or segment == "..":
			return false
	return true


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


static func _write_response(path: String, response: Dictionary) -> bool:
	var output := FileAccess.open(path, FileAccess.WRITE)
	if output == null:
		printerr("Themosis builder could not write response '%s'" % path)
		return false
	output.store_string(JSON.stringify(response, "  ", true, true) + "\n")
	output.close()
	return true


static func _runner_failure(version: Dictionary, code: String, message: String) -> Dictionary:
	return {
		"ok": false,
		"godot_version": version,
		"diagnostics": [_diagnostic(code, message, "", "", "", "")],
	}


static func _item_failure(code: String, message: String) -> Dictionary:
	return {"ok": false, "code": code, "message": message}


static func _diagnostic(
	code: String,
	message: String,
	style: String,
	target: String,
	state: String,
	property: String,
) -> Dictionary:
	return {
		"severity": "error",
		"code": code,
		"message": message,
		"style": style,
		"target": target,
		"state": state,
		"property": property,
	}
