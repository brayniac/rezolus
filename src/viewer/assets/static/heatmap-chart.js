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
        height: 350, // Match line chart height
        colorScale: ["#100060", "#4000A0", "#8000C0", "#A000E0", "#C000FF", "#FF2000", "#FF6000", "#FFA000", "#FFE000"],
        minValue: 0,
        maxValue: 100,
        units: "%"
    };
    
    const config = {...defaults, ...options};
    
    // Extract data structure
    const { timestamps, cpuData } = data;
    const numCPUs = cpuData.length;
    
    // Create canvas for the heatmap with explicit positioning
    const heatmapCanvas = document.createElement('canvas');
    heatmapCanvas.className = 'cpu-heatmap-canvas';
    heatmapCanvas.style.position = 'absolute';
    heatmapCanvas.style.top = '0';
    heatmapCanvas.style.left = '0';
    heatmapCanvas.style.pointerEvents = 'none'; // Allow events to pass through to underlying elements
    
    // Create a separate container for the legend
    const legendContainer = document.createElement('div');
    legendContainer.className = 'heatmap-legend-container';
    legendContainer.style.position = 'absolute';
    legendContainer.style.bottom = '45px';  // Position at bottom but above time labels
    legendContainer.style.left = '70px';    // Position at left but avoid first time label
    legendContainer.style.zIndex = '100';
    container.appendChild(legendContainer);
    
    // Remove the time/dummy legend by adding a style override
    const legendOverride = document.createElement('style');
    legendOverride.textContent = `
        #${containerId} .u-legend [data-idx="0"],
        #${containerId} .u-legend [data-idx="1"] {
            display: none !important;
        }
    `;
    container.appendChild(legendOverride);
    
    // Store current view state
    let currentViewMin = timestamps[0];
    let currentViewMax = timestamps[timestamps.length - 1];
    let isCustomZoom = false;
    
    // Helper function to check if a point is within the chart area
    function isInChartArea(x, y, chartRect) {
        return (
            x >= chartRect.left && 
            x <= chartRect.left + chartRect.width &&
            y >= chartRect.top && 
            y <= chartRect.top + chartRect.height
        );
    }
    
    // Improved tooltip creation for heatmap
    function createEnhancedTooltip() {
        const tooltipEl = document.createElement('div');
        tooltipEl.className = 'heatmap-tooltip';
        tooltipEl.style.position = 'absolute';
        tooltipEl.style.display = 'none';
        tooltipEl.style.backgroundColor = 'rgba(40, 40, 40, 0.95)';
        tooltipEl.style.color = '#eee';
        tooltipEl.style.border = '1px solid #555';
        tooltipEl.style.borderRadius = '4px';
        tooltipEl.style.padding = '8px 12px';
        tooltipEl.style.pointerEvents = 'none';
        tooltipEl.style.zIndex = '1000';
        tooltipEl.style.fontSize = '12px';
        tooltipEl.style.fontFamily = '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace';
        tooltipEl.style.whiteSpace = 'nowrap';
        tooltipEl.style.boxShadow = '0 2px 10px rgba(0, 0, 0, 0.3)';
        
        container.appendChild(tooltipEl);
        
        return tooltipEl;
    }
    
    // Create tooltip element
    const tooltipEl = createEnhancedTooltip();
    
    function drawHeatmap(u) {
        if (!u.data || !u.data[0] || u.data[0].length === 0) return;
        
        // Get pixel dimensions of the data area
        const dataStartX = u.bbox.left;
        const dataEndX = u.bbox.left + u.bbox.width;
        const dataWidth = u.bbox.width;
        const dataStartY = u.bbox.top;
        const dataHeight = u.bbox.height;
        
        // Position the canvas precisely over the data area
        heatmapCanvas.style.position = 'absolute';
        heatmapCanvas.style.left = `${dataStartX}px`;
        heatmapCanvas.style.top = `${dataStartY}px`;
        heatmapCanvas.style.width = `${dataWidth}px`;
        heatmapCanvas.style.height = `${dataHeight}px`;
        
        // Set canvas dimensions to match data area exactly
        heatmapCanvas.width = dataWidth;
        heatmapCanvas.height = dataHeight;
        
        const ctx = heatmapCanvas.getContext('2d');
        
        // Clear canvas
        ctx.clearRect(0, 0, dataWidth, dataHeight);
        
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
            
            // Y position for this CPU (reversed) - now relative to canvas (0,0)
            const yPos = reversedCpuIdx * cellHeight;
            
            // Draw cells for this CPU
            for (let i = startIdx; i < endIdx; i++) {
                const time1 = timestamps[i];
                const time2 = timestamps[i + 1];
                
                if (time1 > visibleEndTime || time2 < visibleStartTime) continue;
                
                // Get value and color
                const value = cpuValues[i];
                const color = getColor(value);
                
                // Calculate pixel positions relative to canvas (not the overall plot)
                const x1 = u.valToPos(time1, 'x') - dataStartX;
                const x2 = u.valToPos(time2, 'x') - dataStartX;
                
                // Draw rectangle using coordinates relative to canvas
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
        
        // Draw the legend
        drawLegend();
        
        // Draw CPU labels outside the canvas (don't include in the canvas drawing)
        drawCpuLabels(u);
    }

    // Separate function for drawing CPU labels
    function drawCpuLabels(u) {
        // Get dimensions for positioning
        const dataStartX = u.bbox.left;
        const dataStartY = u.bbox.top;
        const dataHeight = u.bbox.height;
        
        // Get canvas from uPlot
        const fullCtx = u.ctx;
        
        // Improved CPU labels - styled to match line chart Y-axis
        // Clear the label area first
        fullCtx.clearRect(0, dataStartY, dataStartX - 2, dataHeight);
        
        // Style to match the Y-axis labels
        fullCtx.textAlign = 'right';
        fullCtx.textBaseline = 'middle';
        fullCtx.font = '11px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
        fullCtx.fillStyle = '#888888';  // Match the axis label color
        
        // Calculate cell height
        const cellHeight = dataHeight / numCPUs;
        
        for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
            // Reverse the order - CPU0 at bottom, highest CPU at top
            const reversedCpuIdx = numCPUs - cpuIdx - 1;
            const yPos = dataStartY + (reversedCpuIdx + 0.5) * cellHeight;
            
            // Draw label to the left of the chart area, aligned with Y-axis labels
            fullCtx.fillText(`CPU ${cpuIdx}`, dataStartX - 10, yPos);
        }
    }
    
    // Create a legend for the heatmap
    function drawLegend() {
        // Clear previous legend if any
        while (legendContainer.firstChild) {
            legendContainer.removeChild(legendContainer.firstChild);
        }
        
        // Create canvas for the legend with improved dimensions
        const legendCanvas = document.createElement('canvas');
        legendCanvas.width = 150;
        legendCanvas.height = 30;
        legendContainer.appendChild(legendCanvas);
        
        const ctx = legendCanvas.getContext('2d');
        const width = 150;
        const height = 12; // Smaller height to be less intrusive
        const x = 0;
        const y = 5;
        
        // Draw legend title with better positioning
        ctx.textAlign = 'right';
        ctx.textBaseline = 'bottom';
        ctx.font = '10px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
        ctx.fillStyle = '#888888'; // Match axis label color
        ctx.fillText(config.units, width, y - 2);
        
        // Draw gradient
        const gradient = ctx.createLinearGradient(x, y, x + width, y);
        
        config.colorScale.forEach((color, i) => {
            gradient.addColorStop(i / (config.colorScale.length - 1), color);
        });
        
        ctx.fillStyle = gradient;
        ctx.fillRect(x, y, width, height);
        
        // Draw border
        ctx.strokeStyle = '#444444';
        ctx.lineWidth = 1;
        ctx.strokeRect(x, y, width, height);
        
        // Draw min/max labels with better spacing
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';
        ctx.fillStyle = '#888888';
        ctx.font = '10px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
        
        ctx.fillText(config.minValue, x + 10, y + height + 3);
        ctx.fillText(config.maxValue, x + width - 10, y + height + 3);
    }
    
    // Enhanced mousemove handler for tooltip
    function handleMouseMove(e) {
        const rect = plot.over.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        // Check if mouse is within the exact data area
        if (!(x >= plot.bbox.left && x <= plot.bbox.left + plot.bbox.width && 
              y >= plot.bbox.top && y <= plot.bbox.top + plot.bbox.height)) {
            tooltipEl.style.display = 'none';
            return;
        }
        
        // Calculate relative position within data area
        const relX = x - plot.bbox.left;
        const relY = y - plot.bbox.top;
        
        // Convert to data coordinates with precise positioning
        try {
            const xVal = plot.posToVal(x, 'x');
            
            // Calculate exact CPU based on relative Y position in data area
            const dataHeight = plot.bbox.height;
            const cpuHeight = dataHeight / numCPUs;
            
            // Get CPU index (taking into account reversed order)
            const reversedCpuIdx = Math.floor(relY / cpuHeight);
            const cpuIdx = numCPUs - reversedCpuIdx - 1;
            
            if (cpuIdx < 0 || cpuIdx >= numCPUs) {
                tooltipEl.style.display = 'none';
                return;
            }
            
            // Find closest timestamp
            let closestIdx = 0;
            let closestDist = Infinity;
            
            for (let i = 0; i < timestamps.length; i++) {
                const dist = Math.abs(timestamps[i] - xVal);
                if (dist < closestDist) {
                    closestDist = dist;
                    closestIdx = i;
                }
            }
            
            // Get value at cursor position
            const value = cpuData[cpuIdx][closestIdx];
            if (value === undefined || value === null) {
                tooltipEl.style.display = 'none';
                return;
            }
            
            // Format timestamp
            const date = new Date(timestamps[closestIdx] * 1000);
            const timeStr = date.toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                hour12: false
            });
            
            // Create HTML content
            tooltipEl.innerHTML = `
                <div style="margin-bottom: 6px; font-weight: bold; color: #ddd; border-bottom: 1px solid #555; padding-bottom: 4px;">
                    ${timeStr}
                </div>
                <div style="display: flex; justify-content: space-between; margin: 3px 0; align-items: center;">
                    <span style="margin-right: 16px;">
                        CPU ${cpuIdx}:
                    </span>
                    <span style="font-weight: bold;">${value.toFixed(1)}${config.units}</span>
                </div>
            `;
            
            tooltipEl.style.display = 'block';
            
            // Position tooltip
            const tooltipLeft = x + 10;
            const tooltipTop = y + 10;
            
            // Make sure tooltip stays within chart
            const tooltipRect = tooltipEl.getBoundingClientRect();
            const rightEdge = tooltipLeft + tooltipRect.width;
            const bottomEdge = tooltipTop + tooltipRect.height;
            
            tooltipEl.style.left = (rightEdge > rect.width ? x - tooltipRect.width - 10 : tooltipLeft) + 'px';
            tooltipEl.style.top = (bottomEdge > rect.height ? y - tooltipRect.height - 10 : tooltipTop) + 'px';
            
        } catch (err) {
            console.error("Error handling tooltip:", err);
            tooltipEl.style.display = 'none';
        }
    }
    
    // Create custom plugin to handle heatmap selection
    function createHeatmapSelectionPlugin() {
        // State variables
        let selecting = false;
        let startX = null;
        let selectRect = null;
        
        function init(u) {
            // Create selection rectangle element
            selectRect = document.createElement('div');
            selectRect.className = 'heatmap-selection';
            selectRect.style.position = 'absolute';
            selectRect.style.display = 'none';
            selectRect.style.backgroundColor = 'rgba(86, 156, 214, 0.2)';
            selectRect.style.border = '1px solid rgba(86, 156, 214, 0.5)';
            selectRect.style.pointerEvents = 'none';
            selectRect.style.zIndex = 100;
            
            u.over.appendChild(selectRect);
            
            // Mouse events for selection
            u.over.addEventListener('mousedown', e => {
                const rect = u.over.getBoundingClientRect();
                const x = e.clientX - rect.left;
                const y = e.clientY - rect.top;
                
                // Only start selection if within exact data area
                if (x >= u.bbox.left && x <= u.bbox.left + u.bbox.width && 
                    y >= u.bbox.top && y <= u.bbox.top + u.bbox.height) {
                    selecting = true;
                    startX = x;
                    
                    // Start with zero-width selection at cursor
                    selectRect.style.left = `${x}px`;
                    selectRect.style.top = `${u.bbox.top}px`;
                    selectRect.style.width = '0px';
                    selectRect.style.height = `${u.bbox.height}px`;
                    selectRect.style.display = 'block';
                }
            });
            
            u.over.addEventListener('mousemove', e => {
                if (selecting) {
                    const rect = u.over.getBoundingClientRect();
                    const x = e.clientX - rect.left;
                    
                    // Update selection rectangle
                    const left = Math.min(startX, x);
                    const width = Math.abs(x - startX);
                    
                    selectRect.style.left = `${left}px`;
                    selectRect.style.width = `${width}px`;
                }
            });
            
            u.over.addEventListener('mouseup', e => {
                if (selecting) {
                    selecting = false;
                    
                    const rect = u.over.getBoundingClientRect();
                    const x = e.clientX - rect.left;
                    const width = Math.abs(x - startX);
                    
                    // Only process if selection has meaningful width
                    if (width > 5) {
                        try {
                            // Convert selection to values
                            const left = Math.min(startX, x);
                            const right = Math.max(startX, x);
                            
                            const minX = u.posToVal(left, 'x');
                            const maxX = u.posToVal(right, 'x');
                            
                            debug(`Heatmap selection completed: ${minX.toFixed(2)}-${maxX.toFixed(2)}`);
                            
                            // Trigger zoom
                            if (typeof window.zoomController !== 'undefined') {
                                window.zoomController.syncZoom(u, minX, maxX);
                            } else {
                                // Apply zoom directly if no controller
                                u.batch(() => {
                                    u.setScale('x', {
                                        min: minX,
                                        max: maxX
                                    });
                                });
                                // Redraw the heatmap
                                drawHeatmap(u);
                            }
                        } catch (err) {
                            console.error("Error processing selection:", err);
                        }
                    }
                    
                    // Hide selection rectangle
                    selectRect.style.display = 'none';
                }
            });
            
            // Handle tooltip
            u.over.addEventListener('mousemove', handleMouseMove);
            
            // Clear tooltip on mouseout
            u.over.addEventListener('mouseout', () => {
                tooltipEl.style.display = 'none';
            });
            
            // Double click to reset zoom
            u.over.addEventListener('dblclick', () => {
                if (typeof window.zoomController !== 'undefined') {
                    window.zoomController.resetZoom();
                }
            });
        }
        
        return {
            hooks: {
                init: init
            }
        };
    }
    
    // Create the options for uPlot
    const opts = {
        width: config.width,
        height: 350, // Match line chart height
        title: config.title,
        background: '#1E1E1E',
        class: 'cpu-heatmap',
        padding: [30, 10, 30, 60], // Increase top padding to 30px to match line chart
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
                    
                    // Observe the container for size changes
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
            createHeatmapSelectionPlugin()
        ]
    };
    
    // Create dummy series to make uPlot work (we'll draw the heatmap ourselves)
    const dummyData = [timestamps, Array(timestamps.length).fill(0)];
    const dummySeries = [
        { 
            label: 'Time',
            show: false  // Hide Time label
        },
        {
            label: 'Dummy',
            show: false  // Hide Dummy label
        }
    ];
    
    opts.series = dummySeries;
    
    // Create the plot
    const plot = new uPlot(opts, dummyData, container);
    
    // Add canvas to uPlot
    plot.root.querySelector('.u-over').appendChild(heatmapCanvas);
    
    // Add method to handle external zoom commands
    plot.updateZoom = function(min, max) {
        // Validate the input values to prevent NaN
        if (isNaN(min) || isNaN(max)) {
            console.error("Invalid zoom range:", min, max);
            return;
        }
        
        // Store current view values
        currentViewMin = min;
        currentViewMax = max;
        
        // Ensure scales are properly updated
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
        
        debug(`✅ Heatmap ${this._id} zoom updated to ${min.toFixed(2)}-${max.toFixed(2)}`);
    };
    
    // Force a redraw after a short delay to ensure proper positioning
    setTimeout(() => {
        drawHeatmap(plot);
    }, 50);
    
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

// Helper function to interpolate between two colors
function interpolateColor(color1, color2, factor) {
    if (color1.startsWith('#')) {
        // Parse hex colors to RGB
        const r1 = parseInt(color1.slice(1, 3), 16);
        const g1 = parseInt(color1.slice(3, 5), 16);
        const b1 = parseInt(color1.slice(5, 7), 16);
        
        const r2 = parseInt(color2.slice(1, 3), 16);
        const g2 = parseInt(color2.slice(3, 5), 16);
        const b2 = parseInt(color2.slice(5, 7), 16);
        
        // Interpolate values
        const r = Math.round(r1 + factor * (r2 - r1));
        const g = Math.round(g1 + factor * (g2 - g1));
        const b = Math.round(b1 + factor * (b2 - b1));
        
        // Convert back to rgb
        return `rgb(${r}, ${g}, ${b})`;
    } else if (color1.startsWith('rgb')) {
        // Extract RGB values using regex
        const rgbRegex = /rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)/;
        const match1 = rgbRegex.exec(color1);
        const match2 = rgbRegex.exec(color2);
        
        if (!match1 || !match2) return color1;
        
        const r1 = parseInt(match1[1], 10);
        const g1 = parseInt(match1[2], 10);
        const b1 = parseInt(match1[3], 10);
        
        const r2 = parseInt(match2[1], 10);
        const g2 = parseInt(match2[2], 10);
        const b2 = parseInt(match2[3], 10);
        
        // Interpolate values
        const r = Math.round(r1 + factor * (r2 - r1));
        const g = Math.round(g1 + factor * (g2 - g1));
        const b = Math.round(b1 + factor * (b2 - b1));
        
        return `rgb(${r}, ${g}, ${b})`;
    }
    
    // Fallback
    return color1;
}

// Debug helper function
function debug(message) {
    const timestamp = new Date().toISOString().substr(11, 12);
    const formattedMessage = `[${timestamp}] ${message}`;
    
    console.log(formattedMessage);
}
