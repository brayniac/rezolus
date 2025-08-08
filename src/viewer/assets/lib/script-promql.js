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
    availableCgroups: [],
    selectedCgroups: [],
    leftSelection: new Set(),
    rightSelection: new Set(),
    loading: true,
    
    async oninit() {
        // Fetch available cgroups from the API
        try {
            const response = await fetch('/api/labels/cgroup_cpu_usage');
            if (response.ok) {
                const data = await response.json();
                if (data.status === 'success' && data.data) {
                    // The API now returns cgroup names directly
                    this.availableCgroups = data.data.sort();
                    // Initialize global state
                    window.allCgroups = [...this.availableCgroups];
                    window.selectedCgroups = [];
                }
            }
        } catch (error) {
            console.error('Failed to fetch cgroups:', error);
        }
        this.loading = false;
        m.redraw();
    },
    
    moveToSelected() {
        const toMove = Array.from(this.leftSelection);
        this.selectedCgroups = [...this.selectedCgroups, ...toMove].sort();
        this.availableCgroups = this.availableCgroups.filter(cg => !this.leftSelection.has(cg));
        this.leftSelection.clear();
        // Reset the select element's selection
        const leftSelect = document.querySelector('.selector-column select');
        if (leftSelect) leftSelect.selectedIndex = -1;
        this.updateCharts();
    },
    
    moveToAvailable() {
        const toMove = Array.from(this.rightSelection);
        this.availableCgroups = [...this.availableCgroups, ...toMove].sort();
        this.selectedCgroups = this.selectedCgroups.filter(cg => !this.rightSelection.has(cg));
        this.rightSelection.clear();
        // Reset the select element's selection
        const rightSelect = document.querySelectorAll('.selector-column select')[1];
        if (rightSelect) rightSelect.selectedIndex = -1;
        this.updateCharts();
    },
    
    updateCharts() {
        // Store cgroups in global state for access by charts
        window.selectedCgroups = this.selectedCgroups;
        window.allCgroups = [...this.availableCgroups, ...this.selectedCgroups];
        console.log('Updated selected cgroups:', window.selectedCgroups);
        // Force all PromQL charts to re-execute queries
        // Clear all existing charts to force re-initialization
        chartsState.clear();
        // We need to trigger a full re-render of the section content
        m.redraw();
    },
    
    view({
        attrs
    }) {
        return m("div#cgroups-controls", [
            m("div.cgroup-selector", [
                m("h3", "Cgroup Selector"),
                m("div.selector-columns", [
                    m("div.selector-column", [
                        m("h4", "Available Cgroups (Summed)"),
                        m("select.cgroup-list[multiple]", {
                            size: 10,
                            onchange: (e) => {
                                this.leftSelection.clear();
                                Array.from(e.target.selectedOptions).forEach(opt => {
                                    this.leftSelection.add(opt.value);
                                });
                            }
                        }, 
                            this.loading ? 
                                m("option[disabled]", "Loading...") :
                                this.availableCgroups.map(cg => 
                                    m("option", { 
                                        key: `avail-${cg}`,
                                        value: cg,
                                        selected: this.leftSelection.has(cg)
                                    }, cg)
                                )
                        )
                    ]),
                    m("div.selector-buttons", [
                        m("button", {
                            onclick: () => this.moveToSelected(),
                            disabled: this.leftSelection.size === 0
                        }, "→"),
                        m("button", {
                            onclick: () => this.moveToAvailable(),
                            disabled: this.rightSelection.size === 0
                        }, "←")
                    ]),
                    m("div.selector-column", [
                        m("h4", "Selected Cgroups (Individual)"),
                        m("select.cgroup-list[multiple]", {
                            size: 10,
                            onchange: (e) => {
                                this.rightSelection.clear();
                                Array.from(e.target.selectedOptions).forEach(opt => {
                                    this.rightSelection.add(opt.value);
                                });
                            }
                        },
                            this.selectedCgroups.length === 0 ?
                                m("option[disabled]", "No cgroups selected") :
                                this.selectedCgroups.map(cg => 
                                    m("option", { 
                                        key: `sel-${cg}`,
                                        value: cg,
                                        selected: this.rightSelection.has(cg)
                                    }, cg)
                                )
                        )
                    ])
                ])
            ]),
            m("div.cgroup-options", [
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
        this.section = vnode.attrs.section;
        this.lastCgroups = null;
        this.chartKey = Math.random(); // Unique key for this chart instance
        this.executeQueries(vnode.attrs.panel);
    },
    
    onbeforeupdate(vnode) {
        // Always check if cgroups selection changed before update
        if (this.section === 'cgroups') {
            const currentCgroups = window.selectedCgroups || [];
            const lastCgroups = this.lastCgroups || [];
            
            // Check if cgroups selection changed
            if (currentCgroups.length !== lastCgroups.length || 
                !currentCgroups.every((cg, i) => cg === lastCgroups[i])) {
                console.log(`Panel ${vnode.attrs.panel.id}: Cgroups changed, re-executing queries`);
                this.lastCgroups = [...currentCgroups];
                this.loading = true;
                this.executeQueries(vnode.attrs.panel);
            }
        }
        return true; // Always allow update
    },
    
    async executeQueries(panel) {
        try {
            console.log(`Executing queries for panel ${panel.id}:`, panel.queries);
            
            // Check if we're on the cgroups page
            let allCgroups = null;
            let selectedCgroups = null;
            
            if (this.section === 'cgroups') {
                // Get all available cgroups from global state
                allCgroups = window.allCgroups || [];
                selectedCgroups = window.selectedCgroups || [];
            }
            
            const results = await Promise.all(
                panel.queries.map(query => {
                    // Build query parameters
                    const queryParams = { query: query.expr };
                    
                    // For cgroup panels, pass filter parameters to backend
                    if (this.section === 'cgroups' && query.expr.includes('{{CGROUP_FILTER}}')) {
                        // Add selected cgroups as comma-separated list
                        if (selectedCgroups && selectedCgroups.length > 0) {
                            queryParams.selected_cgroups = selectedCgroups.join(',');
                        }
                        // Add filter type from panel options
                        if (panel.options?.cgroup_filter) {
                            queryParams.cgroup_filter = panel.options.cgroup_filter;
                        }
                        
                        console.log(`Panel ${panel.id}: Sending template query with params:`, queryParams);
                    }
                    
                    return m.request({
                        method: "GET",
                        url: `/api/query`,
                        params: queryParams,
                        withCredentials: true,
                    });
                })
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
                    if (labels.name) {
                        // For cgroups, use the name label
                        return labels.name;
                    } else if (labels.id) {
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
                // Single series - but check if panel type is 'multi'
                const series = response.data.result[0];
                const allTimes = [];
                const allValues = [];
                
                series.values.forEach(([timestamp, value]) => {
                    allTimes.push(timestamp);
                    allValues.push(parseFloat(value));
                });
                
                // For multi panels, always format as multi-series even with one series
                if (panel.type === 'multi') {
                    // Extract series name from labels
                    const labels = series.metric;
                    let seriesName = 'Series';
                    if (labels.name) {
                        seriesName = labels.name;
                    } else if (labels.id) {
                        seriesName = `CPU ${labels.id}`;
                    } else if (labels.state) {
                        seriesName = labels.state;
                    } else if (labels.direction) {
                        seriesName = labels.direction;
                    }
                    
                    this.seriesNames = [seriesName];
                    this.multiSeriesData = [allValues];
                    
                    // Return in multi-series format: [timestamps, series1]
                    return [allTimes, allValues];
                } else {
                    // Regular single series
                    this.multiSeriesData = null;
                    this.seriesNames = null;
                    
                    return [allTimes, allValues];
                }
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
        
        // If no data, create empty chart data to maintain consistent layout
        if (!this.data) {
            console.warn(`No data for panel ${panel.id}, showing empty chart`);
            // Create minimal empty data structure based on panel type
            if (panel.type === 'heatmap') {
                // Empty heatmap structure
                this.data = {
                    value_data: [],
                    time_data: [],
                    heatmap_data: []
                };
            } else if (panel.type === 'scatter' && panel.queries && panel.queries.length > 1) {
                // Empty multi-series structure for scatter charts
                this.data = [];
                this.multiSeriesData = [];
                this.seriesNames = panel.queries.map(q => q.legend || 'Series');
            } else {
                // Empty line chart structure
                this.data = [[], []];  // Empty time and value arrays
            }
        }
        
        // Create a spec compatible with the existing Chart component
        // Map panel types to chart styles
        let chartStyle = panel.type || 'line';
        if (chartStyle === 'stat' || chartStyle === 'gauge') {
            // Stat and gauge panels should be rendered as line charts
            chartStyle = 'line';
        } else if (chartStyle === 'multi') {
            // Multi panels should use the multi style for proper colors
            chartStyle = 'multi';
        }
        
        // Build spec based on chart type
        let spec;
        if (panel.type === 'heatmap') {
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
            // Check if we have multiple series or if panel type is multi
            const hasMultipleSeries = this.multiSeriesData && this.multiSeriesData.length > 1;
            const isMultiPanel = panel.type === 'multi';
            
            if (hasMultipleSeries || isMultiPanel) {
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
        
        // Use a key that changes when data changes to force chart re-creation
        const dataKey = this.data ? JSON.stringify(this.data).substring(0, 100) : 'empty';
        return m(Chart, { 
            key: `${panel.id}-${dataKey}`,
            spec, 
            chartsState: vnode.attrs.chartsState 
        });
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