extends Control

const THEMES := {
    &"light": preload("res://theme/light.tms"),
    &"dark": preload("res://theme/dark.tms"),
}

@onready var status: Label = %ThemeStatus

func _ready() -> void:
    %LightButton.pressed.connect(set_application_theme.bind(&"light"))
    %DarkButton.pressed.connect(set_application_theme.bind(&"dark"))
    %ActionButton.pressed.connect(func() -> void: status.text = "Primary action completed")
    set_application_theme(&"dark")

func set_application_theme(theme_name: StringName) -> void:
    var selected := THEMES.get(theme_name) as Theme
    if selected == null:
        push_error("Unknown Themosis theme: " + str(theme_name))
        return
    theme = selected
    %LightButton.theme_type_variation = &"PrimaryButton" if theme_name == &"light" else &""
    %DarkButton.theme_type_variation = &"PrimaryButton" if theme_name == &"dark" else &""
    %ReadOnlyInput.text = "Imported from res://theme/%s.tms" % theme_name
    status.text = "%s theme loaded from %s.tms" % [
        str(theme_name).capitalize(),
        str(theme_name),
    ]
