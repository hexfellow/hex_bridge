/***************************************************************
 * Copyright 2025 Peng Shaohui. All rights reserved.
 * Author: Peng Shaohui 2375899670@qq.com
 * Date  : 2025-11-06
 ***************************************************************/

#[macro_use]
mod ros_handler;
mod ws_handler;

#[tokio::main]
async fn main() {
    ros_handler::init("hex_bridge")
        .await
        .expect("Failed to initialize ROS node");

    let url =
        ros_handler::get_or_declare_parameter("url", r2r::ParameterValue::String("".to_string()))
            .await;

    let read_only =
        ros_handler::get_or_declare_parameter("read_only", r2r::ParameterValue::Bool(false)).await;

    let url = match url {
        r2r::ParameterValue::String(s) => s,
        _ => "".to_string(),
    };
    let read_only = match read_only {
        r2r::ParameterValue::Bool(b) => b,
        _ => false,
    };

    hex_info!("Starting WebSocket bridge");
    hex_info!("  URL: {}", url);
    hex_info!("  Read-only: {}", read_only);

    // Run until Ctrl+C or WebSocket disconnects
    tokio::select! {
        _ = ws_handler::handle_ws_connection(read_only, url) => {}
        _ = tokio::signal::ctrl_c() => {
            hex_info!("Received Ctrl+C, shutting down...");
        }
    }

    hex_info!("Cleaning up...");
    // Give tasks a moment to clean up
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    hex_info!("Shutdown complete");
}
