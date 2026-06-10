// nq_prompt.js — prompt construction and output validation for NL → PromQL.
//
// The model is small (0.5B), so the system prompt is explicit and grounded in
// worked examples. Node scoping is NOT requested here — the caller injects the
// {node="…"} label deterministically after generation.

const SYSTEM_PROMPT = `You translate a user's request into a single PromQL query over a metrics database.

Output rules:
- Output ONLY the PromQL query. No explanation, no markdown, no code fences.
- The query MUST begin with a metric name or a function call — it must NEVER begin with "{".
- Pick the single most relevant metric from the provided list and use its EXACT name.
- counter  → wrap in a rate over a window:      rate(metric_name[5m])
- gauge    → use the metric name directly:      metric_name
- histogram→ take a quantile:                   histogram_quantile(0.99, metric_name)
- To combine across series, wrap with sum(...) or avg(...).

Examples:
Metrics: cpu_usage (counter), cpu_frequency (gauge)
Request: show cpu usage over time
Query: rate(cpu_usage[5m])

Metrics: memory_used (gauge), memory_free (gauge)
Request: how much memory is in use
Query: memory_used

Metrics: scheduler_runqueue_latency (histogram), cpu_usage (counter)
Request: p99 run queue latency
Query: histogram_quantile(0.99, scheduler_runqueue_latency)

Metrics: network_bytes (counter), network_packets (counter)
Request: total network throughput
Query: sum(rate(network_bytes[5m]))`;

/**
 * Build the chat messages for LLM generation.
 * Returns an array of { role, content } turns; the text-generation pipeline
 * applies the model's chat template automatically.
 */
export function buildPrompt(topKMetrics, userQuery) {
    const metricList = topKMetrics
        .map(m => `${m.name} (${m.type})`)
        .join(', ');

    // Mirror the example format so the model continues it naturally.
    const user = `Metrics: ${metricList}\nRequest: ${userQuery}\nQuery:`;

    return [
        { role: 'system', content: SYSTEM_PROMPT },
        { role: 'user', content: user },
    ];
}

/**
 * Clean the raw LLM output to extract just the PromQL query.
 */
export function cleanOutput(raw) {
    let result = raw.trim();

    // Strip markdown code fences if present.
    result = result.replace(/^```(?:promql|text)?\s*/i, '').replace(/\s*```$/i, '');

    // Strip any leading "PromQL:" / "Query:" prefix the model might echo.
    result = result.replace(/^(?:promql|query):\s*/i, '');

    // Keep only the first line — the model occasionally adds commentary after.
    result = result.split('\n')[0];

    return result.trim();
}

/**
 * Validate that the output looks like a usable PromQL query.
 */
export function looksLikePromQL(query) {
    const q = (query || '').trim();
    if (q.length < 2) return false;
    // A bare label selector ({...}) is the model's most common failure mode and
    // almost never what the user wants — require a metric name or function first.
    if (/^\{/.test(q)) return false;
    // Must start with a metric/function identifier.
    if (!/^[a-z_]/i.test(q)) return false;
    // Accept either a structured query (function call / range / selector) or a
    // bare metric name (a valid gauge query, e.g. "memory_used").
    const hasStructure = /[[(]/.test(q) || /\{[^}]*\}/.test(q);
    const isBareMetric = /^[a-z_]\w*(\s*\{[^}]*\})?$/i.test(q);
    return hasStructure || isBareMetric;
}
