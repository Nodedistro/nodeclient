use macroquad::prelude::*;

/// Original deterministic 16px materials. No downloaded or copied textures.
pub fn atlas() -> Texture2D {
    let mut image = Image::gen_image_color(160, 16, WHITE);
    for tile in 0u32..10 {
        for y in 0u32..16 {
            for x in 0u32..16 {
                let mut hash = (x + tile * 29).wrapping_mul(374761393) ^ y.wrapping_mul(668265263);
                hash = (hash ^ (hash >> 13)).wrapping_mul(1274126177);
                let grain = ((hash >> 16) & 255) as f32 / 255.0;
                let material = match tile {
                    8 if y < 3 + (x * 7 % 3) => 1,
                    8 => 2,
                    9 => 7,
                    _ => tile as usize,
                };
                let base = crate::COLORS[material];
                let pattern = match tile {
                    5 => {
                        if (x + (y / 5)).is_multiple_of(5) {
                            0.60
                        } else {
                            0.95
                        }
                    }
                    7 => {
                        if y.is_multiple_of(5) || (x + (y / 5) * 7).is_multiple_of(16) {
                            0.68
                        } else {
                            1.0
                        }
                    }
                    9 => {
                        let radius = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as u32;
                        if radius.is_multiple_of(3) {
                            0.66
                        } else {
                            1.0
                        }
                    }
                    6 if grain < 0.25 => 0.65,
                    _ => 1.0,
                };
                let shade = (0.82 + grain * 0.24) * pattern;
                image.set_pixel(
                    tile * 16 + x,
                    y,
                    Color::new(base.r * shade, base.g * shade, base.b * shade, 1.0),
                );
            }
        }
    }
    let atlas = Texture2D::from_image(&image);
    atlas.set_filter(FilterMode::Nearest);
    atlas
}
