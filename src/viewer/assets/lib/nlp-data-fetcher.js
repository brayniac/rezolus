// Data fetcher module for NLP query results
// This module fetches actual metric data based on the chart specifications

export class MetricDataFetcher {
    constructor() {
        this.cache = new Map();
    }

    // Fetch all available sections to determine what metrics are available
    async fetchAvailableSections() {
        const sections = ['overview', 'cpu', 'network', 'scheduler', 'syscall', 'softirq', 'blockio', 'cgroups', 'rezolus'];
        const results = {};

        for (const section of sections) {
            try {
                const data = await m.request({
                    method: "GET",
                    url: `/data/${section}.json`,
                    withCredentials: true,
                });
                results[section] = data;
                this.cache.set(section, data);
            } catch (err) {
                console.warn(`Failed to fetch ${section} data:`, err);
            }
        }

        return results;
    }

    // Extract all available charts (not just metrics) from sections
    extractAvailableCharts(sections) {
        const charts = {
            system: [],
            cgroup: []
        };

        for (const [sectionName, sectionData] of Object.entries(sections)) {
            if (!sectionData || !sectionData.groups) continue;

            for (const group of sectionData.groups) {
                if (!group.plots) continue;

                for (const plot of group.plots) {
                    if (!plot.opts?.title || !plot.data || plot.data.length < 2) continue;
                    
                    const title = plot.opts.title;
                    const chartInfo = {
                        title: title,
                        section: sectionName,
                        group: group.name,
                        style: plot.opts?.style || 'line',
                        id: plot.opts?.id || `${sectionName}-${title}`,
                        description: plot.opts?.description || '',
                        hasData: true
                    };

                    // Categorize as system or cgroup chart
                    if (title.toLowerCase().includes('cgroup') || title.toLowerCase().includes('container')) {
                        charts.cgroup.push(chartInfo);
                    } else {
                        charts.system.push(chartInfo);
                    }
                }
            }
        }

        // Sort and deduplicate
        charts.system = this.deduplicateCharts(charts.system);
        charts.cgroup = this.deduplicateCharts(charts.cgroup);

        return charts;
    }

    deduplicateCharts(charts) {
        const seen = new Set();
        return charts.filter(c => {
            const key = `${c.section}-${c.title}`.toLowerCase();
            if (seen.has(key)) return false;
            seen.add(key);
            return true;
        }).sort((a, b) => {
            // Sort by section first, then by title
            if (a.section !== b.section) {
                const sectionOrder = ['overview', 'cpu', 'network', 'blockio', 'scheduler', 'syscall', 'softirq', 'rezolus', 'cgroups'];
                return sectionOrder.indexOf(a.section) - sectionOrder.indexOf(b.section);
            }
            return a.title.localeCompare(b.title);
        });
    }

    deduplicateMetrics(metrics) {
        const seen = new Set();
        return metrics.filter(m => {
            const key = m.title.toLowerCase();
            if (seen.has(key)) return false;
            seen.add(key);
            return true;
        }).sort((a, b) => a.title.localeCompare(b.title));
    }

    // Extract metric data from a section based on metric paths
    extractMetricData(sectionData, metricPaths) {
        const extractedData = [];

        if (!sectionData || !sectionData.groups) {
            return extractedData;
        }

        for (const group of sectionData.groups) {
            if (!group.plots) continue;

            for (const plot of group.plots) {
                // Check if this plot contains any of the requested metrics
                const matchingMetrics = metricPaths.filter(path => {
                    return plot.opts && (
                        plot.opts.title?.toLowerCase().includes(path.toLowerCase()) ||
                        plot.opts.id?.includes(path)
                    );
                });

                if (matchingMetrics.length > 0) {
                    extractedData.push({
                        ...plot,
                        matchedMetrics: matchingMetrics
                    });
                }
            }
        }

        return extractedData;
    }

    // Map NLP-generated specs to actual chart configurations with data
    async mapSpecsToCharts(nlpSpecs) {
        console.log('Fetching available sections for mapping...');
        const sections = await this.fetchAvailableSections();
        const mappedCharts = [];

        for (const spec of nlpSpecs) {
            console.log('Finding best match for spec:', spec);
            const chartConfig = await this.findBestMatchingData(spec, sections);
            if (chartConfig) {
                console.log('Found chart config:', chartConfig);
                mappedCharts.push(chartConfig);
            } else {
                console.log('No matching data found, creating placeholder');
                mappedCharts.push(this.createPlaceholderChart(spec));
            }
        }

        return mappedCharts;
    }

    // Map NLP-generated specs using pre-fetched sections
    async mapSpecsToChartsWithSections(nlpSpecs, sections) {
        const mappedCharts = [];
        
        console.log('Mapping', nlpSpecs.length, 'chart specs to actual charts');

        for (const spec of nlpSpecs) {
            console.log('Looking for exact chart match:', JSON.stringify(spec));
            const chartConfig = this.findExactChart(spec.title, sections);
            if (chartConfig) {
                console.log('✓ Found exact chart:', chartConfig.opts?.title);
                mappedCharts.push(chartConfig);
            } else {
                console.log('✗ Chart not found:', spec.title);
                console.log('  Available titles in first section:', Object.values(sections)[0]?.groups?.[0]?.plots?.slice(0, 3).map(p => p.opts?.title));
                // Don't create placeholder - just skip charts that don't exist
            }
        }
        
        console.log('Successfully mapped', mappedCharts.length, 'out of', nlpSpecs.length, 'charts');

        return mappedCharts;
    }

    // Find exact chart by title
    findExactChart(title, sections) {
        for (const [sectionName, sectionData] of Object.entries(sections)) {
            if (!sectionData || !sectionData.groups) continue;

            for (const group of sectionData.groups) {
                if (!group.plots) continue;

                for (const plot of group.plots) {
                    if (!plot.opts?.title || !plot.data || plot.data.length < 2) continue;
                    
                    if (plot.opts.title === title) {
                        // Exact match found - return the complete chart configuration
                        const chartStyle = plot.opts?.style || plot.type || 'line';
                        return {
                            ...plot,
                            type: chartStyle,
                            data: plot.data,
                            opts: {
                                ...plot.opts,
                                style: chartStyle,
                                format: plot.opts?.format || {
                                    y_axis_label: 'Value',
                                    unit_system: null,
                                    log_scale: false
                                }
                            },
                            nlpGenerated: true
                        };
                    }
                }
            }
        }
        
        return null;
    }

    // Find the best matching data for a given NLP spec
    async findBestMatchingData(spec, sections) {
        // Keywords to section mapping - prefer system-level sections
        const sectionKeywords = {
            cpu: ['cpu', 'processor', 'core', 'utilization', 'usage'],
            network: ['network', 'net', 'traffic', 'bandwidth', 'packets', 'latency'],
            scheduler: ['scheduler', 'scheduling', 'runqueue', 'context'],
            syscall: ['syscall', 'system call', 'kernel'],
            softirq: ['softirq', 'interrupt', 'irq'],
            blockio: ['disk', 'block', 'io', 'storage', 'read', 'write'],
            rezolus: ['rezolus', 'monitoring', 'collector'],
            overview: ['system', 'overview', 'general']
        };

        // Determine which section(s) to search based on the spec
        const relevantSections = [];
        const specText = `${spec.title} ${spec.description || ''} ${(spec.metrics || []).join(' ')}`.toLowerCase();

        // Skip cgroups unless explicitly requested
        const explicitlyCgroups = specText.includes('cgroup') || specText.includes('container');
        
        for (const [section, keywords] of Object.entries(sectionKeywords)) {
            if (keywords.some(keyword => specText.includes(keyword))) {
                relevantSections.push(section);
            }
        }

        // If no specific section matched, search all except cgroups
        if (relevantSections.length === 0) {
            relevantSections.push(...Object.keys(sections).filter(s => s !== 'cgroups'));
        }
        
        // Only add cgroups if explicitly requested
        if (explicitlyCgroups && !relevantSections.includes('cgroups')) {
            relevantSections.push('cgroups');
        }

        // Sort sections to prioritize overview and system-level sections
        const sortedSections = relevantSections.sort((a, b) => {
            const priority = { 'overview': 0, 'cpu': 1, 'network': 2, 'blockio': 3, 'scheduler': 4 };
            return (priority[a] ?? 99) - (priority[b] ?? 99);
        });
        
        console.log('Searching sections in order:', sortedSections);
        
        // Look for matching data in relevant sections
        for (const sectionName of sortedSections) {
            const sectionData = sections[sectionName];
            if (!sectionData || !sectionData.groups) continue;

            for (const group of sectionData.groups) {
                if (!group.plots) continue;

                for (const plot of group.plots) {
                    // Skip plots without proper data
                    if (!plot.data || plot.data.length < 2) continue;
                    
                    // Check for exact title match first (since LLM now uses exact titles)
                    const plotTitle = plot.opts?.title || '';
                    if (plotTitle === spec.title) {
                        // Exact match found
                        const chartStyle = plot.opts?.style || plot.type || spec.type || 'line';
                        console.log(`Exact match found:`, plotTitle);
                        return {
                            ...plot,
                            type: chartStyle,
                            data: plot.data,  // Ensure data is preserved
                            opts: {
                                ...plot.opts,
                                style: chartStyle,
                                title: plotTitle,
                                description: spec.description || plot.opts?.description,
                                format: plot.opts?.format || {
                                    y_axis_label: 'Value',
                                    unit_system: null,
                                    log_scale: false
                                }
                            },
                            nlpGenerated: true,
                            matchScore: 1.0
                        };
                    }
                    
                    // Fall back to fuzzy matching if no exact match
                    const score = this.calculateMatchScore(plot, spec);
                    if (score > 0.3) {  // Lower threshold to get more matches
                        // Found a good match - ensure style is set for opts
                        const chartStyle = plot.opts?.style || plot.type || spec.type || 'line';
                        console.log(`Fuzzy match found with score ${score}:`, plot.opts?.title);
                        return {
                            ...plot,
                            type: chartStyle,
                            data: plot.data,  // Ensure data is preserved
                            opts: {
                                ...plot.opts,
                                style: chartStyle,
                                title: spec.title || plot.opts?.title,
                                description: spec.description || plot.opts?.description,
                                format: plot.opts?.format || {
                                    y_axis_label: 'Value',
                                    unit_system: null,
                                    log_scale: false
                                }
                            },
                            nlpGenerated: true,
                            matchScore: score
                        };
                    }
                }
            }
        }

        // If no good match found, return null (caller will create placeholder)
        return null;
    }

    // Calculate how well a plot matches an NLP spec
    calculateMatchScore(plot, spec) {
        let score = 0;
        const plotTitle = (plot.opts?.title || '').toLowerCase();
        const plotText = `${plotTitle} ${plot.opts?.yAxisLabel || ''} ${plot.type || ''}`.toLowerCase();
        const specText = `${spec.title} ${spec.description || ''} ${(spec.metrics || []).join(' ')}`.toLowerCase();

        // Penalize cgroup metrics unless explicitly requested
        if (plotTitle.includes('cgroup') && !specText.includes('cgroup') && !specText.includes('container')) {
            return 0; // Skip cgroup metrics unless explicitly requested
        }

        // Boost system-level metrics
        const isSystemLevel = !plotTitle.includes('cgroup') && !plotTitle.includes('container');
        if (isSystemLevel) {
            score += 0.1;
        }

        // Check for keyword matches - prioritize exact matches
        const specKeywords = specText.split(/\s+/).filter(k => k.length > 2);
        const plotKeywords = plotText.split(/\s+/).filter(k => k.length > 2);
        
        for (const keyword of specKeywords) {
            // Exact match in title gets higher score
            if (plotTitle.includes(keyword)) {
                score += 0.3;
            } else if (plotText.includes(keyword)) {
                score += 0.1;
            }
        }

        // Check for chart type match
        const chartStyle = plot.opts?.style || plot.type || 'line';
        if (chartStyle === spec.type) {
            score += 0.2;
        }

        // Prefer single metrics over complex ones
        if (plot.data && plot.data.length === 2) { // Simple [time, value] structure
            score += 0.1;
        }

        // Match specific metric names if provided
        if (spec.metrics && spec.metrics.length > 0) {
            const firstMetric = spec.metrics[0].toLowerCase();
            if (plotTitle.includes(firstMetric)) {
                score += 0.4;
            }
        }

        return Math.min(score, 1.0);
    }

    // Create a placeholder chart when no data is available
    createPlaceholderChart(spec) {
        const now = Date.now() / 1000; // Convert to seconds for Rezolus format
        const dataPoints = 60;
        const interval = 1; // 1 second
        const chartStyle = spec.type || 'line';

        // Generate sample data in Rezolus format
        const timeData = [];
        const valueData = [];
        
        for (let i = 0; i < dataPoints; i++) {
            const timestamp = now - (dataPoints - i) * interval;
            const value = Math.random() * 100;
            timeData.push(timestamp);
            valueData.push(value);
        }

        // Rezolus expects data as [timeArray, valueArray] for line charts
        const chartData = [timeData, valueData];

        return {
            type: chartStyle,
            data: chartData,
            opts: {
                id: `nlp-placeholder-${Date.now()}`,
                title: spec.title || 'Generated Chart',
                description: spec.description || 'This is a placeholder chart. Real data will be displayed when available.',
                style: chartStyle,  // IMPORTANT: Use 'style' not 'type'
                format: {
                    y_axis_label: 'Value',
                    unit_system: null,
                    log_scale: false
                },
                placeholder: true
            },
            nlpGenerated: true
        };
    }
}