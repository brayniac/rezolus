// nq_generate.js — fine-tuned NL → PromQL specialist (0.5B) for the browser.
//
// Runs brayniac/promql-0.5b-onnx (a full fine-tune of Qwen2.5-Coder-0.5B,
// execution-validated training data) in the browser via transformers.js (WebGPU
// when available, WASM otherwise). Input is a chat-message array in the exact
// format of nq_prompt.js / PROMPT_FORMAT.md; the model's chat template is applied
// automatically by the text-generation pipeline. Decoding is greedy — the model
// emits one PromQL line (or NO_METRIC).

const TRANSFORMERS_CDN = 'https://cdn.jsdelivr.net/npm/@huggingface/transformers@4.2.0';
const DEFAULT_MODEL = 'brayniac/promql-0.5b-onnx';

let generator = null;
let loadPromise = null;

/**
 * Lazy-load the text-generation pipeline. Weights are cached by
 * transformers.js after the first download. Idempotent and concurrency-safe.
 */
async function load() {
    if (generator) return;
    if (loadPromise) return loadPromise;

    loadPromise = (async () => {
        const { pipeline } = await import(TRANSFORMERS_CDN);
        const device =
            typeof navigator !== 'undefined' && navigator.gpu ? 'webgpu' : 'wasm';
        try {
            generator = await pipeline('text-generation', DEFAULT_MODEL, {
                device,
                dtype: 'q4',
            });
        } catch (e) {
            // navigator.gpu existed but no usable adapter — fall back to WASM.
            if (device === 'webgpu') {
                generator = await pipeline('text-generation', DEFAULT_MODEL, {
                    device: 'wasm',
                    dtype: 'q4',
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
 * Generate text from the LLM.
 * @param {Array<{role: string, content: string}>|string} messages - Chat
 *   messages (preferred) or a raw prompt string, which is wrapped as a single
 *   user turn.
 * @param {object} options
 * @param {number} options.maxNewTokens - Max output tokens (default: 256)
 * @param {number} options.temperature - Sampling temperature (default: 0.1)
 * @returns {Promise<string>} The assistant's generated text.
 */
export async function generate(messages, options = {}) {
    await load();

    // Greedy by default: the specialist is trained for a single deterministic
    // PromQL line, so sampling only adds noise. 64 tokens is ample for one query.
    const { maxNewTokens = 64, temperature = 0 } = options;
    const chat = Array.isArray(messages)
        ? messages
        : [{ role: 'user', content: String(messages) }];

    const response = await generator(chat, {
        max_new_tokens: maxNewTokens,
        temperature,
        do_sample: temperature > 0,
    });

    // With chat-message input, transformers.js returns the full conversation
    // in `generated_text`; the assistant's reply is the final message.
    const generated = response[0]?.generated_text;
    if (Array.isArray(generated)) {
        return (generated[generated.length - 1]?.content || '').trim();
    }
    return String(generated || '').trim();
}

/** Whether the generator is loaded and ready. */
export function isReady() {
    return generator !== null;
}

/** Reset the generator state (for garbage collection). */
export function reset() {
    generator = null;
    loadPromise = null;
}
