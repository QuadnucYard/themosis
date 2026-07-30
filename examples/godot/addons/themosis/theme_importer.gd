@tool
extends EditorImportPlugin

signal import_completed(source: String, result: Dictionary)

const IMPORTER_NAME := "themosis.theme"
const SOURCE_EXTENSION := "tms"
const SAVE_EXTENSION := "tres"
const IMPORTER_VERSION := 1
const DEPENDENCY_FINGERPRINT_META := &"_themosis_dependency_fingerprint"

func _get_importer_name() -> String:
    return IMPORTER_NAME

func _get_visible_name() -> String:
    return "Themosis Theme"

func _get_format_version() -> int:
    return IMPORTER_VERSION

func _get_recognized_extensions() -> PackedStringArray:
    return PackedStringArray([SOURCE_EXTENSION])

func _get_save_extension() -> String:
    return SAVE_EXTENSION

func _get_resource_type() -> String:
    return "Theme"

func _get_preset_count() -> int:
    return 1

func _get_preset_name(_preset_index: int) -> String:
    return "Default"

func _get_import_options(_path: String, _preset_index: int) -> Array[Dictionary]:
    return []

func _can_import_threaded() -> bool:
    # Compilation enters the running engine to inspect native theme metadata.
    return false

func _import(
    source_file: String,
    save_path: String,
    _options: Dictionary,
    _platform_variants: Array[String],
    _gen_files: Array[String],
) -> Error:
    var generator := ThemosisThemeGenerator.new()
    var result: Dictionary = generator.generate_result(source_file)
    import_completed.emit(source_file, result)
    if not bool(result.get("ok", false)):
        for diagnostic in result.get("diagnostics", []):
            push_error("Themosis import [%s]: %s" % [
                str(diagnostic.get("code", "error")),
                str(diagnostic.get("message", result.get("error", "import failed"))),
            ])
        return ERR_PARSE_ERROR

    var theme := result.get("theme") as Theme
    if theme == null:
        push_error("Themosis importer received no Theme for " + source_file)
        return ERR_INVALID_DATA
    theme.resource_name = source_file.get_file().get_basename()
    theme.set_meta(
        DEPENDENCY_FINGERPRINT_META,
        dependency_fingerprint(result.get("dependencies", PackedStringArray([source_file]))),
    )
    return ResourceSaver.save(theme, save_path + "." + SAVE_EXTENSION)

static func dependency_fingerprint(dependencies: PackedStringArray) -> String:
    var paths := dependencies.duplicate()
    paths.sort()
    var entries := PackedStringArray()
    for dependency in paths:
        var path := str(dependency).simplify_path()
        var digest := FileAccess.get_md5(path) if FileAccess.file_exists(path) else "<missing>"
        entries.append(path + ":" + digest)
    return "\n".join(entries).md5_text()
