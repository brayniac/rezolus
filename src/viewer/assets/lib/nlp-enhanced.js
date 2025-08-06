// Enhanced NLP query system using enriched chart metadata
// This replaces the simple title matching with context-aware selection

import { ChartsState, Chart } from './charts/chart.js';
import { chartEnricher } from './chart-enrichment.js';

// Configuration for the OpenAI-compatible API
const API_CONFIG = {
    endpoint: window.localStorage.getItem('nlp_api_endpoint') || 'http://localhost:8080/v1/chat/completions',
    apiKey: window.localStorage.getItem('nlp_api_key') || '',
    model: window.localStorage.getItem('nlp_model') || 'gpt-3.5-turbo'
};

export class EnhancedNLPQuery {
    constructor() {
        this.enrichedCharts = [];
        this.sections = null;
        this.initialized = false;
    }

    // Initialize with sections data
    async initialize(sections) {
        if (this.initialized) return;
        
        console.log('Enriching charts with full context...');
        this.sections = sections;
        this.enrichedCharts = chartEnricher.enrichAllCharts(sections);
        this.initialized = true;
        
        console.log(`Initialized with ${this.enrichedCharts.length} enriched charts`);
        
        // Log samples to show the enrichment
        console.log('Sample enriched charts:', this.enrichedCharts.slice(0, 5).map(c => ({
            original: c.metadata.originalTitle,
            enriched: c.fullTitle,
            description: c.description,
            section: c.metadata.section
        })));
    }

    // Process a natural language query
    async processQuery(query) {
        if (!this.initialized) {
            throw new Error('EnhancedNLPQuery not initialized. Call initialize() first.');
        }

        console.log('Processing query:', query);

        // Step 1: Use LLM to understand intent and select charts
        const selectedCharts = await this.selectChartsWithLLM(query);
        
        // Step 2: Map selected charts to actual chart configurations
        const chartConfigs = this.mapToChartConfigs(selectedCharts);
        
        return chartConfigs;
    }

    // Use LLM to select relevant charts based on enriched metadata
    async selectChartsWithLLM(query) {
        // Prepare chart list for LLM with enriched information
        const systemChartList = this.enrichedCharts
            .filter(c => c.metadata.isSystemLevel)
            .map(c => `- "${c.fullTitle}": ${c.description}`)
            .join('\n');
            
        const containerChartList = this.enrichedCharts
            .filter(c => c.metadata.isContainer)
            .map(c => `- "${c.fullTitle}": ${c.description}`)
            .join('\n');

        const systemPrompt = `You are a monitoring dashboard assistant. Select the most relevant charts based on the user's query.

IMPORTANT RULES:
1. Return ONLY a JSON array of chart titles
2. Use EXACT chart titles from the available list
3. Select 1-6 most relevant charts (prefer fewer, highly relevant charts)
4. Prefer system-level charts unless containers are specifically mentioned
5. Consider the chart descriptions to understand what each chart shows

Response format:
["Exact Chart Title 1", "Exact Chart Title 2", ...]

AVAILABLE SYSTEM CHARTS:
${systemChartList}

AVAILABLE CONTAINER CHARTS (only use if containers/cgroups mentioned):
${containerChartList}

Select charts that best answer the user's query. Focus on relevance over quantity.`;

        try {
            console.log('Calling LLM with enriched chart list...');
            console.log('Number of system charts:', this.enrichedCharts.filter(c => c.metadata.isSystemLevel).length);
            console.log('Number of container charts:', this.enrichedCharts.filter(c => c.metadata.isContainer).length);

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
                        { role: 'user', content: query }
                    ],
                    temperature: 0.3, // Lower temperature for more consistent selection
                    max_tokens: 500
                })
            });

            if (!response.ok) {
                throw new Error(`API request failed: ${response.statusText}`);
            }

            const data = await response.json();
            const content = data.choices[0].message.content;
            
            console.log('LLM response:', content);

            // Parse the response
            let selectedTitles;
            try {
                selectedTitles = JSON.parse(content);
            } catch (e) {
                // Try to extract JSON array from the response
                const jsonMatch = content.match(/\[[\s\S]*?\]/);
                if (jsonMatch) {
                    selectedTitles = JSON.parse(jsonMatch[0]);
                } else {
                    console.error('Failed to parse LLM response:', content);
                    selectedTitles = [];
                }
            }

            console.log('Selected chart titles:', selectedTitles);
            return selectedTitles;

        } catch (error) {
            console.error('Error calling LLM:', error);
            // Fallback to keyword-based selection
            return this.fallbackChartSelection(query);
        }
    }

    // Fallback chart selection using keywords
    fallbackChartSelection(query) {
        const queryLower = query.toLowerCase();
        const keywords = queryLower.split(/\s+/).filter(w => w.length > 2);
        
        console.log('Using fallback selection with keywords:', keywords);

        // Score each chart based on keyword matches
        const scoredCharts = this.enrichedCharts.map(chart => {
            let score = 0;
            
            // Check title matches
            const titleLower = chart.fullTitle.toLowerCase();
            keywords.forEach(keyword => {
                if (titleLower.includes(keyword)) score += 3;
            });
            
            // Check description matches
            const descLower = (chart.description || '').toLowerCase();
            keywords.forEach(keyword => {
                if (descLower.includes(keyword)) score += 2;
            });
            
            // Check metadata keywords
            chart.metadata.keywords?.forEach(kw => {
                if (keywords.some(k => kw.includes(k) || k.includes(kw))) score += 1;
            });
            
            return { chart, score };
        });

        // Sort by score and return top charts
        const topCharts = scoredCharts
            .filter(sc => sc.score > 0)
            .sort((a, b) => b.score - a.score)
            .slice(0, 6)
            .map(sc => sc.chart.fullTitle);

        console.log('Fallback selected:', topCharts);
        return topCharts;
    }

    // Map selected chart titles to actual chart configurations
    mapToChartConfigs(selectedTitles) {
        const configs = [];
        
        for (const title of selectedTitles) {
            // Find the enriched chart
            const enrichedChart = this.enrichedCharts.find(c => c.fullTitle === title);
            
            if (enrichedChart) {
                // Create chart configuration
                const config = {
                    type: enrichedChart.opts?.style || 'line',
                    data: enrichedChart.data,
                    opts: {
                        ...enrichedChart.opts,
                        title: enrichedChart.fullTitle, // Use enriched title
                        description: enrichedChart.description,
                        style: enrichedChart.opts?.style || 'line',
                        id: `nlp-${Date.now()}-${configs.length}`,
                        format: enrichedChart.opts?.format || {
                            y_axis_label: 'Value',
                            unit_system: null,
                            log_scale: false
                        }
                    },
                    nlpGenerated: true
                };
                
                configs.push(config);
                console.log(`Mapped chart: ${title}`);
            } else {
                console.warn(`Chart not found: ${title}`);
            }
        }
        
        return configs;
    }

    // Get sample queries based on available charts
    getSampleQueries() {
        const samples = [];
        
        // Add queries based on available sections
        const sections = new Set(this.enrichedCharts.map(c => c.metadata.section));
        
        if (sections.has('cpu')) samples.push('Show CPU usage and performance');
        if (sections.has('network')) samples.push('Monitor network traffic');
        if (sections.has('blockio')) samples.push('Display disk I/O metrics');
        if (sections.has('memory')) samples.push('Show memory usage');
        if (sections.has('scheduler')) samples.push('Analyze scheduler performance');
        
        // Add some complex queries
        samples.push('System performance overview');
        if (this.enrichedCharts.some(c => c.metadata.isContainer)) {
            samples.push('Container resource usage');
        }
        
        return samples.slice(0, 5);
    }
}

// Export singleton instance
export const enhancedNLPQuery = new EnhancedNLPQuery();