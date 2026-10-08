extends SceneTree

# Fails unless the auto-registered editor classes came from the loaded
# GDExtension library. Run in an editor session, where the dock must exist.

func _initialize() -> void:
	var missing := []
	for name in ["ThemosisEditorPlugin", "ThemosisThemeImporter", "ThemosisThemeDock"]:
		if not ClassDB.class_exists(name):
			missing.append(name)
	if missing.is_empty():
		print("Gallery classes: passed")
		quit()
		return
	push_error("gallery: the GDExtension did not register %s" % [missing])
	quit(1)
