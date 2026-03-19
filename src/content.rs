use std::path::PathBuf;

use image::DynamicImage;
use ratatui::text::Line;
use ratatui_image::thread::ThreadProtocol;

#[allow(clippy::large_enum_variant)]
pub enum ContentBlock {
    Text {
        lines: Vec<Line<'static>>,
    },
    Image {
        alt_text: String,
        display_height: u16,
        protocol: Option<ThreadProtocol>,
        error: Option<String>,
        source: ImageSource,
        /// Cached decoded image to avoid re-reading from disk/network on resize.
        cached_image: Option<DynamicImage>,
        /// Whether the image is currently being loaded in the background.
        loading: bool,
    },
}

pub enum ImageSource {
    Local(PathBuf),
    Remote(String),
    Diagram {
        lang: String,
        content: String,
        content_hash: u64,
        tool_path: String,
        background: String,
        cli_theme: Option<String>,
    },
}
