// natural_query.js — Mithril component for the Natural Query tab.
//
// Turns a natural-language question into PromQL (in the browser, via
// nq_pipeline) and renders the result through the same range-query + chart
// path the Query Explorer uses.

import { runPipeline } from './nq_pipeline.js';
import { renderQueryChart } from './explorers.js';
import { ChartsState } from './charts/chart.js';
import { executePromQLRangeQuery, getSelectedNode, injectLabel } from './data.js';

// Status states
const STATUS_IDLE = 'idle';
const STATUS_LOADING = 'loading';
const STATUS_EMBEDDING = 'embedding';
const STATUS_GENERATING = 'generating';
const STATUS_QUERYING = 'querying';
const STATUS_RESULT = 'result';
const STATUS_ERROR = 'error';

function statusLabel(status) {
    switch (status) {
        case STATUS_LOADING: return 'Loading models…';
        case STATUS_EMBEDDING: return 'Building metrics index…';
        case STATUS_GENERATING: return 'Generating query…';
        case STATUS_QUERYING: return 'Running query…';
        default: return '';
    }
}

export const NaturalQuery = {
    oninit(vnode) {
        const st = vnode.state;
        st.status = STATUS_IDLE;
        st.query = '';
        st.result = null;       // PromQL range-query response
        st.error = null;
        st.loading = false;
        st.promql = '';
        st.rawOutput = '';
        st.chartsState = new ChartsState();
        st.editMode = false;
        st.gpuNote =
            typeof navigator !== 'undefined' && !navigator.gpu
                ? 'WebGPU not available — running models on CPU (slower).'
                : null;

        // Execute a PromQL string through the same range-query path the Query
        // Explorer uses (derives the real time window from metadata and scopes
        // to the selected node).
        st.runPromQL = async (promql) => {
            let q = promql;
            const node = getSelectedNode();
            if (node) q = injectLabel(q, 'node', node);
            st.status = STATUS_QUERYING;
            m.redraw();
            st.result = await executePromQLRangeQuery(q);
            st.status = STATUS_RESULT;
        };

        st.executeQuery = async () => {
            if (!st.query.trim() || st.loading) return;
            st.loading = true;
            st.error = null;
            st.result = null;
            st.promql = '';
            st.editMode = false;
            st.status = STATUS_LOADING;
            m.redraw();

            try {
                const { promql, raw } = await runPipeline(st.query, {
                    onStatus: (msg) => {
                        if (msg.includes('Building')) st.status = STATUS_EMBEDDING;
                        else if (msg.includes('Generating')) st.status = STATUS_GENERATING;
                        else st.status = STATUS_LOADING;
                        m.redraw();
                    },
                });
                st.promql = promql;
                st.rawOutput = raw;
                await st.runPromQL(promql);
            } catch (error) {
                st.status = STATUS_ERROR;
                st.error = error.message || 'Pipeline failed';
            } finally {
                st.loading = false;
                m.redraw();
            }
        };

        // Re-run just the (possibly edited) PromQL — no model generation.
        st.executeEditedPromQL = async () => {
            if (!st.promql.trim() || st.loading) return;
            st.loading = true;
            st.error = null;
            m.redraw();
            try {
                await st.runPromQL(st.promql);
            } catch (error) {
                st.status = STATUS_ERROR;
                st.error = error.message || 'Query failed';
            } finally {
                st.loading = false;
                m.redraw();
            }
        };
    },

    view(vnode) {
        const st = vnode.state;
        const busy = st.status === STATUS_LOADING
            || st.status === STATUS_EMBEDDING
            || st.status === STATUS_GENERATING
            || st.status === STATUS_QUERYING;

        return m('div.natural-query', [
            // Status banner
            busy && m('div.query-status', [
                m('span.status-spinner', st.status === STATUS_LOADING ? '◐' :
                    st.status === STATUS_EMBEDDING ? '◑' :
                    st.status === STATUS_GENERATING ? '◒' : '◓'),
                ' ' + statusLabel(st.status),
            ]),

            // Error state
            st.status === STATUS_ERROR && m('div.error-message', [
                m('strong', 'Error: '), st.error,
                m('button.retry-btn', {
                    onclick: () => { st.status = STATUS_IDLE; st.error = null; },
                }, 'Retry'),
            ]),

            // Input section
            m('div.query-input-section', [
                m('h2', 'Natural Language Query'),
                st.gpuNote && m('div.query-gpu-note', st.gpuNote),
                m('div.query-input-wrapper', [
                    m('input.natural-query-input', {
                        type: 'text',
                        placeholder: 'e.g. "show me cpu usage over time"',
                        value: st.query,
                        oninput: (e) => { st.query = e.target.value; },
                        onkeydown: (e) => {
                            if (e.key === 'Enter' && !e.shiftKey) {
                                e.preventDefault();
                                st.executeQuery();
                            }
                        },
                        disabled: st.loading,
                    }),
                    m('button.execute-btn', {
                        onclick: () => st.executeQuery(),
                        disabled: st.loading || !st.query.trim(),
                    }, st.loading ? 'Running…' : 'Execute'),
                ]),
            ]),

            // Result section
            st.status === STATUS_RESULT && st.result && m('div.query-result', [
                m('h3', 'Generated PromQL'),
                m('div.promql-display', [
                    m('code', st.promql),
                    m('button.copy-btn', {
                        onclick: () => { navigator.clipboard.writeText(st.promql); },
                    }, 'Copy'),
                    m('button.edit-btn', {
                        onclick: () => { st.editMode = !st.editMode; },
                    }, st.editMode ? 'Hide' : 'Edit'),
                ]),
                st.editMode && m('div.promql-edit', [
                    m('textarea.promql-edit-input', {
                        value: st.promql,
                        oninput: (e) => { st.promql = e.target.value; },
                        rows: 3,
                    }),
                    m('button.apply-edit-btn', {
                        onclick: () => st.executeEditedPromQL(),
                        disabled: st.loading,
                    }, 'Apply & Run'),
                ]),
                m('div.chart-container',
                    st.result.status === 'success'
                        ? renderQueryChart(
                            st.result.data && st.result.data.result,
                            st.promql,
                            st.chartsState,
                            undefined,
                        )
                        : m('div.error-message', 'Query failed: ' + (st.result.error || 'Unknown error')),
                ),
            ]),

            // First-load banner (model download)
            st.status === STATUS_LOADING && m('div.model-loading', [
                m('p', 'Loading AI models (first time ~350MB, cached afterwards)…'),
                m('div.progress-bar', m('div.progress-fill.indeterminate')),
            ]),
        ]);
    },
};
