use image::{ImageBuffer, Rgba, RgbaImage};
use std::io::Cursor;

/// Bitmap font: 5×7 pixel patterns for digits 0-9.
/// Each digit is stored as 7 rows of 5 bits (MSB-first).
const DIGIT_FONT: [[u8; 7]; 10] = [
    // 0
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    // 1
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 2
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
    // 3
    [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
    // 4
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    // 5
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    // 6
    [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
    // 7
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    // 8
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    // 9
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
];

/// Render a tray icon with a red badge showing the update count.
/// Returns PNG bytes suitable for `tauri::image::Image::from_bytes`.
pub fn render_tray_icon_with_badge(base_png: &[u8], count: usize) -> Option<Vec<u8>> {
    let base = image::load_from_memory_with_format(base_png, image::ImageFormat::Png).ok()?;
    let mut img: RgbaImage = base.to_rgba8();
    let (w, h) = (img.width(), img.height());

    if count == 0 {
        return Some(encode_png(&img));
    }

    let label = if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    };

    // Badge dimensions scale with icon size
    let badge_h = (h as f32 * 0.45).round() as u32;
    let char_h = 7u32; // bitmap font height
    let scale = (badge_h as f32 / (char_h as f32 + 4.0)).max(1.0).floor() as u32;
    let scaled_char_h = char_h * scale;
    let scaled_char_w = 5 * scale;
    let char_gap = scale;

    let text_w: u32 = label.len() as u32 * scaled_char_w + (label.len() as u32 - 1) * char_gap;
    let padding = scale * 2;
    let badge_w = text_w + padding * 2;
    let badge_h = scaled_char_h + padding * 2;

    // Position badge at top-right corner
    let badge_x = w.saturating_sub(badge_w);
    let badge_y = 0u32;

    let red = Rgba([230u8, 50, 50, 255]);
    let white = Rgba([255u8, 255, 255, 255]);

    // Draw filled red rounded rectangle (approximate with corner radius)
    let radius = (badge_h / 2).min(badge_w / 2);
    for y in badge_y..badge_y + badge_h {
        for x in badge_x..badge_x + badge_w {
            if x < w && y < h && is_inside_rounded_rect(x - badge_x, y - badge_y, badge_w, badge_h, radius) {
                img.put_pixel(x, y, red);
            }
        }
    }

    // Draw text centered in badge
    let text_x = badge_x + (badge_w - text_w) / 2;
    let text_y = badge_y + (badge_h - scaled_char_h) / 2;

    let mut cx = text_x;
    for ch in label.chars() {
        if ch == '+' {
            // Simple plus sign: 5×7 grid
            let plus: [u8; 7] = [
                0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
            ];
            draw_char(&mut img, cx, text_y, &plus, scale, white, w, h);
        } else if let Some(digit) = ch.to_digit(10) {
            draw_char(&mut img, cx, text_y, &DIGIT_FONT[digit as usize], scale, white, w, h);
        }
        cx += scaled_char_w + char_gap;
    }

    Some(encode_png(&img))
}

fn is_inside_rounded_rect(x: u32, y: u32, w: u32, h: u32, r: u32) -> bool {
    if r == 0 {
        return true;
    }

    // Check corners
    let corners = [
        (r, r),                 // top-left
        (w - r - 1, r),        // top-right
        (r, h - r - 1),        // bottom-left
        (w - r - 1, h - r - 1), // bottom-right
    ];

    for &(cx, cy) in &corners {
        let in_corner_region = (x <= cx && y <= cy && cx == r && cy == r)          // top-left
            || (x >= cx && y <= cy && cx == w - r - 1 && cy == r)                  // top-right
            || (x <= cx && y >= cy && cx == r && cy == h - r - 1)                  // bottom-left
            || (x >= cx && y >= cy && cx == w - r - 1 && cy == h - r - 1);         // bottom-right

        if in_corner_region {
            let dx = x as f32 - cx as f32;
            let dy = y as f32 - cy as f32;
            if dx * dx + dy * dy > (r as f32) * (r as f32) {
                return false;
            }
        }
    }

    true
}

fn draw_char(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    pattern: &[u8; 7],
    scale: u32,
    color: Rgba<u8>,
    img_w: u32,
    img_h: u32,
) {
    for (row, &bits) in pattern.iter().enumerate() {
        for col in 0..5u32 {
            if bits & (1 << (4 - col)) != 0 {
                // Draw scaled pixel
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + col * scale + sx;
                        let py = y + row as u32 * scale + sy;
                        if px < img_w && py < img_h {
                            img.put_pixel(px, py, color);
                        }
                    }
                }
            }
        }
    }
}

fn encode_png(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    let mut buf = Vec::new();
    let cursor = Cursor::new(&mut buf);
    let encoder = image::codecs::png::PngEncoder::new(cursor);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    )
    .unwrap_or_default();
    buf
}
