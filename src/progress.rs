/// Stub progress module — osm-godot is CLI-only, no GUI progress bar needed.
/// Replaces arnis's Tauri-based progress system.

pub fn emit_gui_error(_message: &str) {}
pub fn emit_gui_progress_update(_progress: f64, _message: &str) {}
pub fn emit_map_preview_ready() {}
pub fn emit_show_in_folder(_path: &str) {}
pub fn is_running_with_gui() -> bool {
    false
}
