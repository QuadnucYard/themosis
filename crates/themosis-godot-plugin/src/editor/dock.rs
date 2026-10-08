//! Native editor dock listing discovered theme roots with statuses, native
//! previews, and structured diagnostics.
//!
//! The dock renders [`themosis_godot::editor`] views onto Godot controls and
//! forwards user actions as signals; every state decision lives in the portable
//! state machine, and every recorded outcome is a Rust type.

// `#[class(init)]` expands to code that trips these lints on the `base` field.
#![allow(clippy::absolute_paths, clippy::redundant_field_names)]

use std::collections::BTreeMap;

use godot::{
    classes::{
        Button, FileDialog, HBoxContainer, IVBoxContainer, Label, MarginContainer, OptionButton,
        PanelContainer, RichTextLabel, Theme, VBoxContainer, control::SizeFlags, file_dialog,
        text_server::AutowrapMode,
    },
    prelude::*,
};
use themosis_godot::{
    editor::{DiagnosticLine, EditorState, Tone, batch_view},
    reports::{BatchOutcome, Operation, OperationOutcome, file_name, file_stem},
};

use crate::native::materialize::load_theme;

const NEUTRAL_COLOR: Color = Color::from_rgba(1.0, 1.0, 1.0, 1.0);
const SUCCESS_COLOR: Color = Color::from_rgba(0.55, 0.9, 0.65, 1.0);
const WARNING_COLOR: Color = Color::from_rgba(0.95, 0.8, 0.45, 1.0);
const FAILURE_COLOR: Color = Color::from_rgba(1.0, 0.45, 0.4, 1.0);
const IMPORTING_COLOR: Color = Color::from_rgba(0.8, 0.85, 1.0, 1.0);

/// Godot-facing editor dock for Themosis themes.
#[derive(GodotClass)]
#[class(tool, init, base=VBoxContainer)]
pub struct ThemosisThemeDock {
    base: Base<VBoxContainer>,

    state: EditorState,
    themes: BTreeMap<String, Gd<Theme>>,
    updating: bool,

    selector: Option<Gd<OptionButton>>,
    status_label: Option<Gd<Label>>,
    empty_help: Option<Gd<Label>>,
    actions: Option<Gd<HBoxContainer>>,
    preview: Option<Gd<PanelContainer>>,
    preview_name: Option<Gd<Label>>,
    diagnostics_view: Option<Gd<RichTextLabel>>,
    output_dialog: Option<Gd<FileDialog>>,
    directory_dialog: Option<Gd<FileDialog>>,
}

#[godot_api]
impl ThemosisThemeDock {
    /// Emitted when the user requests a reimport of one source.
    #[signal]
    pub fn reimport_requested(source: GString);
    /// Emitted when the user requests a reimport of every source.
    #[signal]
    pub fn reimport_all_requested();
    /// Emitted when the user materializes one source.
    #[signal]
    pub fn materialize_requested(source: GString, output: GString);
    /// Emitted when the user materializes every source into one directory.
    #[signal]
    pub fn materialize_all_requested(directory: GString);
    /// Emitted when a diagnostic path is clicked.
    #[signal]
    pub fn diagnostic_clicked(path: GString);
}

impl ThemosisThemeDock {
    /// Replaces the discovered sources and refreshes the interface.
    pub(crate) fn set_theme_sources(&mut self, sources: Vec<String>) {
        self.state.set_sources(sources);
        if !self.base().is_node_ready() {
            return;
        }
        self.refresh_view();
    }

    /// Marks a source as currently importing.
    pub(crate) fn mark_importing(&mut self, source: &str) {
        self.state.mark_importing(source);
        if self.base().is_node_ready() {
            self.refresh_view();
        }
    }

    /// Marks a source as stale on disk.
    pub(crate) fn mark_stale(&mut self, source: &str) {
        self.state.mark_stale(source);
        if self.base().is_node_ready() {
            self.refresh_view();
        }
    }

    /// Records one operation's outcome and refreshes the interface.
    pub(crate) fn apply_outcome(&mut self, outcome: OperationOutcome) {
        if outcome.ok() {
            let artifact = match outcome.operation {
                Operation::Import => outcome.source.clone(),
                Operation::Materialize => outcome.output.clone(),
                Operation::Validate => String::new(),
            };
            if !artifact.is_empty()
                && let Some(theme) = load_theme(&artifact)
            {
                self.themes.insert(outcome.source.clone(), theme);
            }
        }
        self.state.record(outcome);
        if self.base().is_node_ready() {
            self.refresh_view();
        }
    }

    /// Renders one batch materialization outcome.
    pub(crate) fn show_batch(&mut self, batch: &BatchOutcome) {
        let view = batch_view(batch);
        self.apply_status(&view.status_text, view.tone);
        if let Some(diagnostics) = self.diagnostics_view.as_mut() {
            render_diagnostics(diagnostics, &view.diagnostics);
        }
    }
}

impl ThemosisThemeDock {
    /// Applies a status line to the status label.
    fn apply_status(&mut self, text: &str, tone: Tone) {
        let Some(status) = self.status_label.as_mut() else {
            return;
        };
        status.set_text(text);
        status.set_modulate(tone_color(tone));
    }

    /// Rebuilds the selector and refreshes the selected result.
    fn refresh_view(&mut self) {
        let (Some(selector), Some(actions), Some(preview), Some(empty_help)) = (
            self.selector.as_mut(),
            self.actions.as_mut(),
            self.preview.as_mut(),
            self.empty_help.as_mut(),
        ) else {
            return;
        };
        let sources = self.state.sources().to_vec();
        let selected = self.state.selected().to_owned();
        self.updating = true;
        selector.clear();
        for (index, source) in sources.iter().enumerate() {
            let status = self.state.status(source).label();
            selector.add_item(&format!("{} — {status}", file_name(source)));
            selector.set_item_tooltip(index as i32, source);
            if source == &selected {
                selector.select(index as i32);
            }
        }
        let configured = !sources.is_empty();
        selector.set_visible(configured);
        actions.set_visible(configured);
        preview.set_visible(configured);
        empty_help.set_visible(!configured);
        self.updating = false;
        self.render_selection();
    }

    /// Renders the selected source's status, diagnostics, and preview.
    fn render_selection(&mut self) {
        let (Some(status), Some(preview), Some(preview_name), Some(diagnostics)) = (
            self.status_label.as_mut(),
            self.preview.as_mut(),
            self.preview_name.as_mut(),
            self.diagnostics_view.as_mut(),
        ) else {
            return;
        };
        let selected = self.state.selected().to_owned();
        let has_theme = self.themes.contains_key(&selected);
        let view = self.state.view(has_theme);
        status.set_text(&view.status_text);
        status.set_modulate(tone_color(view.tone));
        render_diagnostics(diagnostics, &view.diagnostics);
        let file = file_name(&selected).to_owned();
        match self.themes.get(&selected) {
            Some(theme) if view.preview.shows_theme() => {
                let name = theme.get_name().to_string();
                preview.set_theme(theme);
                let title = view.preview.title(&name, &file);
                preview_name.set_text(&title);
            }
            _ => {
                preview.set_theme(None::<&Gd<Theme>>);
                let title = view.preview.title("", &file);
                preview_name.set_text(&title);
            }
        }
    }

    fn select_source(&mut self, index: i64) {
        if self.updating || index < 0 {
            return;
        }
        if self.state.select_index(index as usize) {
            self.render_selection();
        }
    }

    fn request_reimport(&mut self) {
        let selected = self.state.selected().to_owned();
        if !selected.is_empty() {
            self.signals().reimport_requested().emit(selected.as_str());
        }
    }

    fn emit_reimport_all(&mut self) {
        self.signals().reimport_all_requested().emit();
    }

    fn choose_materialized_output(&mut self) {
        let selected = self.state.selected().to_owned();
        if selected.is_empty() {
            return;
        }
        let Some(dialog) = self.output_dialog.as_mut() else {
            return;
        };
        dialog.set_current_path(&format!(
            "res://theme/generated/{}.tres",
            file_stem(&selected)
        ));
        dialog.popup_centered_ratio();
    }

    fn choose_materialized_directory(&mut self) {
        let Some(dialog) = self.directory_dialog.as_mut() else {
            return;
        };
        dialog.set_current_path("res://theme/generated");
        dialog.popup_centered_ratio();
    }

    fn materialized_output_selected(&mut self, output: GString) {
        let selected = self.state.selected().to_owned();
        self.signals()
            .materialize_requested()
            .emit(selected.as_str(), &output);
    }

    fn directory_selected(&mut self, directory: GString) {
        self.signals().materialize_all_requested().emit(&directory);
    }

    fn clear_diagnostics(&mut self) {
        if let Some(diagnostics) = self.diagnostics_view.as_mut() {
            diagnostics.clear();
        }
    }

    fn diagnostic_clicked(&mut self, meta: Variant) {
        let path = meta.to_string();
        self.signals().diagnostic_clicked().emit(path.as_str());
    }

    fn build_interface(&mut self) {
        let mut title = Label::new_alloc();
        title.set_text("Themosis themes");
        self.base_mut().add_child(&title);

        let mut help = Label::new_alloc();
        help.set_text("Root .tms files import as native Godot Theme resources.");
        help.set_autowrap_mode(AutowrapMode::WORD_SMART);
        self.base_mut().add_child(&help);

        let mut status = Label::new_alloc();
        status.set_text("Discovering themes…");
        status.set_autowrap_mode(AutowrapMode::WORD_SMART);
        self.base_mut().add_child(&status);
        self.status_label = Some(status);

        let mut empty_help = Label::new_alloc();
        empty_help.set_text(
            "Add a root such as res://theme/light.tms. Shared KDL files remain ordinary .kdl modules.",
        );
        empty_help.set_autowrap_mode(AutowrapMode::WORD_SMART);
        self.base_mut().add_child(&empty_help);
        self.empty_help = Some(empty_help);

        let mut selector = OptionButton::new_alloc();
        selector.set_h_size_flags(SizeFlags::EXPAND_FILL);
        selector
            .signals()
            .item_selected()
            .connect_other(&self.to_gd(), Self::select_source);
        self.base_mut().add_child(&selector);
        self.selector = Some(selector);

        let mut actions = HBoxContainer::new_alloc();
        self.base_mut().add_child(&actions);
        let mut reimport = Button::new_alloc();
        reimport.set_text("Reimport");
        reimport.set_tooltip_text("Recompile the selected .tms asset");
        reimport
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::request_reimport);
        actions.add_child(&reimport);
        let mut reimport_all = Button::new_alloc();
        reimport_all.set_text("Reimport all");
        reimport_all
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::emit_reimport_all);
        actions.add_child(&reimport_all);
        let mut materialize = Button::new_alloc();
        materialize.set_text("Materialize…");
        materialize.set_tooltip_text("Save a visible native .tres copy");
        materialize
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::choose_materialized_output);
        actions.add_child(&materialize);
        let mut materialize_all = Button::new_alloc();
        materialize_all.set_text("All…");
        materialize_all.set_tooltip_text("Materialize every theme into one directory");
        materialize_all
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::choose_materialized_directory);
        actions.add_child(&materialize_all);
        self.actions = Some(actions);

        let mut preview = PanelContainer::new_alloc();
        preview.set_custom_minimum_size(Vector2::new(0.0, 190.0));
        self.base_mut().add_child(&preview);
        let mut margins = MarginContainer::new_alloc();
        margins.add_theme_constant_override("margin_left", 18);
        margins.add_theme_constant_override("margin_top", 16);
        margins.add_theme_constant_override("margin_right", 18);
        margins.add_theme_constant_override("margin_bottom", 16);
        preview.add_child(&margins);
        let mut stack = VBoxContainer::new_alloc();
        stack.add_theme_constant_override("separation", 10);
        margins.add_child(&stack);
        let mut preview_name = Label::new_alloc();
        preview_name.set_theme_type_variation("SectionTitle");
        preview_name.set_text("Theme preview");
        stack.add_child(&preview_name);
        let mut copy = Label::new_alloc();
        copy.set_text("Imported themes are assignable directly from their .tms source path.");
        copy.set_autowrap_mode(AutowrapMode::WORD_SMART);
        stack.add_child(&copy);
        let mut buttons = HBoxContainer::new_alloc();
        stack.add_child(&buttons);
        let mut primary = Button::new_alloc();
        primary.set_theme_type_variation("PrimaryButton");
        primary.set_text("Primary");
        buttons.add_child(&primary);
        let mut secondary = Button::new_alloc();
        secondary.set_text("Secondary");
        buttons.add_child(&secondary);
        self.preview = Some(preview);
        self.preview_name = Some(preview_name);

        let mut diagnostics_header = HBoxContainer::new_alloc();
        self.base_mut().add_child(&diagnostics_header);
        let mut diagnostics_title = Label::new_alloc();
        diagnostics_title.set_text("Diagnostics");
        diagnostics_title.set_h_size_flags(SizeFlags::EXPAND_FILL);
        diagnostics_header.add_child(&diagnostics_title);
        let mut clear = Button::new_alloc();
        clear.set_text("Clear");
        clear
            .signals()
            .pressed()
            .connect_other(&self.to_gd(), Self::clear_diagnostics);
        diagnostics_header.add_child(&clear);
        let mut diagnostics = RichTextLabel::new_alloc();
        diagnostics.set_fit_content(true);
        diagnostics.set_custom_minimum_size(Vector2::new(0.0, 100.0));
        diagnostics
            .signals()
            .meta_clicked()
            .connect_other(&self.to_gd(), Self::diagnostic_clicked);
        self.base_mut().add_child(&diagnostics);
        self.diagnostics_view = Some(diagnostics);

        let mut output_dialog = FileDialog::new_alloc();
        output_dialog.set_access(file_dialog::Access::RESOURCES);
        output_dialog.set_file_mode(file_dialog::FileMode::SAVE_FILE);
        let mut filters = PackedStringArray::new();
        filters.push("*.tres ; Godot theme resource");
        output_dialog.set_filters(&filters);
        output_dialog
            .signals()
            .file_selected()
            .connect_other(&self.to_gd(), Self::materialized_output_selected);
        self.base_mut().add_child(&output_dialog);
        self.output_dialog = Some(output_dialog);

        let mut directory_dialog = FileDialog::new_alloc();
        directory_dialog.set_access(file_dialog::Access::RESOURCES);
        directory_dialog.set_file_mode(file_dialog::FileMode::OPEN_DIR);
        directory_dialog
            .signals()
            .dir_selected()
            .connect_other(&self.to_gd(), Self::directory_selected);
        self.base_mut().add_child(&directory_dialog);
        self.directory_dialog = Some(directory_dialog);
    }
}

#[godot_api]
impl IVBoxContainer for ThemosisThemeDock {
    fn ready(&mut self) {
        self.base_mut().set_name("Themosis");
        self.base_mut()
            .set_custom_minimum_size(Vector2::new(360.0, 0.0));
        self.build_interface();
        self.refresh_view();
    }
}

/// Maps a status tone onto the dock's status colors.
fn tone_color(tone: Tone) -> Color {
    match tone {
        Tone::Neutral => NEUTRAL_COLOR,
        Tone::Success => SUCCESS_COLOR,
        Tone::Warning => WARNING_COLOR,
        Tone::Failure => FAILURE_COLOR,
        Tone::Importing => IMPORTING_COLOR,
    }
}

/// Renders diagnostics into the dock's rich text view.
///
/// Lines with a source path select that file when clicked.
fn render_diagnostics(view: &mut Gd<RichTextLabel>, diagnostics: &[DiagnosticLine]) {
    view.clear();
    for line in diagnostics {
        if line.meta_path.is_empty() {
            view.add_text(&line.text);
        } else {
            view.push_meta(&line.meta_path.to_variant());
            view.add_text(&line.text);
            view.pop();
        }
        view.add_text("\n");
    }
}
