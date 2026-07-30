@tool
extends VBoxContainer

signal selection_changed(source: String)
signal reimport_requested(source: String)
signal reimport_all_requested
signal materialize_requested(source: String, output: String)
signal materialize_all_requested(directory: String)
signal diagnostic_clicked(path: String)

var _sources := PackedStringArray()
var _statuses: Dictionary = {}
var _results: Dictionary = {}
var _selected_source := ""
var _updating := false

var _selector: OptionButton
var _status: Label
var _empty_help: Label
var _actions: HBoxContainer
var _preview: PanelContainer
var _preview_name: Label
var _diagnostics: RichTextLabel
var _output_dialog: FileDialog
var _directory_dialog: FileDialog

func _ready() -> void:
    name = "Themosis"
    custom_minimum_size = Vector2(360, 0)
    _build_interface()
    set_theme_sources(_sources)

func set_theme_sources(sources: PackedStringArray) -> void:
    _sources = sources.duplicate()
    _sources.sort()
    if not _selected_source.is_empty() and not _sources.has(_selected_source):
        _selected_source = ""
    if _selected_source.is_empty() and not _sources.is_empty():
        _selected_source = _sources[0]
    if not is_node_ready():
        return
    _updating = true
    _selector.clear()
    for source in _sources:
        var status := str(_statuses.get(source, "imported"))
        _selector.add_item("%s — %s" % [source.get_file(), status])
        _selector.set_item_tooltip(_selector.item_count - 1, source)
        if source == _selected_source:
            _selector.select(_selector.item_count - 1)
    var configured := not _sources.is_empty()
    _selector.visible = configured
    _actions.visible = configured
    _preview.visible = configured
    _empty_help.visible = not configured
    _updating = false
    _show_selected_result()

func selected_source() -> String:
    return _selected_source

func mark_importing(source: String) -> void:
    _statuses[source] = "importing"
    _status.text = "Importing %s…" % source.get_file()
    _status.modulate = Color(0.8, 0.85, 1.0)
    set_theme_sources(_sources)

func mark_stale(source: String) -> void:
    _statuses[source] = "stale"
    set_theme_sources(_sources)

func show_result(source: String, result: Dictionary, operation := "import") -> void:
    _results[source] = result.duplicate(false)
    var succeeded := bool(result.get("ok", false))
    _statuses[source] = "up-to-date" if succeeded else "failed"
    if source == _selected_source:
        if succeeded:
            _status.text = "%s %s" % [
                "Imported" if operation == "import" else "Materialized",
                source.get_file(),
            ]
            _status.modulate = Color(0.55, 0.9, 0.65)
            var theme := result.get("theme") as Theme
            if theme != null:
                set_preview_theme(theme)
        else:
            _status.text = "%s failed" % ("Import" if operation == "import" else "Materialization")
            _status.modulate = Color(1.0, 0.45, 0.4)
        _render_diagnostics(result.get("diagnostics", []))
    set_theme_sources(_sources)

func show_message(message: String, failed := false) -> void:
    _status.text = message
    _status.modulate = Color(1.0, 0.45, 0.4) if failed else Color(0.55, 0.9, 0.65)

func set_preview_theme(theme: Theme) -> void:
    _preview.theme = theme
    _preview_name.text = (
        theme.resource_name if not theme.resource_name.is_empty() else _selected_source.get_file()
    )

func _build_interface() -> void:
    var title := Label.new()
    title.text = "Themosis themes"
    title.add_theme_font_size_override("font_size", 18)
    add_child(title)

    var help := Label.new()
    help.text = "Root .tms files import as native Godot Theme resources."
    help.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
    add_child(help)

    _status = Label.new()
    _status.text = "Discovering themes…"
    _status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
    add_child(_status)

    _empty_help = Label.new()
    _empty_help.text = "Add a root such as res://theme/light.tms. Shared KDL files remain ordinary .kdl modules."
    _empty_help.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
    add_child(_empty_help)

    _selector = OptionButton.new()
    _selector.size_flags_horizontal = Control.SIZE_EXPAND_FILL
    _selector.item_selected.connect(_select_source)
    add_child(_selector)

    _actions = HBoxContainer.new()
    add_child(_actions)
    var reimport := Button.new()
    reimport.text = "Reimport"
    reimport.tooltip_text = "Recompile the selected .tms asset"
    reimport.pressed.connect(_request_reimport)
    _actions.add_child(reimport)
    var reimport_all := Button.new()
    reimport_all.text = "Reimport all"
    reimport_all.pressed.connect(reimport_all_requested.emit)
    _actions.add_child(reimport_all)
    var materialize := Button.new()
    materialize.text = "Materialize…"
    materialize.tooltip_text = "Save a visible native .tres copy"
    materialize.pressed.connect(_choose_materialized_output)
    _actions.add_child(materialize)
    var materialize_all := Button.new()
    materialize_all.text = "All…"
    materialize_all.tooltip_text = "Materialize every theme into one directory"
    materialize_all.pressed.connect(_choose_materialized_directory)
    _actions.add_child(materialize_all)

    _preview = PanelContainer.new()
    _preview.custom_minimum_size = Vector2(0, 190)
    add_child(_preview)
    var margins := MarginContainer.new()
    margins.add_theme_constant_override("margin_left", 18)
    margins.add_theme_constant_override("margin_top", 16)
    margins.add_theme_constant_override("margin_right", 18)
    margins.add_theme_constant_override("margin_bottom", 16)
    _preview.add_child(margins)
    var stack := VBoxContainer.new()
    stack.add_theme_constant_override("separation", 10)
    margins.add_child(stack)
    _preview_name = Label.new()
    _preview_name.theme_type_variation = &"SectionTitle"
    _preview_name.text = "Theme preview"
    stack.add_child(_preview_name)
    var copy := Label.new()
    copy.text = "Imported themes are assignable directly from their .tms source path."
    copy.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
    stack.add_child(copy)
    var buttons := HBoxContainer.new()
    stack.add_child(buttons)
    var primary := Button.new()
    primary.theme_type_variation = &"PrimaryButton"
    primary.text = "Primary"
    buttons.add_child(primary)
    var secondary := Button.new()
    secondary.text = "Secondary"
    buttons.add_child(secondary)

    var diagnostics_header := HBoxContainer.new()
    add_child(diagnostics_header)
    var diagnostics_title := Label.new()
    diagnostics_title.text = "Diagnostics"
    diagnostics_title.size_flags_horizontal = Control.SIZE_EXPAND_FILL
    diagnostics_header.add_child(diagnostics_title)
    var clear := Button.new()
    clear.text = "Clear"
    clear.pressed.connect(func() -> void: _diagnostics.clear())
    diagnostics_header.add_child(clear)
    _diagnostics = RichTextLabel.new()
    _diagnostics.fit_content = true
    _diagnostics.custom_minimum_size = Vector2(0, 100)
    _diagnostics.meta_clicked.connect(
        func(meta: Variant) -> void: diagnostic_clicked.emit(str(meta))
    )
    add_child(_diagnostics)

    _output_dialog = FileDialog.new()
    _output_dialog.access = FileDialog.ACCESS_RESOURCES
    _output_dialog.file_mode = FileDialog.FILE_MODE_SAVE_FILE
    _output_dialog.filters = PackedStringArray(["*.tres ; Godot theme resource"])
    _output_dialog.file_selected.connect(_materialized_output_selected)
    add_child(_output_dialog)
    _directory_dialog = FileDialog.new()
    _directory_dialog.access = FileDialog.ACCESS_RESOURCES
    _directory_dialog.file_mode = FileDialog.FILE_MODE_OPEN_DIR
    _directory_dialog.dir_selected.connect(
        func(path: String) -> void: materialize_all_requested.emit(path)
    )
    add_child(_directory_dialog)

func _select_source(index: int) -> void:
    if _updating or index < 0 or index >= _sources.size():
        return
    _selected_source = _sources[index]
    _show_selected_result()
    selection_changed.emit(_selected_source)

func _show_selected_result() -> void:
    if _selected_source.is_empty():
        _status.text = "No Themosis theme assets found"
        _diagnostics.clear()
        return
    var result: Dictionary = _results.get(_selected_source, {})
    if result.is_empty():
        _status.text = "%s is ready" % _selected_source.get_file()
        _diagnostics.clear()
        return
    _render_diagnostics(result.get("diagnostics", []))
    if bool(result.get("ok", false)):
        var theme := result.get("theme") as Theme
        if theme != null:
            set_preview_theme(theme)

func _request_reimport() -> void:
    if not _selected_source.is_empty():
        reimport_requested.emit(_selected_source)

func _choose_materialized_output() -> void:
    if _selected_source.is_empty():
        return
    _output_dialog.current_path = (
        "res://theme/generated/%s.tres" % _selected_source.get_file().get_basename()
    )
    _output_dialog.popup_centered_ratio(0.75)

func _choose_materialized_directory() -> void:
    _directory_dialog.current_path = "res://theme/generated"
    _directory_dialog.popup_centered_ratio(0.75)

func _materialized_output_selected(output: String) -> void:
    materialize_requested.emit(_selected_source, output)

func _render_diagnostics(diagnostics: Array) -> void:
    _diagnostics.clear()
    for diagnostic in diagnostics:
        var path := str(diagnostic.get("path", ""))
        if not path.is_empty():
            _diagnostics.push_meta(path)
            _diagnostics.add_text(path)
            _diagnostics.pop()
            var line := int(diagnostic.get("line", -1))
            var column := int(diagnostic.get("column", -1))
            if line >= 0:
                _diagnostics.add_text(":%d" % line)
                if column >= 0:
                    _diagnostics.add_text(":%d" % column)
            _diagnostics.add_text(": ")
        var code := str(diagnostic.get("code", ""))
        if not code.is_empty():
            _diagnostics.add_text("[%s] " % code)
        _diagnostics.add_text(str(diagnostic.get("message", "error")) + "\n")
