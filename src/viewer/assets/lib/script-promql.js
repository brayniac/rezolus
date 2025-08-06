import {
    ChartsState, Chart
} from './charts/chart.js';

// Sidebar component
const Sidebar = {
    view({
        attrs
    }) {
        return m("div#sidebar", [
            attrs.sections.map((section) => m(m.route.Link, {
                class: attrs.activeSection === section.name.toLowerCase() ? 'selected' : '',
                href: `/${section.name.toLowerCase()}`,
            }, section.name)),
            m("hr", { style: "margin: 1rem 0; border: none; border-top: 1px solid #333;" }),
            m("a", {
                href: "/query",
                style: "display: block; padding: 0.5rem 1rem; color: #4a9eff; text-decoration: none; font-weight: 500;"
            }, "→ Query Explorer")
        ]);
    }
};

// Main component
const Main = {
    view({
        attrs: {
            activeSection,
            dashboard,
            sections,
            source,
            version,
            filename
        }
    }) {
        return m("div",
            m("header", [
                m('h1', 'Rezolus', m('span.div', ' » '), dashboard ? dashboard.name : 'Loading...'),
                m('div.metadata', [
                    m('p.filename', `File: ${filename || 'Loading...'}`),
                    m('p.version', `Source: ${source || 'Rezolus'} • Version: ${version || 'unknown'}`),
                ]),
            ]),
            m("main", [
                m(Sidebar, {
                    activeSection,
                    sections
                }),
                m(SectionContent, {
                    section: activeSection,
                    dashboard
                })
            ]));
    }
};

const SectionContent = {
    view({
        attrs
    }) {
        if (!attrs.dashboard) {
            return m("div#section-content", m("div.loading", "Loading dashboard..."));
        }
        
        return m("div#section-content", [
            attrs.section === "cgroups" ? m(CgroupsControls) : undefined,
            m("div#groups",
                attrs.dashboard.groups.map((group) => m(Group, { 
                    ...group,
                    section: attrs.section 
                }))
            )
        ]);
    }
};

const CgroupsControls = {
    view({
        attrs
    }) {
        return m("div#cgroups-controls", [
            m("label.checkbox", [
                m("input[type=checkbox]", {
                    checked: chartsState.colorMapper.getUseConsistentCgroupColors(),
                    onchange: (e) => {
                        chartsState.colorMapper.setUseConsistentCgroupColors(e.target.checked);
                        // All cgroups section charts need to be reinitialized
                        chartsState.charts.forEach(chart => chart.isInitialized() && chart.reinitialize());
                    }
                }),
                "Keep cgroup colors consistent across charts"
            ])
        ]);
    }
};

// Group component
const Group = {
    view({
        attrs
    }) {
        return m("div.group", {
            id: attrs.id
        }, [
            m("h2", `${attrs.name}`),
            m("div.charts", attrs.panels.map(panel => m(PromQLChart, { 
                panel, 
                chartsState,
                section: attrs.section 
            }))),
        ]);
    }
};

// PromQL Chart component that executes queries
const PromQLChart = {
    oninit(vnode) {
        this.data = null;
        this.loading = true;
        this.error = null;
        this.executeQueries(vnode.attrs.panel);
    },
    
    async executeQueries(panel) {
        try {
            console.log(`Executing queries for panel ${panel.id}:`, panel.queries);
            const results = await Promise.all(
                panel.queries.map(query => 
                    m.request({
                        method: "GET",
                        url: `/api/query`,
                        params: { query: query.expr },
                        withCredentials: true,
                    })
                )
            );
            
            console.log(`Query results for ${panel.id}:`, results);
            
            // Transform PromQL results to chart format
            this.data = this.transformResults(panel, results);
            console.log(`Transformed data for ${panel.id}:`, this.data);
            this.loading = false;
            m.redraw();
        } catch (error) {
            console.error(`Failed to execute query for ${panel.id}:`, error);
            this.error = error.message;
            this.loading = false;
            m.redraw();
        }
    },
    
    transformResults(panel, results) {
        // Transform PromQL query results to the format expected by the chart component
        
        // Special handling for heatmaps
        if (panel.type === 'heatmap') {
            return this.transformHeatmapResults(results);
        }
        
        // Process the results
        if (!results || results.length === 0) {
            console.warn('No results to transform');
            return null;
        }
        
        // If we have multiple queries (common for scatter plots with multiple percentiles)
        if (results.length > 1) {
            // Combine all query results into a single multi-series format
            const allSeries = [];
            const allSeriesNames = [];
            
            results.forEach((response, queryIndex) => {
                if (response.status === 'success' && response.data && response.data.resultType === 'matrix') {
                    // Each query should return one series for percentiles
                    if (response.data.result.length > 0) {
                        const series = response.data.result[0]; // Take first series from each query
                        allSeries.push(series);
                        // Use the legend from the query definition
                        const legendName = panel.queries[queryIndex].legend || `Series ${queryIndex + 1}`;
                        allSeriesNames.push(legendName);
                    }
                }
            });
            
            if (allSeries.length > 0) {
                // Collect all unique timestamps
                const timestampSet = new Set();
                allSeries.forEach(series => {
                    series.values.forEach(([timestamp, _]) => {
                        timestampSet.add(timestamp);
                    });
                });
                
                // Sort timestamps
                const timestamps = Array.from(timestampSet).sort((a, b) => a - b);
                
                // Create a map for each series' values
                const seriesData = allSeries.map(series => {
                    const valueMap = new Map();
                    series.values.forEach(([timestamp, value]) => {
                        valueMap.set(timestamp, parseFloat(value));
                    });
                    
                    // Create aligned data array
                    return timestamps.map(ts => valueMap.get(ts) || null);
                });
                
                this.seriesNames = allSeriesNames;
                this.multiSeriesData = seriesData;
                
                // Format: [timestamps, series1, series2, ...]
                return [timestamps, ...seriesData];
            }
        }
        
        // Single query - process as before
        const response = results[0];
        if (response.status !== 'success' || !response.data) {
            console.warn('Response not successful:', response);
            return null;
        }
        
        if (response.data.resultType === 'matrix') {
            if (response.data.result.length > 1) {
                // Multiple series - format for multi-series chart
                // First, collect all unique timestamps
                const timestampSet = new Set();
                response.data.result.forEach(series => {
                    series.values.forEach(([timestamp, _]) => {
                        timestampSet.add(timestamp);
                    });
                });
                
                // Sort timestamps
                const timestamps = Array.from(timestampSet).sort((a, b) => a - b);
                
                // Create a map for each series' values
                const seriesData = response.data.result.map(series => {
                    const valueMap = new Map();
                    series.values.forEach(([timestamp, value]) => {
                        valueMap.set(timestamp, parseFloat(value));
                    });
                    
                    // Create aligned data array
                    return timestamps.map(ts => valueMap.get(ts) || 0);
                });
                
                // Extract series names from labels
                this.seriesNames = response.data.result.map(series => {
                    // Try to create a meaningful name from labels
                    const labels = series.metric;
                    if (labels.id) {
                        return `CPU ${labels.id}`;
                    } else if (labels.state) {
                        return labels.state;
                    } else if (labels.direction) {
                        return labels.direction;
                    } else {
                        return 'Series';
                    }
                });
                
                // Store for checking if multi-series
                this.multiSeriesData = seriesData;
                
                // Format: [timestamps, series1, series2, ...]
                return [timestamps, ...seriesData];
                
            } else if (response.data.result.length === 1) {
                // Single series
                const series = response.data.result[0];
                const allTimes = [];
                const allValues = [];
                
                series.values.forEach(([timestamp, value]) => {
                    allTimes.push(timestamp);
                    allValues.push(parseFloat(value));
                });
                
                this.multiSeriesData = null;
                this.seriesNames = null;
                
                return [allTimes, allValues];
            }
        } else if (response.data.resultType === 'vector') {
            // Handle instant queries
            if (response.data.result.length > 0) {
                // Sum all vector results for now
                let timestamp = response.data.result[0].value[0];
                let sum = 0;
                response.data.result.forEach(item => {
                    sum += parseFloat(item.value[1]);
                });
                
                this.multiSeriesData = null;
                this.seriesNames = null;
                
                return [[timestamp], [sum]];
            }
        }
        
        return null;
    },
    
    transformHeatmapResults(results) {
        // Heatmaps need a different data format
        // Expected: { time_data: [], data: [[timeIndex, cpuId, value], ...], min_value, max_value }
        
        if (!results || results.length === 0) return null;
        
        const response = results[0];
        if (response.status !== 'success' || !response.data) return null;
        
        // Collect all unique timestamps
        const timeSet = new Set();
        const timeToIndex = new Map();
        const heatmapData = [];
        let minValue = Infinity;
        let maxValue = -Infinity;
        
        // Process each CPU's time series
        response.data.result.forEach((series, cpuId) => {
            // Extract CPU ID from labels if available
            let actualCpuId = cpuId;
            if (series.metric && series.metric.id) {
                actualCpuId = parseInt(series.metric.id);
            }
            
            series.values.forEach(([timestamp, value]) => {
                timeSet.add(timestamp);
                const floatValue = parseFloat(value);
                minValue = Math.min(minValue, floatValue);
                maxValue = Math.max(maxValue, floatValue);
            });
        });
        
        // Create time array and index mapping
        const timeData = Array.from(timeSet).sort((a, b) => a - b);
        timeData.forEach((time, index) => {
            timeToIndex.set(time, index);
        });
        
        // Build heatmap data array
        response.data.result.forEach((series, cpuId) => {
            let actualCpuId = cpuId;
            if (series.metric && series.metric.id) {
                actualCpuId = parseInt(series.metric.id);
            }
            
            series.values.forEach(([timestamp, value]) => {
                const timeIndex = timeToIndex.get(timestamp);
                const floatValue = parseFloat(value);
                heatmapData.push([timeIndex, actualCpuId, floatValue]);
            });
        });
        
        return {
            time_data: timeData,
            data: heatmapData,
            min_value: minValue,
            max_value: maxValue
        };
    },
    
    view(vnode) {
        const { panel } = vnode.attrs;
        
        if (this.loading) {
            return m("div.chart-container", {
                style: "height: 300px; display: flex; align-items: center; justify-content: center;"
            }, m("div.loading", "Loading..."));
        }
        
        if (this.error) {
            return m("div.chart-container", {
                style: "height: 300px; display: flex; align-items: center; justify-content: center;"
            }, m("div.error", `Error: ${this.error}`));
        }
        
        // Check if we have data
        if (!this.data) {
            console.warn(`No data for panel ${panel.id}`);
            return m("div.chart-container", {
                style: "height: 300px; display: flex; align-items: center; justify-content: center;"
            }, m("div.error", "No data available"));
        }
        
        // Create a spec compatible with the existing Chart component
        // Map panel types to chart styles
        let chartStyle = panel.type || 'line';
        if (chartStyle === 'stat' || chartStyle === 'gauge') {
            // Stat and gauge panels should be rendered as line charts
            chartStyle = 'line';
        }
        
        // Build spec based on chart type
        let spec;
        if (panel.type === 'heatmap' && this.data) {
            // Heatmap spec is different - it contains the data directly
            spec = {
                opts: {
                    title: panel.title,
                    id: panel.id,
                    style: 'heatmap',
                    format: {
                        unit_system: panel.unit.toLowerCase(),
                        precision: 2
                    }
                },
                ...this.data  // Spread the heatmap data structure
            };
        } else {
            // Check if we have multiple series
            const hasMultipleSeries = this.multiSeriesData && this.multiSeriesData.length > 1;
            
            if (hasMultipleSeries) {
                // Use appropriate style based on panel type
                let multiStyle = 'multi';
                if (panel.type === 'scatter') {
                    multiStyle = 'scatter';
                }
                
                spec = {
                    opts: {
                        title: panel.title,
                        id: panel.id,
                        style: multiStyle,
                        format: {
                            unit_system: panel.unit.toLowerCase(),
                            precision: 2,
                            log_scale: panel.options?.log_scale,
                            percentile_labels: this.seriesNames
                        }
                    },
                    data: this.data || [],
                    series_names: this.seriesNames || ['Series']
                };
            } else {
                // Regular single-series chart
                spec = {
                    opts: {
                        title: panel.title,
                        id: panel.id,
                        style: chartStyle,
                        format: {
                            unit_system: panel.unit.toLowerCase(),
                            precision: 2
                        }
                    },
                    data: this.data || [],
                    series_names: panel.queries.map(q => q.legend || 'Series')
                };
            }
        }
        
        console.log(`Rendering chart ${panel.id} with spec:`, spec);
        
        return m(Chart, { spec, chartsState: vnode.attrs.chartsState });
    }
};

// Application state management
const chartsState = new ChartsState();

// Cache for dashboard definitions
const dashboardCache = {};
let sectionsCache = null;
let metadataCache = null;

// Fetch list of available dashboards
async function fetchDashboards() {
    if (!sectionsCache) {
        try {
            const response = await m.request({
                method: "GET",
                url: "/api/dashboards",
                withCredentials: true,
            });
            
            if (response.status === 'success') {
                // Transform to match expected format
                sectionsCache = response.data.map(item => ({
                    name: item.title,
                    route: `/${item.name}`
                }));
            }
        } catch (error) {
            console.error("Failed to fetch dashboards:", error);
            sectionsCache = [];
        }
    }
    return sectionsCache;
}

// Fetch dashboard definition
async function fetchDashboard(name) {
    if (!dashboardCache[name]) {
        try {
            console.log(`Fetching dashboard: ${name}`);
            const response = await m.request({
                method: "GET",
                url: `/api/dashboard/${name}`,
                withCredentials: true,
            });
            
            console.log(`Dashboard response for ${name}:`, response);
            
            if (response.status === 'success') {
                dashboardCache[name] = response.data;
            }
        } catch (error) {
            console.error(`Failed to fetch dashboard ${name}:`, error);
            return null;
        }
    }
    return dashboardCache[name];
}

// Fetch metadata (source, version, filename)
async function fetchMetadata() {
    if (!metadataCache) {
        try {
            // Try to get metadata from the overview endpoint first
            const response = await m.request({
                method: "GET",
                url: "/data/overview.json",
                withCredentials: true,
            });
            
            metadataCache = {
                source: response.source || 'Rezolus',
                version: response.version || 'unknown',
                filename: response.filename || 'metrics.parquet'
            };
        } catch (error) {
            console.error("Failed to fetch metadata:", error);
            metadataCache = {
                source: 'Rezolus',
                version: 'unknown',
                filename: 'metrics.parquet'
            };
        }
    }
    return metadataCache;
}

// Main application entry point
m.route.prefix = ""; // use regular paths for navigation
m.route(document.body, "/overview", {
    "/:section": {
        async onmatch(params, requestedPath) {
            // Prevent a route change if we're already on this route
            if (m.route.get() === requestedPath) {
                return new Promise(function () { });
            }

            if (requestedPath !== m.route.get()) {
                // Reset charts state
                chartsState.clear();
                
                // Reset scroll position
                window.scrollTo(0, 0);
            }

            try {
                // Fetch all necessary data
                const [sections, dashboard, metadata] = await Promise.all([
                    fetchDashboards().catch(err => {
                        console.error('Failed to fetch dashboards:', err);
                        return [];
                    }),
                    fetchDashboard(params.section).catch(err => {
                        console.error('Failed to fetch dashboard:', err);
                        return null;
                    }),
                    fetchMetadata().catch(err => {
                        console.error('Failed to fetch metadata:', err);
                        return { source: 'Rezolus', version: 'unknown', filename: 'metrics.parquet' };
                    })
                ]);

                return {
                    view() {
                        return m(Main, {
                            activeSection: params.section,
                            dashboard,
                            sections,
                            ...metadata
                        });
                    }
                };
            } catch (error) {
                console.error('Route loading error:', error);
                // Return a simple error view
                return {
                    view() {
                        return m('div', 'Error loading dashboard');
                    }
                };
            }
        }
    }
});