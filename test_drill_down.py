#!/usr/bin/env python3
"""Test drill_down functionality"""

import json
import subprocess

def send_request(proc, request):
    """Send a request and get response"""
    request_str = json.dumps(request) + "\n"
    proc.stdin.write(request_str.encode())
    proc.stdin.flush()
    
    # Read response line
    response_line = proc.stdout.readline().decode()
    if response_line:
        try:
            return json.loads(response_line)
        except:
            return {"raw": response_line}
    return None

def main():
    # Start MCP server
    proc = subprocess.Popen(
        ["./target/debug/rezolus", "mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    
    # Initialize
    init_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "test_client",
                "version": "1.0"
            }
        }
    }
    send_request(proc, init_request)
    
    # Test drill_down on network
    print("Testing drill_down on network subsystem...")
    drill_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "drill_down",
            "arguments": {
                "parquet_file": "kdfw.parquet",
                "subsystem": "network",
                "start_time": 1754359535.0,
                "end_time": 1754359836.0,
                "detailed": True
            }
        }
    }
    response = send_request(proc, drill_request)
    
    if response and "result" in response:
        content = response["result"]["content"][0]["text"]
        print(content)
    else:
        print(f"Error: {response}")
    
    # Clean up
    proc.terminate()
    proc.wait()

if __name__ == "__main__":
    main()