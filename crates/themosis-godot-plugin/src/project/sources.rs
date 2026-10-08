//! Native `res://` discovery of importable `.tms` theme roots.

use godot::classes::DirAccess;

/// Discovers every `.tms` root below `res://`, sorted.
///
/// Hidden entries and linked directories are skipped: links can escape the
/// project or form cycles.
pub(crate) fn discover_roots() -> Vec<String> {
    let mut sources = Vec::new();
    collect("res://", &mut sources);
    sources.sort();
    sources
}

fn collect(directory: &str, sources: &mut Vec<String>) {
    let Some(mut access) = DirAccess::open(directory) else {
        return;
    };
    access.list_dir_begin();
    loop {
        let entry = access.get_next().to_string();
        if entry.is_empty() {
            break;
        }
        if entry.starts_with('.') {
            continue;
        }
        let path = join(directory, &entry);
        if access.current_is_dir() {
            // Linked directories can escape the project or form cycles.
            if !access.is_link(&path) {
                collect(&path, sources);
            }
        } else if extension_of(&entry) == "tms" {
            sources.push(path);
        }
    }
    access.list_dir_end();
}

fn extension_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, extension)) => extension.to_ascii_lowercase(),
        None => String::new(),
    }
}

fn join(directory: &str, name: &str) -> String {
    if directory.ends_with('/') {
        format!("{directory}{name}")
    } else {
        format!("{directory}/{name}")
    }
}
