// nq_pipeline.js — orchestrates embed → search → generate to turn a natural
// language question into a PromQL string. Executing the PromQL (deriving the
// time window, rendering a chart) is the caller's job, via the same
// executePromQLRangeQuery path the Query Explorer uses.

import { queryEmbed, buildIndex, reset as resetEngine } from './nq_engine.js';
import { search, keywordSearch } from './nq_search.js';
import { generate as llmGenerate, reset as resetLlm } from './nq_generate.js';
import { buildPrompt, cleanOutput, looksLikePromQL, isNoMetric } from './nq_prompt.js';
import { getMetricNames, getMetricTypes, getMetricLabels } from './data.js';

const MAX_RETRIES = 2;
const TOP_K = 8;

/**
 * Run the NL→PromQL pipeline.
 * @param {string} nlQuery - User's natural language question
 * @param {object} options
 * @param {number} [options.maxRetries] - Generation retries on invalid output
 * @param {function(string): void} [options.onStatus] - Progress callback
 * @returns {Promise<{ promql: string, raw: string }>}
 */
export async function runPipeline(nlQuery, options = {}) {
    const { maxRetries = MAX_RETRIES, onStatus } = options;
    const emit = (msg) => { onStatus?.(msg); };

    const metricNames = getMetricNames();
    const metricTypes = getMetricTypes();

    if (!metricNames.length) {
        throw new Error('No metrics available yet — load a recording first.');
    }

    // Build the embedding index (skipped when already built for this set).
    emit('Loading models…');
    emit('Building metrics index…');
    await buildIndex(metricNames, metricTypes);

    // Embed the user query and find the most relevant metrics.
    const queryVector = await queryEmbed(nlQuery);
    let topK = search(queryVector, TOP_K);
    if (topK.length === 0) {
        topK = keywordSearch(nlQuery, TOP_K);
    }
    if (topK.length === 0) {
        throw new Error('No matching metrics found. Try a different query.');
    }

    // Attach each retrieved metric's label keys so the model can build
    // {label="value"} filters / `sum by (label)` breakouts / part-of-whole
    // shares (e.g. "system cpu usage" → cpu_usage{state="system"}). Without
    // labels in the card the model has nothing to filter on.
    const labelsByMetric = getMetricLabels();
    topK = topK.map(m => ({ ...m, labels: labelsByMetric[m.name] || [] }));

    // Generate PromQL. The specialist decodes greedily (deterministic), so a
    // first failure won't change on retry — sample a little on retries to escape
    // a bad greedy path.
    emit('Generating query…');
    const messages = buildPrompt(topK, nlQuery);
    let raw = '';
    let promql = '';
    for (let attempt = 0; attempt <= maxRetries; attempt++) {
        raw = await llmGenerate(messages, {
            maxNewTokens: 64,
            temperature: attempt === 0 ? 0 : 0.4,
        });
        promql = cleanOutput(raw);
        if (isNoMetric(promql) || looksLikePromQL(promql)) break;
    }

    if (isNoMetric(promql)) {
        throw new Error('No listed metric answers that request (NO_METRIC). Try rephrasing or naming a specific metric.');
    }
    if (!looksLikePromQL(promql)) {
        throw new Error(`Could not generate valid PromQL. Model output: ${raw}`);
    }

    return { promql, raw };
}

/**
 * Reset all pipeline modules (for garbage collection).
 */
export function reset() {
    resetEngine();
    resetLlm();
}
