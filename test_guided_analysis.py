#!/usr/bin/env python3
"""Test script for guided analysis MCP features"""

import json
import sys

# Test the system_health tool request
system_health_request = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "name": "system_health",
        "arguments": {
            "parquet_file": "kdfw.parquet"
        }
    }
}

print(json.dumps(system_health_request))
print()  # MCP requires newline after each message

# Test the drill_down tool request
drill_down_request = {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
        "name": "drill_down",
        "arguments": {
            "parquet_file": "kdfw.parquet",
            "subsystem": "cpu",
            "start_time": 1754359535.0,
            "end_time": 1754359836.0,
            "detailed": True
        }
    }
}

print(json.dumps(drill_down_request))
print()