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
    writeln!(f, "[rendering]")?;
    writeln!(f)?;
    writeln!(f, "environment/defaults/default_environment=\"res://default_environment.tres\"")?;
    writeln!(f)?;
    writeln!(f, "[editor_plugins]")?;
    writeln!(f)?;
    writeln!(f, "enabled=PackedStringArray()")?;

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
}
