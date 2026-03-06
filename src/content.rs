use std::path::PathBuf;

use ratatui::text::Line;
use ratatui_image::protocol::StatefulProtocol;

#[allow(clippy::large_enum_variant)]
pub enum ContentBlock {
    Text {
        lines: Vec<Line<'static>>,
    },
    Image {
        alt_text: String,
        display_height: u16,
        protocol: Option<StatefulProtocol>,
        error: Option<String>,
        source: ImageSource,
    },
}

pub enum ImageSource {
    Local(PathBuf),
    Remote(String),
}
