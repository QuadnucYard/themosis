use std::collections::BTreeMap;

use themosis_core::{
    CompiledStyle, CompiledTheme, CompiledValue, Diagnostic, Dimension, DimensionUnit, Name,
    Number, ResolvedTokens, ResourceRef,
};

use crate::{
    BackendError, BackendErrors, NATIVE_THEME_BUILDER_GDSCRIPT, NATIVE_THEME_RUNNER_GDSCRIPT,
    plan_theme,
};

fn name(value: &str) -> Name {
    Name::new(value).expect("name is valid")
}

fn theme(properties: BTreeMap<Name, CompiledValue>) -> CompiledTheme {
    let style_name = name("ProbeButton");
    CompiledTheme::new(
        name("Probe"),
        ResolvedTokens::new([]),
        BTreeMap::from([(
            style_name.clone(),
            CompiledStyle::new(style_name, name("Button"), properties, BTreeMap::new()),
        )]),
    )
}

#[test]
fn accepts_integral_pixel_font_size_inputs() {
    let pixels = CompiledValue::Dimension(
        Dimension::new(18.0, DimensionUnit::Pixel).expect("dimension is valid"),
    );
    let rem = CompiledValue::Dimension(
        Dimension::new(1.0, DimensionUnit::Rem).expect("dimension is valid"),
    );
    let fractional = CompiledValue::Number(Number::new(12.5).expect("number is finite"));

    assert!(plan_theme(&theme(BTreeMap::from([(name("font_size"), pixels)]))).is_ok());
    for value in [rem, fractional] {
        let error = plan_theme(&theme(BTreeMap::from([(name("font_size"), value)])))
            .expect_err("value is not an integral pixel count");
        assert!(matches!(
            error.errors(),
            [BackendError::InvalidInteger { .. }]
        ));
    }
}

#[test]
fn validates_resource_namespaces_without_godot_runtime() {
    let invalid = CompiledValue::Resource(
        ResourceRef::new("https://example.com/font.tres").expect("reference is non-empty"),
    );

    let error = plan_theme(&theme(BTreeMap::from([(name("font"), invalid)])))
        .expect_err("external URL is not a Godot resource path");

    assert!(matches!(
        error.errors(),
        [BackendError::InvalidResourceReference { .. }]
    ));
}

#[test]
fn formats_portable_failures_with_stable_codes() {
    let errors = BackendErrors::new(vec![
        BackendError::UnsupportedValue {
            style: name("ProbeButton"),
            target: name("Button"),
            state: None,
            property: name("disabled"),
            value: "boolean",
        },
        BackendError::StateOverridesBaseItem {
            style: name("ProbeButton"),
            state: name("hover"),
            property: name("normal"),
        },
        BackendError::InvalidInteger {
            style: name("ProbeButton"),
            property: name("font_size"),
            expected: "a positive whole number of pixels",
        },
        BackendError::InvalidResourceReference {
            reference: ResourceRef::new("https://example.com/font.tres")
                .expect("reference is non-empty"),
        },
    ]);

    assert_eq!(
        errors
            .errors()
            .iter()
            .map(BackendError::code)
            .collect::<Vec<_>>(),
        ["TMS3001", "TMS3002", "TMS3003", "TMS3004"]
    );
    assert_eq!(errors.to_string().matches("error[TMS3").count(), 4);
}

#[test]
fn serializes_portable_runtime_build_plan() {
    let color = CompiledValue::Color(
        themosis_core::Color::new([0.1, 0.2, 0.3], 1.0).expect("color is valid"),
    );
    let compiled = theme(BTreeMap::from([(name("normal"), color)]));

    let plan = plan_theme(&compiled).expect("portable values are valid");
    let json = serde_json::to_value(&plan).expect("plan is serializable");

    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["theme"], "Probe");
    assert_eq!(json["styles"][0]["name"], "ProbeButton");
    assert_eq!(json["styles"][0]["target"], "Button");
    assert_eq!(json["styles"][0]["items"][0]["property"], "normal");
    assert_eq!(
        json["styles"][0]["items"][0]["candidates"],
        serde_json::json!(["color", "stylebox"])
    );
    assert_eq!(
        json["styles"][0]["items"][0]["value"],
        serde_json::json!({"kind": "color", "rgba": [0.1, 0.2, 0.3, 1.0]})
    );
    assert!(NATIVE_THEME_BUILDER_GDSCRIPT.contains("func build_plan(plan: Dictionary)"));
    assert!(NATIVE_THEME_RUNNER_GDSCRIPT.contains("ResourceSaver.save(theme, temporary)"));
}
