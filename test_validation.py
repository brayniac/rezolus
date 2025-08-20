#!/usr/bin/env python3
"""Test metric validation"""

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
    
    # Test 1: Query non-existent metric (what LLM tried)
    print("Test 1: Querying non-existent metric 'tcp_receive_segments'...")
    query_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "query_metrics",
            "arguments": {
                "parquet_file": "kdfw.parquet",
                "query": "rate(tcp_receive_segments[1m])"
            }
        }
    }
    response = send_request(proc, query_request)
    
    if response:
        if "error" in response:
            print("Got expected error:")
            print(response["error"]["message"])
        elif "result" in response:
            content = response["result"]["content"][0]["text"]
            print("Result:")
            print(content)
    
    print("\n" + "="*60 + "\n")
    
    # Test 2: Query a valid metric
    print("Test 2: Querying valid metric 'tcp_packet_latency'...")
    query_request = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "query_metrics",
            "arguments": {
                "parquet_file": "kdfw.parquet",
                "query": "histogram_quantile(0.99, tcp_packet_latency[5m])"
            }
        }
    }
    response = send_request(proc, query_request)
    
    if response:
        if "error" in response:
            print("Error:", response["error"]["message"])
        elif "result" in response:
            content = response["result"]["content"][0]["text"]
            print("Success! Got data for tcp_packet_latency")
            # Just show first few lines
            lines = content.split('\n')[:5]
            for line in lines:
                print(line)
    
    # Clean up
    proc.terminate()
    proc.wait()

if __name__ == "__main__":
    main()