mod diagnostic;

use godot::{
    classes::{GDScript, Json, Object, Theme},
    global::Error,
    prelude::*,
};
use themosis_core::CompiledTheme;
use themosis_godot::{NATIVE_THEME_BUILDER_GDSCRIPT, plan_theme};

pub use self::diagnostic::{NativeDiagnostic, NativeDiagnostics, ThemeBuildError};

/// Builds a native Godot theme from canonical compiler output.
///
/// Portable validation runs in `themosis-godot`. The resulting build plan is
/// interpreted by the same GDScript builder used by the CLI, inside the Godot
/// engine that loaded this extension.
pub fn build_theme(compiled: &CompiledTheme) -> Result<Gd<Theme>, ThemeBuildError> {
    let plan = plan_theme(compiled)?;
    let json = serde_json::to_string(&plan).map_err(|error| {
        ThemeBuildError::Builder(format!("could not serialize portable build plan: {error}"))
    })?;
    let plan = Json::parse_string(&json)
        .try_to::<VarDictionary>()
        .map_err(|error| {
            ThemeBuildError::Builder(format!(
                "serialized build plan did not become a Godot dictionary: {error}"
            ))
        })?;
    let response = execute_builder(plan)?;
    if response.get("ok").and_then(|value| value.try_to().ok()) != Some(true) {
        return Err(ThemeBuildError::Native(read_diagnostics(&response)?));
    }
    response
        .get("theme")
        .ok_or_else(|| ThemeBuildError::Builder("successful response has no theme".to_owned()))?
        .try_to::<Gd<Theme>>()
        .map_err(|error| {
            ThemeBuildError::Builder(format!(
                "successful response contains an invalid theme: {error}"
            ))
        })
}

fn execute_builder(plan: VarDictionary) -> Result<VarDictionary, ThemeBuildError> {
    let mut script = GDScript::new_gd();
    script.set_source_code(NATIVE_THEME_BUILDER_GDSCRIPT);
    let reload = script.reload();
    if reload != Error::OK {
        return Err(ThemeBuildError::Builder(format!(
            "embedded GDScript could not be compiled: {reload:?}"
        )));
    }
    let instance = script.try_instantiate(&[]).map_err(|error| {
        ThemeBuildError::Builder(format!(
            "embedded GDScript could not be instantiated: {error}"
        ))
    })?;
    let mut builder = instance.try_to::<Gd<Object>>().map_err(|error| {
        ThemeBuildError::Builder(format!(
            "embedded GDScript returned no builder object: {error}"
        ))
    })?;
    builder
        .try_call("build_plan", &[plan.to_variant()])
        .map_err(|error| {
            ThemeBuildError::Builder(format!("embedded GDScript call failed: {error}"))
        })?
        .try_to::<VarDictionary>()
        .map_err(|error| {
            ThemeBuildError::Builder(format!("embedded GDScript returned no response: {error}"))
        })
}

fn read_diagnostics(response: &VarDictionary) -> Result<NativeDiagnostics, ThemeBuildError> {
    let values = response
        .get("diagnostics")
        .ok_or_else(|| ThemeBuildError::Builder("failed response has no diagnostics".to_owned()))?
        .try_to::<VarArray>()
        .map_err(|error| {
            ThemeBuildError::Builder(format!("builder diagnostics are not an array: {error}"))
        })?;
    if values.is_empty() {
        return Err(ThemeBuildError::Builder(
            "failed response contains no diagnostics".to_owned(),
        ));
    }
    let diagnostics = values
        .iter_shared()
        .map(|value| -> Result<_, ThemeBuildError> {
            let diagnostic = value.try_to::<VarDictionary>().map_err(|error| {
                ThemeBuildError::Builder(format!("builder diagnostic is not an object: {error}"))
            })?;
            Ok(NativeDiagnostic {
                code: dictionary_string(&diagnostic, "code")?,
                message: dictionary_string(&diagnostic, "message")?,
                style: dictionary_string(&diagnostic, "style")?,
                target: dictionary_string(&diagnostic, "target")?,
                state: dictionary_string(&diagnostic, "state")?,
                property: dictionary_string(&diagnostic, "property")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NativeDiagnostics::new(diagnostics))
}

fn dictionary_string(dictionary: &VarDictionary, field: &str) -> Result<String, ThemeBuildError> {
    dictionary
        .get(field)
        .ok_or_else(|| {
            ThemeBuildError::Builder(format!("builder diagnostic has no '{field}' field"))
        })?
        .try_to::<String>()
        .map_err(|error| {
            ThemeBuildError::Builder(format!(
                "builder diagnostic field '{field}' is not text: {error}"
            ))
        })
}
