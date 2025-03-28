// --------------------------------------------------------
// Line and Scatter Chart Implementation
// --------------------------------------------------------

// Create a line or scatter chart based on chart type with improved responsiveness
function createLineOrScatterChart(containerId, seriesData, containerWidth, groupName) {
    const container = document.getElementById(containerId);
    if (!container) {
        console.error(`Container element with ID ${containerId} not found`);
        return null;
    }
    
    const isScatter = seriesData.chart_type === 'scatter';
    
    // Create tooltip plugin
    const tooltipPlugin = createTooltipPlugin();
    
    // Create series options
    const seriesOptions = createSeriesOptions(seriesData);
    
    // Create data array
    const data = createDataArray(seriesData.timestamps, seriesData.series);
    
    // Create static legend plugin
    const staticLegend = createStaticLegendPlugin(seriesData.series);
    
    // Improved responsive handling
    const responsivePlugin = {
        hooks: {
            setSize: [
                (u) => {
                    // When size changes, ensure the axes are updated
                    u.setScale("x", {
                        min: u.scales.x.min,
                        max: u.scales.x.max,
                        auto: false
                    });
                    
                    // Force redraw of all elements
                    setTimeout(() => u.redraw(), 0);
                }
            ]
        }
    };
    
    const opts = {
        width: containerWidth,
        height: 350,
        gutters: {
            y: 15,
            x: 8
        },
        scales: {
            x: {
                time: false,
                auto: false // Prevent auto-scaling on resize
            }
        },
        title: seriesData.title,
        
        background: '#1E1E1E',
        
        plugins: [
            tooltipPlugin,
            staticLegend,
            responsivePlugin
        ],
        
        select: {
            show: true,
            stroke: 'rgba(86, 156, 214, 0.5)',
            fill: 'rgba(86, 156, 214, 0.2)',
        },
        
        hooks: {
            setSelect: [],
            ready: [
                (u) => {
                    // Force the canvas to use the container's width
                    const resizeObserver = new ResizeObserver(entries => {
                        for (let entry of entries) {
                            const width = entry.contentRect.width;
                            if (Math.abs(u.width - width) > 5) {
                                u.setSize({
                                    width: width,
                                    height: u.height
                                });
                            }
                        }
                    });
                    
                    // Observe the container for size changes
                    resizeObserver.observe(container);
                    
                    // Store the observer for cleanup
                    u._observer = resizeObserver;
                }
            ],
            destroy: [
                (u) => {
                    // Clean up the observer when the plot is destroyed
                    if (u._observer) {
                        u._observer.disconnect();
                    }
                }
            ]
        },
        
        series: seriesOptions,
        
        axes: [
            {
                stroke: '#888888',
                grid: {
                    stroke: 'rgba(136, 136, 136, 0.1)',
                },
                values: createTimeAxisFormatter(),
                space: 60, // More space for time labels
                ticks: {
                    show: true,
                    stroke: 'rgba(136, 136, 136, 0.5)',
                    width: 1,
                    size: 5,
                },
                splits: createTimeSplitsFn()
            },
            {
                label: seriesData.yUnits ? seriesData.yUnits : '',
                stroke: '#888888',
                grid: {
                    stroke: 'rgba(136, 136, 136, 0.1)',
                },
            }
        ],
        
        cursor: {
            focus: {
                prox: 16,
            },
            points: {
                size: 6,
                width: 2,
                fill: "#1E1E1E",
            },
            lock: false,
            sync: {
                key: groupName, // Sync cursors within the same group
                scales: ['x']
            }
        },
        
        legend: {
            show: false  // We'll use our custom static legend instead
        }
    };
    
    // Special settings for scatter plot
    if (isScatter) {
        // Modify each series to use points only, no lines
        opts.series.forEach((s, i) => {
            if (i > 0) { // Skip the first (timestamp) series
                s.paths = false; // Disable lines
                s.points = {
                    show: true,
                    size: 6,
                    stroke: s.stroke || "#FFFFFF",
                    width: 2,
                    fill: "#1E1E1E"
                };
            }
        });
    }
    
    return new uPlot(opts, data, document.getElementById(containerId));
}

// Updated series creation for the uPlot options
function createSeriesOptions(seriesData) {
    const isScatter = seriesData.chart_type === 'scatter';
    
    // First series is always time
    const seriesOptions = [
        {
            label: 'Time'
        }
    ];
    
    // Add each data series
    seriesData.series.forEach(s => {
        seriesOptions.push({
            label: s.name,
            stroke: s.color,
            width: 2,
            fill: `rgba(${hexToRgb(s.color)}, 0.1)`,
            // For scatter plots, show points instead of lines
            ...(isScatter ? {
                paths: false,
                points: {
                    show: true,
                    size: 6,
                    stroke: s.color,
                    width: 2,
                    fill: "#1E1E1E"
                }
            } : {})
        });
    });
    
    return seriesOptions;
}

// Create a plugin for the static series legend
function createStaticLegendPlugin(seriesArray) {
    let legendEl;
    
    function init(u, opts) {
        // Create legend container
        legendEl = document.createElement('div');
        legendEl.className = 'static-legend';
        
        // Add an item for each series
        seriesArray.forEach(series => {
            const item = document.createElement('div');
            item.className = 'static-legend-item';
            
            const colorDot = document.createElement('span');
            colorDot.className = 'static-legend-color';
            colorDot.style.backgroundColor = series.color;
            
            const labelText = document.createElement('span');
            labelText.textContent = series.name;
            
            item.appendChild(colorDot);
            item.appendChild(labelText);
            legendEl.appendChild(item);
        });
        
        // Add legend below the plot
        u.root.appendChild(legendEl);
    }
    
    return {
        hooks: {
            init: init
        }
    };
}

// Create a tooltip plugin for line and scatter charts
function createTooltipPlugin() {
    let tooltipEl;
    
    function init(u) {
        tooltipEl = document.createElement('div');
        tooltipEl.className = 'u-tooltip';
        u.over.appendChild(tooltipEl);
    }
    
    function update(u) {
        if (!u.cursor.idx) {
            tooltipEl.style.display = 'none';
            return;
        }
        
        // Get timestamp at cursor position
        const timestamp = u.data[0][u.cursor.idx];
        const formattedDate = formatDateTimeISO(timestamp);
        
        // Collect all series values
        const seriesValues = [];
        for (let i = 1; i < u.series.length; i++) {
            const series = u.series[i];
            if (!series.show) continue;
            
            const value = u.data[i][u.cursor.idx];
            
            // Skip null or undefined values
            if (value === null || value === undefined) continue;
            
            // Format the value with appropriate precision
            const formattedValue = Math.abs(value) < 0.01 
                ? value.toFixed(4) 
                : value.toLocaleString(undefined, {
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2
                  });
            
            seriesValues.push({
                name: series.label || `Series ${i}`,
                value: formattedValue,
                color: series.stroke,
                rawValue: value
            });
        }
        
        // Sort values in descending order
        seriesValues.sort((a, b) => b.rawValue - a.rawValue);
        
        // Create HTML content
        let html = `<div class="header">${formattedDate}</div>`;
        
        seriesValues.forEach(sv => {
            html += `
                <div class="value-row">
                    <span class="label">
                        <span class="color-dot" style="background-color: ${sv.color};"></span>
                        ${sv.name}:
                    </span>
                    <span class="value">${sv.value}</span>
                </div>
            `;
        });
        
        tooltipEl.innerHTML = html;
        tooltipEl.style.display = 'block';
        
        // Position the tooltip
        const rect = u.over.getBoundingClientRect();
        const cursorLeft = u.cursor.left;
        const cursorTop = u.cursor.top;
        
        let left = cursorLeft + 10;
        let top = cursorTop + 10;
        
        // Make sure tooltip stays within plot
        const tooltipRect = tooltipEl.getBoundingClientRect();
        if (left + tooltipRect.width > rect.width) {
            left = cursorLeft - tooltipRect.width - 10;
        }
        
        if (top + tooltipRect.height > rect.height) {
            top = cursorTop - tooltipRect.height - 10;
        }
        
        tooltipEl.style.left = left + 'px';
        tooltipEl.style.top = top + 'px';
    }
    
    return {
        hooks: {
            init: init,
            setCursor: update
        }
    };
}