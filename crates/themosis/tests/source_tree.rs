//! End-to-end source-tree loading tests.

use std::path::{Path, PathBuf};

use themosis::{
    FileSystemSourceProvider, InvalidSourcePath, LoadError, MemorySourceProvider, compile_theme,
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
fn rejects_import_cycles_with_a_closed_path() {
    let mut provider = MemorySourceProvider::new();
    provider
        .insert(
            "theme.kdl",
            "theme \"Application\" {\n    import \"a.kdl\"\n}\n",
        )
        .expect("path is valid");
    provider
        .insert(
            "a.kdl",
            "theme \"Application\" {\n    import \"theme.kdl\"\n}\n",
        )
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
            "theme \"Application\" {\n    tokens \"../../outside.tokens.json\"\n}\n",
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
