#[cfg(debug_assertions)]
mod advanced;
mod app;
mod battery;
mod color_picker;
mod fan;
mod fan_gauge;
mod layout;
mod pages;
mod theme;
mod widgets;

pub use app::ClevoLedApp;
pub(crate) use theme::apply as apply_theme;
pub use widgets::install_cjk_font;
