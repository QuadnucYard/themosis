extends RefCounted

const PLAN_SCHEMA_VERSION := 2


func build_plan(plan: Dictionary) -> Dictionary:
	var diagnostics: Array = []
	var schema: Dictionary = _plan_integer(plan.get("schema_version"))
	if not bool(schema.get("ok", false)) or int(schema.get("integer", 0)) != PLAN_SCHEMA_VERSION:
		diagnostics.append(_diagnostic(
			"unsupported_plan",
			"unsupported Themosis Godot build-plan schema",
			"",
			"",
			"",
			"",
		))
		return _build_failure(diagnostics)
	var theme_name_value: Variant = plan.get("theme")
	if not _valid_name(theme_name_value):
		diagnostics.append(_diagnostic(
			"invalid_plan",
			"build plan theme must be a non-empty string without surrounding whitespace",
			"",
			"",
			"",
			"",
		))
		return _build_failure(diagnostics)
	var styles_value: Variant = plan.get("styles")
	if styles_value is not Array:
		diagnostics.append(_diagnostic(
			"invalid_plan",
			"build plan styles must be an array",
			"",
			"",
			"",
			"",
		))
		return _build_failure(diagnostics)
	var default_theme: Theme = ThemeDB.get_default_theme()
	if default_theme == null:
		diagnostics.append(_diagnostic(
			"missing_default_theme",
			"Godot did not provide its default control theme",
			"",
			"",
			"",
			"",
		))
		return _build_failure(diagnostics)

	var theme := Theme.new()
	var style_names: Dictionary = {}
	var styles: Array = styles_value
	for style_value: Variant in styles:
		if style_value is not Dictionary:
			diagnostics.append(_diagnostic(
				"invalid_plan",
				"theme style is not an object",
				"",
				"",
				"",
				"",
			))
			continue
		var style: Dictionary = style_value
		var style_name_value: Variant = style.get("name")
		var target_value: Variant = style.get("target")
		var style_name := str(style_name_value) if style_name_value is String else ""
		var target := str(target_value) if target_value is String else ""
		if not _valid_name(style_name_value):
			diagnostics.append(_diagnostic(
				"invalid_plan",
				"theme style name must be a non-empty string without surrounding whitespace",
				style_name,
				target,
				"",
				"",
			))
			continue
		if style_names.has(style_name):
			diagnostics.append(_diagnostic(
				"invalid_plan",
				"theme contains duplicate style '%s'" % style_name,
				style_name,
				target,
				"",
				"",
			))
			continue
		style_names[style_name] = true
		if not _valid_name(target_value):
			diagnostics.append(_diagnostic(
				"invalid_plan",
				"style '%s' target must be a non-empty string without surrounding whitespace" % style_name,
				style_name,
				target,
				"",
				"",
			))
			continue
		var items_value: Variant = style.get("items")
		if items_value is not Array:
			diagnostics.append(_diagnostic(
				"invalid_plan",
				"style '%s' items must be an array" % style_name,
				style_name,
				target,
				"",
				"",
			))
			continue
		var target_name := StringName(target)
		if not ClassDB.class_exists(target_name) or (
			target_name != &"Control"
			and not ClassDB.is_parent_class(target_name, &"Control")
		):
			diagnostics.append(_diagnostic(
				"unknown_target",
				"style '%s' targets '%s', which is not a Godot Control class" % [
					style_name,
					target,
				],
				style_name,
				target,
				"",
				"",
			))
			continue
		var theme_type := StringName(style_name)
		if theme_type != target_name:
			theme.set_type_variation(theme_type, target_name)
		var items: Array = items_value
		for item_value: Variant in items:
			if item_value is not Dictionary:
				diagnostics.append(_diagnostic(
					"invalid_plan",
					"style '%s' contains a theme item that is not an object" % style_name,
					style_name,
					target,
					"",
					"",
				))
				continue
			var item: Dictionary = item_value
			var property_value: Variant = item.get("property")
			var property := str(property_value) if property_value is String else ""
			var state_value: Variant = item.get("state")
			var state := str(state_value) if state_value is String else ""
			var validation: Dictionary = _validate_item(item)
			if not bool(validation.get("ok", false)):
				diagnostics.append(_diagnostic(
					str(validation.get("code", "invalid_plan")),
					str(validation.get("message", "theme item is invalid")),
					style_name,
					target,
					state,
					property,
				))
				continue
			var resolved: Dictionary = _resolve_kind(default_theme, target, property, item)
			if not bool(resolved.get("ok", false)):
				var matches: Array = resolved.get("matches", [])
				var value_kind := str(item.get("value_kind", "value"))
				var code := "unsupported_property" if matches.is_empty() else "ambiguous_property"
				var message := (
					"style '%s' property '%s' has no compatible %s item on target '%s'" % [
						style_name,
						property,
						value_kind,
						target,
					]
					if matches.is_empty()
					else "style '%s' property '%s' is ambiguous on target '%s'; it matches %s" % [
						style_name,
						property,
						target,
						", ".join(matches),
					]
				)
				diagnostics.append(_diagnostic(
					code,
					message,
					style_name,
					target,
					state,
					property,
				))
				continue
			var application: Dictionary = _apply_item(
				theme,
				default_theme,
				theme_type,
				target,
				str(resolved["kind"]),
				item,
			)
			if not bool(application.get("ok", false)):
				diagnostics.append(_diagnostic(
					str(application.get("code", "invalid_item")),
					str(application.get("message", "could not apply theme item")),
					style_name,
					target,
					state,
					property,
				))

	if not diagnostics.is_empty():
		return _build_failure(diagnostics)
	return {
		"ok": true,
		"theme": theme,
		"diagnostics": diagnostics,
	}


static func _validate_item(item: Dictionary) -> Dictionary:
	var property_value: Variant = item.get("property")
	if not _valid_name(property_value):
		return _item_failure(
			"invalid_plan",
			"theme item property must be a non-empty string without surrounding whitespace",
		)
	var state_value: Variant = item.get("state")
	if state_value != null and not _valid_name(state_value):
		return _item_failure(
			"invalid_plan",
			"theme item state must be null or a non-empty string without surrounding whitespace",
		)
	var value_kind: Variant = item.get("value_kind")
	if not _valid_name(value_kind):
		return _item_failure("invalid_plan", "theme item value_kind must be a non-empty string")
	var candidates_value: Variant = item.get("candidates")
	if candidates_value is not Array or candidates_value.is_empty():
		return _item_failure("invalid_plan", "theme item candidates must be a non-empty array")
	var candidates: Array = candidates_value
	var seen: Dictionary = {}
	for candidate_value: Variant in candidates:
		if candidate_value is not String or not _supported_category(str(candidate_value)):
			return _item_failure(
				"invalid_plan",
				"theme item contains an unsupported candidate category",
			)
		var candidate := str(candidate_value)
		if seen.has(candidate):
			return _item_failure(
				"invalid_plan",
				"theme item contains duplicate candidate category '%s'" % candidate,
			)
		seen[candidate] = true
	var value_value: Variant = item.get("value")
	if value_value is not Dictionary:
		return _item_failure("invalid_plan", "theme item value must be an object")
	var value: Dictionary = value_value
	var kind_value: Variant = value.get("kind")
	if kind_value is not String:
		return _item_failure("invalid_plan", "theme item value kind must be a string")
	var kind := str(kind_value)
	for candidate_value: Variant in candidates:
		var candidate := str(candidate_value)
		if not _category_accepts(candidate, kind):
			return _item_failure(
				"invalid_plan",
				"candidate category '%s' is incompatible with %s value" % [candidate, kind],
			)
	match kind:
		"color":
			var color: Dictionary = _plan_color(value)
			if not bool(color.get("ok", false)):
				return color
		"integer":
			var integer: Dictionary = _plan_integer(value.get("value"))
			if not bool(integer.get("ok", false)):
				return integer
		"resource":
			var path_value: Variant = value.get("path")
			if path_value is not String:
				return _item_failure("invalid_plan", "resource value path must be a string")
			var path := str(path_value)
			if not (
				(path.begins_with("res://") and path.length() > "res://".length())
				or (path.begins_with("uid://") and path.length() > "uid://".length())
			):
				return _item_failure(
					"invalid_plan",
					"resource value must be a non-empty res:// or uid:// reference",
				)
		_:
			return _item_failure(
				"invalid_plan",
				"unsupported theme item value kind '%s'" % kind,
			)
	return {"ok": true}


static func _supported_category(category: String) -> bool:
	return category in ["color", "constant", "font_size", "font", "icon", "stylebox"]


static func _category_accepts(category: String, value_kind: String) -> bool:
	match value_kind:
		"color":
			return category == "color" or category == "stylebox"
		"integer":
			return category == "constant" or category == "font_size"
		"resource":
			return category == "font" or category == "icon" or category == "stylebox"
	return false


static func _resolve_kind(
	default_theme: Theme,
	target: String,
	property: String,
	item: Dictionary,
) -> Dictionary:
	var matches: Array[String] = []
	var candidates: Array = item.get("candidates", [])
	for candidate_value: Variant in candidates:
		var candidate := str(candidate_value)
		if _has_item(default_theme, target, property, candidate):
			matches.append(candidate)
	if matches.size() > 1:
		var value: Dictionary = item.get("value", {})
		if str(value.get("kind", "")) == "resource":
			var resource: Resource = ResourceLoader.load(str(value.get("path", "")))
			if resource != null:
				var compatible: Array[String] = []
				for candidate: String in matches:
					if _resource_matches(resource, candidate):
						compatible.append(candidate)
				matches = compatible
	return {
		"ok": matches.size() == 1,
		"kind": matches[0] if matches.size() == 1 else "",
		"matches": matches,
	}


static func _has_item(
	default_theme: Theme,
	target: String,
	property: String,
	kind: String,
) -> bool:
	var item := StringName(property)
	for theme_type: StringName in _theme_type_chain(StringName(target)):
		match kind:
			"color":
				if default_theme.has_color(item, theme_type):
					return true
			"constant":
				if default_theme.has_constant(item, theme_type):
					return true
			"font_size":
				if default_theme.has_font_size(item, theme_type):
					return true
			"font":
				if default_theme.has_font(item, theme_type):
					return true
			"icon":
				if default_theme.has_icon(item, theme_type):
					return true
			"stylebox":
				if default_theme.has_stylebox(item, theme_type):
					return true
	return false


static func _apply_item(
	theme: Theme,
	default_theme: Theme,
	variation: StringName,
	target: String,
	kind: String,
	item: Dictionary,
) -> Dictionary:
	var property := StringName(str(item.get("property", "")))
	var value: Dictionary = item.get("value", {})
	match kind:
		"color":
			var converted: Dictionary = _plan_color(value)
			if not bool(converted.get("ok", false)):
				return converted
			var color: Color = converted["color"]
			theme.set_color(property, variation, color)
		"stylebox":
			if str(value.get("kind", "")) == "color":
				var converted: Dictionary = _plan_color(value)
				if not bool(converted.get("ok", false)):
					return converted
				var color: Color = converted["color"]
				var stylebox: Dictionary = _colored_stylebox(
					default_theme,
					target,
					property,
					color,
				)
				if not bool(stylebox.get("ok", false)):
					return stylebox
				theme.set_stylebox(property, variation, stylebox["resource"] as StyleBox)
			else:
				var loaded: Dictionary = _load_typed_resource(value, "stylebox")
				if not bool(loaded.get("ok", false)):
					return loaded
				theme.set_stylebox(property, variation, loaded["resource"] as StyleBox)
		"constant":
			var converted: Dictionary = _plan_integer(value.get("value"))
			if not bool(converted.get("ok", false)):
				return converted
			theme.set_constant(property, variation, int(converted["integer"]))
		"font_size":
			var converted: Dictionary = _plan_integer(value.get("value"))
			if not bool(converted.get("ok", false)):
				return converted
			var size := int(converted["integer"])
			if size <= 0:
				return _item_failure(
					"invalid_integer",
					"font size must be a positive whole number of pixels",
				)
			theme.set_font_size(property, variation, size)
		"font":
			var loaded: Dictionary = _load_typed_resource(value, "font")
			if not bool(loaded.get("ok", false)):
				return loaded
			theme.set_font(property, variation, loaded["resource"] as Font)
		"icon":
			var loaded: Dictionary = _load_typed_resource(value, "icon")
			if not bool(loaded.get("ok", false)):
				return loaded
			theme.set_icon(property, variation, loaded["resource"] as Texture2D)
		_:
			return _item_failure(
				"unsupported_category",
				"unsupported Godot theme-item category '%s'" % kind,
			)
	return {"ok": true}


static func _plan_color(value: Dictionary) -> Dictionary:
	if str(value.get("kind", "")) != "color":
		return _item_failure("invalid_color", "theme color value must have kind 'color'")
	var rgba_value: Variant = value.get("rgba")
	if rgba_value is not Array or rgba_value.size() != 4:
		return _item_failure("invalid_color", "theme color must contain four RGBA components")
	var rgba: Array = rgba_value
	var components: Array[float] = []
	for component: Variant in rgba:
		if typeof(component) != TYPE_INT and typeof(component) != TYPE_FLOAT:
			return _item_failure("invalid_color", "theme color components must be numbers")
		var numeric := float(component)
		if not is_finite(numeric) or numeric < 0.0 or numeric > 1.0:
			return _item_failure(
				"invalid_color",
				"theme color components must be finite numbers from 0 through 1",
			)
		components.append(numeric)
	return {"ok": true, "color": Color(components[0], components[1], components[2], components[3])}


static func _plan_integer(raw: Variant) -> Dictionary:
	if typeof(raw) != TYPE_INT and typeof(raw) != TYPE_FLOAT:
		return _item_failure("invalid_integer", "theme integer value must be a number")
	var numeric := float(raw)
	if (
		not is_finite(numeric)
		or numeric != floor(numeric)
		or numeric < -2147483648.0
		or numeric > 2147483647.0
	):
		return _item_failure(
			"invalid_integer",
			"theme integer value must be a whole number in Godot's signed 32-bit range",
		)
	return {"ok": true, "integer": int(numeric)}


static func _load_typed_resource(value: Dictionary, kind: String) -> Dictionary:
	if str(value.get("kind", "")) != "resource":
		return _item_failure("resource_type", "theme item requires a Godot resource reference")
	var path := str(value.get("path", ""))
	var resource: Resource = ResourceLoader.load(path)
	if resource == null:
		return _item_failure("missing_resource", "Godot resource '%s' could not be loaded" % path)
	if not _resource_matches(resource, kind):
		var expected := str({
			"font": "Font",
			"icon": "Texture2D",
			"stylebox": "StyleBox",
		}.get(kind, "supported resource"))
		return _item_failure(
			"resource_type",
			"Godot resource '%s' must inherit %s" % [path, expected],
		)
	return {"ok": true, "resource": resource}


static func _resource_matches(resource: Resource, kind: String) -> bool:
	match kind:
		"font":
			return resource is Font
		"icon":
			return resource is Texture2D
		"stylebox":
			return resource is StyleBox
	return false


static func _colored_stylebox(
	default_theme: Theme,
	target: String,
	item: StringName,
	color: Color,
) -> Dictionary:
	for theme_type: StringName in _theme_type_chain(StringName(target)):
		if not default_theme.has_stylebox(item, theme_type):
			continue
		var source: StyleBox = default_theme.get_stylebox(item, theme_type)
		if source == null:
			return _item_failure(
				"missing_default_stylebox",
				"Godot reports stylebox '%s' on '%s' but did not provide its default" % [
					item,
					theme_type,
				],
			)
		if source is not StyleBoxFlat:
			return _item_failure(
				"incompatible_stylebox",
				"stylebox '%s' on '%s' uses %s; a color can only modify StyleBoxFlat" % [
					item,
					theme_type,
					source.get_class(),
				],
			)
		var duplicated: Resource = source.duplicate(true)
		if duplicated is not StyleBoxFlat:
			return _item_failure(
				"incompatible_stylebox",
				"Godot could not duplicate stylebox '%s' on '%s' as StyleBoxFlat" % [
					item,
					theme_type,
				],
			)
		var stylebox: StyleBoxFlat = duplicated as StyleBoxFlat
		stylebox.bg_color = color
		return {"ok": true, "resource": stylebox}
	return _item_failure(
		"missing_default_stylebox",
		"Godot did not provide a default stylebox '%s' for target '%s'" % [item, target],
	)


static func _theme_type_chain(target: StringName) -> Array[StringName]:
	var chain: Array[StringName] = []
	var current := target
	while not current.is_empty():
		chain.append(current)
		if current == &"Control":
			break
		current = ClassDB.get_parent_class(current)
	return chain


static func _valid_name(value: Variant) -> bool:
	return value is String and not value.is_empty() and value.strip_edges() == value


static func _build_failure(diagnostics: Array) -> Dictionary:
	return {
		"ok": false,
		"theme": null,
		"diagnostics": diagnostics,
	}


static func _item_failure(code: String, message: String) -> Dictionary:
	return {"ok": false, "code": code, "message": message}


static func _diagnostic(
	code: String,
	message: String,
	style: String,
	target: String,
	state: String,
	property: String,
) -> Dictionary:
	return {
		"severity": "error",
		"code": code,
		"message": message,
		"style": style,
		"target": target,
		"state": state,
		"property": property,
	}
