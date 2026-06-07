use crate::coordinate_system::geographic::LLBBox;
use crate::osm_parser::OsmData;
use crate::progress::{emit_gui_error, emit_gui_progress_update, is_running_with_gui};
use colored::Colorize;
use rand::prelude::SliceRandom;
use rand::Rng;
use reqwest::blocking::Client;
use reqwest::blocking::ClientBuilder;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufReader, Cursor, Write};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Extract the host portion of a URL for telemetry
fn url_host(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    after_scheme
        .split(['/', '?'])
        .next()
        .unwrap_or(after_scheme)
        .to_string()
}

/// Function to download data using reqwest
fn download_with_reqwest(
    url: &str,
    query: &str,
    timeout_secs: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    let client: Client = ClientBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(concat!(
            "Arnis/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/louis-e/arnis)"
        ))
        .build()?;

    let response: Result<reqwest::blocking::Response, reqwest::Error> =
        client.get(url).query(&[("data", query)]).send();

    match response {
        Ok(resp) => {
            emit_gui_progress_update(3.0, "Downloading data...");
            if resp.status().is_success() {
                let text = resp.text()?;
                if text.is_empty() {
                    return Err("Received invalid data from server".into());
                }
                Ok(text)
            } else {
                let status = resp.status();
                let user_msg = match status.as_u16() {
                    429 => "Rate limited. Try again later.".to_string(),
                    403 => "Server overloaded. Try again.".to_string(),
                    500 | 502 | 503 | 504 => "Server unavailable. Try again.".to_string(),
                    _ => format!("Response code: {}", status.as_u16()),
                };
                eprintln!("{}", format!("Error! {user_msg}").red().bold());
                Err(user_msg.into())
            }
        }
        Err(e) => {
            if e.is_timeout() {
                let msg = "Request timed out. Try again!";
                eprintln!("{}", format!("Error! {msg}").red().bold());
                Err(msg.into())
            } else if e.is_connect() {
                let msg = "No internet connection.";
                eprintln!("{}", format!("Error! {msg}").red().bold());
                Err(msg.into())
            } else {
                let short: String = e.to_string().chars().take(52).collect();
                eprintln!("{}", format!("Error! {short}").red().bold());
                Err(short.into())
            }
        }
    }
}

/// Function to download data using `curl`
fn download_with_curl(url: &str, query: &str) -> io::Result<String> {
    let output: std::process::Output = Command::new("curl")
        .arg("-s") // Add silent mode to suppress output
        .arg(format!("{url}?data={query}"))
        .output()?;

    if !output.status.success() {
        Err(io::Error::other("Curl command failed"))
    } else {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Function to download data using `wget`
fn download_with_wget(url: &str, query: &str) -> io::Result<String> {
    let output: std::process::Output = Command::new("wget")
        .arg("-qO-") // Use `-qO-` to output the result directly to stdout
        .arg(format!("{url}?data={query}"))
        .output()?;

    if !output.status.success() {
        Err(io::Error::other("Wget command failed"))
    } else {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

pub fn fetch_data_from_file(file: &str) -> Result<OsmData, Box<dyn std::error::Error>> {
    println!("{} Loading data from file...", "[1/7]".bold());
    emit_gui_progress_update(1.0, "Loading data from file...");

    let file: File = File::open(file)?;
    let reader: BufReader<File> = BufReader::new(file);
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let data: OsmData = OsmData::deserialize(&mut deserializer)?;
    Ok(data)
}

/// Main function to fetch data
pub fn fetch_data_from_overpass(
    bbox: LLBBox,
    debug: bool,
    download_method: &str,
    save_file: Option<&str>,
) -> Result<OsmData, Box<dyn std::error::Error>> {
    println!("{} Fetching data...", "[1/7]".bold());
    emit_gui_progress_update(1.0, "Fetching data...");

    // List of Overpass API servers
    let arnis_api_server = "https://api.arnismc.com/overpass/api/interpreter";
    let api_servers: Vec<&str> = vec![
        "https://overpass-api.de/api/interpreter",
        "https://lz4.overpass-api.de/api/interpreter",
        "https://z.overpass-api.de/api/interpreter",
    ];
    let fallback_api_servers: Vec<&str> = vec![
        "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
        "https://overpass.private.coffee/api/interpreter",
    ];

    // Generate Overpass API query for bounding box.
    // Ocean/coastal elements are excluded because ESA WorldCover satellite data
    // handles ocean detection more reliably at 10m resolution (LC_WATER class).
    // Inland water (lakes, rivers, ponds) is still fetched from OSM.
    let query: String = format!(
        r#"[out:json][timeout:360][bbox:{},{},{},{}];
    (
        nwr["building"];
        nwr["building:part"];
        relation["type"="building"];
        nwr["highway"];
        nwr["landuse"]["landuse"!="salt_pond"];
        nwr["natural"]["natural"!="coastline"]["natural"!="bay"]["natural"!="strait"];
        nwr["leisure"];
        nwr["water"]["water"!="bay"]["water"!="ocean"]["water"!="sea"]["tidal"!="yes"];
        nwr["waterway"]["waterway"!="tidal_channel"];
        nwr["amenity"];
        nwr["tourism"];
        nwr["bridge"];
        nwr["railway"];
        nwr["roller_coaster"];
        nwr["barrier"];
        nwr["entrance"];
        nwr["door"];
        nwr["power"];
        nwr["historic"];
        nwr["emergency"];
        nwr["advertising"];
        nwr["man_made"];
        nwr["aeroway"];
        nwr["3dmr"];
        way["place"]["place"!~"^(ocean|sea|bay|strait|sound|fjord)$"];
        way;
    )->.relsinbbox;
    (
        way(r.relsinbbox);
    )->.waysinbbox;
    (
        node(w.waysinbbox);
        node(w.relsinbbox);
    )->.nodesinbbox;
    .relsinbbox out body;
    .waysinbbox out body;
    .nodesinbbox out skel qt;"#,
        bbox.min().lat(),
        bbox.min().lng(),
        bbox.max().lat(),
        bbox.max().lng(),
    );

    {
        // Fetch data from Overpass API.
        // Strategy:
        // 1) 50% chance: probe one random official server first.
        // 2) If the probe does not succeed, run the normal path: arnis API once,
        //    then shuffled official, then shuffled fallback servers.
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum ServerKind {
            Primary,
            Fallback,
        }

        let mut rng = rand::rng();
        let mut request_plan: Vec<(&str, ServerKind)> = Vec::new();
        let mut probed_server: Option<&str> = None;

        if rng.random_bool(0.5) {
            let probe_idx = rng.random_range(0..api_servers.len());
            let probe_server = api_servers[probe_idx];
            request_plan.push((probe_server, ServerKind::Primary));
            probed_server = Some(probe_server);
        }

        request_plan.push((arnis_api_server, ServerKind::Primary));

        let mut shuffled_primary_servers = api_servers.clone();
        shuffled_primary_servers.shuffle(&mut rng);
        if let Some(probed_server) = probed_server {
            shuffled_primary_servers.retain(|&url| url != probed_server);
        }
        request_plan.extend(
            shuffled_primary_servers
                .into_iter()
                .map(|url| (url, ServerKind::Primary)),
        );

        let mut shuffled_fallback_servers = fallback_api_servers.clone();
        shuffled_fallback_servers.shuffle(&mut rng);
        request_plan.extend(
            shuffled_fallback_servers
                .into_iter()
                .map(|url| (url, ServerKind::Fallback)),
        );

        let first_fallback_index = request_plan
            .iter()
            .position(|(_, kind)| *kind == ServerKind::Fallback)
            .unwrap_or(request_plan.len());

        let total = request_plan.len();
        let mut last_error: Option<Box<dyn std::error::Error>> = None;
        let mut attempted_hosts: Vec<String> = Vec::new();
        let response: String = 'server_loop: {
            for (i, (url, kind)) in request_plan.iter().enumerate() {
                let timeout_secs = if url.contains("private.coffee") {
                    120
                } else {
                    360
                };
                println!("Downloading from {url} with method {download_method}...");
                let result = match download_method {
                    "requests" => download_with_reqwest(url, &query, timeout_secs),
                    "curl" => download_with_curl(url, &query).map_err(|e| e.into()),
                    "wget" => download_with_wget(url, &query).map_err(|e| e.into()),
                    _ => download_with_reqwest(url, &query, timeout_secs), // Default to requests
                };

                match result {
                    Ok(response) => break 'server_loop response,
                    Err(error) => {
                        if download_method != "requests" {
                            eprintln!("Request failed: {error}");
                        }
                        attempted_hosts.push(url_host(url));
                        last_error = Some(error);

                        if i + 1 < total {
                            let delay_secs = if *kind == ServerKind::Fallback { 5 } else { 3 };
                            println!("Retrying in {delay_secs}s (attempt {}/{total})...", i + 1);
                            std::thread::sleep(Duration::from_secs(delay_secs));
                            if i + 1 == first_fallback_index {
                                println!("Primary servers exhausted, trying fallback servers...");
                            }
                        }
                    }
                }
            }
            // All servers exhausted
            return Err(last_error.unwrap_or_else(|| "All servers failed".into()));
        };

        if let Some(save_file) = save_file {
            let mut file: File = File::create(save_file)?;
            file.write_all(response.as_bytes())?;
            println!("API response saved to: {save_file}");
        }

        let mut deserializer =
            serde_json::Deserializer::from_reader(Cursor::new(response.as_bytes()));
        let data: OsmData = OsmData::deserialize(&mut deserializer)?;

        if data.is_empty() {
            // Distinguish a real server error (memory/runtime) from a benign
            // "this bbox has no mapped objects" response. The former still
            // aborts; the latter is allowed because Arnis can generate
            // nature/terrain on its own from elevation + land-cover data,
            // and unmapped natural areas are common on OSM.
            if let Some(remark) = data.remark.as_deref() {
                if remark.contains("runtime error") && remark.contains("out of memory") {
                    eprintln!("{}", "Error! The query ran out of memory on the Overpass API server. Try using a smaller area.".red().bold());
                    emit_gui_error("Try using a smaller area.");

                    if debug {
                        println!("Additional debug information: {data:?}");
                    }

                    if !is_running_with_gui() {
                        std::process::exit(1);
                    } else {
                        return Err("Data fetch failed".into());
                    }
                } else {
                    // Non-fatal upstream remark (e.g. timeout that still returned an empty body).
                    eprintln!(
                        "{}",
                        format!("Warning: API returned: {remark}. Continuing without OSM data.")
                            .yellow()
                            .bold()
                    );
                }
            } else {
                eprintln!(
                    "{}",
                    "Warning: OSM API returned no data for this area. Continuing with terrain/nature only."
                        .yellow()
                        .bold()
                );
            }

            if debug {
                println!("Additional debug information: {data:?}");
            }
        }

        emit_gui_progress_update(5.0, "");

        Ok(data)
    }
}

pub fn split_bbox_for_tiled_fetch(bbox: LLBBox, tile_degrees: f64) -> Result<Vec<LLBBox>, String> {
    if !tile_degrees.is_finite() || tile_degrees <= 0.0 {
        return Err("tile_degrees must be a positive finite number".to_string());
    }

    let mut tiles = Vec::new();
    let mut lat = bbox.min().lat();
    while lat < bbox.max().lat() {
        let next_lat = (lat + tile_degrees).min(bbox.max().lat());
        let mut lng = bbox.min().lng();
        while lng < bbox.max().lng() {
            let next_lng = (lng + tile_degrees).min(bbox.max().lng());
            tiles.push(LLBBox::new(lat, lng, next_lat, next_lng)?);
            lng = next_lng;
        }
        lat = next_lat;
    }

    Ok(tiles)
}

pub fn merge_tiled_osm_data(parts: Vec<OsmData>) -> OsmData {
    let mut seen: HashSet<(String, u64)> = HashSet::new();
    let mut elements = Vec::new();
    let mut remarks = Vec::new();

    for part in parts {
        if let Some(remark) = part.remark {
            remarks.push(remark);
        }
        for element in part.elements {
            let key = (element.r#type.clone(), element.id);
            if seen.insert(key) {
                elements.push(element);
            }
        }
    }

    OsmData {
        elements,
        remark: if remarks.is_empty() {
            None
        } else {
            Some(remarks.join("\n"))
        },
    }
}

pub fn fetch_data_from_overpass_tiled(
    bbox: LLBBox,
    debug: bool,
    download_method: &str,
    save_file: Option<&str>,
    tile_degrees: f64,
    tile_cache_dir: Option<&Path>,
) -> Result<OsmData, Box<dyn std::error::Error>> {
    println!(
        "{} Fetching OSM data as tiled Overpass requests...",
        "[1/7]".bold()
    );
    let tiles = split_bbox_for_tiled_fetch(bbox, tile_degrees)?;
    println!("  {} fetch tiles", tiles.len().to_string().bright_white());

    if let Some(cache_dir) = tile_cache_dir {
        std::fs::create_dir_all(cache_dir)?;
    }

    let mut parts = Vec::new();
    let mut tile_sequence = 0usize;
    for (idx, tile) in tiles.iter().enumerate() {
        println!("  Fetching tile {} / {}: {:?}", idx + 1, tiles.len(), tile);
        let mut tile_parts = fetch_tile_adaptive(
            *tile,
            debug,
            download_method,
            tile_cache_dir,
            tile_degrees,
            (tile_degrees / 4.0).max(0.02),
            &mut tile_sequence,
        )?;
        parts.append(&mut tile_parts);
    }

    let merged = merge_tiled_osm_data(parts);
    println!(
        "  Merged tiled OSM data: {} unique elements",
        merged.elements.len().to_string().bright_white()
    );

    if let Some(save_file) = save_file {
        let mut file = File::create(save_file)?;
        serde_json::to_writer(&mut file, &merged)?;
        println!("Merged API response saved to: {save_file}");
    }

    Ok(merged)
}

fn fetch_tile_adaptive(
    tile: LLBBox,
    debug: bool,
    download_method: &str,
    tile_cache_dir: Option<&Path>,
    current_tile_degrees: f64,
    min_tile_degrees: f64,
    tile_sequence: &mut usize,
) -> Result<Vec<OsmData>, Box<dyn std::error::Error>> {
    if let Some(path) = find_cached_tile(tile_cache_dir, tile) {
        println!("  Loading cached tile: {}", path.display());
        return Ok(vec![fetch_data_from_file(path.to_string_lossy().as_ref())?]);
    }

    let cache_path = tile_cache_dir.map(|dir| tile_cache_path(dir, tile, *tile_sequence));
    *tile_sequence += 1;
    let cache_file = cache_path.as_ref().map(|p| p.to_string_lossy().to_string());

    match fetch_data_from_overpass(*&tile, debug, download_method, cache_file.as_deref()) {
        Ok(data) => Ok(vec![data]),
        Err(error) => {
            let lat_span = tile.max().lat() - tile.min().lat();
            let lng_span = tile.max().lng() - tile.min().lng();
            let max_span = lat_span.max(lng_span);
            if max_span <= min_tile_degrees {
                return Err(error);
            }

            let next_degrees = (current_tile_degrees / 2.0).max(min_tile_degrees);
            eprintln!(
                "{}",
                format!(
                    "Warning: tile failed at {:.5} degrees, splitting to {:.5}: {error}",
                    current_tile_degrees, next_degrees
                )
                .yellow()
                .bold()
            );

            let children = split_bbox_for_tiled_fetch(tile, next_degrees)?;
            let mut data = Vec::new();
            for child in children {
                let mut child_data = fetch_tile_adaptive(
                    child,
                    debug,
                    download_method,
                    tile_cache_dir,
                    next_degrees,
                    min_tile_degrees,
                    tile_sequence,
                )?;
                data.append(&mut child_data);
            }
            Ok(data)
        }
    }
}

fn tile_cache_path(dir: &Path, tile: LLBBox, sequence: usize) -> std::path::PathBuf {
    dir.join(format!(
        "tile_{sequence:04}_{:.5}_{:.5}_{:.5}_{:.5}.json",
        tile.min().lat(),
        tile.min().lng(),
        tile.max().lat(),
        tile.max().lng()
    ))
}

fn find_cached_tile(tile_cache_dir: Option<&Path>, tile: LLBBox) -> Option<std::path::PathBuf> {
    let dir = tile_cache_dir?;
    let suffix = format!(
        "{:.5}_{:.5}_{:.5}_{:.5}.json",
        tile.min().lat(),
        tile.min().lng(),
        tile.max().lat(),
        tile.max().lng()
    );
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.ends_with(&suffix) {
            return Some(path);
        }
    }
    None
}

/// Fetches a short area name using Nominatim for the given lat/lon
pub fn fetch_area_name(lat: f64, lon: f64) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!(
            "Arnis/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/louis-e/arnis)"
        ))
        .build()?;

    let url = format!("https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={lat}&lon={lon}&addressdetails=1");

    let resp = client.get(&url).send()?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let json: Value = resp.json()?;

    if let Some(address) = json.get("address") {
        let fields = ["city", "town", "village", "county", "borough", "suburb"];
        for field in fields.iter() {
            if let Some(name) = address.get(*field).and_then(|v| v.as_str()) {
                let mut name_str = name.to_string();

                // Remove "City of " prefix
                if name_str.to_lowercase().starts_with("city of ") {
                    name_str = name_str[name_str.find(" of ").unwrap() + 4..].to_string();
                }

                return Ok(Some(name_str));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osm_parser::OsmElement;

    #[test]
    fn tiled_fetch_split_bbox_covers_area_without_exceeding_bounds() {
        let bbox = LLBBox::new(31.0, 121.0, 31.09, 121.11).unwrap();
        let tiles = split_bbox_for_tiled_fetch(bbox, 0.04).unwrap();

        assert_eq!(tiles.len(), 9);
        assert_eq!(tiles.first().unwrap().min().lat(), bbox.min().lat());
        assert_eq!(tiles.first().unwrap().min().lng(), bbox.min().lng());
        assert_eq!(tiles.last().unwrap().max().lat(), bbox.max().lat());
        assert_eq!(tiles.last().unwrap().max().lng(), bbox.max().lng());
        assert!(tiles.iter().all(|tile| tile.min().lat() >= bbox.min().lat()
            && tile.max().lat() <= bbox.max().lat()
            && tile.min().lng() >= bbox.min().lng()
            && tile.max().lng() <= bbox.max().lng()));
    }

    #[test]
    fn tiled_fetch_merge_osm_data_deduplicates_by_type_and_id() {
        let first = OsmData {
            remark: None,
            elements: vec![
                OsmElement {
                    r#type: "node".to_string(),
                    id: 1,
                    lat: Some(31.0),
                    lon: Some(121.0),
                    nodes: None,
                    tags: None,
                    members: Vec::new(),
                },
                OsmElement {
                    r#type: "way".to_string(),
                    id: 10,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1]),
                    tags: None,
                    members: Vec::new(),
                },
            ],
        };
        let second = OsmData {
            remark: None,
            elements: vec![
                OsmElement {
                    r#type: "node".to_string(),
                    id: 1,
                    lat: Some(31.0),
                    lon: Some(121.0),
                    nodes: None,
                    tags: None,
                    members: Vec::new(),
                },
                OsmElement {
                    r#type: "way".to_string(),
                    id: 11,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1]),
                    tags: None,
                    members: Vec::new(),
                },
            ],
        };

        let merged = merge_tiled_osm_data(vec![first, second]);
        assert_eq!(merged.elements.len(), 3);
        let node_count = merged
            .elements
            .iter()
            .filter(|e| e.r#type == "node" && e.id == 1)
            .count();
        assert_eq!(node_count, 1);
        assert!(merged
            .elements
            .iter()
            .any(|e| e.r#type == "way" && e.id == 10));
        assert!(merged
            .elements
            .iter()
            .any(|e| e.r#type == "way" && e.id == 11));
    }
}
