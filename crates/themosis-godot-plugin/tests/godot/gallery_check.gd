extends SceneTree

# Drives the demo control the way a user does and fails when the imported
# themes are missing or the switcher stops working. A run that never loaded the
# Themosis importer cannot reach the sentinel: the themes would not load.

func _initialize() -> void:
	root.add_child((load("res://theme_switcher.tscn") as PackedScene).instantiate())
	await process_frame
	await process_frame
	var scene: Control = root.get_child(root.get_child_count() - 1)
	var status: Label = scene.get_node("%ThemeStatus")
	var light: Button = scene.get_node("%LightButton")
	light.emit_signal("pressed")
	if scene.theme == null or scene.theme.resource_path != "res://theme/light.tms":
		push_error("gallery: the light theme was not applied")
		quit(1)
		return
	if not status.text.begins_with("Light theme loaded"):
		push_error("gallery: the theme status was not updated (%s)" % status.text)
		quit(1)
		return
	print("Gallery: passed")
	quit()
