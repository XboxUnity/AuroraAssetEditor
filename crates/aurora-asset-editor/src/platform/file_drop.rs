use std::path::PathBuf;

use slint::winit_030::winit::event::WindowEvent;

/// What a window event means for a drop in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropSignal {
    None,
    /// Files are dragged over the window (`true`) or the drag left it (`false`).
    Hover(bool),
    /// The first file of a drop arrived; the rest of the drop follows in the same event-loop turn.
    FirstFile,
}

/// Collects the files of one drop. winit reports each dropped file as its own event, so the first one
/// schedules the batch and `take` hands over every file that arrived until then.
#[derive(Debug, Default)]
pub struct FileDropCollector {
    dropped: Vec<PathBuf>,
}

impl FileDropCollector {
    pub fn on_event(&mut self, event: &WindowEvent) -> DropSignal {
        match event {
            WindowEvent::HoveredFile(_) => DropSignal::Hover(true),
            WindowEvent::HoveredFileCancelled => DropSignal::Hover(false),
            WindowEvent::DroppedFile(path) => {
                self.dropped.push(path.clone());
                if self.dropped.len() == 1 {
                    DropSignal::FirstFile
                } else {
                    DropSignal::None
                }
            }
            _ => DropSignal::None,
        }
    }

    pub fn take(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.dropped)
    }
}
