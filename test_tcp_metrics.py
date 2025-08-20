#!/usr/bin/env python3
"""Test TCP metrics with labels"""

import json
import subprocess

def send_request(proc, request):
    """Send a request and get response"""
    request_str = json.dumps(request) + "\n"
    proc.stdin.write(request_str.encode())
    proc.stdin.flush()
    
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
    
    # Test various TCP metrics
    queries = [
        'tcp_bytes',
        'tcp_bytes{direction="receive"}',
        'rate(tcp_bytes{direction="receive"}[1m])',
        'tcp_packets',
        'tcp_packets{direction="receive"}',
        'network_bytes{direction="receive"}',
    ]
    
    for query in queries:
        print(f"\nTesting: {query}")
        print("-" * 40)
        
        query_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "query_metrics",
                "arguments": {
                    "parquet_file": "kdfw.parquet",
                    "query": query,
                    "end_time": 1754359540.0,
                    "start_time": 1754359540.0
                }
            }
        }
        response = send_request(proc, query_request)
        
        if response:
            if "error" in response:
                error_msg = response["error"]["message"]
                # Just show first line of error
                print(f"Error: {error_msg.split(chr(10))[0][:100]}")
            elif "result" in response:
                content = response["result"]["content"][0]["text"]
                # Parse out key info
                lines = content.split('\n')
                for line in lines[:5]:
                    if line.strip():
                        print(line)
    
    # Clean up
    proc.terminate()
    proc.wait()

if __name__ == "__main__":
    main()