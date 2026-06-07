use crate::coordinate_system::{
    cartesian::XZPoint,
    geographic::LLBBox,
};
use crate::elevation::compute_grid_dims;
use crate::elevation_data::{fetch_elevation_data, ElevationData};
use crate::land_cover::{self, LandCoverData};
use colored::Colorize;

/// Represents terrain data: elevation + land cover classification.
#[derive(Clone)]
pub struct Ground {
    pub elevation_enabled: bool,
    ground_level: i32,
    elevation_data: Option<ElevationData>,
    land_cover: Option<LandCoverData>,
}

impl Ground {
    /// Flat ground (no elevation). Used when --no-terrain is given.
    pub fn new_flat(ground_level: i32) -> Self {
        Self {
            elevation_enabled: false,
            ground_level,
            elevation_data: None,
            land_cover: None,
        }
    }

    /// Fetch elevation + optional land cover for the given bbox.
    pub fn new_enabled(
        bbox: &LLBBox,
        scale: f64,
        ground_level: i32,
        fetch_land_cover: bool,
        disable_height_limit: bool,
        aws_only_elevation: bool,
    ) -> Self {
        // Fetch land cover FIRST so it can feed into elevation post-processing.
        let (_, _, grid_w, grid_h) = compute_grid_dims(bbox, scale);
        let mut land_cover = if fetch_land_cover {
            let lc = land_cover::fetch_land_cover_data(bbox, grid_w, grid_h);
            if lc.is_some() {
                println!("Land cover data loaded successfully");
            } else {
                eprintln!("Warning: Land cover data unavailable, using default ground");
            }
            lc
        } else {
            None
        };

        // Use a reasonable max Y for Godot scenes. In arnis this is Minecraft-specific,
        // but we still need to cap the height range. 2000 blocks = 1000 m Godot scale.
        let extended_max_y = if disable_height_limit { 2000 } else { 320 };

        match fetch_elevation_data(
            bbox,
            scale,
            ground_level,
            disable_height_limit,
            extended_max_y,
            land_cover.as_mut(),
            aws_only_elevation,
        ) {
            Ok(elevation_data) => Self {
                elevation_enabled: true,
                ground_level,
                elevation_data: Some(elevation_data),
                land_cover,
            },
            Err(e) => {
                eprintln!("Failed to fetch elevation data: {}", e);
                eprintln!("Falling back to flat terrain.");
                Self {
                    elevation_enabled: false,
                    ground_level,
                    elevation_data: None,
                    land_cover: None,
                }
            }
        }
    }

    /// Returns whether land cover data is available.
    #[inline(always)]
    pub fn has_land_cover(&self) -> bool {
        self.land_cover.is_some()
    }

    /// Returns the ESA WorldCover land cover class at the given coordinates.
    #[inline(always)]
    pub fn cover_class(&self, coord: XZPoint) -> u8 {
        if let (Some(ref lc), Some(ref data)) = (&self.land_cover, &self.elevation_data) {
            let x_ratio = (coord.x as f64 / (data.world_width - 1).max(1) as f64).clamp(0.0, 1.0);
            let z_ratio = (coord.z as f64 / (data.world_height - 1).max(1) as f64).clamp(0.0, 1.0);
            let x = ((x_ratio * (lc.width - 1) as f64).round() as usize).min(lc.width - 1);
            let z = ((z_ratio * (lc.height - 1) as f64).round() as usize).min(lc.height - 1);
            lc.grid[z][x]
        } else {
            0
        }
    }

    /// Returns the terrain elevation at world coordinates (arnis block units).
    #[inline(always)]
    pub fn level(&self, coord: XZPoint) -> i32 {
        if !self.elevation_enabled || self.elevation_data.is_none() {
            return self.ground_level;
        }

        let data: &ElevationData = self.elevation_data.as_ref().unwrap();
        let (x_ratio, z_ratio) = self.get_data_coordinates(coord, data);
        self.interpolate_height(x_ratio, z_ratio, data)
    }

    /// Returns elevation as a Godot Y coordinate (meters).
    /// Converts from arnis block units using GODOT_SCALE.
    #[inline(always)]
    pub fn level_godot(&self, coord: XZPoint, block_to_meters: f32) -> f32 {
        self.level(coord) as f32 * block_to_meters
    }

    /// Computes terrain slope at the given coordinates (in blocks).
    #[inline(always)]
    pub fn slope(&self, coord: XZPoint) -> i32 {
        if !self.elevation_enabled {
            return 0;
        }

        const STEP: i32 = 4;
        let east = self.level(XZPoint::new(coord.x + STEP, coord.z));
        let west = self.level(XZPoint::new(coord.x - STEP, coord.z));
        let north = self.level(XZPoint::new(coord.x, coord.z - STEP));
        let south = self.level(XZPoint::new(coord.x, coord.z + STEP));

        let max_val = east.max(west).max(north).max(south);
        let min_val = east.min(west).min(north).min(south);
        max_val.saturating_sub(min_val)
    }

    /// Returns the minimum ground level among all points.
    #[allow(unused)]
    #[inline(always)]
    pub fn min_level<I: Iterator<Item = XZPoint>>(&self, coords: I) -> Option<i32> {
        if !self.elevation_enabled {
            return Some(self.ground_level);
        }
        coords.map(|c| self.level(c)).min()
    }

    /// Returns the maximum ground level among all points.
    #[allow(unused)]
    #[inline(always)]
    pub fn max_level<I: Iterator<Item = XZPoint>>(&self, coords: I) -> Option<i32> {
        if !self.elevation_enabled {
            return Some(self.ground_level);
        }
        coords.map(|c| self.level(c)).max()
    }

    /// Width of the world in arnis block units.
    pub fn world_width(&self) -> usize {
        self.elevation_data
            .as_ref()
            .map(|d| d.world_width)
            .unwrap_or(0)
    }

    /// Height of the world in arnis block units.
    pub fn world_height(&self) -> usize {
        self.elevation_data
            .as_ref()
            .map(|d| d.world_height)
            .unwrap_or(0)
    }

    /// Reference to the raw elevation grid (block-space heights, f32).
    pub fn height_grid(&self) -> Option<&[Vec<f32>]> {
        self.elevation_data.as_ref().map(|d| &d.heights[..])
    }

    /// Grid width (may be capped smaller than world).
    pub fn grid_width(&self) -> usize {
        self.elevation_data.as_ref().map(|d| d.width).unwrap_or(0)
    }

    /// Grid height.
    pub fn grid_height(&self) -> usize {
        self.elevation_data.as_ref().map(|d| d.height).unwrap_or(0)
    }

    /// Reference to the land cover grid if available.
    pub fn land_cover_grid(&self) -> Option<&[Vec<u8>]> {
        self.land_cover.as_ref().map(|lc| &lc.grid[..])
    }

    /// Land cover grid width.
    pub fn land_cover_width(&self) -> Option<usize> {
        self.land_cover.as_ref().map(|lc| lc.width)
    }

    /// Land cover grid height.
    pub fn land_cover_height(&self) -> Option<usize> {
        self.land_cover.as_ref().map(|lc| lc.height)
    }

    // ─── private helpers ──────────────────────────────────────────────────

    #[inline(always)]
    fn get_data_coordinates(&self, coord: XZPoint, data: &ElevationData) -> (f64, f64) {
        let x_ratio = coord.x as f64 / (data.world_width - 1).max(1) as f64;
        let z_ratio = coord.z as f64 / (data.world_height - 1).max(1) as f64;
        (x_ratio.clamp(0.0, 1.0), z_ratio.clamp(0.0, 1.0))
    }

    #[inline(always)]
    fn interpolate_height(&self, x_ratio: f64, z_ratio: f64, data: &ElevationData) -> i32 {
        let fx = x_ratio * (data.width - 1) as f64;
        let fz = z_ratio * (data.height - 1) as f64;
        let x0 = fx.floor() as usize;
        let z0 = fz.floor() as usize;
        let x1 = (x0 + 1).min(data.width - 1);
        let z1 = (z0 + 1).min(data.height - 1);
        let dx = fx - x0 as f64;
        let dz = fz - z0 as f64;

        let v00 = data.heights[z0][x0] as f64;
        let v10 = data.heights[z0][x1] as f64;
        let v01 = data.heights[z1][x0] as f64;
        let v11 = data.heights[z1][x1] as f64;
        let lerp_top = v00 + (v10 - v00) * dx;
        let lerp_bot = v01 + (v11 - v01) * dx;
        let result = lerp_top + (lerp_bot - lerp_top) * dz;
        result.round() as i32
    }
}

/// Generate ground data from CLI arguments.
pub fn generate_ground_data(
    terrain: bool,
    bbox: &LLBBox,
    scale: f64,
    ground_level: i32,
    land_cover: bool,
    disable_height_limit: bool,
    aws_only_elevation: bool,
) -> Ground {
    if terrain {
        println!("{} Fetching elevation...", "[2/6]".bold());
        Ground::new_enabled(
            bbox,
            scale,
            ground_level,
            land_cover,
            disable_height_limit,
            aws_only_elevation,
        )
    } else {
        Ground::new_flat(ground_level)
    }
}
