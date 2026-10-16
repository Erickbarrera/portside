//! Artwork for Steam's library, cut from one picture: the tall cover Game Mode and Big
//! Picture show, the wide one desktop Steam shows, and the banner behind a game's page.

use std::path::Path;

use image::imageops::FilterType;

use crate::{SteamError, io};

/// (file name suffix, width, height), as Steam names and sizes them.
const SIZES: [(&str, u32, u32); 3] = [("p", 600, 900), ("", 920, 430), ("_hero", 1920, 620)];

/// Writes the artwork for shortcut `appid` into `grid` from `picture` (PNG, JPEG, WebP or
/// GIF), each cropped from its middle to fit.
pub fn write_art(grid: &Path, appid: u32, picture: &[u8]) -> Result<(), SteamError> {
    let img = image::load_from_memory(picture)?;
    std::fs::create_dir_all(grid).map_err(io(grid))?;
    for (suffix, w, h) in SIZES {
        let path = grid.join(format!("{appid}{suffix}.png"));
        img.resize_to_fill(w, h, FilterType::Triangle)
            .save_with_format(&path, image::ImageFormat::Png)?;
    }
    Ok(())
}
