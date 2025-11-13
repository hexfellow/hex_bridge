/***************************************************************
 * Copyright 2025 Peng Shaohui. All rights reserved.
 * Author: Peng Shaohui 2375899670@qq.com
 * Date  : 2025-11-06
 ***************************************************************/

use log::{error, info, warn};

mod ros_handler;
mod ws_handler;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    )
    .init();

    ros_handler::init("hex_bridge")
        .await
        .expect("Failed to initialize ROS node");

    let url = ros_handler::get_parameter("url", r2r::ParameterValue::String("".to_string())).await;
    let read_only = ros_handler::get_parameter("read_only", r2r::ParameterValue::Bool(false)).await;
    let is_kcp = ros_handler::get_parameter("is_kcp", r2r::ParameterValue::Bool(true)).await;

    let url = match url {
        r2r::ParameterValue::String(s) => s,
        _ => "".to_string(),
    };
    let read_only = match read_only {
        r2r::ParameterValue::Bool(b) => b,
        _ => false,
    };
    let is_kcp = match is_kcp {
        r2r::ParameterValue::Bool(b) => b,
        _ => true,
    };

    info!("Starting bridge");
    info!("  URL: {}", url);
    info!("  Read-Only: {}", read_only);
    info!("  KCP: {}", is_kcp);

    tokio::select! {
        _ = ws_handler::create_bridge(read_only, url.clone()), if is_kcp => {}
        _ = ws_handler::handle_ws_connection(read_only, url.clone()), if !is_kcp => {}
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
    }

    info!("Cleaning up...");
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    info!("Shutdown complete");
}
