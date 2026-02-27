#!/usr/bin/env python3
# -*- coding:utf-8 -*-

from launch import LaunchDescription
from launch_ros.actions import Node
from launch.actions import DeclareLaunchArgument
from launch.substitutions import LaunchConfiguration


def generate_launch_description():
    # Declare the launch arguments
    url = DeclareLaunchArgument(
        "url",
        default_value="0.0.0.0:8439",
        description="server URL"
    )

    read_only = DeclareLaunchArgument(
        "read_only",
        default_value="false",
        description="Read-only mode (no publishing from ROS to WebSocket)",
    )

    is_kcp = DeclareLaunchArgument(
        "is_kcp",
        default_value="true",
        description="Is KCP mode"
    )

    # Define the node
    hex_bridge_node = Node(
        package="hex_bridge",
        executable="hex_bridge",
        name="hex_bridge",
        output="screen",
        emulate_tty=True,
        parameters=[{
            "url": LaunchConfiguration("url"),
            "read_only": LaunchConfiguration("read_only"),
            "is_kcp": LaunchConfiguration("is_kcp"),
        }],
        remappings=[
            # subscribe (ROS -> WebSocket)
            ("/ws_down", "/ws_down"),
            # publish (WebSocket -> ROS)
            ("/ws_up", "/ws_up"),
        ],
    )

    # Return the LaunchDescription
    return LaunchDescription([
        url,
        read_only,
        is_kcp,
        hex_bridge_node
    ])
