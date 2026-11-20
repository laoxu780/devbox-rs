pub mod app;
pub mod pages;
pub mod text_input;
pub mod widgets;

/// Stable, per-instance element IDs for interactive elements built in loops.
///
/// GPUI keys click/hover state by element ID. Reusing one literal ID for every
/// item in a loop means only the first item receives clicks.
pub fn id(name: impl Into<gpui::SharedString>) -> gpui::ElementId {
    gpui::ElementId::Name(name.into())
}

/// Generate a stable, unique ID for interactive elements built inside loops.
/// The counter is reset per render, so a given iteration keeps the same ID
/// across frames as long as the list order is unchanged.
pub fn loop_id(prefix: &str, seq: &std::cell::Cell<usize>) -> gpui::ElementId {
    let n = seq.get();
    seq.set(n + 1);
    id(format!("{prefix}-{n}"))
}
