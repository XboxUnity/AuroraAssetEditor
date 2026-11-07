use std::cell::Cell;

use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};
use image::RgbaImage;
use slint::platform::WindowEvent;
use slint::{LogicalSize, SharedString, Window};

thread_local! {
    static BACKEND_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Renders Slint windows without a display: the Slint testing backend with the software
/// renderer, mocked time and scale factor 1. Slint's platform is per thread, so each test
/// thread gets its own backend.
pub struct HeadlessUi;

impl HeadlessUi {
    pub const WINDOW_SIZE: (u32, u32) = (1024, 680);
    pub const MINIMUM_WINDOW_SIZE: (u32, u32) = (760, 500);

    pub fn new() -> HeadlessUi {
        if !BACKEND_INSTALLED.get() {
            let backend = TestingBackend::new(TestingBackendOptions {
                mock_time: true,
                threading: false,
                renderer_name: Some("software".into()),
            });
            slint::platform::set_platform(Box::new(backend))
                .expect("no other Slint platform is installed on a test thread");
            BACKEND_INSTALLED.set(true);
        }
        HeadlessUi
    }

    pub fn show(&self, window: &Window) {
        self.show_at_size(window, Self::WINDOW_SIZE);
    }

    pub fn show_at_size(&self, window: &Window, (width, height): (u32, u32)) {
        window.set_size(LogicalSize::new(width as f32, height as f32));
        window
            .show()
            .expect("the testing backend can always show a window");
    }

    pub fn snapshot(&self, window: &Window) -> RgbaImage {
        let pixels = window
            .take_snapshot()
            .expect("the software renderer can always take a snapshot");
        RgbaImage::from_raw(pixels.width(), pixels.height(), pixels.as_bytes().to_vec())
            .expect("a snapshot holds width x height RGBA pixels")
    }

    pub fn type_text(&self, window: &Window, text: &str) {
        for character in text.chars() {
            let key = SharedString::from(character.to_string());
            window.dispatch_event(WindowEvent::KeyPressed { text: key.clone() });
            window.dispatch_event(WindowEvent::KeyReleased { text: key });
        }
    }
}

impl Default for HeadlessUi {
    fn default() -> Self {
        Self::new()
    }
}
