use std::path::Path;
use std::process::Command;

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
        ImageSource::Diagram {
            lang,
            content,
            content_hash,
            tool_path,
            background,
            cli_theme,
        } => load_diagram(lang, content, *content_hash, tool_path, background, cli_theme.as_deref()),
    }
}

fn load_diagram(
    lang: &str,
    content: &str,
    content_hash: u64,
    tool_path: &str,
    background: &str,
    cli_theme: Option<&str>,
) -> Result<DynamicImage, String> {
    let tmp_dir = std::env::temp_dir().join("mdw-diagrams");
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("Failed to create temp dir: {e}"))?;

    let ext = match lang {
        "mermaid" => "mmd",
        "d2" => "d2",
        _ => return Err(format!("Unsupported diagram language: {lang}")),
    };

    let input_path = tmp_dir.join(format!("diagram-{content_hash}.{ext}"));
    let output_path = tmp_dir.join(format!("diagram-{content_hash}.png"));

    std::fs::write(&input_path, content)
        .map_err(|e| format!("Failed to write diagram input: {e}"))?;

    let mut cmd = Command::new(tool_path);
    match lang {
        "mermaid" => {
            cmd.arg("-i").arg(&input_path)
                .arg("-o").arg(&output_path)
                .arg("-b").arg(background);
            if let Some(theme) = cli_theme {
                cmd.arg("--theme").arg(theme);
            }
        }
        "d2" => {
            if let Some(theme) = cli_theme {
                cmd.arg("--theme").arg(theme);
            }
            cmd.arg(&input_path).arg(&output_path);
        }
        _ => unreachable!(),
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run {tool_path}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Clean up input file on failure
        let _ = std::fs::remove_file(&input_path);
        return Err(format!("{tool_path} failed: {stderr}"));
    }

    let img = image::open(&output_path)
        .map_err(|e| format!("Failed to read rendered diagram: {e}"))?;

    // Clean up temp files
    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&output_path);

    Ok(img)
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
