// nq_pipeline.js — orchestrates embed → search → generate to turn a natural
// language question into a PromQL string. Executing the PromQL (deriving the
// time window, rendering a chart) is the caller's job, via the same
// executePromQLRangeQuery path the Query Explorer uses.

import { queryEmbed, buildIndex, reset as resetEngine } from './nq_engine.js';
import { search, keywordSearch } from './nq_search.js';
import { generate as llmGenerate, reset as resetLlm } from './nq_generate.js';
import { buildPrompt, cleanOutput, looksLikePromQL } from './nq_prompt.js';
import { getMetricNames, getMetricTypes } from './data.js';

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

    // Generate PromQL, retrying (with a touch more temperature) if the output
    // does not look like a valid query.
    emit('Generating query…');
    let raw = '';
    let promql = '';
    for (let attempt = 0; attempt <= maxRetries; attempt++) {
        const messages = buildPrompt(topK, nlQuery);
        raw = await llmGenerate(messages, {
            maxNewTokens: 256,
            temperature: attempt === 0 ? 0.1 : 0.4,
        });
        promql = cleanOutput(raw);
        if (looksLikePromQL(promql)) break;
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
