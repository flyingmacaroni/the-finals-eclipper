use fast_image_resize as fr;
pub use fast_image_resize::FilterType;
pub use fast_image_resize::ResizeAlg;

pub fn scale_frame(
    frame_data: &[u8],
    width: i32,
    height: i32,
    max_height: u32,
    resize_alg: ResizeAlg,
) -> Vec<u8> {
    let dst_height = max_height;
    let scale_factor = height as f64 / max_height as f64;
    let dst_width = (width as f64 / scale_factor) as u32;
    let mut dst_image = fr::images::Image::new(dst_width, dst_height, fr::PixelType::U8x3);

    let src_image =
        fr::images::ImageRef::new(width as u32, height as u32, frame_data, fr::PixelType::U8x3)
            .unwrap();

    let mut resizer = fr::Resizer::new();

    resizer
        .resize(
            &src_image,
            &mut dst_image,
            Some(&fr::ResizeOptions::new().resize_alg(resize_alg)),
        )
        .expect("image resized");

    dst_image.into_vec()
}

pub fn frame_binarisation(image_data: &mut [u8], min_rgb: [u8; 3], max_rgb: [u8; 3]) {
    for rgb in image_data.chunks_exact_mut(3) {
        if rgb[0] >= min_rgb[0]
            && rgb[0] <= max_rgb[0]
            && rgb[1] >= min_rgb[1]
            && rgb[1] <= max_rgb[1]
            && rgb[2] >= min_rgb[2]
            && rgb[2] <= max_rgb[2]
        {
            // let sum = rgb[0] as u16 + rgb[1] as u16 + rgb[2] as u16;
            // let value = 255 - (sum / 3) as u8;
            let value = 0;
            rgb.fill(value);
        } else {
            rgb.fill(255);
        };
    }
}

pub fn frame_brightness_contrast(
    image_data: &mut [u8],
    brightness: f64,
    contrast: f64,
    invert: bool,
) {
    for rgb in image_data.chunks_exact_mut(3) {
        let mut r = rgb[0] as f64 / 255.0;
        let mut g = rgb[1] as f64 / 255.0;
        let mut b = rgb[2] as f64 / 255.0;

        r += brightness;
        g += brightness;
        b += brightness;

        r *= contrast;
        g *= contrast;
        b *= contrast;

        if invert {
            rgb[0] = 255 - (r.clamp(0., 1.) * 255.0) as u8;
            rgb[1] = 255 - (g.clamp(0., 1.) * 255.0) as u8;
            rgb[2] = 255 - (b.clamp(0., 1.) * 255.0) as u8;
            continue;
        }

        rgb[0] = (r.clamp(0., 1.) * 255.0) as u8;
        rgb[1] = (g.clamp(0., 1.) * 255.0) as u8;
        rgb[2] = (b.clamp(0., 1.) * 255.0) as u8;
    }
}
