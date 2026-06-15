// 用 tiny-skia 画圆环 + ab_glyph 在中间绘制百分比数字（运行时加载系统字体）。
// 渲染参数分平台：Windows 通知区图标小，用更细更大的环 + 更大的粗体带描边数字才看得清；
// macOS / Linux 菜单栏沿用原始参数（细环/描边等改动只针对 Windows）。
// 返回直通(未预乘) RGBA 字节，供 Tauri 托盘 Image::new_owned 使用。

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

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

/// 运行时加载系统字体（不内置以避免授权/打包问题）。Windows 优先粗体（小尺寸更醒目）。
fn load_font() -> Option<FontVec> {
    #[cfg(target_os = "macos")]
    let paths: &[&str] = &[
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/SFNSRounded.ttf",
    ];
    #[cfg(target_os = "windows")]
    let paths: &[&str] = &[
        "C:\\Windows\\Fonts\\segoeuib.ttf", // Segoe UI Bold
        "C:\\Windows\\Fonts\\arialbd.ttf",  // Arial Bold
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
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

/// 画圆环（轨道 + 进度弧）+ 中间百分比。avail = 0..100（剩余可用%），size = 目标像素边长。
pub fn render_ring(avail: f64, color_rgb: (u8, u8, u8), size: u32) -> Vec<u8> {
    let size = size.max(8);
    let mut pm = Pixmap::new(size, size).expect("pixmap");
    pm.fill(Color::TRANSPARENT);

    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;

    // 环几何 + 轨道透明度：Windows 更细更大（内圈大、数字能放大）；其它平台沿用原值
    #[cfg(target_os = "windows")]
    let (r, lw, track_alpha) = (size as f32 * 0.42, size as f32 * 0.09, 80u8);
    #[cfg(not(target_os = "windows"))]
    let (r, lw, track_alpha) = (size as f32 * 0.38, size as f32 * 0.13, 56u8);

    // 轨道圆（白，半透明——比进度弧弱但需看得出整圈，弧才有"剩多少"的参照）
    {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, r);
        let path = pb.finish().unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, track_alpha);
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
        // 字号系数：Windows 放大填满细环内圈；其它平台沿用原值
        #[cfg(target_os = "windows")]
        let (fs_small, fs_big) = (0.48_f32, 0.68_f32);
        #[cfg(not(target_os = "windows"))]
        let (fs_small, fs_big) = (0.36_f32, 0.46_f32);

        let text = format!("{}", avail_c.round() as u32);
        let chars: Vec<char> = text.chars().collect();
        let font_size = if chars.len() >= 3 {
            size as f32 * fs_small
        } else {
            size as f32 * fs_big
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
        let stride = size as i32 * 4;

        // 把字形覆盖率以给定颜色 + 偏移 over 混合到 pixmap（premul：src = color*α，over：src + dst*(1-α)）。
        // 用宏而非闭包：避免多次绘制时对 data 的可变借用打架。
        macro_rules! blit_glyphs {
            ($cr:expr, $cg:expr, $cb:expr, $dx:expr, $dy:expr) => {{
                let data = pm.data_mut();
                for (g_off, og) in &glyphs {
                    let bb = og.px_bounds();
                    og.draw(|gx, gy, cov| {
                        let af = cov.clamp(0.0, 1.0);
                        if af == 0.0 {
                            return;
                        }
                        let px = (ox + g_off + bb.min.x + gx as f32).round() as i32 + $dx;
                        let py = (baseline + bb.min.y + gy as f32).round() as i32 + $dy;
                        if px < 0 || py < 0 || px >= size as i32 || py >= size as i32 {
                            return;
                        }
                        let i = (py * stride + px * 4) as usize;
                        let inv = 1.0 - af;
                        data[i] = ($cr as f32 * af + data[i] as f32 * inv).min(255.0) as u8;
                        data[i + 1] = ($cg as f32 * af + data[i + 1] as f32 * inv).min(255.0) as u8;
                        data[i + 2] = ($cb as f32 * af + data[i + 2] as f32 * inv).min(255.0) as u8;
                        data[i + 3] = (255.0 * af + data[i + 3] as f32 * inv).min(255.0) as u8;
                    });
                }
            }};
        }

        // Windows：白字 + 近黑描边（深/浅任务栏背景都清晰）；其它平台：仅白字（原行为）
        #[cfg(target_os = "windows")]
        {
            let ow = (size as f32 * 0.06).round().max(1.0) as i32;
            let outline: [(i32, i32); 8] = [
                (-ow, -ow),
                (0, -ow),
                (ow, -ow),
                (-ow, 0),
                (ow, 0),
                (-ow, ow),
                (0, ow),
                (ow, ow),
            ];
            for (dx, dy) in outline {
                blit_glyphs!(26, 26, 26, dx, dy);
            }
            blit_glyphs!(255, 255, 255, 0, 0);
        }
        #[cfg(not(target_os = "windows"))]
        {
            blit_glyphs!(255, 255, 255, 0, 0);
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
