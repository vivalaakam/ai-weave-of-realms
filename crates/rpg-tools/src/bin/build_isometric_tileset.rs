//! Generates a shared isometric tileset atlas for Godot and TMX export.
//!
//! The atlas is derived directly from `rpg_engine::map::tile::Tiles`, so
//! tile order and representative colors stay aligned with runtime logic.

use std::fs;
use std::path::Path;

use image::{ImageBuffer, Rgba, RgbaImage};
use tracing::{error, info};

use rpg_engine::map::tile::Tiles;

const TILE_SIZE: u32 = 64;
const HALF_TILE_W: i32 = 32;
const HALF_TILE_H: i32 = 16;
const DIAMOND_CENTER_X: i32 = 32;
const DIAMOND_CENTER_Y: i32 = 32;
const TILESET_OUTPUTS: [&str; 2] = ["godot/assets/tileset.png", "tileset/tileset.png"];

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let atlas = render_tileset();

    for path in TILESET_OUTPUTS {
        if let Some(parent) = Path::new(path).parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                error!(%error, output = %path, "failed to create tileset output directory");
                std::process::exit(1);
            }
        }

        if let Err(error) = atlas.save(path) {
            error!(%error, output = %path, "failed to save tileset atlas");
            std::process::exit(1);
        }

        info!(output = %path, "wrote isometric tileset atlas");
    }
}

fn render_tileset() -> RgbaImage {
    let atlas_width = TILE_SIZE * Tiles::all().len() as u32;
    let mut atlas: RgbaImage = ImageBuffer::from_pixel(atlas_width, TILE_SIZE, Rgba([0, 0, 0, 0]));

    for (index, tile) in Tiles::all().iter().copied().enumerate() {
        render_tile(&mut atlas, index as u32 * TILE_SIZE, tile);
    }

    atlas
}

fn render_tile(image: &mut RgbaImage, offset_x: u32, tile: Tiles) {
    let base = rgba(tile.as_color(), 255);
    let top = lighten(base, 0.22);
    let bottom = darken(base, 0.18);
    let outline = darken(base, 0.42);

    fill_diamond(image, offset_x, top, bottom, outline);
    add_surface_noise(image, offset_x, tile, base);
    draw_marker(image, offset_x, tile, base);
}

fn fill_diamond(
    image: &mut RgbaImage,
    offset_x: u32,
    top_color: Rgba<u8>,
    bottom_color: Rgba<u8>,
    outline: Rgba<u8>,
) {
    for y in 0..TILE_SIZE as i32 {
        for x in 0..TILE_SIZE as i32 {
            if !inside_diamond(x, y) {
                continue;
            }

            let t = ((y - (DIAMOND_CENTER_Y - HALF_TILE_H)) as f32 / (HALF_TILE_H * 2) as f32)
                .clamp(0.0, 1.0);
            let color = mix(top_color, bottom_color, t);
            put_pixel(image, offset_x, x, y, color);
        }
    }

    for x in 0..TILE_SIZE as i32 {
        for y in 0..TILE_SIZE as i32 {
            if !inside_diamond(x, y) {
                continue;
            }
            if is_diamond_border(x, y) {
                put_pixel(image, offset_x, x, y, outline);
            }
        }
    }
}

fn add_surface_noise(image: &mut RgbaImage, offset_x: u32, tile: Tiles, base: Rgba<u8>) {
    let accent = lighten(base, 0.12);
    let shadow = darken(base, 0.12);

    match tile {
        Tiles::Meadow | Tiles::CityEntrance => {
            draw_line(image, offset_x, 20, 28, 28, 24, accent);
            draw_line(image, offset_x, 36, 38, 42, 34, accent);
            draw_line(image, offset_x, 26, 36, 30, 42, shadow);
        }
        Tiles::Forest => {
            draw_line(image, offset_x, 18, 36, 26, 28, shadow);
            draw_line(image, offset_x, 40, 36, 46, 30, shadow);
        }
        Tiles::Mountain => {
            draw_line(image, offset_x, 16, 37, 30, 22, accent);
            draw_line(image, offset_x, 30, 22, 45, 37, shadow);
        }
        Tiles::Water | Tiles::River | Tiles::Bridge => {
            draw_line(image, offset_x, 18, 29, 28, 25, accent);
            draw_line(image, offset_x, 30, 36, 44, 31, accent);
        }
        _ => {}
    }
}

fn draw_marker(image: &mut RgbaImage, offset_x: u32, tile: Tiles, base: Rgba<u8>) {
    match tile {
        Tiles::Meadow => {}
        Tiles::Forest => {
            let trunk = darken(base, 0.55);
            let canopy = darken(base, 0.2);
            fill_triangle(image, offset_x, (24, 37), (32, 21), (40, 37), canopy);
            fill_rect(image, offset_x, 30, 37, 4, 6, trunk);
            fill_triangle(
                image,
                offset_x,
                (15, 35),
                (21, 24),
                (27, 35),
                lighten(canopy, 0.08),
            );
            fill_rect(image, offset_x, 19, 35, 3, 5, trunk);
        }
        Tiles::Mountain => {
            let peak = lighten(base, 0.18);
            let ridge = darken(base, 0.28);
            fill_triangle(image, offset_x, (16, 40), (28, 18), (39, 40), peak);
            fill_triangle(image, offset_x, (28, 18), (39, 40), (47, 40), ridge);
            fill_triangle(
                image,
                offset_x,
                (30, 24),
                (34, 18),
                (37, 25),
                rgba((245, 245, 245), 255),
            );
        }
        Tiles::Water => {
            let foam = rgba((210, 235, 255), 220);
            draw_line(image, offset_x, 16, 30, 25, 27, foam);
            draw_line(image, offset_x, 26, 27, 35, 30, foam);
            draw_line(image, offset_x, 34, 35, 46, 31, foam);
        }
        Tiles::City => {
            let wall = rgba((250, 223, 173), 255);
            let roof = rgba((191, 84, 47), 255);
            fill_rect(image, offset_x, 22, 27, 20, 12, wall);
            fill_triangle(image, offset_x, (20, 27), (32, 18), (44, 27), roof);
            fill_rect(image, offset_x, 29, 32, 6, 7, darken(wall, 0.25));
        }
        Tiles::CityEntrance => {
            let gate = rgba((246, 215, 138), 255);
            let path = rgba((164, 103, 58), 255);
            fill_rect(image, offset_x, 23, 24, 18, 6, gate);
            fill_rect(image, offset_x, 20, 30, 24, 5, gate);
            draw_line(image, offset_x, 22, 35, 32, 40, path);
            draw_line(image, offset_x, 32, 40, 42, 35, path);
        }
        Tiles::Road => {
            let dirt = rgba((166, 110, 69), 255);
            let edge = rgba((227, 196, 165), 220);
            fill_quad(
                image,
                offset_x,
                [(14, 31), (22, 26), (50, 34), (42, 39)],
                dirt,
            );
            draw_line(image, offset_x, 16, 31, 48, 35, edge);
        }
        Tiles::River => {
            let water = rgba((111, 189, 255), 255);
            let foam = rgba((229, 245, 255), 220);
            fill_quad(
                image,
                offset_x,
                [(12, 30), (21, 24), (52, 34), (42, 40)],
                water,
            );
            draw_line(image, offset_x, 16, 31, 46, 35, foam);
        }
        Tiles::Bridge => {
            let water = rgba((111, 189, 255), 220);
            let planks = rgba((142, 95, 58), 255);
            fill_quad(
                image,
                offset_x,
                [(12, 30), (21, 24), (52, 34), (42, 40)],
                water,
            );
            fill_quad(
                image,
                offset_x,
                [(17, 31), (24, 27), (47, 34), (40, 38)],
                planks,
            );
            for step in 0..5 {
                let x = 21 + step * 5;
                draw_line(
                    image,
                    offset_x,
                    x,
                    29 + (step / 2),
                    x + 2,
                    36 - (step / 2),
                    darken(planks, 0.25),
                );
            }
        }
        Tiles::Village => {
            let roof = rgba((222, 78, 106), 255);
            let wall = rgba((255, 221, 195), 255);
            fill_rect(image, offset_x, 24, 28, 16, 10, wall);
            fill_triangle(image, offset_x, (22, 28), (32, 20), (42, 28), roof);
        }
        Tiles::Merchant => {
            let tent = rgba((236, 170, 252), 255);
            let pole = rgba((117, 70, 27), 255);
            fill_triangle(image, offset_x, (21, 38), (32, 22), (43, 38), tent);
            fill_rect(image, offset_x, 31, 38, 2, 5, pole);
        }
        Tiles::Ruins => {
            let stone = rgba((218, 160, 94), 255);
            let crack = rgba((112, 74, 33), 255);
            fill_rect(image, offset_x, 22, 27, 6, 13, stone);
            fill_rect(image, offset_x, 33, 23, 7, 17, stone);
            draw_line(image, offset_x, 36, 23, 34, 31, crack);
            draw_line(image, offset_x, 26, 27, 24, 35, crack);
        }
        Tiles::Gold => {
            let nugget = rgba((251, 233, 92), 255);
            fill_circle(image, offset_x, 32, 30, 8, nugget);
            fill_circle(image, offset_x, 27, 35, 4, lighten(nugget, 0.15));
        }
        Tiles::Resource => {
            let gem = rgba((137, 244, 255), 255);
            fill_quad(
                image,
                offset_x,
                [(32, 21), (41, 30), (32, 39), (23, 30)],
                gem,
            );
            draw_line(image, offset_x, 32, 21, 32, 39, darken(gem, 0.22));
            draw_line(image, offset_x, 23, 30, 41, 30, darken(gem, 0.22));
        }
    }
}

fn inside_diamond(x: i32, y: i32) -> bool {
    let dx = (x - DIAMOND_CENTER_X).abs();
    let dy = (y - DIAMOND_CENTER_Y).abs();
    dx * HALF_TILE_H + dy * HALF_TILE_W <= HALF_TILE_W * HALF_TILE_H
}

fn is_diamond_border(x: i32, y: i32) -> bool {
    if !inside_diamond(x, y) {
        return false;
    }

    !inside_diamond(x - 1, y)
        || !inside_diamond(x + 1, y)
        || !inside_diamond(x, y - 1)
        || !inside_diamond(x, y + 1)
}

fn fill_rect(
    image: &mut RgbaImage,
    offset_x: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: Rgba<u8>,
) {
    for py in y..(y + h) {
        for px in x..(x + w) {
            put_pixel_if_inside(image, offset_x, px, py, color);
        }
    }
}

fn fill_circle(
    image: &mut RgbaImage,
    offset_x: u32,
    cx: i32,
    cy: i32,
    radius: i32,
    color: Rgba<u8>,
) {
    let radius_sq = radius * radius;
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius_sq {
                put_pixel_if_inside(image, offset_x, x, y, color);
            }
        }
    }
}

fn fill_triangle(
    image: &mut RgbaImage,
    offset_x: u32,
    a: (i32, i32),
    b: (i32, i32),
    c: (i32, i32),
    color: Rgba<u8>,
) {
    let min_x = a.0.min(b.0).min(c.0);
    let max_x = a.0.max(b.0).max(c.0);
    let min_y = a.1.min(b.1).min(c.1);
    let max_y = a.1.max(b.1).max(c.1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if point_in_triangle((x, y), a, b, c) {
                put_pixel_if_inside(image, offset_x, x, y, color);
            }
        }
    }
}

fn fill_quad(image: &mut RgbaImage, offset_x: u32, points: [(i32, i32); 4], color: Rgba<u8>) {
    fill_triangle(image, offset_x, points[0], points[1], points[2], color);
    fill_triangle(image, offset_x, points[0], points[2], points[3], color);
}

fn point_in_triangle(p: (i32, i32), a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> bool {
    let d1 = edge_function(p, a, b);
    let d2 = edge_function(p, b, c);
    let d3 = edge_function(p, c, a);

    let has_neg = d1 < 0 || d2 < 0 || d3 < 0;
    let has_pos = d1 > 0 || d2 > 0 || d3 > 0;
    !(has_neg && has_pos)
}

fn edge_function(p: (i32, i32), a: (i32, i32), b: (i32, i32)) -> i32 {
    (p.0 - b.0) * (a.1 - b.1) - (a.0 - b.0) * (p.1 - b.1)
}

fn draw_line(
    image: &mut RgbaImage,
    offset_x: u32,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba<u8>,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        put_pixel_if_inside(image, offset_x, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn put_pixel_if_inside(image: &mut RgbaImage, offset_x: u32, x: i32, y: i32, color: Rgba<u8>) {
    if !inside_diamond(x, y) {
        return;
    }
    put_pixel(image, offset_x, x, y, color);
}

fn put_pixel(image: &mut RgbaImage, offset_x: u32, x: i32, y: i32, color: Rgba<u8>) {
    if x < 0 || y < 0 || x >= TILE_SIZE as i32 || y >= TILE_SIZE as i32 {
        return;
    }

    let atlas_x = offset_x + x as u32;
    image.put_pixel(atlas_x, y as u32, color);
}

fn mix(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
    let mix_channel = |left: u8, right: u8| -> u8 {
        (left as f32 + (right as f32 - left as f32) * t).round() as u8
    };

    Rgba([
        mix_channel(a[0], b[0]),
        mix_channel(a[1], b[1]),
        mix_channel(a[2], b[2]),
        mix_channel(a[3], b[3]),
    ])
}

fn lighten(color: Rgba<u8>, amount: f32) -> Rgba<u8> {
    tint(color, 255, amount)
}

fn darken(color: Rgba<u8>, amount: f32) -> Rgba<u8> {
    tint(color, 0, amount)
}

fn tint(color: Rgba<u8>, target: u8, amount: f32) -> Rgba<u8> {
    let blend = |channel: u8| -> u8 {
        (channel as f32 + (target as f32 - channel as f32) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };

    Rgba([blend(color[0]), blend(color[1]), blend(color[2]), color[3]])
}

fn rgba(rgb: (u8, u8, u8), alpha: u8) -> Rgba<u8> {
    Rgba([rgb.0, rgb.1, rgb.2, alpha])
}
