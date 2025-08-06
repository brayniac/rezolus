// Vector search and intent-based chart discovery
// This module combines embeddings-based search with LLM intent parsing

import { chartEnricher } from './chart-enrichment.js';

export class VectorSearchEngine {
    constructor() {
        this.embeddings = new Map(); // Store chart embeddings
        this.charts = [];
    }

    // Initialize with available charts
    async initialize(sections) {
        // First enrich all charts with full context
        this.charts = chartEnricher.enrichAllCharts(sections);
        console.log(`Initialized vector search with ${this.charts.length} enriched charts`);
        
        // Prepare embeddings data structure using enriched search text
        for (const chart of this.charts) {
            this.embeddings.set(chart.fullTitle, {
                chart: chart,
                text: chart.searchText,
                embedding: null // Would be computed via embedding API
            });
        }
        
        // Log some examples of enriched titles
        console.log('Sample enriched charts:', this.charts.slice(0, 5).map(c => ({
            original: c.metadata.originalTitle,
            full: c.fullTitle,
            description: c.description
        })));
    }

    // Extract all charts with full metadata
    extractChartsWithMetadata(sections) {
        const charts = [];
        
        for (const [sectionName, sectionData] of Object.entries(sections)) {
            if (!sectionData || !sectionData.groups) continue;
            
            for (const group of sectionData.groups) {
                if (!group.plots) continue;
                
                for (const plot of group.plots) {
                    if (!plot.opts?.title || !plot.data || plot.data.length < 2) continue;
                    
                    charts.push({
                        id: `${sectionName}-${group.name}-${plot.opts.title}`.replace(/\s+/g, '-'),
                        title: plot.opts.title,
                        description: plot.opts.description || '',
                        section: sectionName,
                        group: group.name,
                        style: plot.opts?.style || 'line',
                        data: plot.data,
                        opts: plot.opts,
                        plot: plot,
                        metadata: {
                            hasData: true,
                            dataPoints: plot.data[0]?.length || 0,
                            unit: plot.opts?.format?.unit_system,
                            yAxisLabel: plot.opts?.format?.y_axis_label
                        }
                    });
                }
            }
        }
        
        return charts;
    }

    // Perform similarity search using embeddings (simplified version)
    async searchSimilarCharts(query, topK = 10) {
        // In a real implementation, we would:
        // 1. Generate embedding for the query
        // 2. Compute cosine similarity with all chart embeddings
        // 3. Return top K results
        
        // For now, use a simplified keyword-based similarity
        const queryWords = query.toLowerCase().split(/\s+/);
        const scores = [];
        
        for (const [id, data] of this.embeddings) {
            const text = data.text.toLowerCase();
            let score = 0;
            
            // Exact title match gets highest score
            if (data.chart.title.toLowerCase() === query.toLowerCase()) {
                score = 10;
            } else {
                // Score based on keyword matches
                for (const word of queryWords) {
                    if (word.length < 3) continue;
                    
                    // Title matches are worth more
                    if (data.chart.title.toLowerCase().includes(word)) {
                        score += 3;
                    }
                    // Description matches
                    else if (data.chart.description.toLowerCase().includes(word)) {
                        score += 2;
                    }
                    // Section/group matches
                    else if (text.includes(word)) {
                        score += 1;
                    }
                }
            }
            
            if (score > 0) {
                scores.push({
                    chart: data.chart,
                    score: score
                });
            }
        }
        
        // Sort by score and return top K
        scores.sort((a, b) => b.score - a.score);
        return scores.slice(0, topK);
    }

    // Get charts by specific criteria
    getChartsByCriteria(criteria) {
        return this.charts.filter(chart => {
            if (criteria.section && chart.section !== criteria.section) return false;
            if (criteria.group && chart.group !== criteria.group) return false;
            if (criteria.style && chart.style !== criteria.style) return false;
            if (criteria.hasData !== undefined && chart.metadata.hasData !== criteria.hasData) return false;
            return true;
        });
    }
}

// Intent parser for understanding query requirements
export class IntentParser {
    // Parse natural language query into structured intent
    async parseIntent(query, llmEndpoint, model) {
        const systemPrompt = `You are a monitoring query parser. Extract structured information from natural language queries.

Return a JSON object with:
{
  "intent": "monitor|analyze|compare|troubleshoot|explore",
  "metrics": ["list", "of", "metric", "types"],
  "timeRange": "last-hour|last-day|last-week|custom|null",
  "aggregation": "avg|sum|max|min|p95|p99|null",
  "comparison": "time-over-time|between-metrics|null",
  "filters": {
    "section": "cpu|network|disk|memory|null",
    "systemOnly": true|false,
    "containerOnly": true|false
  },
  "keywords": ["important", "keywords", "from", "query"]
}

Examples:
- "Show CPU usage for the last hour" → 
  {"intent": "monitor", "metrics": ["cpu"], "timeRange": "last-hour", "filters": {"section": "cpu"}}
- "Compare network traffic between containers" → 
  {"intent": "compare", "metrics": ["network"], "filters": {"containerOnly": true}}
- "Analyze disk I/O patterns" → 
  {"intent": "analyze", "metrics": ["disk", "io"], "filters": {"section": "disk"}}`;

        try {
            const response = await fetch(llmEndpoint, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    model: model,
                    messages: [
                        { role: 'system', content: systemPrompt },
                        { role: 'user', content: query }
                    ],
                    temperature: 0.3, // Lower temperature for more consistent parsing
                    max_tokens: 500
                })
            });

            if (!response.ok) {
                throw new Error(`LLM API failed: ${response.statusText}`);
            }

            const data = await response.json();
            const content = data.choices[0].message.content;
            
            // Try to parse JSON from response
            let intent;
            try {
                intent = JSON.parse(content);
            } catch (e) {
                // Try to extract JSON from the response
                const jsonMatch = content.match(/\{[\s\S]*\}/);
                if (jsonMatch) {
                    intent = JSON.parse(jsonMatch[0]);
                } else {
                    throw new Error('Could not parse intent from LLM response');
                }
            }

            return intent;
        } catch (error) {
            console.error('Error parsing intent:', error);
            // Fallback to keyword extraction
            return this.fallbackIntentParsing(query);
        }
    }

    // Fallback intent parsing using simple keyword matching
    fallbackIntentParsing(query) {
        const lower = query.toLowerCase();
        const intent = {
            intent: 'monitor',
            metrics: [],
            timeRange: null,
            aggregation: null,
            comparison: null,
            filters: {
                section: null,
                systemOnly: !lower.includes('container') && !lower.includes('cgroup'),
                containerOnly: lower.includes('container') || lower.includes('cgroup')
            },
            keywords: []
        };

        // Extract metrics
        const metricKeywords = {
            cpu: ['cpu', 'processor', 'cores', 'usage', 'idle'],
            network: ['network', 'traffic', 'bandwidth', 'packets', 'bytes'],
            disk: ['disk', 'storage', 'io', 'read', 'write'],
            memory: ['memory', 'ram', 'heap', 'cache', 'swap'],
            system: ['system', 'load', 'uptime', 'processes']
        };

        for (const [metric, keywords] of Object.entries(metricKeywords)) {
            if (keywords.some(k => lower.includes(k))) {
                intent.metrics.push(metric);
                if (!intent.filters.section) {
                    intent.filters.section = metric;
                }
            }
        }

        // Extract intent type
        if (lower.includes('compare')) intent.intent = 'compare';
        else if (lower.includes('analyze')) intent.intent = 'analyze';
        else if (lower.includes('troubleshoot')) intent.intent = 'troubleshoot';
        else if (lower.includes('explore')) intent.intent = 'explore';

        // Extract keywords
        intent.keywords = query.toLowerCase().split(/\s+/).filter(w => w.length > 3);

        return intent;
    }
}

// Dynamic chart builder
export class DynamicChartBuilder {
    // Build chart configuration from intent and available data
    buildChartFromIntent(intent, availableCharts) {
        const charts = [];
        
        // Filter charts based on intent
        let relevantCharts = availableCharts;
        
        if (intent.filters.section) {
            relevantCharts = relevantCharts.filter(c => c.section === intent.filters.section);
        }
        
        if (intent.filters.systemOnly) {
            relevantCharts = relevantCharts.filter(c => 
                !c.title.toLowerCase().includes('cgroup') && 
                !c.title.toLowerCase().includes('container')
            );
        }
        
        if (intent.filters.containerOnly) {
            relevantCharts = relevantCharts.filter(c => 
                c.title.toLowerCase().includes('cgroup') || 
                c.title.toLowerCase().includes('container')
            );
        }
        
        // Filter by metrics if specified
        if (intent.metrics && intent.metrics.length > 0) {
            relevantCharts = relevantCharts.filter(c => {
                const chartText = `${c.title} ${c.description}`.toLowerCase();
                return intent.metrics.some(m => chartText.includes(m));
            });
        }
        
        // Handle different intents
        switch (intent.intent) {
            case 'compare':
                // For comparison, try to find related charts
                if (relevantCharts.length >= 2) {
                    charts.push(...relevantCharts.slice(0, 4)); // Limit to 4 for comparison
                }
                break;
                
            case 'analyze':
                // For analysis, include multiple related charts
                charts.push(...relevantCharts.slice(0, 6));
                break;
                
            case 'troubleshoot':
                // For troubleshooting, prioritize error/issue related metrics
                const troubleshootCharts = relevantCharts.filter(c => 
                    c.title.toLowerCase().includes('error') ||
                    c.title.toLowerCase().includes('fail') ||
                    c.title.toLowerCase().includes('retry') ||
                    c.title.toLowerCase().includes('timeout')
                );
                charts.push(...troubleshootCharts);
                // Add general charts if not enough specific ones
                if (charts.length < 3) {
                    charts.push(...relevantCharts.slice(0, 3 - charts.length));
                }
                break;
                
            default:
                // Default: show most relevant charts
                charts.push(...relevantCharts.slice(0, 4));
        }
        
        return charts;
    }
    
    // Create a custom chart configuration
    createCustomChart(title, data, options = {}) {
        return {
            title: title,
            data: data,
            opts: {
                id: `custom-${Date.now()}`,
                title: title,
                description: options.description || 'Custom generated chart',
                style: options.style || 'line',
                format: {
                    y_axis_label: options.yAxisLabel || 'Value',
                    unit_system: options.unit || null,
                    log_scale: options.logScale || false
                }
            },
            custom: true
        };
    }
}