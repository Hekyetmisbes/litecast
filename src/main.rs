#![windows_subsystem = "windows"]

fn main() {
    let _guard = match litecast::platform_windows::try_acquire_single_instance() {
        Some(guard) => guard,
        None => {
            // Another instance is already running and was signaled to activate.
            return;
        }
    };

    let mut app = litecast::app::App::new();
    app.run();
}
