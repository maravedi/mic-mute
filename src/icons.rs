use anyhow::{Context, Result};
use tao::window::Theme;

pub struct IconColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub fn popup_icon_color(muted: bool, theme: Theme) -> IconColor {
    let _ = theme;
    if muted {
        IconColor {
            r: 255,
            g: 69,
            b: 58,
        }
    } else {
        IconColor {
            r: 174,
            g: 174,
            b: 178,
        }
    }
}

pub fn tray_icon_color(muted: bool, theme: Theme) -> IconColor {
    if muted {
        IconColor {
            r: 239,
            g: 68,
            b: 68,
        }
    } else if theme == Theme::Light {
        IconColor { r: 0, g: 0, b: 0 }
    } else {
        IconColor {
            r: 255,
            g: 255,
            b: 255,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_is_black_in_light_mode_when_unmuted() {
        let color = tray_icon_color(false, Theme::Light);
        assert_eq!((color.r, color.g, color.b), (0, 0, 0));
    }

    #[test]
    fn tray_icon_is_white_in_dark_mode_when_unmuted() {
        let color = tray_icon_color(false, Theme::Dark);
        assert_eq!((color.r, color.g, color.b), (255, 255, 255));
    }

    #[test]
    fn tray_icon_is_red_when_muted() {
        let color = tray_icon_color(true, Theme::Light);
        assert_eq!((color.r, color.g, color.b), (239, 68, 68));
    }
}

/// Rasterizes an SVG with the given stroke color.
/// Returns un-premultiplied RGBA bytes plus the source dimensions.
pub fn rasterize_svg(svg_bytes: &[u8], color: &IconColor) -> Result<(Vec<u8>, u32, u32)> {
    let svg_str = std::str::from_utf8(svg_bytes).context("SVG is not valid UTF-8")?;
    let colored = svg_str.replacen(
        "<svg ",
        &format!("<svg stroke=\"rgb({},{},{})\" ", color.r, color.g, color.b),
        1,
    );

    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&colored, &options).context("Failed to parse SVG")?;
    let size = tree.size();
    let w = size.width() as u32;
    let h = size.height() as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).context("Failed to allocate pixmap")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );

    // tiny-skia produces premultiplied RGBA; un-premultiply for callers.
    let raw = pixmap.take();
    let straight: Vec<u8> = raw
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = p[3];
            if a == 0 {
                [0u8, 0, 0, 0]
            } else {
                let s = 255.0_f32 / a as f32;
                [
                    (p[0] as f32 * s).min(255.) as u8,
                    (p[1] as f32 * s).min(255.) as u8,
                    (p[2] as f32 * s).min(255.) as u8,
                    a,
                ]
            }
        })
        .collect();

    Ok((straight, w, h))
}
