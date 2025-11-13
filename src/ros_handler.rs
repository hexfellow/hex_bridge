/***************************************************************
 * Copyright 2025 Peng Shaohui. All rights reserved.
 * Author: Peng Shaohui 2375899670@qq.com
 * Date  : 2025-11-06
 ***************************************************************/

use futures_util::StreamExt;
use r2r::{Node, QosProfile};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, OnceCell};

static ROS_NODE: OnceCell<Arc<Mutex<Node>>> = OnceCell::const_new();

pub async fn init(node_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = r2r::Context::create()?;
    let node = r2r::Node::create(ctx, node_name, "")?;

    let node = Arc::new(Mutex::new(node));

    ROS_NODE
        .set(node.clone())
        .map_err(|_| "ROS node already initialized")?;

    tokio::spawn(async move {
        loop {
            {
                let mut node = node.lock().await;
                node.spin_once(std::time::Duration::from_millis(10));
            }
            tokio::task::yield_now().await;
        }
    });

    Ok(())
}

pub async fn ws_publisher(mut rx: mpsc::Receiver<Vec<u8>>) {
    let node = ROS_NODE.get().expect("Call init() before ws_publisher");

    let publisher = {
        let mut node = node.lock().await;
        node.create_publisher::<r2r::std_msgs::msg::UInt8MultiArray>(
            "/ws_up",
            QosProfile::default(),
        )
        .expect("Failed to create /ws_up publisher")
    };

    while let Some(data) = rx.recv().await {
        if !data.is_empty() {
            let msg = r2r::std_msgs::msg::UInt8MultiArray {
                layout: r2r::std_msgs::msg::MultiArrayLayout {
                    dim: vec![],
                    data_offset: 0,
                },
                data,
            };
            let _ = publisher.publish(&msg);
        }
    }
}

pub async fn ws_subscriber(tx: mpsc::Sender<Vec<u8>>) {
    let node = ROS_NODE.get().expect("Call init() before ws_subscriber");

    let mut subscription_stream = {
        let mut node = node.lock().await;
        node.subscribe::<r2r::std_msgs::msg::UInt8MultiArray>("/ws_down", QosProfile::default())
            .expect("Failed to create /ws_down subscription")
    };

    while let Some(msg) = subscription_stream.next().await {
        let _ = tx.send(msg.data).await;
    }
}

pub async fn get_parameter(name: &str, default_value: r2r::ParameterValue) -> r2r::ParameterValue {
    let node = ROS_NODE
        .get()
        .expect("Call init() before get_or_declare_parameter");
    let node = node.lock().await;

    let params = node.params.lock().unwrap();
    params
        .get(name)
        .map(|p| p.value.clone())
        .unwrap_or(default_value)
}
