// module that will use png files to load note colors
use std::fs::{create_dir, File};
use std::io::BufReader;
use std::path::{self, absolute, PathBuf};

use image::{GenericImageView, ImageError, ImageReader, RgbImage};
use itertools::Itertools;

use super::color_funcs::hsv_to_rgb;

pub struct ColorPalettes {
    has_palettes: bool,
    palette_master_path: PathBuf,
    pub palette_paths: Vec<String>,
    pub palette_names: Vec<String>,
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
            palette_paths: Vec::new(),
            palette_names: Vec::new()
        };

        if !has_palettes {
            col_palette.create_default_palettes().unwrap();
        }

        col_palette.populate_palette_paths();
        col_palette
    }

    fn generate_palette<F>(&mut self, name: &str, width: u32, height: u32, mut pixel_callback: F) -> Result<(), ImageError> 
    where F: FnMut(usize) -> [f32; 3] {
        let mut img = RgbImage::new(width, height);
        for (pix_idx, pix) in img.iter_mut().enumerate() {
            let i = pix_idx / 3;
            *pix = (pixel_callback(i)[pix_idx % 3] * 255.0) as u8;
        }
        let mut f = File::create_new(absolute(format!("./Palettes/{}.png", name))?)?;
        img.write_to(&mut f, image::ImageFormat::Png)?;
        Ok(())
    }

    pub fn create_default_palettes(&mut self) -> Result<(), ImageError> {
        // 1. rainbow
        {
            /*let mut img = RgbImage::new(16, 4);
            for (pix_idx, pix) in img.iter_mut().enumerate() {
                let i = pix_idx / 3;
                let col = hsv_to_rgb([(i as f32 / 9.0 * 360.0) % 360.0, 1.0, 1.0]);
                *pix = (col[pix_idx % 3] * 255.0) as u8;
            }
            let mut f = File::create_new(absolute("./Palettes/Rainbow.png")?)?;
            img.write_to(&mut f, image::ImageFormat::Png)?;*/
            self.generate_palette("Rainbow", 16, 4, |pix_idx| {
                hsv_to_rgb([(pix_idx as f32 / 9.0 * 360.0) % 360.0, 1.0, 1.0])
            })?;
        }
        // 2. wide rainbow
        {
            /*let mut img = RgbImage::new(16, 8);
            for (pix_idx, pix) in img.iter_mut().enumerate() {
                let i = pix_idx / 3;
                let col = hsv_to_rgb([(i as f32 / 17.0 * 360.0) % 360.0, 1.0, 1.0]);
                *pix = (col[pix_idx % 3] * 255.0) as u8;
            }
            let mut f = File::create_new(absolute("./Palettes/Rainbow Wide.png")?)?;
            img.write_to(&mut f, image::ImageFormat::Png)?;*/
            self.generate_palette("Rainbow Wide", 16, 4, |pix_idx| {
                hsv_to_rgb([(pix_idx as f32 / 17.0 * 360.0) % 360.0, 1.0, 1.0])
            })?;
        }

        Ok(())
    }

    pub fn populate_palette_paths(&mut self) -> () {
        let paths = 
            std::fs::read_dir(absolute("./Palettes/").unwrap())
                .unwrap();
        (self.palette_paths, self.palette_names) = paths
            .into_iter()
            .map(|p| {
                let path = p.unwrap().path();
                (
                    String::from(path.to_str().unwrap()),
                    String::from(path.file_name().unwrap().to_str().unwrap())
                )
            }).unzip();
    }

    pub fn reload_palette_paths(&mut self) -> () {
        self.populate_palette_paths();
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