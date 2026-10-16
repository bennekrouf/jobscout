//! JobScout — finds job posts on LinkedIn and ranks them against your profile.
//!
//! One window, three columns: the search, the jobs found so far, and the one
//! selected. Everything found is kept between runs (see `services::store`).

mod model;
mod services;
mod state;
mod ui;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

fn main() {
    // Before anything can raise a notification — see services::notify.
    services::notify::init();
    // Data from before countries is Swiss: move it into its country folder.
    services::store::migrate();
    // The default menu stays: without its Edit menu, macOS text fields lose
    // copy and paste, and the app loses Cmd+Q.
    let cfg = Config::new()
        .with_data_directory(services::store::data_dir().join("webview"))
        .with_window(
            WindowBuilder::new()
                .with_title(format!("JobScout {}", env!("CARGO_PKG_VERSION")))
                .with_inner_size(LogicalSize::new(1360.0, 860.0)),
        );
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(ui::App);
}
