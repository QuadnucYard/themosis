use std::collections::BTreeMap;

use godot::{
    builtin::{Color as GodotColor, Corner},
    classes::{FileAccess, StyleBoxFlat, Theme, file_access::ModeFlags},
    prelude::*,
};
use themosis_core::{
    Color, CompiledState, CompiledStyle, CompiledTheme, CompiledValue, Dimension, DimensionUnit,
    Name, Number, ResolvedTokens, ResourceRef,
};

use crate::native::{
    backend::{ThemeBuildError, build_theme},
    generation::generate_from_project_path,
};

#[derive(GodotClass)]
#[class(init, base=RefCounted)]
struct ThemosisBackendTests;

#[godot_api]
impl ThemosisBackendTests {
    #[func]
    fn verify_native_mappings() -> bool {
        fn to_godot_color(color: Color) -> GodotColor {
            let [red, green, blue] = color.components();
            GodotColor::from_rgba(
                red.get() as f32,
                green.get() as f32,
                blue.get() as f32,
                color.alpha().get() as f32,
            )
        }

        let style_name = Name::new("ProbeButton").expect("probe name is valid");
        let normal = Name::new("normal").expect("probe name is valid");
        let hover = Name::new("hover").expect("probe name is valid");
        let font_color = Name::new("font_color").expect("probe name is valid");
        let font_size = Name::new("font_size").expect("probe name is valid");
        let font = Name::new("font").expect("probe name is valid");
        let focus = Name::new("focus").expect("probe name is valid");
        let hover_name = Name::new("hover").expect("probe name is valid");
        let base_background = Color::new([0.1, 0.2, 0.3], 1.0).expect("probe color is valid");
        let hover_background = Color::new([0.3, 0.4, 0.5], 1.0).expect("probe color is valid");
        let text_color = Color::new([0.9, 0.8, 0.7], 1.0).expect("probe color is valid");
        let size = Number::new(18.0).expect("probe number is valid");
        let properties = BTreeMap::from([
            (normal, CompiledValue::Color(base_background)),
            (font_color.clone(), CompiledValue::Color(text_color)),
            (font_size.clone(), CompiledValue::Number(size)),
            (
                font,
                CompiledValue::Resource(
                    ResourceRef::new("res://probe_font.tres").expect("probe reference is valid"),
                ),
            ),
            (
                focus,
                CompiledValue::Resource(
                    ResourceRef::new("res://probe_stylebox.tres")
                        .expect("probe reference is valid"),
                ),
            ),
        ]);
        let mut hover_properties = properties.clone();
        hover_properties.insert(hover, CompiledValue::Color(hover_background));
        let states = BTreeMap::from([(
            hover_name.clone(),
            CompiledState::new(hover_name, hover_properties),
        )]);
        let style = CompiledStyle::new(
            style_name.clone(),
            Name::new("Button").expect("probe target is valid"),
            properties,
            states,
        );
        let stack_name = Name::new("ProbeStack").expect("probe name is valid");
        let separation = Name::new("separation").expect("probe name is valid");
        let stack = CompiledStyle::new(
            stack_name.clone(),
            Name::new("VBoxContainer").expect("probe target is valid"),
            BTreeMap::from([(
                separation,
                CompiledValue::Dimension(
                    Dimension::new(12.0, DimensionUnit::Pixel).expect("probe dimension is valid"),
                ),
            )]),
            BTreeMap::new(),
        );
        let option_name = Name::new("ProbeOption").expect("probe name is valid");
        let option = CompiledStyle::new(
            option_name.clone(),
            Name::new("OptionButton").expect("probe target is valid"),
            BTreeMap::from([(
                Name::new("arrow").expect("probe property is valid"),
                CompiledValue::Resource(
                    ResourceRef::new("res://probe_icon.tres").expect("probe reference is valid"),
                ),
            )]),
            BTreeMap::new(),
        );
        let label_name = Name::new("Label").expect("probe name is valid");
        let label = CompiledStyle::new(
            label_name.clone(),
            label_name.clone(),
            BTreeMap::from([(font_color, CompiledValue::Color(text_color))]),
            BTreeMap::new(),
        );
        let compiled = CompiledTheme::new(
            Name::new("Probe").expect("probe name is valid"),
            ResolvedTokens::new([]),
            BTreeMap::from([
                (style_name, style),
                (stack_name.clone(), stack),
                (option_name.clone(), option),
                (label_name, label),
            ]),
        );
        let Ok(theme) = build_theme(&compiled) else {
            return false;
        };
        let Some(normal) = theme.get_stylebox("normal", "ProbeButton") else {
            return false;
        };
        let Some(hover) = theme.get_stylebox("hover", "ProbeButton") else {
            return false;
        };
        let Ok(normal) = normal.try_cast::<StyleBoxFlat>() else {
            return false;
        };
        let Ok(hover) = hover.try_cast::<StyleBoxFlat>() else {
            return false;
        };
        let Some(focus) = theme.get_stylebox("focus", "ProbeButton") else {
            return false;
        };
        let Ok(focus) = focus.try_cast::<StyleBoxFlat>() else {
            return false;
        };

        theme.get_type_variation_base("ProbeButton") == "Button"
            && theme.get_type_variation_base("Label").is_empty()
            && theme.get_color("font_color", "Label") == to_godot_color(text_color)
            && theme.get_font_size("font_size", "ProbeButton") == 18
            && theme.get_font("font", "ProbeButton").is_some()
            && theme.get_icon("arrow", "ProbeOption").is_some()
            && theme.get_constant("separation", "ProbeStack") == 12
            && theme.get_color("font_color", "ProbeButton") == to_godot_color(text_color)
            && normal.get_bg_color() == to_godot_color(base_background)
            && hover.get_bg_color() == to_godot_color(hover_background)
            && focus.get_corner_radius(Corner::TOP_LEFT) == 9
    }

    /// Verifies that recompiling a theme reads edited resources while leaving
    /// the previously compiled theme untouched.
    #[func]
    fn verify_fresh_resources() -> bool {
        const SOURCE: &str = "res://theme.tms";
        if !write_probe_box("StyleBoxFlat", "bg_color = Color(1, 0, 0, 1)") {
            return false;
        }
        let Ok(previous) = generate_from_project_path(SOURCE).result else {
            return false;
        };
        if !write_probe_box("StyleBoxFlat", "bg_color = Color(0, 0, 1, 1)") {
            return false;
        }
        let Ok(current) = generate_from_project_path(SOURCE).result else {
            return false;
        };
        if probe_box_color(&current) != Some(GodotColor::BLUE) {
            return false;
        }
        if probe_box_color(&previous) != Some(GodotColor::RED) {
            return false;
        }
        if !write_probe_box("SystemFont", "") {
            return false;
        }
        if generate_from_project_path(SOURCE).result.is_ok() {
            return false;
        }
        probe_box_color(&current) == Some(GodotColor::BLUE)
    }

    #[func]
    fn verify_invalid_combinations_are_rejected() -> bool {
        let style_name = Name::new("BadButton").expect("probe name is valid");
        let style = CompiledStyle::new(
            style_name.clone(),
            Name::new("Button").expect("probe target is valid"),
            BTreeMap::from([(
                Name::new("separation").expect("probe property is valid"),
                CompiledValue::Number(Number::new(8.0).expect("probe number is valid")),
            )]),
            BTreeMap::new(),
        );
        let compiled = CompiledTheme::new(
            Name::new("Probe").expect("probe name is valid"),
            ResolvedTokens::new([]),
            BTreeMap::from([(style_name, style)]),
        );

        let unsupported_rejected = build_theme(&compiled)
            .expect_err("unsupported native property is rejected")
            .native_diagnostics()
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.code() == "unsupported_property"
                        && diagnostic.style() == "BadButton"
                        && diagnostic.target() == "Button"
                        && diagnostic.state().is_empty()
                        && diagnostic.property() == "separation"
                })
            });

        let separator_style_name = Name::new("BadSeparator").expect("probe name is valid");
        let separator_color = Color::new([0.1, 0.2, 0.3], 1.0).expect("probe color is valid");
        let separator_style = CompiledStyle::new(
            separator_style_name.clone(),
            Name::new("HSeparator").expect("probe target is valid"),
            BTreeMap::from([(
                Name::new("separator").expect("probe property is valid"),
                CompiledValue::Color(separator_color),
            )]),
            BTreeMap::new(),
        );
        let separator_compiled = CompiledTheme::new(
            Name::new("Probe").expect("probe name is valid"),
            ResolvedTokens::new([]),
            BTreeMap::from([(separator_style_name, separator_style)]),
        );
        let non_flat_stylebox_rejected = build_theme(&separator_compiled)
            .expect_err("color cannot replace a non-flat default stylebox")
            .native_diagnostics()
            .is_some_and(|diagnostics| {
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.code() == "incompatible_stylebox"
                        && diagnostic.style() == "BadSeparator"
                        && diagnostic.target() == "HSeparator"
                        && diagnostic.property() == "separator"
                })
            });

        let state_style_name = Name::new("BadStateButton").expect("probe name is valid");
        let normal = Name::new("normal").expect("probe property is valid");
        let base_color = Color::new([0.1, 0.2, 0.3], 1.0).expect("probe color is valid");
        let state_color = Color::new([0.3, 0.2, 0.1], 1.0).expect("probe color is valid");
        let base = BTreeMap::from([(normal.clone(), CompiledValue::Color(base_color))]);
        let state_name = Name::new("hover").expect("probe state is valid");
        let state = CompiledState::new(
            state_name.clone(),
            BTreeMap::from([(normal, CompiledValue::Color(state_color))]),
        );
        let state_style = CompiledStyle::new(
            state_style_name.clone(),
            Name::new("Button").expect("probe target is valid"),
            base,
            BTreeMap::from([(state_name, state)]),
        );
        let state_compiled = CompiledTheme::new(
            Name::new("Probe").expect("probe name is valid"),
            ResolvedTokens::new([]),
            BTreeMap::from([(state_style_name, state_style)]),
        );
        let state_override_rejected = matches!(
            build_theme(&state_compiled),
            Err(ThemeBuildError::Preparation(errors))
                if matches!(
                    errors.errors(),
                    [themosis_godot::BackendError::StateOverridesBaseItem { .. }]
                )
        );

        let resource_style_name = Name::new("BadResourceLabel").expect("probe name is valid");
        let resource_style = CompiledStyle::new(
            resource_style_name.clone(),
            Name::new("Label").expect("probe target is valid"),
            BTreeMap::from([(
                Name::new("font").expect("probe property is valid"),
                CompiledValue::Resource(
                    ResourceRef::new("theme://font/body").expect("core reference is valid"),
                ),
            )]),
            BTreeMap::new(),
        );
        let resource_compiled = CompiledTheme::new(
            Name::new("Probe").expect("probe name is valid"),
            ResolvedTokens::new([]),
            BTreeMap::from([(resource_style_name, resource_style)]),
        );
        let godot_reference_rejected = matches!(
            build_theme(&resource_compiled),
            Err(ThemeBuildError::Preparation(errors))
                if matches!(
                    errors.errors(),
                    [themosis_godot::BackendError::InvalidResourceReference { .. }]
                )
        );

        unsupported_rejected
            && non_flat_stylebox_rejected
            && state_override_rejected
            && godot_reference_rejected
    }
}

/// Writes the probe resource the fresh-resource fixture theme references.
fn write_probe_box(kind: &str, properties: &str) -> bool {
    let Some(mut file) = FileAccess::open("res://box.tres", ModeFlags::WRITE) else {
        return false;
    };
    file.store_string(&format!(
        "[gd_resource type=\"{kind}\" format=3]\n[resource]\n{properties}\n"
    ));
    true
}

/// Returns the probe stylebox background, when the theme defines one.
fn probe_box_color(theme: &Gd<Theme>) -> Option<GodotColor> {
    theme
        .get_stylebox("normal", "Probe")
        .and_then(|stylebox| stylebox.try_cast::<StyleBoxFlat>().ok())
        .map(|stylebox| stylebox.get_bg_color())
}
