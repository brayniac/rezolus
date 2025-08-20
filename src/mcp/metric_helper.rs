use crate::viewer::tsdb::Tsdb;
use std::collections::HashMap;

/// Helper for metric validation and suggestions
pub struct MetricHelper;

impl MetricHelper {
    /// Extract metric name from a PromQL query
    pub fn extract_metric_names(query: &str) -> Vec<String> {
        let mut names = Vec::new();
        
        // Common PromQL functions that wrap metrics
        let functions = ["irate", "rate", "increase", "delta", "idelta", 
                        "sum", "avg", "min", "max", "count", "stddev",
                        "histogram_quantile", "sum_over_time", "avg_over_time",
                        "min_over_time", "max_over_time", "count_over_time"];
        
        // Find metrics within function calls
        for func in &functions {
            let pattern = format!("{}(", func);
            let mut search_pos = 0;
            while let Some(pos) = query[search_pos..].find(&pattern) {
                let actual_pos = search_pos + pos;
                let after_func = &query[actual_pos + pattern.len()..];
                if let Some(metric) = Self::extract_first_metric(after_func) {
                    // Don't add PromQL functions as metrics
                    if !functions.contains(&metric.as_str()) {
                        names.push(metric);
                    }
                }
                search_pos = actual_pos + pattern.len();
            }
        }
        
        // If no metrics found in functions, check for plain metrics
        if names.is_empty() {
            if let Some(metric) = Self::extract_first_metric(query) {
                // Don't add PromQL functions as metrics
                if !functions.contains(&metric.as_str()) {
                    names.push(metric);
                }
            }
        }
        
        // Deduplicate
        names.sort();
        names.dedup();
        
        names
    }
    
    fn extract_first_metric(text: &str) -> Option<String> {
        // Extract metric name, handling labels and time ranges
        let mut chars = text.chars().peekable();
        let mut metric = String::new();
        let mut in_label = false;
        let mut started = false;
        
        while let Some(ch) = chars.next() {
            match ch {
                '{' | '[' | '(' | ' ' | ')' | ',' => {
                    if !metric.is_empty() && !in_label && started {
                        // Check if this looks like a metric name (starts with letter or underscore)
                        if metric.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') {
                            return Some(metric);
                        }
                    }
                    if ch == '{' {
                        in_label = true;
                    }
                    if ch == '(' || ch == ' ' || ch == ',' {
                        metric.clear();
                        started = false;
                    }
                }
                '}' => {
                    in_label = false;
                }
                c if !in_label && (c.is_alphabetic() || c == '_') => {
                    metric.push(c);
                    started = true;
                }
                c if !in_label && c.is_numeric() && started => {
                    metric.push(c);
                }
                '.' if !in_label && !started => {
                    // Skip numbers like 0.99
                    while chars.peek().map_or(false, |c| c.is_numeric()) {
                        chars.next();
                    }
                }
                _ => {
                    if started && !metric.is_empty() {
                        // Check if this looks like a metric name
                        if metric.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') {
                            return Some(metric);
                        }
                    }
                    metric.clear();
                    started = false;
                }
            }
        }
        
        if !metric.is_empty() && started {
            // Check if this looks like a metric name
            if metric.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') {
                Some(metric)
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Check if metrics exist in the TSDB
    pub fn validate_metrics(
        tsdb: &Tsdb,
        metric_names: &[String],
    ) -> HashMap<String, bool> {
        let mut results = HashMap::new();
        
        let all_metrics = Self::get_all_metric_names(tsdb);
        
        for name in metric_names {
            results.insert(name.clone(), all_metrics.contains(&name.as_str()));
        }
        
        results
    }
    
    /// Get all metric names from TSDB
    fn get_all_metric_names(tsdb: &Tsdb) -> Vec<&str> {
        let mut names = Vec::new();
        
        for name in tsdb.counter_names() {
            names.push(name);
        }
        for name in tsdb.gauge_names() {
            names.push(name);
        }
        for name in tsdb.histogram_names() {
            names.push(name);
        }
        
        names
    }
    
    /// Find similar metrics (for suggestions)
    pub fn find_similar_metrics(
        tsdb: &Tsdb,
        target: &str,
        max_suggestions: usize,
    ) -> Vec<(String, f32)> {
        let all_metrics = Self::get_all_metric_names(tsdb);
        let target_lower = target.to_lowercase();
        
        let mut scores: Vec<(String, f32)> = all_metrics
            .iter()
            .map(|metric| {
                let metric_lower = metric.to_lowercase();
                let score = Self::calculate_similarity(&target_lower, &metric_lower);
                (metric.to_string(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();
        
        // Sort by score (highest first)
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        scores.truncate(max_suggestions);
        scores
    }
    
    /// Calculate similarity score between two strings
    fn calculate_similarity(s1: &str, s2: &str) -> f32 {
        // Multiple scoring strategies
        let mut score = 0.0;
        
        // 1. Exact match
        if s1 == s2 {
            return 1.0;
        }
        
        // 2. Prefix match
        if s2.starts_with(s1) || s1.starts_with(s2) {
            score += 0.7;
        }
        
        // 3. Contains match
        if s2.contains(s1) || s1.contains(s2) {
            score += 0.5;
        }
        
        // 4. Token-based matching (split by _ and match tokens)
        let tokens1: Vec<&str> = s1.split('_').collect();
        let tokens2: Vec<&str> = s2.split('_').collect();
        
        let mut matched_tokens = 0;
        for token1 in &tokens1 {
            if tokens2.iter().any(|t| t.contains(token1) || token1.contains(t)) {
                matched_tokens += 1;
            }
        }
        
        if matched_tokens > 0 {
            let token_score = matched_tokens as f32 / tokens1.len().max(tokens2.len()) as f32;
            score = f32::max(score, token_score * 0.8);
        }
        
        // 5. Levenshtein distance (simplified)
        if score == 0.0 {
            let max_len = s1.len().max(s2.len()) as f32;
            let distance = Self::levenshtein_distance(s1, s2) as f32;
            if distance < max_len * 0.5 {
                score = (1.0 - distance / max_len) * 0.4;
            }
        }
        
        score
    }
    
    /// Simple Levenshtein distance calculation
    fn levenshtein_distance(s1: &str, s2: &str) -> usize {
        let len1 = s1.len();
        let len2 = s2.len();
        
        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }
        
        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
        
        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }
        
        for (i, c1) in s1.chars().enumerate() {
            for (j, c2) in s2.chars().enumerate() {
                let cost = if c1 == c2 { 0 } else { 1 };
                matrix[i + 1][j + 1] = *[
                    matrix[i][j + 1] + 1,     // deletion
                    matrix[i + 1][j] + 1,     // insertion
                    matrix[i][j] + cost,      // substitution
                ].iter().min().unwrap();
            }
        }
        
        matrix[len1][len2]
    }
    
    /// Generate helpful error message with suggestions
    pub fn generate_metric_error_message(
        tsdb: &Tsdb,
        missing_metrics: &[String],
    ) -> String {
        let mut message = String::new();
        
        message.push_str("The following metrics were not found:\n");
        
        for metric in missing_metrics {
            message.push_str(&format!("  - {}\n", metric));
            
            // Special case: common TCP metric confusions
            if metric.contains("tcp_receive") || metric.contains("tcp_transmit") {
                message.push_str("    Did you mean:\n");
                if metric.contains("receive") {
                    message.push_str("      • tcp_packets{direction=\"receive\"} - TCP packets received\n");
                    message.push_str("      • tcp_bytes{direction=\"receive\"} - TCP bytes received\n");
                    message.push_str("      • network_bytes{direction=\"receive\"} - Network bytes received\n");
                } else if metric.contains("transmit") || metric.contains("send") {
                    message.push_str("      • tcp_packets{direction=\"transmit\"} - TCP packets sent\n");
                    message.push_str("      • tcp_bytes{direction=\"transmit\"} - TCP bytes sent\n");
                    message.push_str("      • network_bytes{direction=\"transmit\"} - Network bytes sent\n");
                }
            } else {
                // Find suggestions
                let suggestions = Self::find_similar_metrics(tsdb, metric, 3);
                if !suggestions.is_empty() {
                    message.push_str("    Did you mean:\n");
                    for (suggestion, score) in suggestions {
                        if score > 0.3 {
                            message.push_str(&format!("      • {}\n", suggestion));
                        }
                    }
                }
            }
        }
        
        // Add common metric patterns as hints
        message.push_str("\nHint: Common metric patterns:\n");
        message.push_str("  • CPU: cpu_usage, cpu_cores, cpu_cycles, cpu_aperf, cpu_mperf\n");
        message.push_str("  • Memory: memory_used, memory_total, memory_free\n");
        message.push_str("  • Network bytes: network_bytes{direction=\"receive\"}, network_bytes{direction=\"transmit\"}\n");
        message.push_str("  • TCP metrics:\n");
        message.push_str("    - tcp_packets{direction=\"receive\"} or tcp_packets{direction=\"transmit\"}\n");
        message.push_str("    - tcp_bytes{direction=\"receive\"} or tcp_bytes{direction=\"transmit\"}\n");
        message.push_str("    - tcp_retransmit, tcp_connect_latency, tcp_packet_latency, tcp_srtt\n");
        message.push_str("  • Disk: block_io_requests, block_io_request_latency\n");
        message.push_str("\nUse 'list_metrics' tool to see all available metrics.\n");
        
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_metric_names() {
        let cases = vec![
            ("cpu_usage", vec!["cpu_usage"]),
            ("irate(cpu_usage[5m])", vec!["cpu_usage"]),
            ("sum(irate(network_transmit_bytes[5m]))", vec!["network_transmit_bytes"]),
            ("cpu_usage{cpu=\"0\"}", vec!["cpu_usage"]),
            ("histogram_quantile(0.99, tcp_latency[5m])", vec!["tcp_latency"]),
        ];
        
        for (query, expected) in cases {
            let result = MetricHelper::extract_metric_names(query);
            assert_eq!(result, expected, "Failed for query: {}", query);
        }
    }
    
    #[test]
    fn test_similarity() {
        assert_eq!(MetricHelper::calculate_similarity("tcp_receive", "tcp_receive"), 1.0);
        assert!(MetricHelper::calculate_similarity("tcp_receive", "tcp_receive_bytes") > 0.5);
        assert!(MetricHelper::calculate_similarity("tcp_recv", "tcp_receive") > 0.3);
        assert!(MetricHelper::calculate_similarity("network_rx", "network_receive_bytes") > 0.2);
    }
}