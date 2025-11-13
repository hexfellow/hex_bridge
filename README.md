# hex_bridge

## Overview

This is a ROS2 package that provides a WebSocket-to-ROS bridge for communicating with hex devices. It forwards WebSocket messages from hex devices to a ROS topic (`/ws_up`) and can send messages from a ROS topic (`/ws_down`) back to hex devices via WebSocket.

## Prerequisites

1. **Rust Installation**

   Install Rust using rustup:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

   Verify installation:
   ```bash
   rustc --version
   cargo --version
   ```

2. Navigate to your ROS2 workspace:
   ```bash
   cd /path/to/your/ros2_workspace
   ```

3. Build the package using colcon:
   ```bash
   colcon build
   ```

4. Source the workspace:
   ```bash
   source install/setup.bash
   ```

## Usage

Launch the node with parameters:
```bash
ros2 launch hex_bridge hex_bridge.launch.py url:="{YOUR_HEX_IP}:8439" read_only:=false is_kcp:=true
```

## Publish

| Topic    | Msg Type                   | Description     |
| -------- | -------------------------- | --------------- |
| `/ws_up` | `std_msgs/UInt8MultiArray` | Message from ws |


## Subscribe

| Topic      | Msg Type                   | Description           |
| ---------- | -------------------------- | --------------------- |
| `/ws_down` | `std_msgs/UInt8MultiArray` | Message to send to ws |

## Parameter

| Name        | Data Type | Required | Default Value | Description                                      | Note |
| ----------- | --------- | -------- | ------------- | ------------------------------------------------ | ---- |
| `read_only` | `bool`    | yes      | false         | Makes node only listen from ws. Ignoring ws_down |      |
| `url`       | `string`  | yes      | ""            | URL of the websocket server, begin with "ws://"  |      |
| `is_kcp`    | `bool`    | yes      | true          | Use KCP instead of WebSocket                     |      |