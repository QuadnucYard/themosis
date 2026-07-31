//! End-to-end source-tree loading tests.

use std::path::{Path, PathBuf};

use themosis::{
    FileSystemSourceProvider, InvalidSourcePath, LoadError, MemorySourceProvider, compile_theme,
    compile_theme_with_report,
};
use themosis_core::{CompiledValue, Name, Number};

const ROOT: &str = include_str!("fixtures/valid/theme.kdl");
const BUTTONS: &str = include_str!("fixtures/valid/styles/buttons.kdl");
const TOKENS: &str = include_str!("fixtures/valid/tokens/theme.tokens.json");

fn name(value: &str) -> Name {
    Name::new(value).expect("fixture name is valid")
}

fn memory_fixture() -> MemorySourceProvider {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert("theme.kdl", ROOT)
        .expect("fixture path is valid");
    provider
        .insert("styles/buttons.kdl", BUTTONS)
        .expect("fixture path is valid");
    provider
        .insert("tokens/theme.tokens.json", TOKENS)
        .expect("fixture path is valid");
    provider
}

#[test]
fn filesystem_and_memory_compile_the_same_source_tree() {
    let memory_theme =
        compile_theme(&memory_fixture(), "theme.kdl").expect("memory fixture compiles");
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid");
    let filesystem =
        FileSystemSourceProvider::new(fixture_root).expect("fixture root is accessible");
    let filesystem_theme =
        compile_theme(&filesystem, "theme.kdl").expect("filesystem fixture compiles");

    assert_eq!(filesystem_theme, memory_theme);
    assert_eq!(memory_theme.tokens().len(), 2);
    let primary = memory_theme
        .styles()
        .get(&name("PrimaryButton"))
        .expect("imported child style is compiled");
    assert_eq!(
        primary.properties().get(&name("font-size")),
        Some(&CompiledValue::Number(
            Number::new(18.0).expect("number is finite")
        ))
    );
    assert_eq!(
        primary
            .states()
            .get(&name("hover"))
            .expect("inherited state is expanded")
            .properties()
            .len(),
        2
    );
}

#[test]
fn reports_all_dependencies_after_successful_loading() {
    let report = compile_theme_with_report(&memory_fixture(), "theme.kdl");

    assert!(report.result().is_ok());
    assert_eq!(
        report.dependencies().iter().cloned().collect::<Vec<_>>(),
        vec![
            PathBuf::from("styles/buttons.kdl"),
            PathBuf::from("theme.kdl"),
            PathBuf::from("tokens/theme.tokens.json"),
        ]
    );
}

#[test]
fn imported_fragments_inherit_the_root_theme() {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert(
            "light.tms",
            r#"theme Application { import "controls.kdl" }"#,
        )
        .expect("root path is valid");
    provider
        .insert(
            "controls.kdl",
            r#"style PrimaryButton target=Button { number font_size 16}"#,
        )
        .expect("module path is valid");

    let compiled = compile_theme(&provider, "light.tms").expect("fragment compiles");

    assert_eq!(compiled.name().as_str(), "Application");
    assert!(compiled.styles().contains_key(&name("PrimaryButton")));
}

#[test]
fn retains_discovered_dependencies_after_a_read_failure() {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert(
            "theme.kdl",
            r#"theme Application {
                import "missing.kdl"
                tokens "tokens.json"
            }"#,
        )
        .expect("path is valid");
    provider
        .insert("tokens.json", r#"{"value":{"$type":"number","$value":1}}"#)
        .expect("path is valid");

    let report = compile_theme_with_report(&provider, "theme.kdl");

    assert!(report.result().is_err());
    assert_eq!(
        report.dependencies().iter().cloned().collect::<Vec<_>>(),
        vec![
            PathBuf::from("missing.kdl"),
            PathBuf::from("theme.kdl"),
            PathBuf::from("tokens.json"),
        ]
    );
}

#[test]
fn rejects_import_cycles_with_a_closed_path() {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert("theme.kdl", r#"theme Application { import "a.kdl" }"#)
        .expect("path is valid");
    provider
        .insert("a.kdl", r#"theme Application { import "theme.kdl" }"#)
        .expect("path is valid");

    let error = compile_theme(&provider, "theme.kdl").expect_err("imports contain a cycle");

    assert!(matches!(
        error,
        LoadError::ImportCycle { cycle }
            if cycle == vec![
                PathBuf::from("theme.kdl"),
                PathBuf::from("a.kdl"),
                PathBuf::from("theme.kdl")
            ]
    ));
}

#[test]
fn rejects_dependencies_that_escape_the_theme_root() {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert(
            "styles/theme.kdl",
            r#"theme Application {
                tokens "../../outside.tokens.json"
            }"#,
        )
        .expect("root path is valid");

    let error = compile_theme(&provider, "styles/theme.kdl").expect_err("dependency escapes root");

    assert!(matches!(
        error,
        LoadError::InvalidPath {
            source: InvalidSourcePath::EscapesRoot,
            ..
        }
    ));
}

#[test]
fn renders_semantic_diagnostics_with_source_names_and_suggestions() {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert(
            "theme.kdl",
            r#"theme Application {
                tokens "tokens.json"
                style PrimaryButton target=Button {
                    token background color.primray
                }
            }"#,
        )
        .expect("path is valid");
    provider
        .insert(
            "tokens.json",
            r#"{"color":{"$type":"color","primary":{"$value":{"colorSpace":"srgb","components":[0.1,0.2,0.3],"alpha":1.0}}}}"#,
        )
        .expect("path is valid");

    let error = compile_theme(&provider, "theme.kdl").expect_err("token is misspelled");
    let rendered = error.to_string();

    assert!(rendered.contains("error[TMS2207]"));
    assert!(rendered.contains("theme.kdl:"));
    assert!(rendered.contains("did you mean 'color.primary'?"));
}

#[test]
fn root_document_establishes_the_theme_name() {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert(
            "z-root.kdl",
            r#"theme Application { import "a-import.kdl" }"#,
        )
        .expect("path is valid");
    provider
        .insert("a-import.kdl", r#"theme WrongName {}"#)
        .expect("path is valid");

    let error = compile_theme(&provider, "z-root.kdl").expect_err("theme names conflict");
    let rendered = error.to_string();

    assert!(rendered.contains("uses theme 'WrongName', expected 'Application'"));
}
