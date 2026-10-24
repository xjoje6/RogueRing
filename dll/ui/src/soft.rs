//! Rasterizador por CPU de `imgui::DrawData` a una imagen RGBA. Solo para vistas previas fuera del
//! juego: pinta los mismos triángulos que hudhook enviaría a DirectX 12, sin GPU.

use std::collections::HashMap;

use imgui::{DrawCmd, DrawCmdParams, DrawData, DrawVert, TextureId};

pub struct Texture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Texture {
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        // Bilineal con bordes fijados.
        let x = (u * self.w as f32 - 0.5).clamp(0.0, (self.w - 1) as f32);
        let y = (v * self.h as f32 - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let px = |xx: u32, yy: u32| {
            let i = ((yy * self.w + xx) * 4) as usize;
            [self.rgba[i] as f32, self.rgba[i + 1] as f32, self.rgba[i + 2] as f32, self.rgba[i + 3] as f32]
        };
        let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
        let mut o = [0.0; 4];
        for k in 0..4 {
            o[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy;
            o[k] /= 255.0;
        }
        o
    }
}

pub fn render(dd: &DrawData, textures: &HashMap<TextureId, Texture>, w: u32, h: u32, clear: [u8; 4]) -> image::RgbaImage {
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba(clear));
    let off = dd.display_pos;
    for list in dd.draw_lists() {
        let vtx = list.vtx_buffer();
        let idx = list.idx_buffer();
        for cmd in list.commands() {
            let DrawCmd::Elements { count, cmd_params: DrawCmdParams { clip_rect, texture_id, vtx_offset, idx_offset } } = cmd else { continue };
            let Some(tex) = textures.get(&texture_id) else { continue };
            let clip = [clip_rect[0] - off[0], clip_rect[1] - off[1], clip_rect[2] - off[0], clip_rect[3] - off[1]];
            for t in (0..count).step_by(3) {
                let v = |k: usize| &vtx[vtx_offset + idx[idx_offset + t + k] as usize];
                tri(&mut img, tex, v(0), v(1), v(2), off, clip);
            }
        }
    }
    img
}

fn tri(img: &mut image::RgbaImage, tex: &Texture, a: &DrawVert, b: &DrawVert, c: &DrawVert, off: [f32; 2], clip: [f32; 4]) {
    let p = |v: &DrawVert| [v.pos[0] - off[0], v.pos[1] - off[1]];
    let (pa, pb, pc) = (p(a), p(b), p(c));
    let area = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
    if area.abs() < 1e-6 {
        return;
    }
    let (w, h) = (img.width() as f32, img.height() as f32);
    let minx = pa[0].min(pb[0]).min(pc[0]).max(clip[0]).max(0.0).floor() as i32;
    let maxx = pa[0].max(pb[0]).max(pc[0]).min(clip[2]).min(w).ceil() as i32;
    let miny = pa[1].min(pb[1]).min(pc[1]).max(clip[1]).max(0.0).floor() as i32;
    let maxy = pa[1].max(pb[1]).max(pc[1]).min(clip[3]).min(h).ceil() as i32;
    let col = |v: &DrawVert| [v.col[0] as f32 / 255.0, v.col[1] as f32 / 255.0, v.col[2] as f32 / 255.0, v.col[3] as f32 / 255.0];
    let (ca, cb, cc) = (col(a), col(b), col(c));
    for y in miny..maxy {
        for x in minx..maxx {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((pb[0] - px) * (pc[1] - py) - (pb[1] - py) * (pc[0] - px)) / area;
            let w1 = ((pc[0] - px) * (pa[1] - py) - (pc[1] - py) * (pa[0] - px)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                continue;
            }
            let u = a.uv[0] * w0 + b.uv[0] * w1 + c.uv[0] * w2;
            let v = a.uv[1] * w0 + b.uv[1] * w1 + c.uv[1] * w2;
            let t = tex.sample(u, v);
            let s: [f32; 4] = std::array::from_fn(|k| (ca[k] * w0 + cb[k] * w1 + cc[k] * w2) * t[k]);
            let d = img.get_pixel_mut(x as u32, y as u32);
            let al = s[3];
            for (c, &v) in d.0.iter_mut().zip(&s).take(3) {
                *c = ((v * al + *c as f32 / 255.0 * (1.0 - al)) * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            d.0[3] = 255;
        }
    }
}
