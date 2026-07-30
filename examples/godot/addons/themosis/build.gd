extends SceneTree

const ThemeBuilder := preload("res://addons/themosis/theme_builder.gd")

func _initialize() -> void:
    var selection := _parse_selection(OS.get_cmdline_user_args())
    if not bool(selection["ok"]):
        printerr("Themosis: " + str(selection["error"]))
        _print_usage()
        quit(2)
        return
    var loaded := ThemeBuilder.load_profiles()
    if not bool(loaded["ok"]):
        printerr("Themosis: " + str(loaded["error"]))
        quit(1)
        return
    if not bool(loaded["configured"]):
        printerr("Themosis: no profiles are configured in " + str(loaded["path"]))
        quit(1)
        return
    var config: Dictionary = loaded["config"]
    var result: Dictionary
    if bool(selection["all"]):
        result = ThemeBuilder.build_all(config)
    else:
        var profile_result := ThemeBuilder.build_named(config, str(selection["profile"]))
        result = {
            "ok": bool(profile_result["ok"]),
            "results": [profile_result],
            "outputs": PackedStringArray(
                [str(profile_result["output"])] if bool(profile_result["ok"]) else []
            ),
        }
    for profile_result in result["results"]:
        if bool(profile_result["ok"]):
            print(
                "Themosis[%s]: compiled %s -> %s" % [
                    profile_result["profile"],
                    profile_result["source"],
                    profile_result["output"],
                ]
            )
        else:
            printerr(
                "Themosis[%s]: %s" % [
                    profile_result["profile"],
                    profile_result["error"],
                ]
            )
    for output in result["outputs"]:
        print("Themosis: generated " + str(output))
    quit(0 if bool(result["ok"]) else 1)

func _parse_selection(arguments: PackedStringArray) -> Dictionary:
    if arguments.size() == 1 and arguments[0] == "--all":
        return {"ok": true, "error": "", "all": true, "profile": ""}
    if arguments.size() == 2 and arguments[0] == "--profile" and not arguments[1].is_empty():
        return {"ok": true, "error": "", "all": false, "profile": arguments[1]}
    return {
        "ok": false,
        "error": "select exactly one profile with --profile NAME or use --all",
        "all": false,
        "profile": "",
    }

func _print_usage() -> void:
    printerr(
        "usage: godot --headless --path . --script "res://addons/themosis/build.gd -- --profile NAME"
    )
    printerr(
        "   or: godot --headless --path . --script res://addons/themosis/build.gd -- --all"
    )
