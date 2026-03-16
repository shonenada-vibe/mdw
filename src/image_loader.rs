use std::path::Path;

use image::DynamicImage;

use crate::content::ImageSource;

pub fn load_image(source: &ImageSource, base_dir: &Path) -> Result<DynamicImage, String> {
    match source {
        ImageSource::Local(path) => {
            let resolved = if path.is_absolute() {
                path.clone()
            } else {
                base_dir.join(path)
            };
            image::open(&resolved)
                .map_err(|e| format!("Failed to open {}: {e}", resolved.display()))
        }
        ImageSource::Remote(url) => {
            let resp = ureq::get(url)
                .call()
                .map_err(|e| format!("Failed to fetch {url}: {e}"))?;
            let bytes = resp
                .into_body()
                .read_to_vec()
                .map_err(|e| format!("Failed to read response body: {e}"))?;
            image::load_from_memory(&bytes)
                .map_err(|e| format!("Failed to decode image from {url}: {e}"))
        }
    }
}

pub fn compute_display_height(
    img: &DynamicImage,
    available_cols: u16,
    font_size: (u16, u16),
) -> u16 {
    let (img_w, img_h) = (img.width() as f64, img.height() as f64);
    // Use sensible defaults if font_size detection returned zeros.
    // Typical terminal cell: ~8x16 pixels.
    let fw = if font_size.0 > 0 { font_size.0 as f64 } else { 8.0 };
    let fh = if font_size.1 > 0 { font_size.1 as f64 } else { 16.0 };

    // Each terminal cell is fw x fh pixels
    // Available pixel width = available_cols * fw
    let available_px_w = available_cols as f64 * fw;

    // Scale image to fit available width
    let scale = (available_px_w / img_w).min(1.0);
    let display_px_h = img_h * scale;

    // Convert pixel height to terminal rows
    let rows = (display_px_h / fh).ceil() as u16;
    rows.clamp(1, 50)
}
