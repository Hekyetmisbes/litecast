use crate::config::AppConfig;
use crate::telemetry_local::log_info;
use crate::ui::run_ui;
use crate::updater::{cleanup_old_update_artifacts, start_background_update_worker};

#[derive(Default)]
pub struct App;

impl App {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&mut self) {
        log_info("LiteCast application starting up.");

        // Clean up any leftover .old or pending update binaries from previous update
        cleanup_old_update_artifacts();

        // Start background auto-updater if enabled
        let config = AppConfig::load();
        if config.auto_update {
            start_background_update_worker(config.update_url.clone());
        }

        run_ui();
        log_info("LiteCast application shut down cleanly.");
    }
}
