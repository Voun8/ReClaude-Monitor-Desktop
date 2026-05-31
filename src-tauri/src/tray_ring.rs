// 用 tiny-skia 画圆环 + ab_glyph 绘制中间百分比数字（运行时加载系统字体）。
// 返回 44×44 直通(未预乘) RGBA 字节，供 Tauri 托盘 Image::new_owned 使用。

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub const SIZE: u32 = 44;

/// 按用量比例返回 (R,G,B) —— 与前端的健康配色保持一致。
pub fn color_for_ratio(ratio: f64) -> (u8, u8, u8) {
    if ratio >= 0.95 {
        (239, 83, 80) // err
    } else if ratio >= 0.8 {
        (224, 169, 63) // warn
    } else {
        (217, 119, 87) // accent
    }
}

/// 运行时加载系统字体（不内置以避免授权/打包问题）。
fn load_font() -> Option<FontVec> {
    #[cfg(target_os = "macos")]
    let paths: &[&str] = &[
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/SFNSRounded.ttf",
    ];
    #[cfg(target_os = "windows")]
    let paths: &[&str] = &[
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
        "C:\\Windows\\Fonts\\tahoma.ttf",
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let paths: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    for p in paths {
        if let Ok(bytes) = std::fs::read(p) {
            if let Ok(font) = FontVec::try_from_vec(bytes) {
                return Some(font);
            }
        }
    }
    None
}

/// 画圆环（轨道 + 进度弧）+ 中间百分比。avail = 0..100。
pub fn render_ring(avail: f64, color_rgb: (u8, u8, u8)) -> Vec<u8> {
    let mut pm = Pixmap::new(SIZE, SIZE).expect("pixmap");
    pm.fill(Color::TRANSPARENT);

    let cx = SIZE as f32 / 2.0;
    let cy = SIZE as f32 / 2.0;
    let r = SIZE as f32 * 0.38;
    let lw = SIZE as f32 * 0.13;

    // 轨道圆（faint 白）
    {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, r);
        let path = pb.finish().unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, 56);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: lw,
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // 进度弧（可用余额%，从顶部 -90° 顺时针）
    let avail_c = avail.clamp(0.0, 100.0);
    if avail_c > 0.1 {
        let sweep = std::f32::consts::PI * 2.0 * (avail_c as f32 / 100.0);
        let start = -std::f32::consts::FRAC_PI_2;
        // 用 ~120 段线逼近，配合 round cap 视觉平滑
        let segs = (sweep.abs() / 0.05).ceil().max(2.0) as usize;
        let mut pb = PathBuilder::new();
        pb.move_to(cx + r * start.cos(), cy + r * start.sin());
        for i in 1..=segs {
            let a = start + sweep * (i as f32 / segs as f32);
            pb.line_to(cx + r * a.cos(), cy + r * a.sin());
        }
        let path = pb.finish().unwrap();
        let mut paint = Paint::default();
        let (rc, gc, bc) = color_rgb;
        paint.set_color_rgba8(rc, gc, bc, 255);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: lw,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    // 中央百分比数字（不带 %，避免拥挤；3 位时缩小）
    if let Some(font) = load_font() {
        let text = format!("{}", avail_c.round() as u32);
        let chars: Vec<char> = text.chars().collect();
        let font_size = if chars.len() >= 3 {
            SIZE as f32 * 0.36
        } else {
            SIZE as f32 * 0.46
        };
        let scale = PxScale::from(font_size);
        let scaled = font.as_scaled(scale);

        // 收集 glyph + 累计 x 偏移以居中
        let mut x_cursor = 0.0_f32;
        let mut glyphs: Vec<(f32, ab_glyph::OutlinedGlyph)> = Vec::new();
        for &ch in &chars {
            let gid = font.glyph_id(ch);
            let advance = scaled.h_advance(gid);
            let g = gid.with_scale(scale);
            if let Some(o) = font.outline_glyph(g) {
                glyphs.push((x_cursor, o));
            }
            x_cursor += advance;
        }
        let total_w = x_cursor;
        let ox = cx - total_w / 2.0;
        // 数字视觉居中（基线在 cy 下方一点，因为数字主体在基线上方）
        let baseline = cy + font_size * 0.32;

        let stride = SIZE as i32 * 4;
        let data = pm.data_mut();
        for (g_off, og) in glyphs {
            let bb = og.px_bounds();
            og.draw(|gx, gy, cov| {
                let af = cov.clamp(0.0, 1.0);
                if af == 0.0 {
                    return;
                }
                let px = (ox + g_off + bb.min.x + gx as f32).round() as i32;
                let py = (baseline + bb.min.y + gy as f32).round() as i32;
                if px < 0 || py < 0 || px >= SIZE as i32 || py >= SIZE as i32 {
                    return;
                }
                let i = (py * stride + px * 4) as usize;
                // Porter-Duff over：白色文字（premul (a,a,a,a)）混合到下层 premul RGBA。
                // out = src + dst*(1 - src_a)；这样数字若叠到圆环上不会硬覆盖。
                let a = (af * 255.0).round() as u32;
                let inv = 1.0 - af;
                data[i]     = (a as f32 + data[i]     as f32 * inv).min(255.0) as u8;
                data[i + 1] = (a as f32 + data[i + 1] as f32 * inv).min(255.0) as u8;
                data[i + 2] = (a as f32 + data[i + 2] as f32 * inv).min(255.0) as u8;
                data[i + 3] = (a as f32 + data[i + 3] as f32 * inv).min(255.0) as u8;
            });
        }
    }

    // tiny-skia 内部是预乘 alpha；托盘需要直通 → 反预乘
    let mut out = pm.take();
    for px in out.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 || a == 255 {
            continue;
        }
        let a_u = a as u32;
        px[0] = ((px[0] as u32 * 255 + a_u / 2) / a_u).min(255) as u8;
        px[1] = ((px[1] as u32 * 255 + a_u / 2) / a_u).min(255) as u8;
        px[2] = ((px[2] as u32 * 255 + a_u / 2) / a_u).min(255) as u8;
    }
    out
}
