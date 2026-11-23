// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Thumbnails of attached images.
//!
//! A thumbnail is a PNG at most [`EDGE`] pixels wide and high, never larger
//! than the image, turned upright by the image's orientation tag. It is made
//! for PNG, JPEG, GIF (the first frame) and WebP images, kept in the
//! workspace's rebuildable cache under the stored file's hash, and made
//! again when the cache has lost it.
//!
//! Images are decoded with limits, so a small file that claims a huge
//! picture can't exhaust memory: at most [`MAX_SIDE`] pixels on a side,
//! [`MAX_PIXELS`] pixels in all, and [`MAX_ALLOC`] bytes of memory.

use std::io::Cursor;
use std::path::PathBuf;

use gk_store::WorkspaceLayout;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};

/// The longest side of a thumbnail, in pixels.
pub(crate) const EDGE: u32 = 256;
/// The longest side of an image a thumbnail is made of.
const MAX_SIDE: u32 = 16_384;
/// The most pixels of an image a thumbnail is made of.
const MAX_PIXELS: u64 = 50_000_000;
/// The most memory decoding one image may take.
const MAX_ALLOC: u64 = 512 * 1024 * 1024;

/// A thumbnail in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Thumb {
    /// Its file.
    pub(crate) path: PathBuf,
    /// Its width in pixels.
    pub(crate) width: u32,
    /// Its height in pixels.
    pub(crate) height: u32,
}

/// The decoder for a sniffed media type, for the types thumbnails are made
/// of.
fn format(mime: &str) -> Option<ImageFormat> {
    match mime {
        "image/png" => Some(ImageFormat::Png),
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/gif" => Some(ImageFormat::Gif),
        "image/webp" => Some(ImageFormat::WebP),
        _ => None,
    }
}

/// Whether thumbnails are made for files of this media type.
pub(crate) fn supported(mime: &str) -> bool {
    format(mime).is_some()
}

/// The thumbnail of `bytes` (of type `mime`) as PNG, with its width and
/// height. Fails for other types, broken images and images over the limits.
pub(crate) fn make(bytes: &[u8], mime: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let format = format(mime).ok_or_else(|| format!("no thumbnails are made for {mime}"))?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(format!("the image is too large ({w} × {h} pixels)"));
    }
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    image.apply_orientation(orientation);
    let small = if image.width() > EDGE || image.height() > EDGE {
        image.thumbnail(EDGE, EDGE)
    } else {
        image
    };
    let small = if small.color().has_alpha() {
        DynamicImage::ImageRgba8(small.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(small.to_rgb8())
    };
    let mut png = Vec::new();
    small
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok((png, small.width(), small.height()))
}

/// Makes the thumbnail of the stored file `sha256` from its `bytes` and
/// caches it.
pub(crate) fn cache(
    layout: &WorkspaceLayout,
    sha256: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<Thumb, String> {
    let (png, width, height) = make(bytes, mime)?;
    let path = gk_store::store_thumb(layout, sha256, &png)
        .map_err(|e| format!("the thumbnail can't be cached: {e}"))?;
    Ok(Thumb {
        path,
        width,
        height,
    })
}

/// The cached thumbnail of the stored file `sha256`, made from the stored
/// file when the cache doesn't have it.
pub(crate) fn get(layout: &WorkspaceLayout, sha256: &str, mime: &str) -> Result<Thumb, String> {
    if !supported(mime) {
        return Err(format!("no thumbnails are made for {mime}"));
    }
    if let Some(path) = gk_store::thumb_path(layout, sha256)
        && let Ok(bytes) = std::fs::read(&path)
        && let Some((width, height)) = gk_core::image_size(&bytes)
    {
        return Ok(Thumb {
            path,
            width,
            height,
        });
    }
    let bytes = gk_store::read_file(layout, sha256, u64::MAX)
        .map_err(|e| format!("the stored file can't be read: {e}"))?;
    cache(layout, sha256, mime, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        image.write_to(&mut Cursor::new(&mut out), format).unwrap();
        out
    }

    fn png_size(png: &[u8]) -> (u32, u32) {
        let image = image::load_from_memory_with_format(png, ImageFormat::Png).unwrap();
        (image.width(), image.height())
    }

    #[test]
    fn large_images_shrink_to_the_edge_keeping_their_shape() {
        let wide = DynamicImage::new_rgb8(1000, 500);
        let (png, w, h) = make(&encoded(&wide, ImageFormat::Png), "image/png").unwrap();
        assert_eq!((w, h), (EDGE, EDGE / 2));
        assert_eq!(png_size(&png), (w, h));

        let tall = DynamicImage::new_rgb8(300, 900);
        let (_, w, h) = make(&encoded(&tall, ImageFormat::Jpeg), "image/jpeg").unwrap();
        assert_eq!(h, EDGE);
        assert!((85..=86).contains(&w), "{w}");
    }

    #[test]
    fn small_images_keep_their_size_and_transparency() {
        let small = DynamicImage::new_rgba8(40, 30);
        let (png, w, h) = make(&encoded(&small, ImageFormat::Png), "image/png").unwrap();
        assert_eq!((w, h), (40, 30));
        let back = image::load_from_memory_with_format(&png, ImageFormat::Png).unwrap();
        assert!(back.color().has_alpha());
    }

    #[test]
    fn gif_and_webp_are_read() {
        let picture = DynamicImage::new_rgba8(600, 600);
        for (format, mime) in [
            (ImageFormat::Gif, "image/gif"),
            (ImageFormat::WebP, "image/webp"),
        ] {
            let (_, w, h) = make(&encoded(&picture, format), mime).unwrap();
            assert_eq!((w, h), (EDGE, EDGE), "{mime}");
        }
    }

    #[test]
    fn other_types_broken_images_and_bombs_are_refused() {
        assert!(make(b"<svg/>", "image/svg+xml").is_err());
        assert!(make(b"not a picture", "image/png").is_err());
        // A valid PNG header claiming 60,000 × 60,000 pixels.
        let mut bomb = encoded(&DynamicImage::new_luma8(1, 1), ImageFormat::Png);
        bomb[16..20].copy_from_slice(&60_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&60_000u32.to_be_bytes());
        assert!(make(&bomb, "image/png").is_err());
    }

    #[test]
    fn a_lost_thumbnail_is_made_again_from_the_stored_file() {
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        let bytes = encoded(&DynamicImage::new_rgb8(512, 512), ImageFormat::Png);
        let stored = gk_store::store_file(&layout, &bytes).unwrap();
        let made = get(&layout, &stored.sha256, "image/png").unwrap();
        assert_eq!((made.width, made.height), (EDGE, EDGE));
        std::fs::remove_dir_all(layout.cache_dir()).unwrap();
        let again = get(&layout, &stored.sha256, "image/png").unwrap();
        assert_eq!(again, made);
        assert!(get(&layout, &stored.sha256, "text/plain").is_err());
    }
}
