pub mod app;
pub mod components;
pub mod emoji;
pub mod ipc_client;
pub mod style;
pub mod window;

use app::CopyslApp;

pub fn ui_main(socket_path: &str) {
    let config = {
        let mut ipc = ipc_client::IpcClient::connect_to(socket_path).ok();
        let ipc_config = ipc.as_mut().and_then(|c| {
            c.send(&common::DaemonRequest::GetConfig).ok().and_then(|r| {
                if let common::DaemonResponse::Config(cfg) = r {
                    Some(cfg)
                } else {
                    None
                }
            })
        });
        ipc_config.unwrap_or_else(|| config::load().unwrap_or_default())
    };

    // Query screen dimensions via XCB so we can pass a position hint.
    // On pure Wayland (no DISPLAY) this returns None and the compositor
    // decides placement — for "Center of screen" that is fine because
    // GNOME centers new windows by default.
    let (_, screen_size) = window::get_display_info();

    let options = window::build_native_options(screen_size, &config);
    let socket_path = socket_path.to_string();

    if let Err(e) = eframe::run_native(
        "Copysl",
        options,
        Box::new(move |cc: &eframe::CreationContext<'_>| {
            Ok(Box::new(CopyslApp::new(cc, &socket_path)) as Box<dyn eframe::App>)
        }),
    ) {
        eprintln!("Copysl UI error: {e}");
    }
}
