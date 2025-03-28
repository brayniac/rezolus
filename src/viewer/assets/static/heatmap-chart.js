// --------------------------------------------------------
// CPU Heatmap Implementation
// --------------------------------------------------------

/**
 * Creates a heatmap chart for per-CPU metrics visualization with improved responsiveness
 * @param {string} containerId - DOM element ID to place the chart
 * @param {Object} data - Data for the heatmap including timestamps and per-CPU values
 * @param {Object} options - Chart configuration options
 * @returns {uPlot} The created uPlot instance
 */
function createCpuHeatmap(containerId, data, options = {}) {
    const container = document.getElementById(containerId);
    if (!container) {
        console.error(`Container element with ID ${containerId} not found`);
        return null;
    }
    
    const defaults = {
        title: "CPU Heatmap",
        width: container.clientWidth,
        height: 400,
        colorScale: ["#100060", "#4000A0", "#8000C0", "#A000E0", "#C000FF", "#FF2000", "#FF6000", "#FFA000", "#FFE000"],
        minValue: 0,
        maxValue: 100,
        units: "%"
    };
    
    const config = {...defaults, ...options};
    
    // Extract data structure
    const { timestamps, cpuData } = data;
    const numCPUs = cpuData.length;
    
    // Create canvas for the heatmap
    const heatmapCanvas = document.createElement('canvas');
    heatmapCanvas.className = 'cpu-heatmap-canvas';
    heatmapCanvas.style.position = 'absolute';
    heatmapCanvas.style.top = '0';
    heatmapCanvas.style.left = '0';
    
    // Create a separate container for the legend
    const legendContainer = document.createElement('div');
    legendContainer.className = 'heatmap-legend-container';
    legendContainer.style.position = 'absolute';
    legendContainer.style.top = '10px';
    legendContainer.style.right = '10px';
    legendContainer.style.zIndex = '5';
    container.appendChild(legendContainer);
    
    // Store current view state
    let currentViewMin = timestamps[0];
    let currentViewMax = timestamps[timestamps.length - 1];
    let isCustomZoom = false;
    
    // Create drawing functions for uPlot
    function drawHeatmap(u) {
        if (!u.data || !u.data[0] || u.data[0].length === 0) return;
        
        const ctx = heatmapCanvas.getContext('2d');
        const width = u.width;
        const height = u.height;
        
        // Set canvas dimensions
        heatmapCanvas.width = width;
        heatmapCanvas.height = height;
        
        // Clear canvas
        ctx.clearRect(0, 0, width, height);
        
        // Get pixel dimensions
        const dataStartX = u.bbox.left;
        const dataEndX = u.bbox.left + u.bbox.width;
        const dataWidth = u.bbox.width;
        const dataStartY = u.bbox.top;
        const dataHeight = u.bbox.height;
        
        // Calculate cell size
        const cellHeight = dataHeight / numCPUs;
        
        // Get visible time range
        const visibleStartTime = u.scales.x.min;
        const visibleEndTime = u.scales.x.max;
        
        // Update the current view state
        currentViewMin = visibleStartTime;
        currentViewMax = visibleEndTime;
        
        // Find visible data range
        let startIdx = 0;
        let endIdx = timestamps.length - 1;
        
        for (let i = 0; i < timestamps.length; i++) {
            if (timestamps[i] >= visibleStartTime) {
                startIdx = i > 0 ? i - 1 : 0;
                break;
            }
        }
        
        for (let i = startIdx; i < timestamps.length; i++) {
            if (timestamps[i] > visibleEndTime) {
                endIdx = i;
                break;
            }
        }
        
        // Get interpolation function for color scale
        const getColor = createColorInterpolator(config.colorScale, config.minValue, config.maxValue);
        
        // Draw each CPU row (in reverse order, highest CPU number at the top)
        for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
            // Reverse the order - CPU0 at bottom, highest CPU at top
            const reversedCpuIdx = numCPUs - cpuIdx - 1;
            const cpuValues = cpuData[cpuIdx];
            
            // Y position for this CPU (reversed)
            const yPos = dataStartY + reversedCpuIdx * cellHeight;
            
            // Draw cells for this CPU
            for (let i = startIdx; i < endIdx; i++) {
                const time1 = timestamps[i];
                const time2 = timestamps[i + 1];
                
                if (time1 > visibleEndTime || time2 < visibleStartTime) continue;
                
                // Get value and color
                const value = cpuValues[i];
                const color = getColor(value);
                
                // Calculate pixel positions
                const x1 = u.valToPos(time1, 'x');
                const x2 = u.valToPos(time2, 'x');
                
                // Draw rectangle
                ctx.fillStyle = color;
                ctx.fillRect(x1, yPos, x2 - x1, cellHeight);
                
                // Add border if cell is large enough
                if (x2 - x1 > 4) {
                    ctx.strokeStyle = 'rgba(0,0,0,0.1)';
                    ctx.lineWidth = 0.5;
                    ctx.strokeRect(x1, yPos, x2 - x1, cellHeight);
                }
            }
        }
        
        // Add CPU labels
        ctx.textAlign = 'left';
        ctx.textBaseline = 'middle';
        ctx.font = '10px sans-serif';
        ctx.fillStyle = '#CCCCCC';
        
        for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
            // Reverse the order - CPU0 at bottom, highest CPU at top
            const reversedCpuIdx = numCPUs - cpuIdx - 1;
            const yPos = dataStartY + (reversedCpuIdx + 0.5) * cellHeight;
            ctx.fillText(`CPU ${cpuIdx}`, 5, yPos);
        }
        
        // Draw the legend
        drawLegend();
    }
    
    // Create a legend for the heatmap (now rendered in its own container)
    function drawLegend() {
        // Clear previous legend if any
        while (legendContainer.firstChild) {
            legendContainer.removeChild(legendContainer.firstChild);
        }
        
        // Create canvas for the legend
        const legendCanvas = document.createElement('canvas');
        legendCanvas.width = 150;
        legendCanvas.height = 40;
        legendContainer.appendChild(legendCanvas);
        
        const ctx = legendCanvas.getContext('2d');
        const width = 150;
        const height = 15;
        const x = 0;
        const y = 5;
        
        // Draw legend title
        ctx.textAlign = 'right';
        ctx.textBaseline = 'bottom';
        ctx.font = '11px sans-serif';
        ctx.fillStyle = '#CCCCCC';
        ctx.fillText(`${config.units}`, width, y - 2);
        
        // Draw gradient
        const gradient = ctx.createLinearGradient(x, y, x + width, y);
        
        config.colorScale.forEach((color, i) => {
            gradient.addColorStop(i / (config.colorScale.length - 1), color);
        });
        
        ctx.fillStyle = gradient;
        ctx.fillRect(x, y, width, height);
        
        // Draw border
        ctx.strokeStyle = '#888888';
        ctx.lineWidth = 1;
        ctx.strokeRect(x, y, width, height);
        
        // Draw min/max labels
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';
        ctx.fillStyle = '#CCCCCC';
        ctx.font = '10px sans-serif';
        
        ctx.fillText(config.minValue, x, y + height + 5);
        ctx.fillText(config.maxValue, x + width, y + height + 5);
    }
    
    // Create cursor plugin to show value at cursor position
    function createCursorPlugin() {
        let tooltipEl;
        
        function init(u) {
            tooltipEl = document.createElement('div');
            tooltipEl.className = 'u-tooltip';
            tooltipEl.style.display = 'none';
            u.over.appendChild(tooltipEl);
        }
        
        function update(u) {
            if (!u.cursor.idx || !u.cursor.left) {
                tooltipEl.style.display = 'none';
                return;
            }
            
            const idx = u.cursor.idx;
            const timestamp = timestamps[idx];
            
            // Calculate which CPU is under cursor
            const rect = u.bbox;
            const mouseY = u.cursor.top;
            const cpuHeight = rect.height / numCPUs;
            
            // Convert mouseY to CPU index (taking into account reversed order)
            const relativeY = mouseY - rect.top;
            const reversedCpuIdx = Math.floor(relativeY / cpuHeight);
            const cpuIdx = numCPUs - reversedCpuIdx - 1;
            
            if (cpuIdx < 0 || cpuIdx >= numCPUs) {
                tooltipEl.style.display = 'none';
                return;
            }
            
            // Get value at cursor position
            const value = cpuData[cpuIdx][idx];
            if (value === undefined || value === null) {
                tooltipEl.style.display = 'none';
                return;
            }
            
            // Format timestamp
            const date = new Date(timestamp * 1000);
            const timeStr = date.toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                hour12: false
            });
            
            // Create HTML content
            tooltipEl.innerHTML = `
                <div class="header">${timeStr}</div>
                <div class="value-row">
                    <span class="label">CPU ${cpuIdx}</span>
                    <span class="value">${value.toFixed(1)}${config.units}</span>
                </div>
            `;
            
            tooltipEl.style.display = 'block';
            
            // Position tooltip
            const left = u.cursor.left + 10;
            const top = u.cursor.top + 10;
            
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
    
    // Create a responsive plugin to handle resize events properly
    function createResponsivePlugin() {
        // Track resize state to prevent loops
        let isHandlingResize = false;
        
        return {
            hooks: {
                setSize: [
                    (u) => {
                        // Skip if currently handling a resize to prevent loops
                        if (isHandlingResize || u._resizing || u._skipNextDraw) {
                            return;
                        }
                        
                        // Set flag to prevent recursive resize
                        isHandlingResize = true;
                        
                        // When size changes, maintain the current view
                        debug(`🔄 ResponsivePlugin maintaining view: ${currentViewMin.toFixed(2)}-${currentViewMax.toFixed(2)}`);
                        
                        try {
                            u.batch(() => {
                                // First update the scale
                                u.setScale('x', {
                                    min: currentViewMin,
                                    max: currentViewMax,
                                    auto: false
                                });
                                
                                // Override the axes explicitly
                                u.axes[0]._min = currentViewMin;
                                u.axes[0]._max = currentViewMax;
                            });
                        } catch (err) {
                            console.error("Error updating scales during resize:", err);
                        }
                        
                        // Clear flag after a delay to prevent immediate retriggering
                        setTimeout(() => {
                            isHandlingResize = false;
                        }, 50);
                    }
                ]
            }
        };
    }
    
    // Create the options for uPlot
    const opts = {
        width: config.width,
        height: config.height,
        title: config.title,
        background: '#1E1E1E',
        class: 'cpu-heatmap',
        cursor: {
            show: true,
            x: true,
            y: true,
            lock: false
        },
        scales: {
            x: {
                time: false,
                min: timestamps[0],
                max: timestamps[timestamps.length - 1],
                auto: false // Prevent auto-scaling on resize
            },
            y: {
                min: 0,
                max: numCPUs,
            }
        },
        axes: [
            {
                stroke: '#888888',
                grid: {
                    stroke: 'rgba(136, 136, 136, 0.1)',
                },
                values: createTimeAxisFormatter(),
                space: 60,
                ticks: {
                    show: true,
                    stroke: 'rgba(136, 136, 136, 0.5)',
                    width: 1,
                    size: 5,
                },
                splits: createTimeSplitsFn()
            },
            {
                show: false // Hide y-axis
            }
        ],
        hooks: {
            drawClear: [
                (u) => {
                    drawHeatmap(u);
                }
            ],
            setSize: [
                (u) => {
                    // Update heatmap on resize, but only if not triggered by our observer
                    if (!u._skipNextDraw) {
                        drawHeatmap(u);
                    }
                    u._skipNextDraw = false;
                }
            ],
            setSelect: [
                (u) => {
                    // When selection happens, note that we have a custom zoom
                    isCustomZoom = true;
                }
            ],
            ready: [
                (u) => {
                    // We need a way to track the current width to avoid resize loops
                    u._lastWidth = u.width;
                    u._resizing = false;
                    u._skipNextDraw = false;
                    
                    // Use ResizeObserver for better size detection with debouncing
                    const resizeObserver = new ResizeObserver(entries => {
                        // Skip if already processing a resize
                        if (u._resizing) return;
                        
                        for (let entry of entries) {
                            const width = Math.floor(entry.contentRect.width);
                            
                            // Only process if width changed by more than threshold and not currently resizing
                            if (Math.abs(u._lastWidth - width) > 5) {
                                // Mark as resizing to prevent loops
                                u._resizing = true;
                                u._skipNextDraw = true;
                                
                                // Update the last width
                                u._lastWidth = width;
                                
                                // Debug only once per actual resize
                                debug(`🔄 Heatmap resizing to ${width}px with view: ${currentViewMin.toFixed(2)}-${currentViewMax.toFixed(2)}`);
                                
                                // Resize the plot
                                u.setSize({
                                    width: width,
                                    height: u.height
                                });
                                
                                // Force a redraw with a slight delay to ensure proper rendering
                                setTimeout(() => {
                                    drawHeatmap(u);
                                    
                                    // Reset resizing flag with a delay
                                    setTimeout(() => {
                                        u._resizing = false;
                                    }, 50);
                                }, 10);
                            }
                        }
                    });
                    
                    // Observe the container for size changes, with a threshold to reduce callbacks
                    resizeObserver.observe(container, { box: 'border-box' });
                    
                    // Store the observer for cleanup
                    u._observer = resizeObserver;
                    
                    // Initial draw
                    drawHeatmap(u);
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
        plugins: [
            createCursorPlugin(),
            createResponsivePlugin()
        ],
        select: {
            show: true,
            stroke: 'rgba(86, 156, 214, 0.5)',
            fill: 'rgba(86, 156, 214, 0.2)',
        }
    };
    
    // Create dummy series to make uPlot work (we'll draw the heatmap ourselves)
    const dummyData = [timestamps, Array(timestamps.length).fill(0)];
    const dummySeries = [
        { 
            label: 'Time'
        },
        {
            label: 'Dummy',
            show: false
        }
    ];
    
    opts.series = dummySeries;
    
    // Add a chart type badge
    const chartTypeBadge = document.createElement('div');
    chartTypeBadge.className = 'chart-type-badge';
    chartTypeBadge.textContent = 'Heatmap';
    container.appendChild(chartTypeBadge);
    
    // Create the plot
    const plot = new uPlot(opts, dummyData, container);
    
    // Add canvas to uPlot
    plot.root.querySelector('.u-over').appendChild(heatmapCanvas);
    
    // Add method to handle external zoom commands
    plot.updateZoom = function(min, max) {
        currentViewMin = min;
        currentViewMax = max;
        
        this.batch(() => {
            this.setScale('x', {
                min: min,
                max: max,
                auto: false
            });
            
            // Override the axes explicitly
            this.axes[0]._min = min;
            this.axes[0]._max = max;
        });
        
        // Redraw after update
        drawHeatmap(this);
    };
    
    return plot;
}

/**
 * Helper function to create a color interpolator for the heatmap
 * @param {Array} colorScale - Array of color strings for the scale
 * @param {number} min - Minimum value
 * @param {number} max - Maximum value
 * @returns {Function} Function that returns color for a given value
 */
function createColorInterpolator(colorScale, min, max) {
    return function(value) {
        // Handle value outside range
        if (value <= min) return colorScale[0];
        if (value >= max) return colorScale[colorScale.length - 1];
        
        // Normalize value to 0-1 range
        const normalizedValue = (value - min) / (max - min);
        
        // Find position in color scale
        const position = normalizedValue * (colorScale.length - 1);
        const index = Math.floor(position);
        const remainder = position - index;
        
        // If exact match to a color in scale
        if (remainder === 0) return colorScale[index];
        
        // Interpolate between two colors
        const color1 = colorScale[index];
        const color2 = colorScale[index + 1];
        
        return interpolateColor(color1, color2, remainder);
    };
}