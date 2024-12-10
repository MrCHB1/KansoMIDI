// module that will use png files to load note colors
use std::fs::{create_dir, File};
use std::io::BufReader;
use std::path::{absolute, PathBuf};

use image::{ImageReader, RgbImage, GenericImageView};
use itertools::Itertools;

use super::color_funcs::hsv_to_rgb;

pub struct ColorPalettes {
    has_palettes: bool,
    palette_master_path: PathBuf,
    pub palette_paths: Vec<String>,
}

impl ColorPalettes {
    pub fn new() -> Self {
        let palette_path = absolute("./Palettes/").unwrap();
        let mut has_palettes = true;
        if !palette_path.exists() {
            create_dir(absolute("./Palettes/").unwrap()).unwrap();
            has_palettes = false;
        }

        let mut col_palette = Self {
            has_palettes,
            palette_master_path: palette_path,
            palette_paths: Vec::new()
        };

        if !has_palettes {
            col_palette.create_default_palettes();
        }

        col_palette.populate_palette_paths();
        col_palette
    }

    pub fn create_default_palettes(&mut self) -> () {
        // 1. rainbow
        {
            let mut img = RgbImage::new(16, 4);
            for (pix_idx, pix) in img.iter_mut().enumerate() {
                let i = pix_idx / 3;
                let col = hsv_to_rgb([(i as f32 / 9.0 * 360.0) % 360.0, 1.0, 1.0]);
                *pix = (col[pix_idx % 3] * 255.0) as u8;
            }
            let mut f = File::create_new(absolute("./Palettes/Rainbow.png").unwrap()).unwrap();
            img.write_to(&mut f, image::ImageFormat::Png).unwrap();
        }
        // 2. wide rainbow
        {
            let mut img = RgbImage::new(16, 8);
            for (pix_idx, pix) in img.iter_mut().enumerate() {
                let i = pix_idx / 3;
                let col = hsv_to_rgb([(i as f32 / 17.0 * 360.0) % 360.0, 1.0, 1.0]);
                *pix = (col[pix_idx % 3] * 255.0) as u8;
            }
            let mut f = File::create_new(absolute("./Palettes/Rainbow Wide.png").unwrap()).unwrap();
            img.write_to(&mut f, image::ImageFormat::Png).unwrap();
        }
    }

    pub fn populate_palette_paths(&mut self) -> () {
        let paths = 
            std::fs::read_dir(absolute("./Palettes/").unwrap())
                .unwrap();
        self.palette_paths = paths
            .into_iter()
            .map(|p| {
                String::from(p.unwrap().path().to_str().unwrap())
            }).collect_vec()
    }

    pub fn get_color_table_from_palette_idx(&mut self, idx: usize) -> Vec<u32> {
        let f = File::open(&self.palette_paths[idx]).unwrap();
        let image = ImageReader::new(
            BufReader::new(
                f
            ))
            .with_guessed_format()
            .unwrap();

        let dec = image.decode().unwrap();

        let mut colors: Vec<u32> = Vec::new();
        for chan in 0..16 {
            for y in 0..dec.height() {
                let pix = dec.get_pixel(chan, y).0;
                let pix_u32 =
                        (pix[0] as u32) | 
                        ((pix[1] as u32) << 8) |
                        ((pix[2] as u32) << 16);
                    colors.push(pix_u32);
            }
        }
        colors
    }
}