// nq_engine.js — transformers.js feature-extraction model + metric index.
//
// Loads a small sentence-embedding model in the browser (WebGPU when
// available, WASM otherwise), embeds every metric name once into an index,
// and exposes that index for cosine-similarity search (see nq_search.js).

const TRANSFORMERS_CDN = 'https://cdn.jsdelivr.net/npm/@huggingface/transformers@4.2.0';
const EMBED_MODEL = 'Xenova/all-MiniLM-L6-v2';

let extractor = null;
let metricIndex = null; // Array of { name, type, vector: number[] }
let indexedSignature = null; // identifies the metric set currently indexed
let loadPromise = null;

/**
 * Lazy-load the feature-extraction pipeline. Weights are cached by
 * transformers.js after the first download. Idempotent and concurrency-safe:
 * overlapping callers await the same in-flight promise.
 */
async function load() {
    if (extractor) return;
    if (loadPromise) return loadPromise;

    loadPromise = (async () => {
        const { pipeline } = await import(TRANSFORMERS_CDN);
        const device =
            typeof navigator !== 'undefined' && navigator.gpu ? 'webgpu' : 'wasm';
        try {
            extractor = await pipeline('feature-extraction', EMBED_MODEL, {
                device,
                // WebGPU runs the tiny MiniLM encoder at fp32; WASM uses the
                // quantized q8 weights to keep the download small.
                dtype: device === 'webgpu' ? 'fp32' : 'q8',
            });
        } catch (e) {
            // navigator.gpu existed but no usable adapter — fall back to WASM.
            if (device === 'webgpu') {
                extractor = await pipeline('feature-extraction', EMBED_MODEL, {
                    device: 'wasm',
                    dtype: 'q8',
                });
            } else {
                throw e;
            }
        }
    })();

    try {
        await loadPromise;
    } finally {
        loadPromise = null;
    }
}

/**
 * Build the metric embedding index from metric names and types.
 * Embeds every metric in one batched call and stores a per-row vector.
 */
export async function buildIndex(metricNames, metricTypes) {
    // Skip re-embedding when the same metric set is already indexed.
    const signature = `${metricNames.length}:${metricNames[0] || ''}:${metricNames[metricNames.length - 1] || ''}`;
    if (metricIndex && indexedSignature === signature) return;

    await load();

    const texts = metricNames.map((name) =>
        `${name} ${metricTypes?.[name] || ''}`.trim(),
    );

    // Batched feature-extraction yields a [N, D] tensor. tolist() converts it
    // to a proper number[][] so each metric gets its own distinct vector.
    const output = await extractor(texts, { pooling: 'mean', normalize: true });
    const rows = output.tolist();

    metricIndex = metricNames.map((name, i) => ({
        name,
        type: metricTypes?.[name] || '',
        vector: rows[i],
    }));
    indexedSignature = signature;
}

/** Get the current metric embedding index (or null before buildIndex). */
export function getIndex() {
    return metricIndex;
}

/** Encode a single user query into an embedding vector (number[]). */
export async function queryEmbed(query) {
    await load();
    const output = await extractor(query, { pooling: 'mean', normalize: true });
    // A single string input produces a [1, D] tensor; return its only row.
    return output.tolist()[0];
}

/** Whether the engine is loaded and the index has been built. */
export function isReady() {
    return extractor !== null && metricIndex !== null;
}

/** Reset engine state (for garbage collection). */
export function reset() {
    extractor = null;
    metricIndex = null;
    indexedSignature = null;
    loadPromise = null;
}
