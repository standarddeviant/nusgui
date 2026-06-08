#![crate_name = "nusgui"]

use tracing::metadata::LevelFilter;
use tracing_subscriber::filter;
use tracing_subscriber::prelude::*;

// use tracing::{error, info, warn};

mod app;
mod btnus;
mod scan_table;

use app::NusGui;

pub fn main() -> eframe::Result<()> {
    // NOTE: logging/tracing config first
    let filter = filter::Targets::new()
        // Enable the `INFO` level for anything in `my_crate`
        .with_default(LevelFilter::INFO)
        .with_target("nusgui", LevelFilter::INFO)
        .with_target("bluest", LevelFilter::WARN);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().boxed())
        .with(filter.boxed())
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]), // Set default size here
        ..Default::default()
    };

    eframe::run_native(
        "NUS GUI",
        options,
        Box::new(|cc| {
            Ok(Box::new(
                //
                NusGui::new(&cc.egui_ctx.clone(), cc), //
            ))
        }), //
    )
}
