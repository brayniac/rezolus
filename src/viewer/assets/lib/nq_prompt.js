// nq_prompt.js — prompt construction and output handling for NL → PromQL.
//
// MUST stay byte-for-byte in sync with the fine-tune's training/runtime format,
// documented in tools/nl-query-finetune/PROMPT_FORMAT.md (the SYSTEM string and
// the metric-card format are the single source of truth). If these drift from the
// model (brayniac/promql-0.5b-onnx), accuracy collapses silently.
//
// Node scoping is NOT requested here — the caller injects {node="…"} after
// generation. Retrieval supplies the metric cards; for "A per B"/ratio requests
// the retriever must include BOTH metrics or even a perfect model can't answer.

const SYSTEM_PROMPT = `Convert the request into ONE PromQL query using ONLY the listed metrics.
counter -> irate(x[1s]); gauge -> x; histogram -> histogram_quantile(q, x); aggregate with sum()/avg(); filter with {label="value"}; ratio "A per B" -> sum(A_expr)/sum(B_expr).
If no listed metric answers the request, output exactly: NO_METRIC. Output only PromQL, nothing else.`;

const NO_METRIC = 'NO_METRIC';

/**
 * Format one metric card: `  name (type; labels: a,b) — description`.
 * Omits `; labels: …` when there are none and ` — description` when absent,
 * exactly as datagen/generate.py:format_card does.
 */
function formatCard(m) {
    const labelKeys = Array.isArray(m.labels)
        ? m.labels
        : (m.labels && typeof m.labels === 'object' ? Object.keys(m.labels) : []);
    let head = `${m.name} (${m.type}`;
    head += labelKeys.length ? `; labels: ${labelKeys.join(',')})` : ')';
    const desc = (m.description || m.help || '').trim();
    return desc ? `  ${head} — ${desc}` : `  ${head}`;
}

/**
 * Build the chat messages for LLM generation. Returns [{role, content}]; the
 * text-generation pipeline applies the model's chat template automatically.
 */
export function buildPrompt(topKMetrics, userQuery) {
    const lines = ['Metrics:', ...topKMetrics.map(formatCard), `Request: ${userQuery}`];
    return [
        { role: 'system', content: SYSTEM_PROMPT },
        { role: 'user', content: lines.join('\n') },
    ];
}

/** True when the model refused (no listed metric fits the request). */
export function isNoMetric(query) {
    return (query || '').trim() === NO_METRIC;
}

/**
 * Clean the raw LLM output to the single PromQL line (or NO_METRIC). The model is
 * trained to emit one line, but we strip stray fences and take the first line.
 */
export function cleanOutput(raw) {
    let r = String(raw || '').trim();
    r = r.replace(/```[a-z]*\n?/gi, '');                 // drop code fences
    for (const line of r.split('\n')) {
        const s = line.trim().replace(/^(?:promql|query):\s*/i, '').trim();
        if (s) return s;
    }
    return r.trim();
}

/**
 * Validate that the output looks like a usable PromQL query. NO_METRIC is a valid
 * model output but not an executable query, so it returns false here (the caller
 * checks isNoMetric separately to message the user).
 */
export function looksLikePromQL(query) {
    const q = (query || '').trim();
    if (q.length < 2 || isNoMetric(q)) return false;
    if (/^\{/.test(q)) return false;            // bare label selector — never wanted
    if (!/^[a-z_]/i.test(q)) return false;       // must start with metric/function ident
    const hasStructure = /[[(]/.test(q) || /\{[^}]*\}/.test(q);
    const isBareMetric = /^[a-z_]\w*(\s*\{[^}]*\})?$/i.test(q);
    return hasStructure || isBareMetric;
}
