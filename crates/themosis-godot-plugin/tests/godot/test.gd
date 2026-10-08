extends SceneTree

func _initialize() -> void:
    if (
        ThemosisBackendTests.verify_native_mappings()
        and ThemosisBackendTests.verify_invalid_combinations_are_rejected()
        and ThemosisBackendTests.verify_uid_dependency_fingerprints()
    ):
        quit()
    else:
        push_error("native Themosis theme mappings did not match")
        quit(1)
