#!/usr/bin/env python3
"""Test TCP suggestions"""

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
    
    print("Testing TCP metric suggestions...")
    print("="*60)
    
    # Test the exact query the LLM tried
    print("\nWhat LLM tried: rate(tcp_receive_segments[1m])")
    print("-" * 40)
    
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
    
    if response and "error" in response:
        error_msg = response["error"]["message"]
        # Print the suggestions part
        lines = error_msg.split('\n')
        printing = False
        for line in lines:
            if "tcp_receive_segments" in line:
                printing = True
            if printing:
                print(line)
                if "network_bytes{direction" in line:
                    break
    
    # Now test the correct query
    print("\n" + "="*60)
    print("\nCorrect query: rate(tcp_packets{direction=\"receive\"}[1m])")
    print("-" * 40)
    
    query_request = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "query_metrics",
            "arguments": {
                "parquet_file": "kdfw.parquet",
                "query": "rate(tcp_packets{direction=\"receive\"}[1m])",
                "start_time": 1754359540.0,
                "end_time": 1754359600.0,
                "step": 10.0
            }
        }
    }
    response = send_request(proc, query_request)
    
    if response:
        if "error" in response:
            print("Error:", response["error"]["message"][:100])
        elif "result" in response:
            print("✓ Query executed successfully!")
            content = response["result"]["content"][0]["text"]
            lines = content.split('\n')[:3]
            for line in lines:
                if line.strip():
                    print(line)
    
    # Clean up
    proc.terminate()
    proc.wait()

if __name__ == "__main__":
    main()