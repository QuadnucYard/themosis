extends SceneTree

# Entry point for the fresh-resource check. The assertions live in the
# debug-only Rust hook, so no test data crosses the script boundary.
func _initialize() -> void:
    if ThemosisBackendTests.verify_fresh_resources():
        print("Fresh resources: passed")
        quit()
    else:
        push_error("fresh resources: edited .tres resources were not re-read")
        quit(1)
