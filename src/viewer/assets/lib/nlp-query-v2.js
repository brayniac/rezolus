// Simplified NLP Query Component using enhanced chart metadata
import { ChartsState, Chart } from './charts/chart.js';
import { enhancedNLPQuery } from './nlp-enhanced.js';
import { MetricDataFetcher } from './nlp-data-fetcher.js';

// Configuration for the OpenAI-compatible API
const API_CONFIG = {
    endpoint: window.localStorage.getItem('nlp_api_endpoint') || 'http://localhost:8080/v1/chat/completions',
    apiKey: window.localStorage.getItem('nlp_api_key') || '',
    model: window.localStorage.getItem('nlp_model') || 'gpt-3.5-turbo'
};

// NLP Query component v2
export const NLPQueryV2 = {
    oninit(vnode) {
        this.query = '';
        this.isLoading = false;
        this.error = null;
        this.generatedCharts = [];
        this.chartsState = new ChartsState();
        this.dataFetcher = new MetricDataFetcher();
        this.sections = null;
        this.initialized = false;
        this.sampleQueries = [];
    },

    async initialize() {
        if (this.initialized) return;
        
        try {
            console.log('Initializing NLP Query v2...');
            
            // Fetch sections if not cached
            if (!this.sections) {
                this.sections = await this.dataFetcher.fetchAvailableSections();
            }
            
            // Initialize the enhanced NLP query processor
            await enhancedNLPQuery.initialize(this.sections);
            
            // Get sample queries based on available data
            this.sampleQueries = enhancedNLPQuery.getSampleQueries();
            
            this.initialized = true;
            console.log('NLP Query v2 initialized successfully');
            m.redraw();
        } catch (err) {
            console.error('Failed to initialize NLP Query v2:', err);
            this.error = 'Failed to initialize. Please refresh and try again.';
            m.redraw();
        }
    },

    async submitQuery() {
        if (!this.query.trim()) return;
        
        // Initialize if needed
        if (!this.initialized) {
            await this.initialize();
        }
        
        this.isLoading = true;
        this.error = null;
        this.generatedCharts = [];
        m.redraw();

        try {
            console.log('Processing query:', this.query);
            
            // Use enhanced NLP to process the query
            const chartConfigs = await enhancedNLPQuery.processQuery(this.query);
            
            if (chartConfigs.length === 0) {
                this.error = 'No relevant charts found for your query. Try different keywords.';
            } else {
                this.generatedCharts = chartConfigs;
                console.log(`Generated ${chartConfigs.length} charts`);
            }
            
        } catch (err) {
            this.error = err.message;
            console.error('Error processing query:', err);
        } finally {
            this.isLoading = false;
            m.redraw();
        }
    },

    view() {
        // Initialize on first view if not already done
        if (!this.initialized && !this.isLoading) {
            this.initialize();
        }

        return m('div.nlp-query-container', [
            m('div.nlp-query-input', [
                m('h2', 'Natural Language Query'),
                
                // Show sample queries if available
                this.sampleQueries.length > 0 && m('div.example-queries', [
                    m('small', 'Try these queries:'),
                    m('ul', this.sampleQueries.map(q => 
                        m('li', { 
                            onclick: () => { 
                                this.query = q; 
                                m.redraw(); 
                            }
                        }, q)
                    ))
                ]),
                
                m('div.input-group', [
                    m('textarea.nlp-input', {
                        placeholder: this.initialized ? 
                            'Ask about your system metrics... (e.g., "Show CPU and memory usage" or "Network performance over time")' :
                            'Initializing...',
                        value: this.query,
                        oninput: (e) => this.query = e.target.value,
                        disabled: this.isLoading || !this.initialized,
                        rows: 3
                    }),
                    m('button.nlp-submit', {
                        onclick: () => this.submitQuery(),
                        disabled: this.isLoading || !this.query.trim() || !this.initialized
                    }, this.isLoading ? 'Processing...' : 'Generate Dashboard')
                ]),
                
                this.error && m('div.error-message', this.error),
                
                !this.initialized && !this.error && m('div.info-message', 'Loading available metrics...')
            ]),
            
            // Render generated charts
            this.generatedCharts && this.generatedCharts.length > 0 && m('div.nlp-results', [
                m('h3', `Generated Dashboard (${this.generatedCharts.length} charts)`),
                m('div.charts', this.generatedCharts.map((spec, index) => {
                    // Validate chart before rendering
                    if (!spec || !spec.opts || !spec.data) {
                        console.warn(`Invalid chart at index ${index}:`, spec);
                        return null;
                    }
                    
                    return m(Chart, { 
                        key: spec.opts.id || `nlp-chart-${index}`,
                        spec, 
                        chartsState: this.chartsState 
                    });
                }).filter(Boolean)) // Remove null entries
            ])
        ]);
    }
};

// Settings component for API configuration
export const NLPSettingsV2 = {
    oninit() {
        this.endpoint = API_CONFIG.endpoint;
        this.apiKey = API_CONFIG.apiKey;
        this.model = API_CONFIG.model;
        this.showSettings = false;
    },

    saveSettings() {
        window.localStorage.setItem('nlp_api_endpoint', this.endpoint);
        window.localStorage.setItem('nlp_api_key', this.apiKey);
        window.localStorage.setItem('nlp_model', this.model);
        
        // Update the global config
        API_CONFIG.endpoint = this.endpoint;
        API_CONFIG.apiKey = this.apiKey;
        API_CONFIG.model = this.model;
        
        this.showSettings = false;
        m.redraw();
    },

    view() {
        return m('div.nlp-settings', [
            m('button.settings-toggle', {
                onclick: () => this.showSettings = !this.showSettings
            }, 'API Settings'),
            
            this.showSettings && m('div.settings-panel', [
                m('div.setting-group', [
                    m('label', 'API Endpoint:'),
                    m('input[type=text]', {
                        value: this.endpoint,
                        oninput: (e) => this.endpoint = e.target.value,
                        placeholder: 'http://localhost:8080/v1/chat/completions'
                    })
                ]),
                m('div.setting-group', [
                    m('label', 'API Key (optional):'),
                    m('input[type=password]', {
                        value: this.apiKey,
                        oninput: (e) => this.apiKey = e.target.value,
                        placeholder: 'Enter API key if required'
                    })
                ]),
                m('div.setting-group', [
                    m('label', 'Model:'),
                    m('input[type=text]', {
                        value: this.model,
                        oninput: (e) => this.model = e.target.value,
                        placeholder: 'gpt-3.5-turbo'
                    })
                ]),
                m('div.setting-actions', [
                    m('button', { onclick: () => this.saveSettings() }, 'Save'),
                    m('button', { onclick: () => this.showSettings = false }, 'Cancel')
                ])
            ])
        ]);
    }
};