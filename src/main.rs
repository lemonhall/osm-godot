mod args;
mod bresenham;
mod clipping;
mod coordinate_system;
mod data_processing;
mod element_processing;
mod elevation;
mod elevation_data;
mod ground;
mod ground_generation;
mod land_cover;
mod osm_parser;
mod progress;
mod retrieve_data;
mod scene_writer;
#[cfg(test)]
mod test_utilities;

use args::Args;
use clap::Parser;
use colored::*;
use std::sync::Arc;

fn run() {
    let version: &str = env!("CARGO_PKG_VERSION");
    let repository: &str = "https://github.com/lemon/osm-godot";
    println!(
        r#"
  ___  ____  __  __    ____   ___  _____ ___  ____
 / _ \/ ___||  \/  |  / ___| / _ \|_   _/ _ \|  _ \
| | | \___ \| |\/| | | |  _ | | | | | || | | | | | |
| |_| |___) | |  | | | |_| || |_| | | || |_| | |_| |
 \___/|____/|_|  |_|  \____(_)___/  |_| \___/|____/

              version {}
        {}
        "#,
        version,
        repository.bright_white().bold()
    );

    // Parse input arguments
    let args: Args = Args::parse();

    if !args.terrain {
        println!(
            "{} Flat terrain mode (use --terrain for real elevation)",
            "Note:".yellow()
        );
    }

    // ── 1. Fetch OSM data ──────────────────────────────────────────────
    println!("{} Fetching OSM data...", "[1/6]".bold());
    let raw_data = match &args.file {
        Some(file) => retrieve_data::fetch_data_from_file(file),
        None if args.tiled_fetch => retrieve_data::fetch_data_from_overpass_tiled(
            args.bbox,
            args.debug,
            args.downloader.as_str(),
            args.save_json_file.as_deref(),
            args.fetch_tile_degrees,
            args.tile_cache_dir.as_deref(),
        ),
        None => retrieve_data::fetch_data_from_overpass(
            args.bbox,
            args.debug,
            args.downloader.as_str(),
            args.save_json_file.as_deref(),
        ),
    }
    .expect("Failed to fetch OSM data");

    // ── 2. Generate ground (elevation + land cover) ────────────────────
    let ground = Arc::new(ground::generate_ground_data(
        args.terrain,
        &args.bbox,
        args.scale,
        args.ground_level,
        args.land_cover,
        false,
        false,
    ));

    // ── 3. Parse OSM data ──────────────────────────────────────────────
    println!("{} Parsing OSM data...", "[3/6]".bold());
    let (parsed_elements, xzbbox) =
        osm_parser::parse_osm_data_no_outline(raw_data, args.bbox, args.scale, args.debug);

    // Print area stats
    let total = parsed_elements.len();
    println!(
        "  {} elements in {} × {} block world",
        total.to_string().bright_white(),
        (xzbbox.max_x() - xzbbox.min_x()).to_string().bright_white(),
        (xzbbox.max_z() - xzbbox.min_z()).to_string().bright_white()
    );

    // ── 4. Process elements → SceneWriter ────────────────────────────
    let mut scene = data_processing::process_elements(
        parsed_elements,
        &xzbbox,
        Arc::clone(&ground),
        args.path.clone(),
        args.godot_scale,
        args.chunk_size,
    )
    .expect("Failed to process elements");
    scene.stream_radius = args.stream_radius;

    // ── 5. Generate terrain meshes ────────────────────────────────────
    println!("{} Generating terrain...", "[5/6]".bold());
    ground_generation::generate_terrain(&mut scene.chunk_grid, &ground, args.godot_scale);

    // ── 6. Save all scenes ────────────────────────────────────────────
    println!("{} Saving Godot scenes...", "[6/6]".bold());
    scene.save_all().expect("Failed to save scene files");

    println!();
    println!(
        "{} Output written to: {}",
        "Done!".green().bold(),
        args.path.display().to_string().bright_white()
    );
    println!(
        "  Open {} in Godot 4.x Editor to view the world.",
        args.path.join("project.godot").display()
    );
}

fn main() {
    run();
}
