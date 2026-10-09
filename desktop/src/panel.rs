//! The simulator's panel: a RAM model of the CO5300's frame memory, shown as
//! a round watch face in the window or saved as a PNG.

use std::{fs::File, io::BufWriter, path::Path};

use orbit_gfx::{Panel, Rectangle, SCREEN_SIZE};

pub const SIZE: usize = SCREEN_SIZE as usize;
/// Shown outside the round glass so the simulator reads as a watch face.
const BEZEL: u32 = 0x1a1a1a;

/// RAM model of the CO5300's frame memory.
pub struct RamPanel {
    ram: Vec<u8>,
}

impl RamPanel {
    pub fn new() -> Self {
        Self {
            ram: vec![0; SIZE * SIZE * 2],
        }
    }

    fn rgb565(&self, x: usize, y: usize) -> u16 {
        let i = (y * SIZE + x) * 2;
        u16::from_be_bytes([self.ram[i], self.ram[i + 1]])
    }

    /// The window's pixels: the panel inside the glass, bezel outside it.
    pub fn to_argb(&self, out: &mut [u32]) {
        for (i, px) in out.iter_mut().enumerate() {
            let (x, y) = (i % SIZE, i / SIZE);
            *px = if on_glass(x, y) {
                expand(self.rgb565(x, y))
            } else {
                BEZEL
            };
        }
    }

    pub fn save_png(&self, path: &Path) {
        let mut argb = vec![0u32; SIZE * SIZE];
        self.to_argb(&mut argb);
        let rgb: Vec<u8> = argb
            .iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
            .collect();
        let file = BufWriter::new(File::create(path).expect("create png"));
        let mut encoder = png::Encoder::new(file, SCREEN_SIZE, SCREEN_SIZE);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer.write_image_data(&rgb).expect("png data");
    }
}

impl Panel for RamPanel {
    fn write(&mut self, area: Rectangle, bytes: &[u8]) {
        let row_bytes = area.size.width as usize * 2;
        for (row, src) in bytes.chunks_exact(row_bytes).enumerate() {
            let y = area.top_left.y as usize + row;
            let start = (y * SIZE + area.top_left.x as usize) * 2;
            self.ram[start..start + row_bytes].copy_from_slice(src);
        }
    }
}

fn on_glass(x: usize, y: usize) -> bool {
    let r = SIZE as f32 / 2.0;
    let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
    dx * dx + dy * dy <= r * r
}

fn expand(c: u16) -> u32 {
    let r = ((c >> 11) & 0x1f) as u32;
    let g = ((c >> 5) & 0x3f) as u32;
    let b = (c & 0x1f) as u32;
    (r << 3 | r >> 2) << 16 | (g << 2 | g >> 4) << 8 | (b << 3 | b >> 2)
}
