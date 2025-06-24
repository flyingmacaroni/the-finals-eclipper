use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;

#[rustfmt::skip]
const BMP_HEADER: &[u8] = &[
    b'B', b'M', // BM HEADER
    58, 0, 0, 0, // size of bmp in bytes
    0, 0, 0, 0, // reserved
    54, 0, 0, 0, // offset of pixel array
    40, 0, 0, 0, // BITMAPINFOHEADER the size of this header in bytes
    1, 0, 0, 0, // width
    1, 0, 0, 0, // height
    1, 0, // number of color planes
    24, 0, // bits per pixel
    0, 0, 0, 0, // compression method used. 0 = no compression
    4, 0, 0, 0, // the image size in bytes, can be ignored when compression = 0.
    0x23, 0x2e, 0x00, 0x00, // the horizontal resolution of the image
    0x23, 0x2e, 0x00, 0x00, // the vertical resolution of the image. These only matter for printing.
    0, 0, 0, 0, // the number of colors in the color palette, or 0 to default to 2n
    0, 0, 0, 0, // the number of important colors used, or 0 when every color is important; generally ignored
];

pub fn pixels_to_bmp(pixels: &mut [u8], width: u32, height: u32) -> Bytes {
    let row_padding = width * 3 % 4;
    let mut bytes =
        Vec::with_capacity(BMP_HEADER.len() + pixels.len() + (row_padding * height) as usize);
    bytes.extend_from_slice(BMP_HEADER);
    bytes[18..22].copy_from_slice(&width.to_le_bytes());
    bytes[22..26].copy_from_slice(&height.to_le_bytes());

    // rgb to bgr
    for rgb in pixels.chunks_exact_mut(3) {
        rgb.swap(0, 2);
    }

    for row in pixels.rchunks_exact(width as usize * 3) {
        bytes.extend_from_slice(row);
        bytes.resize(bytes.len() + row_padding as usize, 0);
    }
    let len = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&len.to_le_bytes());

    Bytes::from(bytes)
}

pub fn pixels_to_base64_image(pixels: &mut [u8], width: u32, height: u32) -> String {
    let bytes = pixels_to_bmp(pixels, width, height);

    format!("data:image/bmp;base64,{}", BASE64_STANDARD.encode(bytes))
}
