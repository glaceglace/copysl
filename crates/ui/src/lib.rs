pub mod app;
pub mod components;
pub mod ipc_client;
pub mod window;

use app::CopieurApp;

pub fn ui_main(socket_path: &str) {
    let cursor_pos = window::get_cursor_pos();

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

    let options = window::build_native_options(cursor_pos, &config);
    let socket_path = socket_path.to_string();

    if let Err(e) = eframe::run_native(
        "Copieur",
        options,
        Box::new(move |cc: &eframe::CreationContext<'_>| {
            Ok(Box::new(CopieurApp::new(cc, &socket_path)) as Box<dyn eframe::App>)
        }),
    ) {
        eprintln!("Copieur UI error: {e}");
    }
}
