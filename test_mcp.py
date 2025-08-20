#!/usr/bin/env python3
"""Interactive test for MCP server"""

import json
import subprocess
import sys

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
    
    print("Testing MCP server...")
    
    # 1. Initialize
    print("\n1. Sending initialize...")
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
    response = send_request(proc, init_request)
    print(f"Response: {json.dumps(response, indent=2) if response else 'None'}")
    
    # 2. List tools
    print("\n2. Listing tools...")
    list_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }
    response = send_request(proc, list_request)
    if response and "result" in response:
        tools = response["result"].get("tools", [])
        print(f"Found {len(tools)} tools:")
        for tool in tools:
            print(f"  - {tool['name']}: {tool['description']}")
    
    # 3. Test system_health
    print("\n3. Testing system_health tool...")
    health_request = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "system_health",
            "arguments": {
                "parquet_file": "kdfw.parquet"
            }
        }
    }
    response = send_request(proc, health_request)
    print(f"Response: {json.dumps(response, indent=2) if response else 'None'}")
    
    # Clean up
    proc.terminate()
    proc.wait()

if __name__ == "__main__":
    main()