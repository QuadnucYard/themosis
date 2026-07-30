@tool
extends EditorPlugin

const ThemeImporter := preload("res://addons/themosis/theme_importer.gd")
const ThemeBuilder := preload("res://addons/themosis/theme_builder.gd")
const ThemeDock := preload("res://addons/themosis/theme_dock.gd")

var _theme_importer: EditorImportPlugin
var _dock: VBoxContainer
var _button: Button
var _filesystem: EditorFileSystem
var _refresh_timer: Timer
var _sources := PackedStringArray()
var _dependencies: Dictionary = {}
var _snapshots: Dictionary = {}
var _pending_reimports: Dictionary = {}
var _reimporting := false
var _check_scheduled := false
var _waiting_for_initial_scan := false

func _enter_tree() -> void:
    _theme_importer = ThemeImporter.new()
    _theme_importer.import_completed.connect(_on_import_completed)
    add_import_plugin(_theme_importer)

    _dock = ThemeDock.new()
    _dock.selection_changed.connect(_show_preview)
    _dock.reimport_requested.connect(_reimport_one)
    _dock.reimport_all_requested.connect(_reimport_all)
    _dock.materialize_requested.connect(_materialize)
    _dock.materialize_all_requested.connect(_materialize_all)
    _dock.diagnostic_clicked.connect(_open_diagnostic)
    add_control_to_dock(DOCK_SLOT_RIGHT_UL, _dock)

    _button = Button.new()
    _button.text = "Reimport Themosis"
    _button.tooltip_text = "Reimport all Themosis .tms theme assets"
    _button.pressed.connect(_reimport_all)
    add_control_to_container(CONTAINER_TOOLBAR, _button)

    _refresh_timer = Timer.new()
    _refresh_timer.one_shot = true
    _refresh_timer.wait_time = 0.35
    _refresh_timer.timeout.connect(_reimport_pending)
    add_child(_refresh_timer)

    _filesystem = get_editor_interface().get_resource_filesystem()
    _filesystem.filesystem_changed.connect(_on_filesystem_changed)
    call_deferred("_initialize_themes")

func _exit_tree() -> void:
    if is_instance_valid(_filesystem) and _filesystem.filesystem_changed.is_connected(
        _on_filesystem_changed
    ):
        _filesystem.filesystem_changed.disconnect(_on_filesystem_changed)
    if is_instance_valid(_dock):
        remove_control_from_docks(_dock)
        _dock.queue_free()
    if is_instance_valid(_button):
        remove_control_from_container(CONTAINER_TOOLBAR, _button)
        _button.queue_free()
    if is_instance_valid(_refresh_timer):
        _refresh_timer.queue_free()
    if _theme_importer != null:
        if _theme_importer.import_completed.is_connected(_on_import_completed):
            _theme_importer.import_completed.disconnect(_on_import_completed)
        remove_import_plugin(_theme_importer)
        _theme_importer = null

func _initialize_themes() -> void:
    _refresh_sources()
    # Registering an importer normally schedules discovery. An explicit scan
    # also handles .tms files that predate plugin activation.
    _waiting_for_initial_scan = true
    _filesystem.scan()

func _refresh_sources() -> bool:
    var discovered := PackedStringArray()
    _collect_theme_sources("res://", discovered)
    discovered.sort()
    if discovered == _sources:
        return false
    _sources = discovered
    for source in _dependencies.keys():
        if not _sources.has(str(source)):
            _dependencies.erase(source)
            _snapshots.erase(source)
            _pending_reimports.erase(source)
    _dock.set_theme_sources(_sources)
    return true

func _collect_theme_sources(directory: String, result: PackedStringArray) -> void:
    var access := DirAccess.open(directory)
    if access == null:
        return
    access.list_dir_begin()
    var entry := access.get_next()
    while not entry.is_empty():
        if entry.begins_with("."):
            entry = access.get_next()
            continue
        var path := directory.path_join(entry)
        if access.current_is_dir():
            _collect_theme_sources(path, result)
        elif entry.get_extension().to_lower() == ThemeImporter.SOURCE_EXTENSION:
            result.append(path)
        entry = access.get_next()
    access.list_dir_end()

func _index_all_sources() -> void:
    for source in _sources:
        var stored_fingerprint := _stored_fingerprint(source)
        var result := _index_source(source)
        if not bool(result.get("ok", false)):
            continue
        var dependencies: PackedStringArray = result.get(
            "dependencies",
            PackedStringArray([source]),
        )
        var current_fingerprint := ThemeImporter.dependency_fingerprint(dependencies)
        if stored_fingerprint != current_fingerprint:
            _pending_reimports[source] = true
            _dock.mark_stale(source)

func _stored_fingerprint(source: String) -> String:
    if not ResourceLoader.exists(source, "Theme"):
        return ""
    var imported := ResourceLoader.load(
        source,
        "Theme",
        ResourceLoader.CACHE_MODE_IGNORE,
    ) as Theme
    if imported == null:
        return ""
    return str(imported.get_meta(ThemeImporter.DEPENDENCY_FINGERPRINT_META, ""))

func _index_source(source: String) -> Dictionary:
    var generator := ThemosisThemeGenerator.new()
    var result: Dictionary = generator.generate_result(source)
    _record_result(source, result)
    return result

func _record_result(source: String, result: Dictionary) -> void:
    var dependencies := result.get("dependencies", PackedStringArray([source])) as PackedStringArray
    if dependencies.is_empty():
        dependencies = PackedStringArray([source])
    _dependencies[source] = dependencies
    _snapshots[source] = _snapshot(dependencies)
    _dock.show_result(source, result, "import")

func _on_import_completed(source: String, result: Dictionary) -> void:
    _record_result(source, result)

func _on_filesystem_changed() -> void:
    if _check_scheduled:
        return
    _check_scheduled = true
    call_deferred("_check_dependency_changes")

func _check_dependency_changes() -> void:
    _check_scheduled = false
    var roots_changed := _refresh_sources()
    if _waiting_for_initial_scan:
        _waiting_for_initial_scan = false
        # Compare persisted dependency fingerprints only after Godot has
        # completed its ordinary source/importer scan. This avoids rebuilding
        # roots that Godot just imported because their .tms file changed.
        _index_all_sources()
        if not _pending_reimports.is_empty():
            _reimport_pending()
        return
    if roots_changed:
        for source in _sources:
            if not _dependencies.has(source):
                _index_source(source)
    if _reimporting:
        return
    for source in _sources:
        var dependencies: PackedStringArray = _dependencies.get(
            source,
            PackedStringArray([source]),
        )
        var current := _snapshot(dependencies)
        var previous: Dictionary = _snapshots.get(source, {})
        if current == previous:
            continue
        _snapshots[source] = current
        _pending_reimports[source] = true
        _dock.mark_stale(source)
    if not _pending_reimports.is_empty():
        _refresh_timer.start()

func _snapshot(paths: PackedStringArray) -> Dictionary:
    var result := {}
    for path in paths:
        var normalized := str(path).simplify_path()
        result[normalized] = (
            FileAccess.get_md5(normalized) if FileAccess.file_exists(normalized) else "<missing>"
        )
    return result

func _reimport_one(source: String) -> void:
    if source.is_empty():
        return
    _pending_reimports[source] = true
    _reimport_pending()

func _reimport_all() -> void:
    _refresh_sources()
    for source in _sources:
        _pending_reimports[source] = true
    _reimport_pending()

func _reimport_pending() -> void:
    if _reimporting or _pending_reimports.is_empty():
        return
    for source in _pending_reimports:
        if _filesystem.get_file_type(str(source)).is_empty():
            # Startup discovery has not registered this source yet. Keep the
            # request queued until the filesystem scan completes.
            _refresh_timer.start()
            return
    var sources := PackedStringArray()
    for source in _sources:
        if _pending_reimports.has(source):
            sources.append(source)
            _dock.mark_importing(source)
    _pending_reimports.clear()
    if sources.is_empty():
        return
    _reimporting = true
    _button.disabled = true
    _button.text = "Importing Themosis…"
    _filesystem.reimport_files(sources)
    # A failed importer may not emit a usable resource notification on every
    # supported Godot version, so refresh the structured result explicitly.
    for source in sources:
        _index_source(source)
    _button.disabled = false
    _button.text = "Reimport Themosis"
    _reimporting = false

func _show_preview(source: String) -> void:
    if source.is_empty() or not ResourceLoader.exists(source, "Theme"):
        return
    var loaded := ResourceLoader.load(source, "Theme", ResourceLoader.CACHE_MODE_REPLACE) as Theme
    if loaded != null:
        _dock.set_preview_theme(loaded)

func _materialize(source: String, output: String) -> void:
    var result := ThemeBuilder.materialize_source(source, output)
    _dock.show_result(source, result, "materialize")
    if bool(result.get("ok", false)):
        _filesystem.update_file(output)

func _materialize_all(directory: String) -> void:
    var failures := 0
    var generated := 0
    for source in _sources:
        var output := directory.path_join(source.get_file().get_basename() + ".tres")
        var result := ThemeBuilder.materialize_source(source, output)
        _dock.show_result(source, result, "materialize")
        if bool(result.get("ok", false)):
            generated += 1
            _filesystem.update_file(output)
        else:
            failures += 1
    _dock.show_message(
        "Materialized %d theme(s)%s" % [
            generated,
            " with %d failure(s)" % failures if failures > 0 else "",
        ],
        failures > 0,
    )

func _open_diagnostic(path: String) -> void:
    var editor := get_editor_interface()
    if editor.has_method("select_file"):
        editor.call("select_file", path)
