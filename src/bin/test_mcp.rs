use std::io::{Write, BufRead, BufReader};
use std::process::{Command, Stdio};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing MCP Server Tool Responses\n");
    println!("{}\n", "=".repeat(60));

    // Test files
    let test_files = vec!["metrics.parquet", "kiad.parquet"];
    
    for file in test_files {
        println!("Testing with: {}", file);
        println!("{}", "-".repeat(40));
        
        // Start MCP server
        let mut child = Command::new("./target/debug/rezolus")
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        
        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        
        // Initialize
        let init_req = json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "id": 0
        });
        
        writeln!(stdin, "{}", init_req)?;
        stdin.flush()?;
        
        // Read init response
        let mut line = String::new();
        reader.read_line(&mut line)?;
        
        // Test list_cgroups
        let list_req = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "list_cgroups",
                "arguments": {
                    "file_path": file
                }
            },
            "id": 1
        });
        
        writeln!(stdin, "{}", list_req)?;
        stdin.flush()?;
        
        // Read response with timeout
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(10);
        
        line.clear();
        
        // Try to read response
        loop {
            if start.elapsed() > timeout {
                println!("  ⏱️  Timeout after 10 seconds");
                break;
            }
            
            match reader.read_line(&mut line) {
                Ok(0) => {
                    println!("  ❌ Connection closed");
                    break;
                }
                Ok(_) => {
                    // Parse response
                    if let Ok(response) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(result) = response.get("result") {
                            if let Some(content) = result.get("content") {
                                if let Some(text) = content[0].get("text") {
                                    let text_str = text.as_str().unwrap_or("");
                                    let lines: Vec<&str> = text_str.lines().collect();
                                    
                                    // Show summary
                                    for line in &lines {
                                        if line.contains("Found") && line.contains("cgroups") {
                                            println!("  ✅ {}", line);
                                        }
                                        if line.contains("Top CPU Consumers") {
                                            println!("  📊 Found top consumers section");
                                            break;
                                        }
                                    }
                                    
                                    // Show top 3 CPU consumers if found
                                    let mut in_top = false;
                                    let mut count = 0;
                                    for line in &lines {
                                        if line.contains("Top CPU Consumers") {
                                            in_top = true;
                                        } else if in_top && line.contains("cores") {
                                            println!("     {}", line.trim());
                                            count += 1;
                                            if count >= 3 {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        } else if let Some(error) = response.get("error") {
                            println!("  ❌ Error: {:?}", error);
                        }
                    }
                    break;
                }
                Err(e) => {
                    println!("  ❌ Read error: {}", e);
                    break;
                }
            }
        }
        
        // Kill the process
        child.kill()?;
        println!();
    }
    
    Ok(())
}