//! The launcher's embedded app icon, rendered by phantom-kit's brand module.

pub use phantom_kit::brand::BrandAssets;
use phantom_kit::egui::IconData;

const ICON_256_PNG: &[u8] = include_bytes!("../assets/icons/icon-256.png");

/// The icon shown in the title bar and taskbar.
pub fn load_window_icon() -> Option<IconData> {
    phantom_kit::brand::window_icon(ICON_256_PNG)
}

/// Icon and wordmark renderer for the sidebar and About page.
pub fn load_brand() -> BrandAssets {
    BrandAssets::from_png(ICON_256_PNG)
}
