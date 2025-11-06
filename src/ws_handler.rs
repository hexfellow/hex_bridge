/***************************************************************
 * Copyright 2025 Peng Shaohui. All rights reserved.
 * Author: Peng Shaohui 2375899670@qq.com
 * Date  : 2025-11-06
 ***************************************************************/

use futures_util::{SinkExt, StreamExt};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::MaybeTlsStream;

use crate::ros_handler;

pub(crate) async fn handle_ws_connection(read_only: bool, url: String) {
    let connect_timeout = Duration::from_secs(5);
    let res = timeout(connect_timeout, tokio_tungstenite::connect_async(&url)).await;
    let ws_stream = match res {
        Ok(Ok((ws, _))) => {
            hex_info!("Connected to: {} successfully", url);
            ws
        }
        Ok(Err(e)) => {
            hex_err!("Error during websocket handshake: {}", e);
            return;
        }
        Err(_) => {
            hex_err!("Connection timeout");
            return;
        }
    };
    match ws_stream.get_ref() {
        MaybeTlsStream::Plain(stream) => {
            stream.set_nodelay(true).unwrap();
        }
        _ => {
            hex_warn!("set_nodelay not implemented for TLS streams");
        }
    }
    let (ws_sink, mut ws_stream) = ws_stream.split();

    // Wrap ws_sink in Arc<Mutex> to share between tasks
    let ws_sink = std::sync::Arc::new(tokio::sync::Mutex::new(ws_sink));

    // Create disconnect notification channel
    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Create a channel to track last activity time
    let (activity_tx, activity_rx) =
        tokio::sync::watch::channel::<std::time::Instant>(std::time::Instant::now());

    let (pub_tx, pub_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);
    let disconnect_tx_read = disconnect_tx.clone();
    tokio::spawn(async move {
        while let Some(msg) = ws_stream.next().await {
            use tokio_tungstenite::tungstenite::Message::*;
            if let Ok(msg) = msg {
                match msg {
                    Binary(bin) => {
                        let _ = pub_tx.send(bin.into()).await;
                        let _ = activity_tx.send(std::time::Instant::now());
                    }
                    Pong(_) => {
                        // Update last activity on receiving Pong
                        let _ = activity_tx.send(std::time::Instant::now());
                    }
                    Close(_) => {
                        hex_warn!("WebSocket received Close frame");
                        break;
                    }
                    _ => {
                        // Ignore other message types (Text, Ping, etc.)
                    }
                }
            } else {
                hex_warn!("WebSocket receive error, connection likely closed");
                break;
            }
        }
        hex_warn!("WebSocket read stream closed");
        let _ = disconnect_tx_read.send(()).await;
    });
    tokio::spawn(ros_handler::ws_publisher(pub_rx));

    if !read_only {
        let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);
        tokio::spawn(ros_handler::ws_subscriber(sub_tx));
        let disconnect_tx_write = disconnect_tx.clone();
        let ws_sink_clone = ws_sink.clone();
        tokio::spawn(async move {
            while let Some(msg) = sub_rx.recv().await {
                use tokio_tungstenite::tungstenite::Message::*;
                let mut sink = ws_sink_clone.lock().await;
                if let Err(e) = sink.send(Binary(msg.into())).await {
                    hex_err!("WebSocket send error: {}", e);
                    let _ = disconnect_tx_write.send(()).await;
                    break;
                }
            }
        });
    }

    // Heartbeat task: send Ping every 2 seconds
    let disconnect_tx_ping = disconnect_tx.clone();
    let ws_sink_ping = ws_sink.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            use tokio_tungstenite::tungstenite::Message::*;
            let mut sink = ws_sink_ping.lock().await;
            if let Err(e) = sink.send(Ping(vec![].into())).await {
                hex_err!("WebSocket ping error: {}", e);
                let _ = disconnect_tx_ping.send(()).await;
                break;
            }
        }
    });

    // Timeout detection task: check if we received any activity in the last 5 seconds
    let disconnect_tx_timeout = disconnect_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let timeout_duration = Duration::from_secs(5);
        loop {
            interval.tick().await;
            let last_activity = *activity_rx.borrow();
            if last_activity.elapsed() > timeout_duration {
                hex_err!("WebSocket timeout: no activity for {:?}", timeout_duration);
                let _ = disconnect_tx_timeout.send(()).await;
                break;
            }
        }
    });

    // Drop the sender held by main thread, so receiver will return None when all senders are dropped
    drop(disconnect_tx);

    // Wait for disconnect signal from any task
    disconnect_rx.recv().await;
    hex_err!("WebSocket disconnected");
}
