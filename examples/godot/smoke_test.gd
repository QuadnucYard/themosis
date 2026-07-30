extends SceneTree

const DARK_CANVAS := Color(0.035, 0.047, 0.075, 1.0)
const DARK_PRIMARY := Color(0.18, 0.42, 0.95, 1.0)
const DARK_RAISED := Color(0.12, 0.145, 0.21, 1.0)
const LIGHT_CANVAS := Color(0.94, 0.96, 1.0, 1.0)
const LIGHT_PRIMARY := Color(0.12, 0.36, 0.88, 1.0)
const LIGHT_RAISED := Color(0.84, 0.88, 0.96, 1.0)

const FONT_PATH := "res://theme/assets/ui_font.tres"
const FOCUS_PATH := "res://theme/assets/focus_ring.tres"
const ICON_PATH := "res://theme/assets/chevron_down.svg"

func _initialize() -> void:
    var light := load("res://theme/light.tms") as Theme
    var dark := load("res://theme/dark.tms") as Theme
    if light == null or dark == null or light == dark:
        _fail("the .tms importer did not produce two independent Theme resources")
        return

    var scene := load("res://theme_switcher.tscn") as PackedScene
    if scene == null:
        _fail("the switching example scene did not load")
        return
    var application := scene.instantiate() as Control
    root.add_child.call_deferred(application)
    await application.ready

    var canvas := application.get_node("Canvas") as Panel
    var action := application.get_node("%ActionButton") as Button
    var secondary := application.get_node("%SecondaryAction") as Button
    var status := application.get_node("%ThemeStatus") as Label
    var light_button := application.get_node("%LightButton") as Button
    var dark_button := application.get_node("%DarkButton") as Button
    var gallery := application.get_node("%Gallery") as GridContainer
    var name_input := application.get_node("%NameInput") as LineEdit
    var read_only_input := application.get_node("%ReadOnlyInput") as LineEdit
    var density := application.get_node("%DensityOption") as OptionButton
    var include_states := application.get_node("%IncludeStates") as CheckBox
    var progress := application.get_node("%BuildProgress") as ProgressBar
    var card_margins := application.get_node(
        "Canvas/Margins/Content/GalleryScroll/Gallery/ButtonsCard/Margins"
    ) as MarginContainer
    var unsupported := application.get_node(
        "Canvas/Margins/Content/GalleryScroll/Gallery/FeedbackCard/Margins/Stack/UnsupportedValues"
    ) as Label

    var dark_canvas := canvas.get_theme_stylebox(&"panel") as StyleBoxFlat
    var dark_action := action.get_theme_stylebox(&"normal") as StyleBoxFlat
    var default_button := secondary.get_theme_stylebox(&"normal") as StyleBoxFlat
    var dark_input := name_input.get_theme_stylebox(&"normal") as StyleBoxFlat
    var dark_progress := progress.get_theme_stylebox(&"fill") as StyleBoxFlat
    var focus := action.get_theme_stylebox(&"focus")
    var font := action.get_theme_font(&"font")
    var arrow := density.get_theme_icon(&"arrow")
    if (
        application.theme != dark
        or dark_canvas == null
        or not dark_canvas.bg_color.is_equal_approx(DARK_CANVAS)
        or dark_action == null
        or not dark_action.bg_color.is_equal_approx(DARK_PRIMARY)
        or default_button == null
        or not default_button.bg_color.is_equal_approx(DARK_RAISED)
        or dark_input == null
        or not dark_input.bg_color.is_equal_approx(DARK_RAISED)
        or dark_progress == null
        or not dark_progress.bg_color.is_equal_approx(DARK_PRIMARY)
        or secondary.theme_type_variation != &""
        or action.get_theme_font_size(&"font_size") != 17
        or name_input.get_theme_font_size(&"font_size") != 16
        or progress.get_theme_font_size(&"font_size") != 14
        or include_states.get_theme_font_size(&"font_size") != 16
        or gallery.get_theme_constant(&"h_separation") != 18
        or gallery.get_theme_constant(&"v_separation") != 18
        or card_margins.get_theme_constant(&"margin_left") != 24
        or card_margins.get_theme_constant(&"margin_bottom") != 24
        or font == null
        or font.resource_path != FONT_PATH
        or focus == null
        or focus.resource_path != FOCUS_PATH
        or arrow == null
        or arrow.resource_path != ICON_PATH
        or dark_button.theme_type_variation != &"PrimaryButton"
        or light_button.theme_type_variation != &""
        or density.selected != 1
        or not include_states.button_pressed
        or read_only_input.text != "Imported from res://theme/dark.tms"
        or not unsupported.text.contains("Boolean and string")
        or not unsupported.text.contains("rem dimensions")
        or status.text != "Dark theme loaded from dark.tms"
    ):
        _fail("the imported dark theme did not apply the gallery's native item categories")
        return

    light_button.pressed.emit()
    await process_frame
    var light_canvas := canvas.get_theme_stylebox(&"panel") as StyleBoxFlat
    var light_action := action.get_theme_stylebox(&"normal") as StyleBoxFlat
    var light_input := name_input.get_theme_stylebox(&"normal") as StyleBoxFlat
    var light_progress := progress.get_theme_stylebox(&"fill") as StyleBoxFlat
    if (
        application.theme != light
        or light_canvas == null
        or not light_canvas.bg_color.is_equal_approx(LIGHT_CANVAS)
        or light_action == null
        or not light_action.bg_color.is_equal_approx(LIGHT_PRIMARY)
        or light_action.bg_color.is_equal_approx(dark_action.bg_color)
        or light_input == null
        or not light_input.bg_color.is_equal_approx(LIGHT_RAISED)
        or light_progress == null
        or not light_progress.bg_color.is_equal_approx(LIGHT_PRIMARY)
        or light_button.theme_type_variation != &"PrimaryButton"
        or dark_button.theme_type_variation != &""
        or read_only_input.text != "Imported from res://theme/light.tms"
        or status.text != "Light theme loaded from light.tms"
    ):
        _fail("switching to the imported light theme did not update the palette")
        return

    quit()

func _fail(message: String) -> void:
    push_error("Themosis smoke test: " + message)
    quit(1)
