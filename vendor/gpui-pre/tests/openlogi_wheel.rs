// Exercise the production motion owner without compiling unrelated upstream
// unit tests that require fonts outside the published source archive.
pub use gpui::{Modifiers, Pixels, Point, ScrollDelta, ScrollWheelEvent, point, px};
#[path = "../src/window/wheel/motion.rs"]
mod motion;
