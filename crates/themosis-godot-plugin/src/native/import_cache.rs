//! Contract of imported `.tres` artifacts: metadata keys, fingerprinting, and
//! lookups.
//!
//! The editor importer writes these artifacts while the editor plugin and the
//! headless import gate read them back. Keeping the keys, the fingerprint
//! algorithm, and the readers in one module means every consumer agrees on the
//! same serialization.

use std::collections::BTreeMap;

use godot::{
    classes::{FileAccess, ResourceSaver, ResourceUid, Theme},
    global::{Error, error_string},
    obj::Singleton,
    prelude::*,
};
use themosis_godot::{reports::file_stem, runner::RunnerDiagnostic};

use crate::native::materialize::load_theme;

/// Meta key carrying a theme's source and resource dependencies.
pub(crate) const DEPENDENCIES_META: &str = "_themosis_dependencies";
/// Meta key carrying the dependency fingerprint of the last import.
pub(crate) const DEPENDENCY_FINGERPRINT_META: &str = "_themosis_dependency_fingerprint";

/// A failed save of an imported theme.
pub(crate) struct SaveFailure {
    /// Engine error returned to the importer.
    pub(crate) error: Error,
    /// Human-readable failure summary.
    pub(crate) message: String,
    /// Structured failures with source locations.
    pub(crate) diagnostics: Vec<RunnerDiagnostic>,
}

/// Saves a compiled theme with its dependency manifest and fingerprint.
pub(crate) fn save_imported(
    theme: &mut Gd<Theme>,
    dependencies: &[String],
    source_file: &str,
    save_path: &str,
) -> Result<(), SaveFailure> {
    let dependency_paths = if dependencies.is_empty() {
        vec![source_file.to_owned()]
    } else {
        dependencies.to_vec()
    };
    theme.set_name(file_stem(source_file));
    theme.set_meta(
        DEPENDENCIES_META,
        &packed_strings(&dependency_paths).to_variant(),
    );
    theme.set_meta(
        DEPENDENCY_FINGERPRINT_META,
        &dependency_fingerprint(&dependency_paths).to_variant(),
    );
    let target = format!("{save_path}.tres");
    let error = ResourceSaver::singleton()
        .save_ex(&*theme)
        .path(target.as_str())
        .done();
    if error != Error::OK {
        let message = format!(
            "cannot save imported theme '{target}' ({})",
            error_string(error.ord() as i64)
        );
        return Err(SaveFailure {
            error,
            diagnostics: vec![
                RunnerDiagnostic::new("save_failed", message.as_str()).at_source(source_file),
            ],
            message,
        });
    }
    Ok(())
}

/// Reads a string-array meta entry, or an empty vector.
pub(crate) fn meta_string_array(object: &Gd<Theme>, key: &str) -> Vec<String> {
    object
        .get_meta_ex(key)
        .default(&PackedStringArray::new().to_variant())
        .done()
        .try_to::<PackedStringArray>()
        .map(|dependencies| {
            dependencies
                .to_vec()
                .into_iter()
                .map(|dependency| dependency.to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Reads a string meta entry, or the empty string.
pub(crate) fn meta_string(object: &Gd<Theme>, key: &str) -> String {
    object
        .get_meta_ex(key)
        .default(&"".to_variant())
        .done()
        .to_string()
}

/// Loads an imported theme for a source path, if one exists.
pub(crate) fn load_imported_theme(source: &str) -> Option<Gd<Theme>> {
    load_theme(source)
}

/// Returns the dependency fingerprint of a dependency list.
///
/// The snapshot is order-independent: paths are sorted and each entry pairs
/// the path with the MD5 of its current contents, or `<missing>`.
pub(crate) fn dependency_fingerprint(dependencies: &[String]) -> String {
    let mut snapshot = BTreeMap::new();
    for dependency in dependencies {
        snapshot.insert(dependency.clone(), content_hash(dependency));
    }
    fingerprint_snapshot(&snapshot)
}

/// Returns the fingerprint of a path-to-hash snapshot.
pub(crate) fn fingerprint_snapshot(snapshot: &BTreeMap<String, String>) -> String {
    let entries = snapshot
        .iter()
        .map(|(path, hash)| format!("{path}:{hash}"))
        .collect::<Vec<_>>()
        .join("\n");
    GString::from(entries.as_str()).md5_text().to_string()
}

/// Returns the MD5 of a dependency's contents, or its fingerprint state.
///
/// A `uid://` dependency hashes the path the UID currently resolves to as well
/// as the resolved file's contents, so re-pointing a UID invalidates an import
/// exactly like editing the resource does. Unresolvable UIDs and missing files
/// keep distinct markers.
pub(crate) fn content_hash(path: &str) -> String {
    if path.starts_with("uid://") {
        return resolved_uid_path(path).map_or_else(
            || "<unresolved uid>".to_owned(),
            |resolved| format!("{resolved}:{}", file_hash(&resolved)),
        );
    }
    file_hash(path)
}

/// Returns the current path of a `uid://` reference, or `None`.
///
/// Resolution is checked against [`ResourceUid`] before reading, so an unknown
/// UID reports a fingerprint state instead of an engine error.
pub(crate) fn resolved_uid_path(reference: &str) -> Option<String> {
    let uid = ResourceUid::singleton();
    let id = uid.text_to_id(reference);
    if id < 0 || !uid.has_id(id) {
        return None;
    }
    let path = uid.get_id_path(id).to_string();
    if path.is_empty() { None } else { Some(path) }
}

/// Returns the MD5 of a file's contents, or `<missing>`.
fn file_hash(path: &str) -> String {
    if FileAccess::file_exists(path) {
        FileAccess::get_md5(path).to_string()
    } else {
        "<missing>".to_owned()
    }
}

/// Builds a packed string array from owned strings.
fn packed_strings(items: &[String]) -> PackedStringArray {
    let mut array = PackedStringArray::new();
    for item in items {
        array.push(item.as_str());
    }
    array
}
