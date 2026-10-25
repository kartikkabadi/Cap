#![allow(dead_code)]
mod theme {
    use gpui::{Hsla, Rgba};
    pub struct Editor { pub ctl: Rgba, pub accent: Rgba, pub text_1: Rgba, pub text_3: Rgba }
    pub struct Theme { pub editor: Editor }
    impl Theme {
        pub const SETTINGS_ACCENT: u32 = 0x3366cc;
        pub fn dark() -> Self { Self { editor: Editor { ctl: gpui::rgb(0x222222), accent: gpui::rgb(Self::SETTINGS_ACCENT), text_1: gpui::rgb(0xffffff), text_3: gpui::rgb(0xaaaaaa) } } }
        pub fn gray(&self, _: u8) -> Hsla { gpui::white() }
        pub fn settings_fill(&self) -> Hsla { gpui::black() }
        pub fn settings_border(&self) -> Hsla { gpui::white() }
        pub fn settings_text(&self) -> Hsla { gpui::white() }
        pub fn settings_muted(&self) -> Hsla { gpui::white() }
        pub fn body_fill(&self, _: u8) -> Hsla { gpui::black() }
        pub fn body_border(&self, _: u8) -> Hsla { gpui::white() }
        pub fn with_alpha(color: Rgba, alpha: f32) -> Hsla { let mut c=Hsla::from(color); c.a=alpha; c }
    }
}
mod ui {
    #[path = "/Users/user/Documents/Codex/2026-10-24/ok-x20/work/Cap/apps/desktop-gpui/src/ui/keyboard_nav.rs"]
    pub mod keyboard_nav;
    #[path = "/Users/user/Documents/Codex/2026-10-24/ok-x20/work/Cap/apps/desktop-gpui/src/ui/text_input.rs"]
    pub mod text_input;
}
fn main() { ui::text_input::native_review::run(); }
