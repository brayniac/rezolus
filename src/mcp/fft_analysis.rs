use crate::viewer::promql::{QueryEngine, QueryResult};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FFTResult {
    pub metric_name: String,
    pub metric_query: String,
    pub sample_rate: f64, // samples per second
    pub total_samples: usize,
    pub dominant_frequencies: Vec<FrequencyPeak>,
    pub periodogram: Vec<(f64, f64)>, // (frequency_hz, power)
}

#[derive(Debug, Clone)]
pub struct FrequencyPeak {
    pub frequency_hz: f64,
    pub period_seconds: f64,
    pub power: f64,
    pub relative_power: f64, // as percentage of total power
    pub confidence: f64, // how prominent this peak is
}

/// Detect periodic patterns in metrics using FFT analysis
/// 
/// Supports label filtering in queries like:
/// - `cpu_usage{cpu="0"}` - Analyze CPU 0 only
/// - `cgroup_cpu_usage{name="web"}` - Analyze specific cgroup
/// - `network_bytes{direction="transmit"}` - Analyze transmit only
pub fn analyze_fft_patterns(
    engine: &Arc<QueryEngine>,
    metric_query: &str,
    metric_name: Option<&str>,
    start: f64,
    end: f64,
    step: f64,
) -> Result<FFTResult, Box<dyn std::error::Error>> {
    // Query the metric
    let result = engine.query_range(metric_query, start, end, step)?;
    
    // Extract time series data (use first series if multiple)
    let values = extract_time_series(&result)?;
    
    if values.len() < 8 {
        return Err("Need at least 8 samples for FFT analysis".into());
    }
    
    // Calculate recording constraints
    let sample_rate = 1.0 / step; // samples per second
    let recording_duration = end - start; // seconds
    let n_samples = values.len();
    
    // Validate recording is sufficient for meaningful FFT
    if recording_duration < 4.0 * step {
        return Err(format!(
            "Recording too short for FFT analysis. Need at least 4 samples, got {} samples over {:.1}s",
            n_samples, recording_duration
        ).into());
    }
    
    let signal: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
    
    // Remove DC component (mean) to focus on oscillations
    let mean = signal.iter().sum::<f64>() / signal.len() as f64;
    let signal: Vec<f64> = signal.iter().map(|v| v - mean).collect();
    
    // Apply window function to reduce spectral leakage
    let windowed_signal = apply_hann_window(&signal);
    
    // Perform FFT
    let frequencies = compute_fft(&windowed_signal);
    
    // Convert to power spectral density with constraints
    let periodogram = compute_periodogram(&frequencies, sample_rate, recording_duration);
    
    // Find dominant frequencies within valid range
    let dominant_frequencies = find_dominant_frequencies(&periodogram, 5, sample_rate, recording_duration);
    
    Ok(FFTResult {
        metric_name: metric_name.unwrap_or(metric_query).to_string(),
        metric_query: metric_query.to_string(),
        sample_rate,
        total_samples: signal.len(),
        dominant_frequencies,
        periodogram,
    })
}

/// Extract time series from query result
/// If multiple series are returned (e.g., without label filter), uses the first one
/// and includes series info in the error message if there are multiple
fn extract_time_series(result: &QueryResult) -> Result<Vec<(f64, f64)>, Box<dyn std::error::Error>> {
    match result {
        QueryResult::Matrix { result } => {
            if result.is_empty() {
                return Err("No data in result".into());
            }
            if result.len() > 1 {
                // Multiple series found - inform user about label filtering
                let mut series_info = String::from("Multiple series found. Consider using label filters:\n");
                for (i, sample) in result.iter().take(5).enumerate() {
                    let labels: Vec<String> = sample.metric.iter()
                        .filter(|(k, _)| *k != "__name__")
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect();
                    if !labels.is_empty() {
                        series_info.push_str(&format!("  Series {}: {{{}}}\n", i + 1, labels.join(", ")));
                    }
                }
                if result.len() > 5 {
                    series_info.push_str(&format!("  ... and {} more series\n", result.len() - 5));
                }
                series_info.push_str("\nUsing first series for analysis.");
                eprintln!("{}", series_info);
            }
            Ok(result[0].values.clone())
        }
        QueryResult::Vector { result } => {
            if result.is_empty() {
                return Err("No data in result".into());
            }
            Ok(vec![result[0].value])
        }
        QueryResult::Scalar { result } => Ok(vec![*result]),
    }
}

/// Apply Hann window to reduce spectral leakage
fn apply_hann_window(signal: &[f64]) -> Vec<f64> {
    let n = signal.len();
    signal
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let window = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos();
            x * window
        })
        .collect()
}

/// Compute FFT using simple DFT (for basic implementation)
/// For production, you'd want to use a proper FFT library like rustfft
fn compute_fft(signal: &[f64]) -> Vec<(f64, f64)> {
    let n = signal.len();
    let mut result = Vec::with_capacity(n / 2);
    
    // Compute only positive frequencies (first half of spectrum)
    for k in 0..n/2 {
        let mut real = 0.0;
        let mut imag = 0.0;
        
        for i in 0..n {
            let angle = -2.0 * std::f64::consts::PI * (k * i) as f64 / n as f64;
            real += signal[i] * angle.cos();
            imag += signal[i] * angle.sin();
        }
        
        result.push((real, imag));
    }
    
    result
}

/// Convert FFT result to power spectral density
fn compute_periodogram(frequencies: &[(f64, f64)], sample_rate: f64, recording_duration: f64) -> Vec<(f64, f64)> {
    let n = frequencies.len();
    let nyquist_freq = sample_rate / 2.0;
    let min_detectable_freq = 1.0 / (recording_duration / 2.0); // Can't detect periods longer than half the recording
    
    frequencies
        .iter()
        .enumerate()
        .map(|(k, (real, imag))| {
            let freq_hz = k as f64 * sample_rate / (2.0 * n as f64);
            let power = (real * real + imag * imag) / n as f64;
            (freq_hz, power)
        })
        .filter(|(freq, _)| {
            // Only include frequencies within detectable range
            *freq >= min_detectable_freq && *freq <= nyquist_freq
        })
        .collect()
}

/// Find the most significant frequency peaks
fn find_dominant_frequencies(periodogram: &[(f64, f64)], max_peaks: usize, _sample_rate: f64, _recording_duration: f64) -> Vec<FrequencyPeak> {
    if periodogram.len() < 3 {
        return Vec::new();
    }
    
    // Calculate total power for relative measurements
    let total_power: f64 = periodogram.iter().map(|(_, p)| p).sum();
    
    // Find local maxima
    let mut peaks = Vec::new();
    
    for i in 1..periodogram.len()-1 {
        let (freq, power) = periodogram[i];
        let prev_power = periodogram[i-1].1;
        let next_power = periodogram[i+1].1;
        
        // Skip DC component (frequency = 0)
        if freq == 0.0 {
            continue;
        }
        
        // Check if this is a local maximum
        if power > prev_power && power > next_power && power > total_power * 0.01 {
            let relative_power = 100.0 * power / total_power;
            
            // Calculate confidence based on how much higher than neighbors
            let confidence = if prev_power > 0.0 && next_power > 0.0 {
                (power / (prev_power + next_power) * 0.5).min(10.0)
            } else {
                1.0
            };
            
            peaks.push(FrequencyPeak {
                frequency_hz: freq,
                period_seconds: if freq > 0.0 { 1.0 / freq } else { f64::INFINITY },
                power,
                relative_power,
                confidence,
            });
        }
    }
    
    // Sort by power and take top peaks
    peaks.sort_by(|a, b| b.power.partial_cmp(&a.power).unwrap_or(std::cmp::Ordering::Equal));
    peaks.truncate(max_peaks);
    
    peaks
}

/// Format FFT analysis result for display
pub fn format_fft_result(result: &FFTResult) -> String {
    let mut output = String::new();
    
    output.push_str(&format!(
        "FFT Pattern Analysis\n\
         ===================\n\
         Metric: {}\n",
        result.metric_name
    ));
    
    if result.metric_name != result.metric_query {
        output.push_str(&format!("Query: {}\n", result.metric_query));
    }
    
    let recording_duration = result.total_samples as f64 / result.sample_rate;
    let nyquist_freq = result.sample_rate / 2.0;
    let min_detectable_freq = 1.0 / (recording_duration / 2.0);
    let max_detectable_period = 1.0 / min_detectable_freq;
    let min_detectable_period = 1.0 / nyquist_freq;
    
    output.push_str(&format!(
        "\nAnalysis Parameters:\n\
         Sample rate: {:.3} Hz ({:.1}s intervals)\n\
         Recording duration: {:.1} seconds\n\
         Total samples: {}\n\
         Frequency resolution: {:.6} Hz\n\
         \n\
         Detectable Range (Nyquist + Recording Limits):\n\
         Min detectable period: {:.3}s (Nyquist limit: < {:.1} Hz)\n\
         Max detectable period: {:.1}s (Recording limit: > {:.6} Hz)\n",
        result.sample_rate,
        1.0 / result.sample_rate,
        recording_duration,
        result.total_samples,
        result.sample_rate / (2.0 * result.total_samples as f64),
        min_detectable_period,
        nyquist_freq,
        max_detectable_period,
        min_detectable_freq
    ));
    
    // Add warnings if constraints are very restrictive
    if max_detectable_period < 60.0 {
        output.push_str(&format!(
            "\n⚠️  WARNING: Short recording duration ({:.1}s) limits detection to very short periods (< {:.1}s).\n\
             Consider using a longer time window for meaningful pattern analysis.\n",
            recording_duration, max_detectable_period
        ));
    }
    
    if min_detectable_period > 10.0 {
        output.push_str(&format!(
            "\n⚠️  WARNING: Low sample rate ({:.3} Hz) limits detection to slow patterns (> {:.1}s periods).\n\
             Consider using a smaller step size for higher frequency pattern detection.\n",
            result.sample_rate, min_detectable_period
        ));
    }
    
    if result.dominant_frequencies.is_empty() {
        output.push_str("\nNo significant periodic patterns detected.\n");
    } else {
        output.push_str(&format!("\nDominant Frequencies ({} found):\n", result.dominant_frequencies.len()));
        output.push_str("---------------------------------------\n");
        
        for (i, peak) in result.dominant_frequencies.iter().enumerate() {
            output.push_str(&format!(
                "{}. Frequency: {:.6} Hz\n\
                 \x20\x20\x20Period: {}\n\
                 \x20\x20\x20Power: {:.2e} ({:.2}% of total)\n\
                 \x20\x20\x20Confidence: {:.2}\n\n",
                i + 1,
                peak.frequency_hz,
                format_period(peak.period_seconds),
                peak.power,
                peak.relative_power,
                peak.confidence
            ));
        }
        
        // Provide interpretation
        output.push_str("Pattern Interpretation:\n");
        output.push_str("-----------------------\n");
        
        for peak in &result.dominant_frequencies {
            if peak.period_seconds < 60.0 {
                output.push_str(&format!(
                    "• {:.1}s period: High-frequency oscillation (sub-minute pattern)\n",
                    peak.period_seconds
                ));
            } else if peak.period_seconds < 3600.0 {
                output.push_str(&format!(
                    "• {:.1}min period: Medium-term cycle (scheduling/batching pattern)\n",
                    peak.period_seconds / 60.0
                ));
            } else if peak.period_seconds < 86400.0 {
                output.push_str(&format!(
                    "• {:.1}hr period: Long-term cycle (hourly patterns)\n",
                    peak.period_seconds / 3600.0
                ));
            } else {
                output.push_str(&format!(
                    "• {:.1}day period: Daily cycle (diurnal patterns)\n",
                    peak.period_seconds / 86400.0
                ));
            }
        }
    }
    
    output
}

/// Format period in human-readable form
fn format_period(seconds: f64) -> String {
    if seconds.is_infinite() {
        "∞".to_string()
    } else if seconds < 60.0 {
        format!("{:.2}s", seconds)
    } else if seconds < 3600.0 {
        format!("{:.1}min", seconds / 60.0)
    } else if seconds < 86400.0 {
        format!("{:.1}hr", seconds / 3600.0)
    } else {
        format!("{:.1}day", seconds / 86400.0)
    }
}