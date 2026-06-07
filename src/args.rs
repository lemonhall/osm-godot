use crate::coordinate_system::geographic::LLBBox;
use clap::Parser;
use std::path::PathBuf;

/// osm-godot — Generate Godot Engine 3D scenes from real-world OpenStreetMap data.
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// Bounding box of the area (min_lat,min_lng,max_lat,max_lng)
    #[arg(long, allow_hyphen_values = true, value_parser = LLBBox::from_str)]
    pub bbox: LLBBox,

    /// JSON file containing pre-fetched OSM data (optional — fetches from Overpass if omitted)
    #[arg(long)]
    pub file: Option<String>,

    /// JSON file to save downloaded OSM data to (for caching/reuse)
    #[arg(long)]
    pub save_json_file: Option<String>,

    /// Output directory for the Godot project
    #[arg(long = "output-dir", alias = "path", default_value = "./osm_godot_output")]
    pub path: PathBuf,

    /// World scale: arnis blocks per meter. 1.0 = 1 block/m, 2.0 = 2 blocks/m (more detail)
    #[arg(long, default_value_t = 1.0)]
    pub scale: f64,

    /// Godot unit scale: meters per arnis block. Default 0.5 = 1 block = 50cm in Godot
    #[arg(long, default_value_t = 0.5)]
    pub godot_scale: f32,

    /// Ground level in arnis block units (0 = sea level)
    #[arg(long, default_value_t = 0)]
    pub ground_level: i32,

    /// Enable terrain elevation (fetches DEM satellite data)
    #[arg(long)]
    pub terrain: bool,

    /// Enable land cover classification (ESA WorldCover)
    #[arg(long = "land-cover", default_value_t = true)]
    pub land_cover: bool,

    /// Chunk size in Godot meters. Default 128 = 64m × 64m chunks.
    #[arg(long, default_value_t = 128)]
    pub chunk_size: i32,

    /// Enable debug mode (extra logging + debug files)
    #[arg(long)]
    pub debug: bool,

    /// Downloader method (requests/curl/wget)
    #[arg(long, default_value = "requests")]
    pub downloader: String,
}
