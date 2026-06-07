//! Writes the Godot project.godot file.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Write a minimal Godot 4.x project.godot file.
pub fn write_project_file(output_dir: &Path, project_name: &str) -> io::Result<()> {
    let path = output_dir.join("project.godot");
    let mut f = fs::File::create(&path)?;

    writeln!(f, "; Engine configuration file.")?;
    writeln!(f, "; It's best edited using the editor UI and not directly,")?;
    writeln!(f, "; since the parameters that go here are not all obvious.")?;
    writeln!(f)?;
    writeln!(f, "config_version=5")?;
    writeln!(f)?;
    writeln!(f, "[application]")?;
    writeln!(f)?;
    writeln!(f, "config/name=\"{project_name}\"")?;
    writeln!(f, "run/main_scene=\"res://scenes/master.tscn\"")?;
    writeln!(f, "config/features=PackedStringArray(\"4.6\", \"Forward Plus\")")?;
    writeln!(f, "config/icon=\"res://icon.svg\"")?;
    writeln!(f)?;
    writeln!(f, "[animation]")?;
    writeln!(f)?;
    writeln!(f, "compatibility/default_parent_skeleton_in_mesh_instance_3d=true")?;
    writeln!(f)?;
    writeln!(f, "[input]")?;
    writeln!(f)?;
    write_key_action(&mut f, "move_forward", &[87, 4194320])?;
    write_key_action(&mut f, "move_backward", &[83, 4194322])?;
    write_key_action(&mut f, "move_left", &[65, 4194319])?;
    write_key_action(&mut f, "move_right", &[68, 4194321])?;
    write_key_action(&mut f, "jump", &[32])?;
    write_key_action(&mut f, "descend", &[4194326])?;
    write_key_action(&mut f, "sprint", &[4194325])?;
    write_key_action(&mut f, "mouse_capture_toggle", &[4194305])?;
    write_key_action(&mut f, "noclip_toggle", &[86])?;
    writeln!(f)?;
    writeln!(f, "[rendering]")?;
    writeln!(f)?;
    writeln!(f, "environment/defaults/default_environment=\"res://default_environment.tres\"")?;
    writeln!(f)?;
    writeln!(f, "[editor_plugins]")?;
    writeln!(f)?;
    writeln!(f, "enabled=PackedStringArray()")?;

    Ok(())
}

fn write_key_action(f: &mut fs::File, name: &str, keycodes: &[u32]) -> io::Result<()> {
    writeln!(f, "{name}={{")?;
    writeln!(f, "\"deadzone\": 0.5,")?;
    writeln!(f, "\"events\": [")?;
    for (idx, keycode) in keycodes.iter().enumerate() {
        let suffix = if idx + 1 == keycodes.len() { "" } else { "," };
        writeln!(f, "Object(InputEventKey,\"resource_local_to_scene\":false,\"resource_name\":\"\",\"device\":-1,\"window_id\":0,\"alt_pressed\":false,\"shift_pressed\":false,\"ctrl_pressed\":false,\"meta_pressed\":false,\"pressed\":false,\"keycode\":{keycode},\"physical_keycode\":{keycode},\"key_label\":0,\"unicode\":0,\"location\":0,\"echo\":false,\"script\":null){suffix}")?;
    }
    writeln!(f, "]")?;
    writeln!(f, "}}")?;
    Ok(())
}

/// Write a default environment resource.
pub fn write_default_environment(output_dir: &Path) -> io::Result<()> {
    let path = output_dir.join("default_environment.tres");
    let mut f = fs::File::create(&path)?;

    writeln!(f, "[gd_resource type=\"Environment\" load_steps=0 format=3 uid=\"uid://denv00000001\"]")?;
    writeln!(f)?;
    writeln!(f, "[resource]")?;
    writeln!(f, "background_mode = 0")?; // Clear color
    writeln!(f, "background_color = Color(0.4, 0.6, 0.9, 1)")?;

    Ok(())
}

/// Write a metadata.json with geo reference info.
pub fn write_metadata(
    output_dir: &Path,
    min_lat: f64,
    max_lat: f64,
    min_lng: f64,
    max_lng: f64,
    godot_scale: f32,
    chunk_size: i32,
) -> io::Result<()> {
    let path = output_dir.join("metadata.json");
    let contents = serde_json::json!({
        "min_latitude": min_lat,
        "max_latitude": max_lat,
        "min_longitude": min_lng,
        "max_longitude": max_lng,
        "godot_scale_m_per_block": godot_scale,
        "chunk_size_blocks": chunk_size,
        "coordinate_system": "Godot 3D: X=east, Y=up, Z=south (Z=-arnis_z)",
    });
    fs::write(&path, serde_json::to_string_pretty(&contents)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_file_targets_godot_46_without_migration_prompt() {
        let tmp = tempfile::tempdir().unwrap();

        write_project_file(tmp.path(), "Test World").unwrap();

        let project = std::fs::read_to_string(tmp.path().join("project.godot")).unwrap();
        assert!(project.contains("config/features=PackedStringArray(\"4.6\", \"Forward Plus\")"));
        assert!(!project.contains("\"4.3\""));
    }

    #[test]
    fn project_file_defines_fps_input_actions() {
        let tmp = tempfile::tempdir().unwrap();

        write_project_file(tmp.path(), "Test World").unwrap();

        let project = std::fs::read_to_string(tmp.path().join("project.godot")).unwrap();
        assert!(project.contains("[input]"));
        assert!(project.contains("move_forward="));
        assert!(project.contains("move_backward="));
        assert!(project.contains("move_left="));
        assert!(project.contains("move_right="));
        assert!(project.contains("jump="));
        assert!(project.contains("mouse_capture_toggle="));
        assert!(project.contains("noclip_toggle="));
        assert!(project.contains("\"keycode\":87"));
        assert!(project.contains("\"physical_keycode\":87"));
    }
}
