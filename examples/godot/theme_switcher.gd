## Demo control switching the gallery between the imported theme assets.
##
## The addon owns import, materialization, and validation; the demo only
## consumes the imported `Theme` resources, so it stays plain GDScript.
extends Control

const THEMES := {
	&"light": "res://theme/light.tms",
	&"dark": "res://theme/dark.tms",
}

@onready var _light_button: Button = %LightButton
@onready var _dark_button: Button = %DarkButton
@onready var _action_button: Button = %ActionButton
@onready var _read_only_input: LineEdit = %ReadOnlyInput
@onready var _status: Label = %ThemeStatus


func _ready() -> void:
	_light_button.pressed.connect(_apply.bind(&"light"))
	_dark_button.pressed.connect(_apply.bind(&"dark"))
	_action_button.pressed.connect(_on_action_pressed)
	_apply(&"dark")


## Applies the imported theme named by `name`.
func _apply(name: StringName) -> void:
	var imported := load(THEMES[name]) as Theme
	if imported == null:
		push_error("Unknown Themosis theme: %s" % name)
		return
	theme = imported
	var light := name == &"light"
	_light_button.theme_type_variation = &"PrimaryButton" if light else &""
	_dark_button.theme_type_variation = &"" if light else &"PrimaryButton"
	_read_only_input.text = "Imported from %s" % THEMES[name]
	_status.text = "%s theme loaded from %s.tms" % [String(name).capitalize(), name]


func _on_action_pressed() -> void:
	_status.text = "Primary action completed"
