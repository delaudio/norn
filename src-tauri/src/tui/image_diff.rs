use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::Protocol,
    Resize,
};

use crate::services::bitbucket::{PrFilePreview, MAX_PR_IMAGE_PREVIEW_BYTES};

pub const MAX_IMAGE_DIMENSION: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 40_000_000;
const MAX_IMAGE_DECODE_BYTES: u64 = MAX_IMAGE_PIXELS * 4 + 16 * 1024 * 1024;
const MIN_COMPARISON_WIDTH: u16 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSide {
    Old,
    New,
}

impl ImageSide {
    pub fn provider_value(self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::New => "new",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Old => "base",
            Self::New => "changed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageChangeKind {
    Added,
    Modified,
    Deleted,
}

impl ImageChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageDiffCandidate {
    pub kind: ImageChangeKind,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
}

impl ImageDiffCandidate {
    pub fn path_for(&self, side: ImageSide) -> Option<&str> {
        match side {
            ImageSide::Old => self.old_path.as_deref(),
            ImageSide::New => self.new_path.as_deref(),
        }
    }

    pub fn default_side(&self) -> ImageSide {
        if self.new_path.is_some() {
            ImageSide::New
        } else {
            ImageSide::Old
        }
    }

    pub fn has_side(&self, side: ImageSide) -> bool {
        self.path_for(side).is_some()
    }
}

pub struct TerminalImageSupport {
    picker: Option<Picker>,
    label: &'static str,
}

impl TerminalImageSupport {
    pub fn detect() -> Self {
        Self::from_picker(Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()))
    }

    pub fn metadata_only() -> Self {
        Self::from_picker(Picker::halfblocks())
    }

    fn from_picker(picker: Picker) -> Self {
        let label = match picker.protocol_type() {
            ProtocolType::Kitty => "Kitty",
            ProtocolType::Sixel => "Sixel",
            ProtocolType::Iterm2 => "iTerm2",
            ProtocolType::Halfblocks => "unsupported",
        };
        Self {
            picker: (picker.protocol_type() != ProtocolType::Halfblocks).then_some(picker),
            label,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    fn picker(&self) -> Option<&Picker> {
        self.picker.as_ref()
    }
}

pub struct ImageDiffState {
    pub selected_file: usize,
    pub candidate: ImageDiffCandidate,
    pub selected_side: ImageSide,
    pub old: Option<ImageVersionState>,
    pub new: Option<ImageVersionState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageComparisonAreas {
    pub base_label: Rect,
    pub base_image: Rect,
    pub changed_label: Rect,
    pub changed_image: Rect,
}

fn comparison_areas(area: Rect) -> Option<ImageComparisonAreas> {
    if area.width < MIN_COMPARISON_WIDTH || area.height < 2 {
        return None;
    }
    let [base_column, gap, changed_column] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 2),
            Constraint::Length(1),
            Constraint::Ratio(1, 2),
        ])
        .split(area)
    else {
        return None;
    };
    let [base_label, base_image] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(base_column)
    else {
        return None;
    };
    let [changed_label, changed_image] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(changed_column)
    else {
        return None;
    };
    debug_assert_eq!(gap.width, 1);
    Some(ImageComparisonAreas {
        base_label,
        base_image,
        changed_label,
        changed_image,
    })
}

impl ImageDiffState {
    pub fn load(
        selected_file: usize,
        candidate: ImageDiffCandidate,
        mut fetch: impl FnMut(ImageSide, &str) -> Result<PrFilePreview, String>,
    ) -> Self {
        let old = candidate
            .old_path
            .as_deref()
            .map(|path| load_version(ImageSide::Old, path, &mut fetch));
        let new = candidate
            .new_path
            .as_deref()
            .map(|path| load_version(ImageSide::New, path, &mut fetch));
        let selected_side = candidate.default_side();
        Self {
            selected_file,
            candidate,
            selected_side,
            old,
            new,
        }
    }

    pub fn selected(&self) -> Option<&ImageVersionState> {
        match self.selected_side {
            ImageSide::Old => self.old.as_ref(),
            ImageSide::New => self.new.as_ref(),
        }
    }

    pub fn comparison_areas(&self, area: Rect) -> Option<ImageComparisonAreas> {
        matches!(
            (&self.old, &self.new),
            (
                Some(ImageVersionState::Ready(_)),
                Some(ImageVersionState::Ready(_))
            )
        )
        .then(|| comparison_areas(area))
        .flatten()
    }

    fn selected_mut(&mut self) -> Option<&mut ImageVersionState> {
        match self.selected_side {
            ImageSide::Old => self.old.as_mut(),
            ImageSide::New => self.new.as_mut(),
        }
    }

    pub fn toggle_side(&mut self) -> bool {
        let next = match self.selected_side {
            ImageSide::Old => ImageSide::New,
            ImageSide::New => ImageSide::Old,
        };
        if self.candidate.has_side(next) {
            self.selected_side = next;
            true
        } else {
            false
        }
    }

    pub fn prepare_protocol(
        &mut self,
        support: &TerminalImageSupport,
        area: Rect,
    ) -> Result<(), String> {
        let Some(picker) = support.picker() else {
            return Ok(());
        };
        if let Some(areas) = self.comparison_areas(area) {
            if let Some(ImageVersionState::Ready(version)) = self.old.as_mut() {
                prepare_version_protocol(version, picker, areas.base_image);
            }
            if let Some(ImageVersionState::Ready(version)) = self.new.as_mut() {
                prepare_version_protocol(version, picker, areas.changed_image);
            }
            return Ok(());
        }
        let Some(ImageVersionState::Ready(version)) = self.selected_mut() else {
            return Ok(());
        };
        prepare_version_protocol(version, picker, area);
        Ok(())
    }
}

fn prepare_version_protocol(version: &mut DecodedImageVersion, picker: &Picker, area: Rect) {
    let protocol_area = Rect::new(0, 0, area.width.max(1), area.height.max(1));
    if version.protocol_area == Some(protocol_area)
        && (version.protocol.is_some() || version.protocol_error.is_some())
    {
        return;
    }
    version.protocol = None;
    version.protocol_error = None;
    version.protocol_area = Some(protocol_area);
    match picker.new_protocol(version.image.clone(), protocol_area, Resize::Fit(None)) {
        Ok(protocol) => version.protocol = Some(protocol),
        Err(error) => {
            version.protocol_error = Some(format!("Could not prepare terminal image: {error}"));
        }
    }
}

pub enum ImageVersionState {
    Ready(DecodedImageVersion),
    Failed(FailedImageVersion),
}

impl ImageVersionState {
    pub fn path(&self) -> &str {
        match self {
            Self::Ready(version) => &version.path,
            Self::Failed(version) => &version.path,
        }
    }
}

pub struct DecodedImageVersion {
    pub path: String,
    pub side: ImageSide,
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
    pub byte_size: usize,
    pub protocol: Option<Protocol>,
    pub protocol_error: Option<String>,
    protocol_area: Option<Rect>,
    image: DynamicImage,
}

pub struct FailedImageVersion {
    pub path: String,
    pub side: ImageSide,
    pub error: String,
}

fn load_version(
    side: ImageSide,
    path: &str,
    fetch: &mut impl FnMut(ImageSide, &str) -> Result<PrFilePreview, String>,
) -> ImageVersionState {
    match fetch(side, path).and_then(|preview| decode_preview(side, preview)) {
        Ok(version) => ImageVersionState::Ready(version),
        Err(error) => ImageVersionState::Failed(FailedImageVersion {
            path: path.to_string(),
            side,
            error,
        }),
    }
}

fn decode_preview(side: ImageSide, preview: PrFilePreview) -> Result<DecodedImageVersion, String> {
    let (_, encoded) = preview
        .data_url
        .split_once(',')
        .ok_or_else(|| "Provider returned an invalid image data URL.".to_string())?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| format!("Provider returned invalid base64 image data: {error}"))?;
    if bytes.len() != preview.size {
        return Err(format!(
            "Provider image size mismatch: expected {} bytes, decoded {}.",
            preview.size,
            bytes.len()
        ));
    }
    if bytes.len() > MAX_PR_IMAGE_PREVIEW_BYTES {
        return Err(format!(
            "Image is {} bytes; the preview limit is {} bytes.",
            bytes.len(),
            MAX_PR_IMAGE_PREVIEW_BYTES
        ));
    }

    let mut metadata_reader = image_reader(&bytes)?;
    let format = metadata_reader
        .format()
        .ok_or_else(|| "Image format could not be detected.".to_string())?;
    let format_label = supported_format_label(format)?;
    metadata_reader.limits(decode_limits());
    let (width, height) = metadata_reader
        .into_dimensions()
        .map_err(|error| format!("Could not read image dimensions: {error}"))?;
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_IMAGE_PIXELS {
        return Err(format!(
            "Image dimensions {width}x{height} exceed the {MAX_IMAGE_PIXELS}-pixel limit."
        ));
    }

    let mut decoder = image_reader(&bytes)?;
    decoder.limits(decode_limits());
    let image = decoder
        .decode()
        .map_err(|error| format!("Could not decode image: {error}"))?;

    Ok(DecodedImageVersion {
        path: preview.path,
        side,
        format: format_label,
        width,
        height,
        byte_size: bytes.len(),
        protocol: None,
        protocol_error: None,
        protocol_area: None,
        image,
    })
}

fn image_reader(bytes: &[u8]) -> Result<ImageReader<Cursor<&[u8]>>, String> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("Could not inspect image format: {error}"))
}

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_DECODE_BYTES);
    limits
}

fn supported_format_label(format: ImageFormat) -> Result<&'static str, String> {
    match format {
        ImageFormat::Png => Ok("PNG"),
        ImageFormat::Jpeg => Ok("JPEG"),
        ImageFormat::Gif => Ok("GIF"),
        ImageFormat::WebP => Ok("WebP"),
        _ => Err(format!(
            "Decoded format {format:?} is not supported in terminal image diffs."
        )),
    }
}

pub fn image_candidate_from_patch(patch: &str) -> Option<ImageDiffCandidate> {
    let mut old_path = None;
    let mut new_path = None;
    let mut added = false;
    let mut deleted = false;
    for line in patch.lines() {
        if let Some(path) = line.strip_prefix("--- ") {
            old_path = normalize_path(path);
        } else if let Some(path) = line.strip_prefix("+++ ") {
            new_path = normalize_path(path);
        } else if line.starts_with("new file mode ") {
            added = true;
        } else if line.starts_with("deleted file mode ") {
            deleted = true;
        }
    }
    if old_path.is_none() && new_path.is_none() {
        let header = patch.lines().find(|line| line.starts_with("diff --git "))?;
        let (old_header_path, new_header_path) = parse_diff_header_paths(header)?;
        old_path = normalize_path(&old_header_path);
        new_path = normalize_path(&new_header_path);
    }
    if added {
        old_path = None;
    }
    if deleted {
        new_path = None;
    }
    old_path = old_path.filter(|path| is_supported_image_path(path));
    new_path = new_path.filter(|path| is_supported_image_path(path));
    let kind = match (old_path.is_some(), new_path.is_some()) {
        (false, true) => ImageChangeKind::Added,
        (true, false) => ImageChangeKind::Deleted,
        (true, true) => ImageChangeKind::Modified,
        (false, false) => return None,
    };
    Some(ImageDiffCandidate {
        kind,
        old_path,
        new_path,
    })
}

fn normalize_path(path: &str) -> Option<String> {
    let path = decode_git_path(path.trim())?;
    if path == "/dev/null" {
        return None;
    }
    Some(
        path.strip_prefix("a/")
            .or_else(|| path.strip_prefix("b/"))
            .unwrap_or(&path)
            .to_string(),
    )
}

fn parse_diff_header_paths(header: &str) -> Option<(String, String)> {
    let input = header.strip_prefix("diff --git ")?;
    let (old_path, rest) = parse_git_path_token(input)?;
    let (new_path, rest) = parse_git_path_token(rest)?;
    rest.trim().is_empty().then_some((old_path, new_path))
}

fn decode_git_path(input: &str) -> Option<String> {
    if !input.starts_with('"') {
        return Some(input.to_string());
    }
    let (path, rest) = parse_git_path_token(input)?;
    rest.trim().is_empty().then_some(path)
}

fn parse_git_path_token(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if !input.starts_with('"') {
        let end = input.find(char::is_whitespace).unwrap_or(input.len());
        return (end > 0).then(|| (input[..end].to_string(), &input[end..]));
    }

    let bytes = input.as_bytes();
    let mut decoded = Vec::new();
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                let path = String::from_utf8(decoded).ok()?;
                return Some((path, &input[index + 1..]));
            }
            b'\\' => {
                index += 1;
                let escaped = *bytes.get(index)?;
                if (b'0'..=b'7').contains(&escaped) {
                    let mut value = 0_u16;
                    let mut count = 0;
                    while count < 3 && index < bytes.len() && (b'0'..=b'7').contains(&bytes[index])
                    {
                        value = value * 8 + u16::from(bytes[index] - b'0');
                        index += 1;
                        count += 1;
                    }
                    decoded.push(u8::try_from(value).ok()?);
                    continue;
                }
                decoded.push(match escaped {
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'v' => 0x0b,
                    b'f' => 0x0c,
                    b'r' => b'\r',
                    b'\\' => b'\\',
                    b'"' => b'"',
                    _ => return None,
                });
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    None
}

fn is_supported_image_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn image_preview(
        path: &str,
        width: u32,
        height: u32,
        format: ImageFormat,
        mime_type: &str,
    ) -> PrFilePreview {
        let image =
            DynamicImage::ImageRgb8(ImageBuffer::from_pixel(width, height, Rgb([10, 20, 30])));
        let mut bytes = Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, format)
            .expect("encode test image");
        let bytes = bytes.into_inner();
        PrFilePreview {
            path: path.to_string(),
            mime_type: mime_type.to_string(),
            data_url: format!("data:{mime_type};base64,{}", STANDARD.encode(&bytes)),
            size: bytes.len(),
        }
    }

    fn png_preview(path: &str, width: u32, height: u32) -> PrFilePreview {
        image_preview(path, width, height, ImageFormat::Png, "image/png")
    }

    #[test]
    fn detects_added_modified_and_deleted_image_patches() {
        let added = image_candidate_from_patch(
            "diff --git a/new.png b/new.png\n--- /dev/null\n+++ b/new.png\nBinary files differ\n",
        )
        .expect("added image");
        assert_eq!(added.kind, ImageChangeKind::Added);
        assert_eq!(added.old_path, None);
        assert_eq!(added.new_path.as_deref(), Some("new.png"));

        let modified = image_candidate_from_patch(
            "diff --git a/image.webp b/image.webp\n--- a/image.webp\n+++ b/image.webp\nBinary files differ\n",
        )
        .expect("modified image");
        assert_eq!(modified.kind, ImageChangeKind::Modified);
        assert!(modified.has_side(ImageSide::Old));
        assert!(modified.has_side(ImageSide::New));

        let deleted = image_candidate_from_patch(
            "diff --git a/old.gif b/old.gif\n--- a/old.gif\n+++ /dev/null\nBinary files differ\n",
        )
        .expect("deleted image");
        assert_eq!(deleted.kind, ImageChangeKind::Deleted);
        assert_eq!(deleted.default_side(), ImageSide::Old);

        let added_without_markers = image_candidate_from_patch(
            "diff --git a/new.png b/new.png\nnew file mode 100644\nBinary files /dev/null and b/new.png differ\n",
        )
        .expect("added image from mode");
        assert_eq!(added_without_markers.kind, ImageChangeKind::Added);
        assert_eq!(added_without_markers.old_path, None);
    }

    #[test]
    fn parses_quoted_image_paths_and_cross_type_renames() {
        let spaced = image_candidate_from_patch(
            "diff --git \"a/assets/foo bar.png\" \"b/assets/foo bar.png\"\nBinary files \"a/assets/foo bar.png\" and \"b/assets/foo bar.png\" differ\n",
        )
        .expect("quoted image path");
        assert_eq!(spaced.kind, ImageChangeKind::Modified);
        assert_eq!(spaced.new_path.as_deref(), Some("assets/foo bar.png"));

        let image_to_text = image_candidate_from_patch(
            "diff --git a/logo.png b/logo.txt\nsimilarity index 100%\nrename from logo.png\nrename to logo.txt\n",
        )
        .expect("image renamed to text");
        assert_eq!(image_to_text.kind, ImageChangeKind::Deleted);
        assert_eq!(image_to_text.old_path.as_deref(), Some("logo.png"));
        assert_eq!(image_to_text.new_path, None);

        let text_to_image = image_candidate_from_patch(
            "diff --git a/logo.txt b/logo.png\nsimilarity index 100%\nrename from logo.txt\nrename to logo.png\n",
        )
        .expect("text renamed to image");
        assert_eq!(text_to_image.kind, ImageChangeKind::Added);
        assert_eq!(text_to_image.old_path, None);
        assert_eq!(text_to_image.new_path.as_deref(), Some("logo.png"));
    }

    #[test]
    fn ignores_text_and_unsupported_binary_files() {
        assert!(image_candidate_from_patch(
            "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n"
        )
        .is_none());
        assert!(image_candidate_from_patch(
            "diff --git a/archive.zip b/archive.zip\nBinary files differ\n"
        )
        .is_none());
    }

    #[test]
    fn decodes_bounded_image_metadata_and_rejects_corrupt_content() {
        let decoded =
            decode_preview(ImageSide::New, png_preview("image.png", 12, 7)).expect("decode image");
        assert_eq!(decoded.format, "PNG");
        assert_eq!((decoded.width, decoded.height), (12, 7));
        assert!(decoded.byte_size > 0);

        let corrupt = PrFilePreview {
            path: "broken.png".to_string(),
            mime_type: "image/png".to_string(),
            data_url: format!("data:image/png;base64,{}", STANDARD.encode(b"not an image")),
            size: 12,
        };
        assert!(decode_preview(ImageSide::New, corrupt).is_err());
    }

    #[test]
    fn decodes_every_supported_raster_format() {
        let formats = [
            (ImageFormat::Png, "image.png", "image/png", "PNG"),
            (ImageFormat::Jpeg, "image.jpg", "image/jpeg", "JPEG"),
            (ImageFormat::Gif, "image.gif", "image/gif", "GIF"),
            (ImageFormat::WebP, "image.webp", "image/webp", "WebP"),
        ];

        for (format, path, mime_type, expected_label) in formats {
            let decoded =
                decode_preview(ImageSide::New, image_preview(path, 5, 3, format, mime_type))
                    .unwrap_or_else(|error| panic!("decode {path}: {error}"));
            assert_eq!(decoded.format, expected_label);
            assert_eq!((decoded.width, decoded.height), (5, 3));
        }
    }

    #[test]
    fn rejects_oversized_dimensions_before_full_decode() {
        let error = match decode_preview(
            ImageSide::New,
            png_preview("wide.png", MAX_IMAGE_DIMENSION + 1, 1),
        ) {
            Ok(_) => panic!("oversized dimensions must be rejected"),
            Err(error) => error,
        };
        assert!(error.contains("dimensions") || error.contains("limits"));
    }

    #[test]
    fn halfblocks_are_treated_as_metadata_only_fallback() {
        let support = TerminalImageSupport::from_picker(Picker::halfblocks());
        assert_eq!(support.label(), "unsupported");
        assert!(support.picker().is_none());
    }

    #[test]
    fn supported_terminal_prepares_an_inline_protocol() {
        let candidate = ImageDiffCandidate {
            kind: ImageChangeKind::Added,
            old_path: None,
            new_path: Some("image.png".to_string()),
        };
        let mut state = ImageDiffState::load(0, candidate, |side, path| {
            assert_eq!(side, ImageSide::New);
            Ok(png_preview(path, 4, 3))
        });
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        let support = TerminalImageSupport::from_picker(picker);

        state
            .prepare_protocol(&support, Rect::new(0, 0, 20, 10))
            .expect("prepare Kitty protocol");

        assert_eq!(support.label(), "Kitty");
        assert!(matches!(
            state.selected(),
            Some(ImageVersionState::Ready(version)) if version.protocol.is_some()
        ));
    }

    #[test]
    fn modified_images_load_and_toggle_both_versions() {
        let candidate = ImageDiffCandidate {
            kind: ImageChangeKind::Modified,
            old_path: Some("before.png".to_string()),
            new_path: Some("after.png".to_string()),
        };
        let mut fetched = Vec::new();
        let mut state = ImageDiffState::load(2, candidate, |side, path| {
            fetched.push((side, path.to_string()));
            Ok(png_preview(path, 4, 3))
        });

        assert_eq!(
            fetched,
            vec![
                (ImageSide::Old, "before.png".to_string()),
                (ImageSide::New, "after.png".to_string())
            ]
        );
        assert_eq!(state.selected_side, ImageSide::New);
        assert!(matches!(state.old, Some(ImageVersionState::Ready(_))));
        assert!(matches!(state.new, Some(ImageVersionState::Ready(_))));
        assert!(state.toggle_side());
        assert_eq!(state.selected_side, ImageSide::Old);
    }

    #[test]
    fn modified_images_prepare_both_versions_for_a_wide_comparison() {
        let candidate = ImageDiffCandidate {
            kind: ImageChangeKind::Modified,
            old_path: Some("before.png".to_string()),
            new_path: Some("after.png".to_string()),
        };
        let mut state =
            ImageDiffState::load(2, candidate, |_side, path| Ok(png_preview(path, 4, 3)));
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        let support = TerminalImageSupport::from_picker(picker);
        let area = Rect::new(0, 0, 100, 12);

        let areas = state.comparison_areas(area).expect("wide comparison areas");
        assert_eq!(areas.base_label.width, 49);
        assert_eq!(areas.changed_label.width, 50);
        assert_eq!(areas.base_image.height, 11);
        assert_eq!(areas.changed_image.height, 11);
        state
            .prepare_protocol(&support, area)
            .expect("prepare both Kitty protocols");

        assert!(matches!(
            state.old,
            Some(ImageVersionState::Ready(DecodedImageVersion {
                protocol: Some(_),
                ..
            }))
        ));
        assert!(matches!(
            state.new,
            Some(ImageVersionState::Ready(DecodedImageVersion {
                protocol: Some(_),
                ..
            }))
        ));
        assert!(state.comparison_areas(Rect::new(0, 0, 79, 12)).is_none());
    }
}
