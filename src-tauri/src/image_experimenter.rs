use crate::pixels_to_base64_image::pixels_to_base64_image;
use common::process_frame::{FilterType, ResizeAlg};
use common::tesseract::Tesseract;
use common::{SearchParam, SEARCH_PARAMS};
use image::codecs::jpeg::JpegEncoder;
use image::io::Reader as ImageReader;
use image::{ColorType, EncodableLayout};
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use tracing::error;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ImageFilterType {
    MinMaxRgb { min_rgb: [u8; 3], max_rgb: [u8; 3] },
    BrightnessContrast { brightness: f64, contrast: f64 },
}

#[tauri::command(async)]
pub fn process_image(
    image: &str,
    image_filter_type: ImageFilterType,
    scale_height: u32,
    resize_algorithm: ResizeAlgorithm,
) -> Result<ProcessImageResult, ProcessImageError> {
    let mut img = ImageReader::open(image)?.decode()?;

    let img = img
        .as_mut_rgb8()
        .ok_or(ProcessImageError::Rgb8ConversionError)?;

    match image_filter_type {
        ImageFilterType::MinMaxRgb { min_rgb, max_rgb } => {
            common::process_frame::frame_binarisation(img.as_mut(), min_rgb, max_rgb);
        }
        ImageFilterType::BrightnessContrast {
            brightness,
            contrast,
        } => {
            common::process_frame::frame_brightness_contrast(
                img.as_mut(),
                brightness,
                contrast,
                true,
            );
        }
    }

    let scale_factor = img.height() as f64 / scale_height as f64;
    let dst_width = (img.width() as f64 / scale_factor) as u32;
    let mut scaled = common::process_frame::scale_frame(
        img.as_bytes(),
        img.width() as i32,
        img.height() as i32,
        scale_height,
        resize_algorithm.into(),
    );

    let ocr_result = recognize(&scaled, dst_width as i32, scale_height as i32);

    let mut bytes: Vec<u8> = Vec::new();
    let mut writer = std::io::Cursor::new(&mut bytes);
    let mut encoder = JpegEncoder::new_with_quality(&mut writer, 95);

    encoder.encode(&scaled, dst_width, scale_height, ColorType::Rgb8)?;

    Ok(ProcessImageResult {
        image: pixels_to_base64_image(&mut scaled, dst_width, scale_height),
        ocr_result,
    })
}

fn recognize(frame_data: &[u8], width: i32, height: i32) -> String {
    let mut text = String::new();
    for search in SEARCH_PARAMS.iter() {
        match search {
            SearchParam::Text {
                search_area,
                patterns: _,
                clip_length_after: _,
                clip_length_before: _,
                timeout: _timeout,
                resize: _,
                binarisation_params: _,
                brightness_contrast_params: _,
            } => {
                let left = (search_area.left * width as f64) as i32;
                let top = (search_area.top * height as f64) as i32;
                let inner_width = (search_area.width * width as f64) as i32;
                let inner_height = (search_area.height * height as f64) as i32;

                // info!("search_area left: {left}, top: {top}, width: {inner_width}, height: {inner_height}");

                let mut tess = Tesseract::new(None, Some("eng"))
                    .unwrap()
                    .set_frame(frame_data, width, height, 3, width * 3)
                    .unwrap()
                    .set_rectangle(left, top, inner_width, inner_height)
                    .recognize()
                    .unwrap();

                let Ok(result) = tess.get_text() else {
                    error!("failed to tesseract");
                    eprintln!("failed to tesseract");
                    continue;
                };

                text += format!("\n {result}").as_str();
            }
            SearchParam::AveragePixelValue { .. } => {}
        }
    }
    text
}

#[derive(Serialize, Deserialize)]
pub struct ProcessImageResult {
    image: String,
    ocr_result: String,
}

#[derive(Serialize, Deserialize, Default)]
pub enum ResizeAlgorithm {
    Nearest,
    /// Each pixel of source image contributes to one pixel of the
    /// destination image with identical weights. For upscaling is equivalent
    /// of `Nearest` resize algorithm.    
    Box,
    /// Bilinear filter calculate the output pixel value using linear
    /// interpolation on all pixels that may contribute to the output value.
    Bilinear,
    /// Hamming filter has the same performance as `Bilinear` filter while
    /// providing the image downscaling quality comparable to bicubic
    /// (`CatmulRom` or `Mitchell`). Produces a sharper image than `Bilinear`,
    /// doesn't have dislocations on local level like with `Box`.
    /// The filter don’t show good quality for the image upscaling.
    Hamming,
    /// Catmull-Rom bicubic filter calculate the output pixel value using
    /// cubic interpolation on all pixels that may contribute to the output
    /// value.
    CatmullRom,
    /// Mitchell–Netravali bicubic filter calculate the output pixel value
    /// using cubic interpolation on all pixels that may contribute to the
    /// output value.
    Mitchell,
    /// Lanczos3 filter calculate the output pixel value using a high-quality
    /// Lanczos filter (a truncated sinc) on all pixels that may contribute
    /// to the output value.
    #[default]
    Lanczos3,
}

impl Into<ResizeAlg> for ResizeAlgorithm {
    fn into(self) -> ResizeAlg {
        match self {
            ResizeAlgorithm::Nearest => ResizeAlg::Nearest,
            ResizeAlgorithm::Box => ResizeAlg::Convolution(FilterType::Box),
            ResizeAlgorithm::Bilinear => ResizeAlg::Convolution(FilterType::Bilinear),
            ResizeAlgorithm::Hamming => ResizeAlg::Convolution(FilterType::Hamming),
            ResizeAlgorithm::CatmullRom => ResizeAlg::Convolution(FilterType::CatmullRom),
            ResizeAlgorithm::Mitchell => ResizeAlg::Convolution(FilterType::Mitchell),
            ResizeAlgorithm::Lanczos3 => ResizeAlg::Convolution(FilterType::Lanczos3),
        }
    }
}

#[derive(Error, Debug)]
pub enum ProcessImageError {
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("image error")]
    ImageError(#[from] image::ImageError),
    #[error("failed to convert image to rgb8")]
    Rgb8ConversionError,
}

// we must manually implement serde::Serialize
impl serde::Serialize for ProcessImageError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
