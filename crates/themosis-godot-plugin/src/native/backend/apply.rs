//! Direct construction of native Godot themes from a portable build plan.
//!
//! `themosis-godot` normalizes a compiled theme into a portable [`GodotBuildPlan`]:
//! each item carries the native categories that could accept it, but only a running
//! engine can resolve them, because resolution depends on the engine's default
//! theme and its control-type inheritance chain.
//!
//! This module performs that resolution and mutation directly: it reads the
//! default theme through `[`ThemeDb`], walks each target's [`ClassDb`] chain, and
//! writes items onto a native [`Theme`]. Both the editor importer and the native
//! CLI runner (the native runner) construct themes through this one implementation.

use godot::{
    classes::{
        ClassDb, Font, ResourceLoader, StyleBox, StyleBoxFlat, Texture2D, Theme, ThemeDb,
        resource_loader::CacheMode,
    },
    obj::{NewGd, Singleton},
    prelude::*,
};
use themosis_core::Color as CoreColor;
use themosis_godot::{GodotBuildPlan, GodotItemKind, PlannedItem, PreparedValue};

use super::diagnostic::{NativeDiagnostic, NativeDiagnostics};

/// One item the running engine could not map onto its default theme.
#[derive(Debug)]
pub(super) struct ItemFailure {
    code: &'static str,
    message: String,
}

impl ItemFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Builds a native theme, or reports every item the running engine rejected.
///
/// Items are applied in plan order and failures do not stop later items, so one
/// invalid property does not hide the remaining diagnostics.
pub(super) fn build_native_theme(plan: &GodotBuildPlan) -> Result<Gd<Theme>, NativeDiagnostics> {
    let Some(default_theme) = ThemeDb::singleton().get_default_theme() else {
        return Err(NativeDiagnostics::new(vec![diagnostic(
            "missing_default_theme",
            "Godot did not provide its default control theme",
            "",
            "",
            "",
            "",
        )]));
    };
    let class_db = ClassDb::singleton();
    let mut theme = Theme::new_gd();
    let mut failures: Vec<NativeDiagnostic> = Vec::new();

    for style in plan.styles().values() {
        let style_name = style.name().as_str();
        let target_name = style.target().as_str();
        if !class_db.class_exists(target_name)
            || (target_name != "Control" && !class_db.is_parent_class(target_name, "Control"))
        {
            failures.push(diagnostic(
                "unknown_target",
                format!("style '{style_name}' targets '{target_name}', which is not a Godot Control class"),
                style_name,
                target_name,
                "",
                "",
            ));
            continue;
        }

        if style_name != target_name {
            theme.set_type_variation(style_name, target_name);
        }

        for item in style.items() {
            let outcome = resolve_kind(&default_theme, &class_db, target_name, style_name, item)
                .and_then(|kind| {
                    apply_item(
                        &mut theme,
                        &default_theme,
                        &class_db,
                        target_name,
                        style_name,
                        kind,
                        item,
                    )
                });
            if let Err(failure) = outcome {
                failures.push(diagnostic(
                    failure.code,
                    failure.message,
                    style_name,
                    target_name,
                    item.state().map_or("", |state| state.as_str()),
                    item.property().as_str(),
                ));
            }
        }
    }

    if failures.is_empty() {
        Ok(theme)
    } else {
        Err(NativeDiagnostics::new(failures))
    }
}

/// Resolves one item's native category against the running engine.
pub(super) fn resolve_kind(
    default_theme: &Gd<Theme>,
    class_db: &Gd<ClassDb>,
    target: &str,
    style_name: &str,
    item: &PlannedItem,
) -> Result<GodotItemKind, ItemFailure> {
    let property = item.property().as_str();
    let mut matches: Vec<GodotItemKind> = item
        .candidates()
        .iter()
        .copied()
        .filter(|kind| has_item(default_theme, class_db, target, property, *kind))
        .collect();

    // One reference can satisfy several categories, so narrow the match set by
    // the class the reference actually loads as.
    if matches.len() > 1
        && let PreparedValue::Resource(reference) = item.value()
        && let Some(resource) = load_fresh(reference.as_str())
    {
        matches.retain(|kind| resource_matches(&resource, *kind));
    }

    match matches.as_slice() {
        [kind] => Ok(*kind),
        [] => Err(ItemFailure::new(
            "unsupported_property",
            format!(
                "style '{style_name}' property '{property}' has no compatible {} item on target '{target}'",
                item.value_kind(),
            ),
        )),
        matches => Err(ItemFailure::new(
            "ambiguous_property",
            format!(
                "style '{style_name}' property '{property}' is ambiguous on target '{target}'; it matches {}",
                matches
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        )),
    }
}

/// Applies one resolved item to the theme under construction.
fn apply_item(
    theme: &mut Gd<Theme>,
    default_theme: &Gd<Theme>,
    class_db: &Gd<ClassDb>,
    target: &str,
    style_name: &str,
    kind: GodotItemKind,
    item: &PlannedItem,
) -> Result<(), ItemFailure> {
    let property = item.property().as_str();
    match (kind, item.value()) {
        (GodotItemKind::Color, PreparedValue::Color(color)) => {
            theme.set_color(property, style_name, to_godot_color(color));
        }
        (GodotItemKind::StyleBox, PreparedValue::Color(color)) => {
            let stylebox = colored_stylebox(
                default_theme,
                class_db,
                target,
                property,
                to_godot_color(color),
            )?;
            theme.set_stylebox(property, style_name, &stylebox);
        }
        (GodotItemKind::StyleBox, PreparedValue::Resource(reference)) => {
            let resource = load_typed(reference.as_str(), GodotItemKind::StyleBox)?;
            let stylebox = resource.try_cast::<StyleBox>().map_err(|_| {
                ItemFailure::new(
                    "resource_type",
                    format!(
                        "Godot resource '{}' must inherit StyleBox",
                        reference.as_str()
                    ),
                )
            })?;
            theme.set_stylebox(property, style_name, &stylebox);
        }
        (GodotItemKind::Constant, PreparedValue::Integer(integer)) => {
            theme.set_constant(property, style_name, *integer);
        }
        (GodotItemKind::FontSize, PreparedValue::Integer(integer)) => {
            if *integer <= 0 {
                return Err(ItemFailure::new(
                    "invalid_integer",
                    "font size must be a positive whole number of pixels",
                ));
            }
            theme.set_font_size(property, style_name, *integer);
        }
        (GodotItemKind::Font, PreparedValue::Resource(reference)) => {
            let resource = load_typed(reference.as_str(), GodotItemKind::Font)?;
            let font = resource.try_cast::<Font>().map_err(|_| {
                ItemFailure::new(
                    "resource_type",
                    format!("Godot resource '{}' must inherit Font", reference.as_str()),
                )
            })?;
            theme.set_font(property, style_name, &font);
        }
        (GodotItemKind::Icon, PreparedValue::Resource(reference)) => {
            let resource = load_typed(reference.as_str(), GodotItemKind::Icon)?;
            let texture = resource.try_cast::<Texture2D>().map_err(|_| {
                ItemFailure::new(
                    "resource_type",
                    format!(
                        "Godot resource '{}' must inherit Texture2D",
                        reference.as_str()
                    ),
                )
            })?;
            theme.set_icon(property, style_name, &texture);
        }
        (kind, _) => {
            return Err(ItemFailure::new(
                "unsupported_category",
                format!("unsupported Godot theme-item category '{}'", kind.as_str()),
            ));
        }
    }
    Ok(())
}

// Ignore the complete resource cache so a rebuild reads disk without mutating
// resources held by the previous successful theme, including external children.
fn load_fresh(path: &str) -> Option<Gd<Resource>> {
    ResourceLoader::singleton()
        .load_ex(path)
        .cache_mode(CacheMode::IGNORE_DEEP)
        .done()
}

/// Loads a resource reference and checks the class the category requires.
fn load_typed(path: &str, kind: GodotItemKind) -> Result<Gd<Resource>, ItemFailure> {
    let resource = load_fresh(path).ok_or_else(|| {
        ItemFailure::new(
            "missing_resource",
            format!("Godot resource '{path}' could not be loaded"),
        )
    })?;
    if !resource_matches(&resource, kind) {
        return Err(ItemFailure::new(
            "resource_type",
            format!(
                "Godot resource '{path}' must inherit {}",
                expected_class(kind)
            ),
        ));
    }
    Ok(resource)
}

/// Copies the engine's default stylebox and recolors it, mirroring the engine's
/// own theming behavior for color-backed stylebox items.
fn colored_stylebox(
    default_theme: &Gd<Theme>,
    class_db: &Gd<ClassDb>,
    target: &str,
    property: &str,
    color: Color,
) -> Result<Gd<StyleBoxFlat>, ItemFailure> {
    for theme_type in type_chain(class_db, target) {
        if !default_theme.has_stylebox(property, theme_type.as_str()) {
            continue;
        }
        let source = default_theme
            .get_stylebox(property, theme_type.as_str())
            .ok_or_else(|| {
                ItemFailure::new(
                    "missing_default_stylebox",
                    format!(
                        "Godot reports stylebox '{}' on '{}' but did not provide its default",
                        property, theme_type,
                    ),
                )
            })?;
        let flat = source.try_cast::<StyleBoxFlat>().map_err(|other| {
            ItemFailure::new(
                "incompatible_stylebox",
                format!(
                    "stylebox '{}' on '{}' uses {}; a color can only modify StyleBoxFlat",
                    property,
                    theme_type,
                    other.get_class(),
                ),
            )
        })?;
        let mut copy = flat.duplicate_resource_ex().deep_internal().done();
        copy.set_bg_color(color);
        return Ok(copy);
    }
    Err(ItemFailure::new(
        "missing_default_stylebox",
        format!(
            "Godot did not provide a default stylebox '{}' for target '{}'",
            property, target,
        ),
    ))
}

/// Returns the target's own type followed by its ancestors up to `Control`.
fn type_chain(class_db: &Gd<ClassDb>, target: &str) -> Vec<String> {
    let mut chain = vec![target.to_owned()];
    if target == "Control" {
        return chain;
    }
    let mut current = class_db.get_parent_class(target);
    while !current.is_empty() {
        let name = current.to_string();
        let control = name == "Control";
        chain.push(name);
        if control {
            break;
        }
        current = class_db.get_parent_class(&current);
    }
    chain
}

/// Returns whether the default theme defines an item on the target's chain.
fn has_item(
    default_theme: &Gd<Theme>,
    class_db: &Gd<ClassDb>,
    target: &str,
    property: &str,
    kind: GodotItemKind,
) -> bool {
    for theme_type in type_chain(class_db, target) {
        let found = match kind {
            GodotItemKind::Color => default_theme.has_color(property, theme_type.as_str()),
            GodotItemKind::Constant => default_theme.has_constant(property, theme_type.as_str()),
            // `has_font_size()` and `has_font()` fall back to the theme's default
            // font and report true for any name, so only the enumerated lists are
            // reliable membership tests.
            GodotItemKind::FontSize => default_theme
                .get_font_size_list(theme_type.as_str())
                .contains(property),
            GodotItemKind::Font => default_theme
                .get_font_list(theme_type.as_str())
                .contains(property),
            GodotItemKind::Icon => default_theme.has_icon(property, theme_type.as_str()),
            GodotItemKind::StyleBox => default_theme.has_stylebox(property, theme_type.as_str()),
        };
        if found {
            return true;
        }
    }
    false
}

fn resource_matches(resource: &Gd<Resource>, kind: GodotItemKind) -> bool {
    match kind {
        GodotItemKind::Font => resource.is_class("Font"),
        GodotItemKind::Icon => resource.is_class("Texture2D"),
        GodotItemKind::StyleBox => resource.is_class("StyleBox"),
        GodotItemKind::Color | GodotItemKind::Constant | GodotItemKind::FontSize => false,
    }
}

fn expected_class(kind: GodotItemKind) -> &'static str {
    match kind {
        GodotItemKind::Font => "Font",
        GodotItemKind::Icon => "Texture2D",
        GodotItemKind::StyleBox => "StyleBox",
        GodotItemKind::Color | GodotItemKind::Constant | GodotItemKind::FontSize => "Resource",
    }
}

fn to_godot_color(color: &CoreColor) -> Color {
    let [red, green, blue] = color.components();
    Color::from_rgba(
        red.get() as f32,
        green.get() as f32,
        blue.get() as f32,
        color.alpha().get() as f32,
    )
}

fn diagnostic(
    code: &str,
    message: impl Into<String>,
    style: &str,
    target: &str,
    state: &str,
    property: &str,
) -> NativeDiagnostic {
    NativeDiagnostic {
        code: code.to_owned(),
        message: message.into(),
        style: style.to_owned(),
        target: target.to_owned(),
        state: state.to_owned(),
        property: property.to_owned(),
    }
}
