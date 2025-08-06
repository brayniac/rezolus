import { ChartsState, Chart } from './charts/chart.js';
import { MetricDataFetcher } from './nlp-data-fetcher.js';

// Configuration for the OpenAI-compatible API
const API_CONFIG = {
    endpoint: window.localStorage.getItem('nlp_api_endpoint') || 'http://localhost:8080/v1/chat/completions',
    apiKey: window.localStorage.getItem('nlp_api_key') || '',
    model: window.localStorage.getItem('nlp_model') || 'gpt-3.5-turbo'
};

// NLP Query component
export const NLPQuery = {
    oninit(vnode) {
        this.query = '';
        this.isLoading = false;
        this.error = null;
        this.generatedCharts = [];
        this.chartsState = new ChartsState();
        this.dataFetcher = new MetricDataFetcher();
        this.availableCharts = null; // Cache available charts
        this.sections = null; // Cache sections data
    },

    async submitQuery() {
        if (!this.query.trim()) return;
        
        this.isLoading = true;
        this.error = null;
        m.redraw();

        try {
            // First, fetch available charts (cache if not already fetched)
            if (!this.sections) {
                console.log('Fetching available charts...');
                this.sections = await this.dataFetcher.fetchAvailableSections();
                this.availableCharts = this.dataFetcher.extractAvailableCharts(this.sections);
            }
            
            console.log('Available system charts:', this.availableCharts.system.length);
            console.log('Available cgroup charts:', this.availableCharts.cgroup.length);
            
            // Log first few charts for debugging
            console.log('Sample system charts:', this.availableCharts.system.slice(0, 10).map(c => ({
                title: c.title,
                section: c.section,
                group: c.group
            })));
            console.log('Total available charts:', {
                system: this.availableCharts.system.length,
                cgroup: this.availableCharts.cgroup.length
            });
            
            // Create formatted lists of available charts
            const systemChartsList = this.availableCharts.system.map(c => 
                `"${c.title}" (${c.section}/${c.group})`
            ).join('\n');
            
            const cgroupChartsList = this.availableCharts.cgroup.map(c => 
                `"${c.title}" (${c.section}/${c.group})`
            ).join('\n');
            
            // Prepare the prompt for the LLM
            const systemPrompt = `You are an assistant that helps select monitoring charts from Rezolus.

IMPORTANT RULES:
1. Respond ONLY with valid JSON. No explanations, no markdown, just the JSON object.
2. Select charts from the AVAILABLE CHARTS list below based on the user's query
3. Use the EXACT chart titles from the list - do not modify or create new titles
4. Prefer system-level charts over cgroup charts (unless containers/cgroups are specifically mentioned)
5. Select distinct charts - don't select the same chart multiple times

JSON Response Format:
{
  "charts": [
    {
      "title": "Exact Chart Title From List"
    }
  ]
}

AVAILABLE SYSTEM CHARTS (prefer these):
${systemChartsList || 'No system charts available'}

AVAILABLE CGROUP CHARTS (only select if containers/cgroups mentioned):
${cgroupChartsList || 'No cgroup charts available'}

Based on the user's query, select the most relevant charts from the above lists. 
For example, if asked for "CPU usage", select the chart with that exact title from the list.
Do NOT create new chart titles - only use exact titles from the lists above.`;

            const userPrompt = this.query;
            
            console.log('User query:', userPrompt);
            console.log('System prompt length:', systemPrompt.length);
            console.log('First 500 chars of system prompt:', systemPrompt.substring(0, 500));

            // Call the OpenAI-compatible API
            console.log('Calling API:', API_CONFIG.endpoint);
            console.log('Model:', API_CONFIG.model);
            const response = await fetch(API_CONFIG.endpoint, {
                method: 'POST',
                mode: 'cors',
                headers: {
                    'Content-Type': 'application/json',
                    ...(API_CONFIG.apiKey ? { 'Authorization': `Bearer ${API_CONFIG.apiKey}` } : {})
                },
                body: JSON.stringify({
                    model: API_CONFIG.model,
                    messages: [
                        { role: 'system', content: systemPrompt },
                        { role: 'user', content: userPrompt }
                    ],
                    temperature: 0.7,
                    max_tokens: 1000
                })
            }).catch(err => {
                if (err.message.includes('Failed to fetch')) {
                    throw new Error('Failed to connect to API. Please check that your OpenAI-compatible server is running at ' + API_CONFIG.endpoint);
                }
                throw err;
            });

            if (!response.ok) {
                throw new Error(`API request failed: ${response.statusText}`);
            }

            const data = await response.json();
            console.log('Full API Response:', data);
            
            const content = data.choices[0].message.content;
            console.log('LLM Response Content:', content);
            console.log('Response length:', content.length);
            
            // Try to parse the JSON response
            let chartSpecs;
            
            // First, check if content starts with markdown code block
            if (content.trim().startsWith('```')) {
                console.log('Response contains markdown code block');
                const codeBlockMatch = content.match(/```(?:json)?\s*([\s\S]*?)```/);
                if (codeBlockMatch && codeBlockMatch[1]) {
                    try {
                        chartSpecs = JSON.parse(codeBlockMatch[1].trim());
                        console.log('Extracted from code block:', chartSpecs);
                    } catch (e) {
                        console.error('Failed to parse code block JSON:', e);
                        console.log('Code block content:', codeBlockMatch[1]);
                    }
                }
            }
            
            // If not parsed yet, try direct JSON parse
            if (!chartSpecs) {
                try {
                    chartSpecs = JSON.parse(content);
                    console.log('Direct JSON parse successful:', chartSpecs);
                } catch (parseErr) {
                    console.warn('Direct JSON parse failed:', parseErr);
                    
                    // Try to extract any JSON object from the response
                    // This regex looks for a complete JSON object with nested arrays
                    const jsonStart = content.indexOf('{');
                    if (jsonStart !== -1) {
                        // Find the matching closing brace
                        let braceCount = 0;
                        let inString = false;
                        let escapeNext = false;
                        let jsonEnd = -1;
                        
                        for (let i = jsonStart; i < content.length; i++) {
                            const char = content[i];
                            
                            if (escapeNext) {
                                escapeNext = false;
                                continue;
                            }
                            
                            if (char === '\\') {
                                escapeNext = true;
                                continue;
                            }
                            
                            if (char === '"' && !escapeNext) {
                                inString = !inString;
                                continue;
                            }
                            
                            if (!inString) {
                                if (char === '{') braceCount++;
                                else if (char === '}') {
                                    braceCount--;
                                    if (braceCount === 0) {
                                        jsonEnd = i + 1;
                                        break;
                                    }
                                }
                            }
                        }
                        
                        if (jsonEnd > jsonStart) {
                            const jsonStr = content.substring(jsonStart, jsonEnd);
                            try {
                                chartSpecs = JSON.parse(jsonStr);
                                console.log('Extracted JSON object:', chartSpecs);
                            } catch (e) {
                                console.error('Failed to parse extracted JSON:', e);
                                console.log('Extracted string:', jsonStr);
                            }
                        }
                    }
                }
            }
            
            // If still no valid specs, use fallback
            if (!chartSpecs) {
                console.log('No valid JSON found in LLM response');
                console.log('Raw response was:', content);
                console.log('Creating empty chart list as fallback');
                chartSpecs = {
                    charts: []
                };
            }

            // Ensure we have a charts array
            if (!chartSpecs.charts && Array.isArray(chartSpecs)) {
                chartSpecs = { charts: chartSpecs };
            } else if (!chartSpecs.charts) {
                chartSpecs = { charts: [chartSpecs] };
            }

            // Validate chart specs - we only need titles now
            if (chartSpecs.charts) {
                chartSpecs.charts = chartSpecs.charts.map(chart => ({
                    title: chart.title || 'Chart'
                }));
            }

            console.log('Final parsed chart specs:', chartSpecs);
            console.log('Charts array:', chartSpecs.charts);
            console.log('Number of charts selected:', chartSpecs.charts ? chartSpecs.charts.length : 0);
            
            if (chartSpecs.charts) {
                chartSpecs.charts.forEach((chart, i) => {
                    console.log(`Chart ${i}:`, chart);
                });
            }

            // Generate the dashboard based on the LLM response
            await this.generateDashboard(chartSpecs.charts);
            
        } catch (err) {
            this.error = err.message;
            console.error('Error processing NLP query:', err);
        } finally {
            this.isLoading = false;
            m.redraw();
        }
    },

    async generateDashboard(chartSpecs) {
        // Clear previous charts and errors
        this.chartsState.clear();
        this.generatedCharts = [];
        this.error = null;

        console.log('Generating dashboard with specs:', chartSpecs);

        if (!chartSpecs || chartSpecs.length === 0) {
            console.log('No chart specs provided');
            this.error = 'No charts were selected. Please try a different query.';
            m.redraw();
            return;
        }

        try {
            // Use cached sections if available, otherwise fetch
            if (!this.sections) {
                this.sections = await this.dataFetcher.fetchAvailableSections();
            }
            
            // Map the NLP specs to actual charts with data using cached sections
            const mappedCharts = await this.dataFetcher.mapSpecsToChartsWithSections(chartSpecs, this.sections);
            console.log('Mapped charts:', mappedCharts);
            
            // Filter out any null/undefined charts
            this.generatedCharts = mappedCharts.filter(chart => chart && chart.opts && chart.data);
            
            if (this.generatedCharts.length === 0) {
                this.error = 'No matching charts found. The selected charts may not have data available.';
            }
        } catch (err) {
            console.error('Error mapping specs to charts:', err);
            this.error = 'Error generating dashboard: ' + err.message;
        }
        
        console.log('Final generated charts:', this.generatedCharts);
        m.redraw();
    },

    createChartConfig(spec, index) {
        // Map the LLM spec to the Rezolus chart format
        const chartStyle = spec.type || 'line';
        const baseConfig = {
            id: `nlp-chart-${index}`,
            title: spec.title,
            type: chartStyle,
            data: [], // This would be populated from actual metrics
            opts: {
                id: `nlp-chart-${index}`,
                title: spec.title,
                description: spec.description,
                style: chartStyle  // IMPORTANT: Use 'style' for chart type
            }
        };

        // Add type-specific configuration
        switch (spec.type) {
            case 'line':
                return {
                    ...baseConfig,
                    opts: {
                        ...baseConfig.opts,
                        style: 'line',
                        yAxisLabel: spec.yAxisLabel || 'Value',
                        metrics: spec.metrics
                    }
                };
            case 'scatter':
                return {
                    ...baseConfig,
                    opts: {
                        ...baseConfig.opts,
                        style: 'scatter',
                        metrics: spec.metrics
                    }
                };
            case 'heatmap':
                return {
                    ...baseConfig,
                    opts: {
                        ...baseConfig.opts,
                        style: 'heatmap',
                        metrics: spec.metrics
                    }
                };
            case 'multi':
                return {
                    ...baseConfig,
                    opts: {
                        ...baseConfig.opts,
                        style: 'multi',
                        metrics: spec.metrics
                    }
                };
            default:
                return baseConfig;
        }
    },

    view() {
        return m('div.nlp-query-container', [
            m('div.nlp-query-input', [
                m('h2', 'Natural Language Query'),
                m('div.example-queries', [
                    m('small', 'Example queries:'),
                    m('ul', [
                        m('li', { onclick: () => { this.query = 'Show CPU metrics'; m.redraw(); }}, 'Show CPU metrics'),
                        m('li', { onclick: () => { this.query = 'Monitor network traffic'; m.redraw(); }}, 'Monitor network traffic'),
                        m('li', { onclick: () => { this.query = 'Display disk I/O'; m.redraw(); }}, 'Display disk I/O'),
                        m('li', { onclick: () => { this.query = 'Show system overview'; m.redraw(); }}, 'Show system overview'),
                        m('li', { onclick: () => { this.query = 'Monitor container metrics'; m.redraw(); }}, 'Monitor container metrics')
                    ])
                ]),
                m('div.input-group', [
                    m('textarea.nlp-input', {
                        placeholder: 'Describe your monitoring needs or experiment...',
                        value: this.query,
                        oninput: (e) => this.query = e.target.value,
                        disabled: this.isLoading,
                        rows: 3
                    }),
                    m('button.nlp-submit', {
                        onclick: () => this.submitQuery(),
                        disabled: this.isLoading || !this.query.trim()
                    }, this.isLoading ? 'Generating...' : 'Generate Dashboard')
                ]),
                this.error && m('div.error-message', this.error)
            ]),
            
            (() => {
                const validCharts = this.generatedCharts ? this.generatedCharts.filter(spec => spec && spec.opts && spec.data) : [];
                
                if (validCharts.length > 0) {
                    return m('div.nlp-results', [
                        m('h3', 'Generated Dashboard'),
                        m('div.charts', validCharts.map((spec, index) => {
                            console.log(`Rendering chart ${index}:`, spec);
                            return m(Chart, { 
                                key: spec.opts?.id || `nlp-chart-${index}`,
                                spec, 
                                chartsState: this.chartsState 
                            });
                        }))
                    ]);
                }
                return null;
            })()
        ]);
    }
};

// Settings component for API configuration
export const NLPSettings = {
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