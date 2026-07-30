extends SceneTree

const ProfileStore := preload("res://addons/themosis/profile_store.gd")
const ThemeBuilder := preload("res://addons/themosis/theme_builder.gd")
const ThemeImporter := preload("res://addons/themosis/theme_importer.gd")

const LIGHT_SOURCE := "res://theme/light.tms"
const DARK_SOURCE := "res://theme/dark.tms"
const LIGHT_OUTPUT := "res://.themosis/tests/light.tres"
const DARK_OUTPUT := "res://.themosis/tests/dark.tres"
const VALIDATE_OUTPUT := "res://.themosis/tests/validate-only.tres"

func _initialize() -> void:
    _remove_test_outputs()

    var first_run := ProfileStore.load_config(
        "res://.themosis/tests/missing-profiles.json",
        false,
    )
    if not bool(first_run["ok"]) or bool(first_run["configured"]):
        _fail("a missing profile file must produce an unconfigured first-run state")
        return

    var migration := ProfileStore.migrate_legacy_values({
        "source": LIGHT_SOURCE,
        "output": LIGHT_OUTPUT,
        "auto_refresh": false,
    }, true)
    if (
        not bool(migration["migrated"])
        or str(migration["config"]["active_profile"]) != "default"
        or bool(migration["config"]["profiles"][0]["auto_refresh"])
        or str(migration["config"]["profiles"][0]["preview"]) != ProfileStore.PREVIEW_NONE
    ):
        _fail("legacy settings were not migrated into a safe materialization profile")
        return

    var light := ProfileStore.new_profile("light", LIGHT_SOURCE, LIGHT_OUTPUT)
    var dark := ProfileStore.new_profile("dark", DARK_SOURCE, DARK_OUTPUT)
    var collision := _config([light, dark], "light")
    collision["profiles"][1]["output"] = LIGHT_OUTPUT
    if bool(ProfileStore.validate_config(collision)["ok"]):
        _fail("profiles with the same output must be rejected")
        return

    if (
        ProfileStore.validate_source_path("res://theme/../light.tms").is_empty()
        or ProfileStore.validate_source_path("res://theme\\light.tms").is_empty()
        or ProfileStore.validate_output_path("res://.themosis/../escape.tres").is_empty()
        or ProfileStore.validate_output_directory("res://theme/./generated").is_empty()
    ):
        _fail("resource paths containing escape or ambiguous segments were accepted")
        return

    var escaped := ThemeBuilder.materialize_source(
        LIGHT_SOURCE,
        "res://.themosis/tests/../escape.tres",
    )
    if bool(escaped["ok"]):
        _fail("materialization accepted an output that escapes its selected directory")
        return

    var invalid_root: Dictionary = ThemosisThemeGenerator.new().generate_result(
        "res://theme/../light.tms"
    )
    if (
        bool(invalid_root["ok"])
        or invalid_root.get("diagnostics", []).is_empty()
        or str(invalid_root["diagnostics"][0].get("code", "")).is_empty()
    ):
        _fail("the extension did not return a structured invalid-root diagnostic")
        return

    var result := ThemeBuilder.build_all(_config([light, dark], "light"))
    if (
        not bool(result["ok"])
        or result["results"].size() != 2
        or not FileAccess.file_exists(LIGHT_OUTPUT)
        or not FileAccess.file_exists(DARK_OUTPUT)
        or FileAccess.get_md5(LIGHT_OUTPUT) == FileAccess.get_md5(DARK_OUTPUT)
    ):
        _fail("two enabled profiles did not materialize independent outputs: %s" % result)
        return

    var light_dependencies: PackedStringArray = result["results"][0]["dependencies"]
    var dark_dependencies: PackedStringArray = result["results"][1]["dependencies"]
    var shared := PackedStringArray([
        "res://theme/tokens/common.tokens.json",
        "res://theme/styles/surfaces.kdl",
        "res://theme/styles/typography.kdl",
        "res://theme/styles/buttons.kdl",
        "res://theme/styles/layout.kdl",
        "res://theme/styles/inputs.kdl",
        "res://theme/styles/feedback.kdl",
        "res://theme/assets/ui_font.tres",
        "res://theme/assets/focus_ring.tres",
        "res://theme/assets/chevron_down.svg",
    ])
    for dependency in shared:
        if not light_dependencies.has(dependency) or not dark_dependencies.has(dependency):
            _fail("both roots did not report shared dependency '%s'" % dependency)
            return
    if (
        not light_dependencies.has(LIGHT_SOURCE)
        or not light_dependencies.has("res://theme/tokens/light.tokens.json")
        or light_dependencies.has("res://theme/tokens/dark.tokens.json")
        or not dark_dependencies.has(DARK_SOURCE)
        or not dark_dependencies.has("res://theme/tokens/dark.tokens.json")
        or dark_dependencies.has("res://theme/tokens/light.tokens.json")
    ):
        _fail("materialization did not report an exact per-root dependency graph")
        return

    if (
        ThemeImporter.dependency_fingerprint(light_dependencies)
        == ThemeImporter.dependency_fingerprint(dark_dependencies)
    ):
        _fail("independent roots received the same dependency fingerprint")
        return

    var previous_md5 := FileAccess.get_md5(LIGHT_OUTPUT)
    var broken := light.duplicate(true)
    broken["source"] = "res://theme/missing.tms"
    var failure := ThemeBuilder.build_profile(broken)
    if (
        bool(failure["ok"])
        or not bool(failure["previous_output_remains_valid"])
        or FileAccess.get_md5(LIGHT_OUTPUT) != previous_md5
        or not (failure["dependencies"] as PackedStringArray).has("res://theme/missing.tms")
    ):
        _fail("a failed materialization did not preserve the last valid output")
        return

    var validate_only := light.duplicate(true)
    validate_only["name"] = "validate_only"
    validate_only["output"] = VALIDATE_OUTPUT
    var validation := ThemeBuilder.validate_profile(validate_only)
    if not bool(validation["ok"]) or FileAccess.file_exists(VALIDATE_OUTPUT):
        _fail("validation must compile without saving an output")
        return

    _remove_test_outputs()
    quit()

func _config(profiles: Array, active: String) -> Dictionary:
    return {
        "version": ProfileStore.VERSION,
        "active_profile": active,
        "profiles": profiles.duplicate(true),
    }

func _remove_test_outputs() -> void:
    for path in [LIGHT_OUTPUT, DARK_OUTPUT, VALIDATE_OUTPUT]:
        if FileAccess.file_exists(path):
            DirAccess.remove_absolute(ProjectSettings.globalize_path(path))

func _fail(message: String) -> void:
    _remove_test_outputs()
    push_error("Themosis profile test: " + message)
    quit(1)
