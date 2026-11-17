//! A tiny RGB canvas and PNG encoder, used to draw the captcha picture on the server.

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Canvas {
    pub fn new(w: usize, h: usize, bg: [u8; 3]) -> Self {
        Self {
            w,
            h,
            px: vec![bg; w * h],
        }
    }

    /// Blend a colour into one pixel with coverage `a` (0..=1).
    pub fn blend(&mut self, x: i32, y: i32, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h || a <= 0.0 {
            return;
        }
        let a = a.min(1.0);
        let p = &mut self.px[y as usize * self.w + x as usize];
        for i in 0..3 {
            p[i] = (p[i] as f32 * (1.0 - a) + c[i] as f32 * a).round() as u8;
        }
    }

    /// Paint every pixel in a box whose distance to a shape (given by `dist`) is below
    /// `half_width`, with a 1 px soft edge.
    pub fn stroke(
        &mut self,
        bbox: (f32, f32, f32, f32),
        half_width: f32,
        c: [u8; 3],
        dist: impl Fn(f32, f32) -> f32,
    ) {
        let (x0, y0, x1, y1) = bbox;
        let pad = half_width + 2.0;
        for y in (y0 - pad).floor() as i32..=(y1 + pad).ceil() as i32 {
            for x in (x0 - pad).floor() as i32..=(x1 + pad).ceil() as i32 {
                let d = dist(x as f32 + 0.5, y as f32 + 0.5);
                let a = (half_width + 0.5 - d).clamp(0.0, 1.0);
                self.blend(x, y, c, a);
            }
        }
    }

    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), hw: f32, c: [u8; 3]) {
        let bbox = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
        self.stroke(bbox, hw, c, |x, y| seg_dist((x, y), a, b));
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, hw: f32, c: [u8; 3]) {
        self.stroke((cx - r, cy - r, cx + r, cy + r), hw, c, |x, y| {
            (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r).abs()
        });
    }

    pub fn polygon(&mut self, pts: &[(f32, f32)], hw: f32, c: [u8; 3]) {
        for i in 0..pts.len() {
            self.line(pts[i], pts[(i + 1) % pts.len()], hw, c);
        }
    }

    pub fn to_png(&self) -> Vec<u8> {
        let mut raw = Vec::with_capacity((self.w * 3 + 1) * self.h);
        for y in 0..self.h {
            raw.push(0); // filter: none
            for x in 0..self.w {
                raw.extend_from_slice(&self.px[y * self.w + x]);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, RGB
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(
            &mut out,
            b"IDAT",
            &miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6),
        );
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

pub fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt()
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut c = Crc::new();
    c.update(kind);
    c.update(data);
    out.extend_from_slice(&c.finish().to_be_bytes());
}

struct Crc(u32);

impl Crc {
    fn new() -> Self {
        Crc(0xFFFF_FFFF)
    }
    fn update(&mut self, data: &[u8]) {
        for &b in data {
            let mut c = (self.0 ^ b as u32) & 0xFF;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            self.0 = (self.0 >> 8) ^ c;
        }
    }
    fn finish(self) -> u32 {
        self.0 ^ 0xFFFF_FFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_known_vector() {
        let mut c = Crc::new();
        c.update(b"123456789");
        assert_eq!(c.finish(), 0xCBF4_3926);
    }

    #[test]
    fn png_has_signature_and_iend() {
        let mut cv = Canvas::new(4, 3, [255, 255, 255]);
        cv.ring(2.0, 1.5, 1.0, 0.5, [0, 0, 0]);
        let png = cv.to_png();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }
}
