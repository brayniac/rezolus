#!/usr/bin/env python3
"""Test metric suggestions"""

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
    
    # Test different wrong metrics to see suggestions
    test_cases = [
        ("tcp_receive_segments", "LLM tried to use this"),
        ("network_rx_bytes", "Common abbreviation"),
        ("cpu_freq", "Shortened name"),
        ("memory_usage", "Close but not exact"),
    ]
    
    for wrong_metric, description in test_cases:
        print(f"\nTest: {description}")
        print(f"Querying: {wrong_metric}")
        print("-" * 40)
        
        query_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "query_metrics",
                "arguments": {
                    "parquet_file": "kdfw.parquet",
                    "query": f"irate({wrong_metric}[5m])"
                }
            }
        }
        response = send_request(proc, query_request)
        
        if response and "error" in response:
            # Extract just the suggestions part
            error_msg = response["error"]["message"]
            lines = error_msg.split('\n')
            for i, line in enumerate(lines):
                if "not found:" in line or "Did you mean:" in line:
                    # Print the metric and suggestions
                    for j in range(i, min(i+5, len(lines))):
                        if lines[j].strip():
                            print(lines[j])
                    break
    
    # Now test with a valid metric
    print("\n" + "="*60)
    print("\nTest: Valid metric")
    print("Querying: cpu_usage")
    print("-" * 40)
    
    query_request = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "query_metrics",
            "arguments": {
                "parquet_file": "kdfw.parquet",
                "query": "avg(irate(cpu_usage[5m]))"
            }
        }
    }
    response = send_request(proc, query_request)
    
    if response:
        if "error" in response:
            print("Error:", response["error"]["message"][:100])
        elif "result" in response:
            content = response["result"]["content"][0]["text"]
            lines = content.split('\n')[:3]
            print("Success! Query executed.")
            for line in lines:
                if line.strip():
                    print(line)
    
    # Clean up
    proc.terminate()
    proc.wait()

if __name__ == "__main__":
    main()