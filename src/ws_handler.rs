/***************************************************************
 * Copyright 2025 Peng Shaohui. All rights reserved.
 * Author: Peng Shaohui 2375899670@qq.com
 * Date  : 2025-11-06
 ***************************************************************/

use futures_util::{SinkExt, StreamExt};
use kcp_bindings::{HexSocketOpcode, HexSocketParser, KcpPortOwner};
use log::{error, info, warn};
use prost::Message;
use std::net::{SocketAddr, SocketAddrV4};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite;

use crate::ros_handler;

// Protobuf generated code.
pub mod base_backend {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

const ACCEPTABLE_PROTOCOL_MAJOR_VERSION: u32 = 1;

pub(crate) async fn handle_ws_connection(read_only: bool, url: String) {
    let url = format!("ws://{}", url);
    let res = tokio_tungstenite::connect_async(&url).await;
    let ws_stream = match res {
        Ok((ws, _)) => ws,
        Err(e) => {
            error!("Error during websocket handshake: {}", e);
            return;
        }
    };
    info!("Connected to: {}", url);
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    let (pub_tx, pub_rx) = mpsc::channel::<Vec<u8>>(100);
    tokio::spawn(async move {
        while let Some(msg) = ws_stream.next().await {
            use tungstenite::Message::*;
            if let Ok(msg) = msg {
                if let Binary(bin) = msg {
                    let _ = pub_tx.send(bin.into()).await;
                }
            }
        }
    });
    tokio::spawn(ros_handler::ws_publisher(pub_rx));

    if !read_only {
        let (sub_tx, mut sub_rx) = mpsc::channel::<Vec<u8>>(100);
        tokio::spawn(ros_handler::ws_subscriber(sub_tx));
        tokio::spawn(async move {
            while let Some(msg) = sub_rx.recv().await {
                use tungstenite::Message::*;
                if let Err(_) = ws_sink.send(Binary(msg.into())).await {
                    break;
                }
            }
        });
    }

    std::future::pending::<()>().await;
}

pub(crate) async fn create_bridge(read_only: bool, url: String) {
    let socket_addr: SocketAddrV4 = url.parse().unwrap();
    let (kcp_port_owner, tx, mut rx) = match handle_kcp_connection(socket_addr).await {
        Ok((kcp_port_owner, tx, rx)) => (kcp_port_owner, tx, rx),
        Err(e) => {
            error!("Error during websocket handshake: {}", e);
            return;
        }
    };

    // Spawn KCP data incoming handle task
    let (pub_tx, pub_rx) = mpsc::channel::<Vec<u8>>(100);
    tokio::spawn(async move {
        let mut parser = HexSocketParser::new();
        loop {
            let bytes = match rx.recv().await {
                Some(bytes) => bytes,
                None => {
                    println!("KCP connection lost");
                    break;
                }
            };
            if let Some(messages) = parser.parse(&bytes).unwrap() {
                for (optcode, bytes) in messages {
                    if optcode == HexSocketOpcode::Binary {
                        let _ = decode_message(&bytes, true).unwrap();
                        let _ = pub_tx.send(bytes).await;
                    }
                }
            }
        }
    });
    tokio::spawn(ros_handler::ws_publisher(pub_rx));

    if !read_only {
        let (sub_tx, mut sub_rx) = mpsc::channel::<Vec<u8>>(100);
        tokio::spawn(ros_handler::ws_subscriber(sub_tx));
        tokio::spawn(async move {
            let tx = tx;
            while let Some(msg) = sub_rx.recv().await {
                KcpPortOwner::send_binary(&tx, msg).await.unwrap();
            }
        });

        std::future::pending::<()>().await;
    }
}

pub(crate) async fn handle_kcp_connection(
    url: SocketAddrV4,
) -> Result<(KcpPortOwner, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>), anyhow::Error> {
    let urlstr = format!("ws://{}", url);
    let res = tokio_tungstenite::connect_async(&urlstr).await;
    let ws_stream = match res {
        Ok((ws, _)) => ws,
        Err(e) => {
            return Err(e.into());
        }
    };
    info!("Connected to: {}", url);
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    let (session_id, mut ws_stream) = loop {
        let msg = decode_websocket_message(ws_stream.next().await.unwrap().unwrap()).unwrap();
        break (msg.session_id, ws_stream);
    };

    let kcp_socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let local_port = kcp_socket.local_addr().unwrap().port();

    // Enable KCP
    ws_sink
        .send(tungstenite::Message::Binary(
            base_backend::ApiDown {
                down: Some(base_backend::api_down::Down::EnableKcp(
                    base_backend::EnableKcp {
                        client_peer_port: local_port as u32,
                        kcp_config: Some(base_backend::KcpConfig {
                            window_size_snd_wnd: 64,
                            window_size_rcv_wnd: 64,
                            interval_ms: 10,
                            no_delay: true,
                            nc: true,
                            resend: 2,
                        }),
                    },
                )),
            }
            .encode_to_vec()
            .into(),
        ))
        .await
        .expect("Failed to send enable KCP message");

    let kcp_server_status = loop {
        let msg = decode_websocket_message(ws_stream.next().await.unwrap().unwrap()).unwrap();
        if msg.kcp_server_status.is_some() {
            info!("KCP Enabled");
            break msg.kcp_server_status.unwrap();
        }
    };

    // KCP port is in kcp_config
    let kcp_server_addr = SocketAddr::V4(SocketAddrV4::new(
        url.ip().clone(),
        kcp_server_status.server_port as u16,
    ));

    // Please makesure kcp_port_owner lives long enough.
    // You can consider moving it to Arc
    let (kcp_port_owner, tx, mut rx) =
        kcp_bindings::KcpPortOwner::new_costom_socket(kcp_socket, session_id, kcp_server_addr)
            .await
            .unwrap();

    // Send any message to activate KCP connection.
    // Here we just send a placeholder message.
    KcpPortOwner::send_binary(
        &tx,
        base_backend::ApiDown {
            down: Some(base_backend::api_down::Down::PlaceholderMessage(true)),
        }
        .encode_to_vec(),
    )
    .await
    .expect("Failed to send placeholder message");

    // Set websocket report frequency to 1Hz.
    // Because we will be decoding KCP messages from now on.
    ws_sink
        .send(tungstenite::Message::Binary(
            base_backend::ApiDown {
                down: Some(base_backend::api_down::Down::SetReportFrequency(
                    base_backend::ReportFrequency::Rf1Hz as i32,
                )),
            }
            .encode_to_vec()
            .into(),
        ))
        .await
        .expect("Failed to send set report frequency message");

    // Spawn the websocket handle task
    // Just ignore all messages from websocket.
    // You can ofc still decode message if want to.
    tokio::spawn(async move {
        let ws_sink = ws_sink;
        loop {
            let _ = ws_stream.next().await;
        }
    });

    return Ok((kcp_port_owner, tx, rx));
}

fn decode_message(bytes: &[u8], log: bool) -> Result<base_backend::ApiUp, anyhow::Error> {
    let msg = base_backend::ApiUp::decode(bytes).unwrap();
    let ret = msg.clone();
    if log {
        if let Some(log) = msg.log {
            warn!("Log from base: {:?}", log); // Having a log usually means something went boom, so lets print it.
        }
    }
    if msg.protocol_major_version != ACCEPTABLE_PROTOCOL_MAJOR_VERSION {
        let w = format!(
            "Protocol major version is not {}, current version: {}. This might cause compatibility issues. Consider upgrading the base firmware.",
            ACCEPTABLE_PROTOCOL_MAJOR_VERSION, msg.protocol_major_version
        );
        warn!("{}", w);
        // If protocol major version does not match, lets just stop printing odometry.
        return Err(anyhow::anyhow!(w));
    }
    return Ok(ret);
}

fn decode_websocket_message(
    msg: tungstenite::Message,
) -> Result<base_backend::ApiUp, anyhow::Error> {
    match msg {
        tungstenite::Message::Binary(bytes) => return decode_message(&bytes, false),
        _ => {
            return Err(anyhow::anyhow!("Unexpected message type"));
        }
    };
}
