// new-heatmap.js - Complete replacement for heatmap chart implementation

/**
 * Create a CPU heatmap chart with proper alignment
 * @param {string} containerId - DOM element ID for the chart container
 * @param {Object} data - Data for the heatmap
 * @param {Object} options - Configuration options
 * @returns {Object} Plot object with interface for zoom controller
 */
function createNewCpuHeatmap(containerId, data, options = {}) {
    const container = document.getElementById(containerId);
    if (!container) {
        console.error(`Container element with ID ${containerId} not found`);
        return null;
    }
    
    // Get dimensions that match line chart exactly
    const originalWidth = container.clientWidth;
    const originalHeight = 350; // Fixed height to match uPlot
    
    // Clear the container first
    container.innerHTML = '';
    
    // Set container style
    container.style.position = 'relative';
    container.style.width = originalWidth + 'px';
    container.style.height = originalHeight + 'px';
    container.style.backgroundColor = '#1E1E1E';
    container.style.overflow = 'hidden'; // Prevent overflow issues
    
    // Create layout elements with exact dimensions
    const titleHeight = 30;            // Height of the title area
    const timeAxisHeight = 30;         // Height of the bottom time axis
    const leftPadding = 100;           // Width of the left label area
    
    // Calculate the main chart area dimensions
    const chartTop = titleHeight;
    const chartLeft = leftPadding;
    const chartWidth = originalWidth - leftPadding;
    const chartHeight = originalHeight - titleHeight - timeAxisHeight;
    
    // Extract data
    const { timestamps, cpuData } = data;
    const numCPUs = cpuData.length;
    
    // Set up configuration
    const config = {
        title: options.title || "CPU Heatmap",
        colorScale: options.colorScale || ["#100060", "#4000A0", "#8000C0", "#A000E0", "#C000FF", "#FF2000", "#FF6000", "#FFA000", "#FFE000"],
        minValue: options.minValue || 0, 
        maxValue: options.maxValue || 100,
        units: options.units || "%"
    };
    
    // Create title element
    const titleEl = document.createElement('div');
    titleEl.textContent = config.title;
    titleEl.style.cssText = `
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: ${titleHeight}px;
        line-height: ${titleHeight}px;
        text-align: center;
        font-size: 16px;
        font-weight: 500;
        color: #CCCCCC;
    `;
    container.appendChild(titleEl);
    
    // Create the main chart canvas
    const chartCanvas = document.createElement('canvas');
    chartCanvas.width = chartWidth;
    chartCanvas.height = chartHeight;
    chartCanvas.style.cssText = `
        position: absolute;
        top: ${chartTop}px;
        left: ${chartLeft}px;
        width: ${chartWidth}px;
        height: ${chartHeight}px;
        image-rendering: pixelated;
        z-index: 1;
    `;
    container.appendChild(chartCanvas);
    
    // Calculate cell height
    const cellHeight = chartHeight / numCPUs;
    
    // Draw CPU labels with exact positioning
    for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
        // Reverse order (CPU0 at bottom)
        const reversedCpuIdx = numCPUs - cpuIdx - 1;
        
        // Calculate vertical position (center of cell)
        const labelY = chartTop + reversedCpuIdx * cellHeight + (cellHeight / 2);
        
        const labelEl = document.createElement('div');
        labelEl.textContent = `CPU ${cpuIdx}`;
        labelEl.style.cssText = `
            position: absolute;
            top: ${labelY}px;
            left: 0;
            width: ${leftPadding - 10}px; 
            text-align: right;
            transform: translateY(-50%);
            font-size: 11px;
            color: #888888;
            white-space: nowrap;
            pointer-events: none;
        `;
        container.appendChild(labelEl);
    }
    
    // Create time axis
    const timeAxisEl = document.createElement('div');
    timeAxisEl.style.cssText = `
        position: absolute;
        bottom: 0;
        left: ${chartLeft}px;
        width: ${chartWidth}px;
        height: ${timeAxisHeight}px;
    `;
    container.appendChild(timeAxisEl);
    
    // Add time labels (5 evenly spaced)
    const timeLabels = 4; // Use 4 intervals = 5 labels
    const timeFormat = {
        hour: '2-digit',
        minute: '2-digit'
    };
    
    for (let i = 0; i <= timeLabels; i++) {
        const percent = i / timeLabels;
        const labelX = percent * chartWidth;
        
        // Calculate time at this position
        const timeIndex = Math.min(Math.floor(percent * timestamps.length), timestamps.length - 1);
        const time = timestamps[timeIndex];
        const date = new Date(time * 1000);
        
        // Format time
        const timeStr = date.toLocaleTimeString([], timeFormat);
        
        const timeLabel = document.createElement('div');
        timeLabel.textContent = timeStr;
        timeLabel.style.cssText = `
            position: absolute;
            bottom: 5px;
            left: ${labelX}px;
            transform: translateX(-50%);
            font-size: 11px;
            color: #888888;
            pointer-events: none;
        `;
        timeAxisEl.appendChild(timeLabel);
    }
    
    // Create color legend
    const legendEl = document.createElement('div');
    legendEl.style.cssText = `
        position: absolute;
        bottom: 40px;
        left: ${chartLeft + 10}px;
        background-color: rgba(30, 30, 30, 0.8);
        border: 1px solid #333;
        border-radius: 4px;
        padding: 5px;
        z-index: 10;
    `;
    container.appendChild(legendEl);
    
    // Create legend canvas
    const legendCanvas = document.createElement('canvas');
    legendCanvas.width = 150;
    legendCanvas.height = 30;
    legendEl.appendChild(legendCanvas);
    
    // Draw the legend
    const lctx = legendCanvas.getContext('2d');
    const width = 150;
    const height = 12;
    const x = 0;
    const y = 5;
    
    // Draw unit label
    lctx.textAlign = 'right';
    lctx.textBaseline = 'bottom';
    lctx.font = '10px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
    lctx.fillStyle = '#888888';
    lctx.fillText(config.units, width, y - 2);
    
    // Draw color gradient
    const gradient = lctx.createLinearGradient(x, y, x + width, y);
    config.colorScale.forEach((color, i) => {
        gradient.addColorStop(i / (config.colorScale.length - 1), color);
    });
    
    lctx.fillStyle = gradient;
    lctx.fillRect(x, y, width, height);
    
    // Draw gradient border
    lctx.strokeStyle = '#444444';
    lctx.lineWidth = 1;
    lctx.strokeRect(x, y, width, height);
    
    // Draw min/max labels
    lctx.textAlign = 'center';
    lctx.textBaseline = 'top';
    lctx.fillStyle = '#888888';
    lctx.font = '10px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
    
    lctx.fillText(config.minValue, x + 10, y + height + 3);
    lctx.fillText(config.maxValue, x + width - 10, y + height + 3);
    
    // Function to get color for a value
    function getColor(value) {
        // Normalize value to 0-1 range
        const normalized = Math.max(0, Math.min(1, (value - config.minValue) / (config.maxValue - config.minValue)));
        
        // Find position in color scale
        const position = normalized * (config.colorScale.length - 1);
        const index = Math.floor(position);
        const fraction = position - index;
        
        // If exact match to a color in scale
        if (fraction === 0 || index >= config.colorScale.length - 1) {
            return config.colorScale[Math.min(index, config.colorScale.length - 1)];
        }
        
        // Interpolate between two colors
        const color1 = parseColor(config.colorScale[index]);
        const color2 = parseColor(config.colorScale[index + 1]);
        
        // Blend the colors
        const r = Math.round(color1.r + fraction * (color2.r - color1.r));
        const g = Math.round(color1.g + fraction * (color2.g - color1.g));
        const b = Math.round(color1.b + fraction * (color2.b - color1.b));
        
        return `rgb(${r}, ${g}, ${b})`;
    }
    
    // Helper to parse color
    function parseColor(color) {
        // Handle hex color
        if (color.startsWith('#')) {
            const r = parseInt(color.slice(1, 3), 16);
            const g = parseInt(color.slice(3, 5), 16);
            const b = parseInt(color.slice(5, 7), 16);
            return { r, g, b };
        }
        
        // Handle rgb color
        const match = color.match(/rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)/);
        if (match) {
            return {
                r: parseInt(match[1]),
                g: parseInt(match[2]),
                b: parseInt(match[3])
            };
        }
        
        // Default fallback
        return { r: 0, g: 0, b: 0 };
    }
    
    // Current view state (for synchronized zooming)
    let currentViewMin = null;
    let currentViewMax = null;
    
    // Draw the heatmap function
    function drawHeatmap() {
        const ctx = chartCanvas.getContext('2d');
        ctx.clearRect(0, 0, chartWidth, chartHeight);
        
        // Current view range (all data initially)
        const viewStartTime = currentViewMin || timestamps[0];
        const viewEndTime = currentViewMax || timestamps[timestamps.length - 1];
        
        // Calculate time to pixel conversion
        const timeToPixel = (time) => {
            const timeRange = viewEndTime - viewStartTime;
            const timeOffset = time - viewStartTime;
            return (timeOffset / timeRange) * chartWidth;
        };
        
        // Draw each CPU row
        for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
            const reversedCpuIdx = numCPUs - cpuIdx - 1;
            const cpuValues = cpuData[cpuIdx];
            
            // Y position for this CPU
            const yPos = reversedCpuIdx * cellHeight;
            
            // Draw each time segment
            for (let i = 0; i < timestamps.length - 1; i++) {
                const time1 = timestamps[i];
                const time2 = timestamps[i + 1];
                
                // Skip if outside view range
                if (time2 < viewStartTime || time1 > viewEndTime) continue;
                
                // Get pixel positions
                const x1 = timeToPixel(Math.max(time1, viewStartTime));
                const x2 = timeToPixel(Math.min(time2, viewEndTime));
                
                // Skip if segment is too narrow
                if (x2 - x1 < 0.1) continue;
                
                // Get color for this value
                const value = cpuValues[i];
                const color = getColor(value);
                
                // Draw rectangle
                ctx.fillStyle = color;
                ctx.fillRect(x1, yPos, x2 - x1, cellHeight);
                
                // Add borders for larger cells
                if (x2 - x1 > 4) {
                    ctx.strokeStyle = 'rgba(0,0,0,0.1)';
                    ctx.lineWidth = 0.5;
                    ctx.strokeRect(x1, yPos, x2 - x1, cellHeight);
                }
            }
        }
    }
    
    // Draw the initial heatmap
    drawHeatmap();
    
    // Function to update zoom range
    function updateZoom(min, max) {
        currentViewMin = min;
        currentViewMax = max;
        drawHeatmap();
    }
    
    // Create overlay with precise dimensions matching the canvas
    const overlayEl = document.createElement('div');
    overlayEl.style.cssText = `
        position: absolute;
        top: ${chartTop}px;
        left: ${chartLeft}px;
        width: ${chartWidth}px;
        height: ${chartHeight}px;
        cursor: crosshair;
        z-index: 2;
        box-sizing: border-box;
    `;
    container.appendChild(overlayEl);
    
    // Create selection rectangle
    const selectionRect = document.createElement('div');
    selectionRect.style.cssText = `
        position: fixed;
        display: none;
        background-color: rgba(86, 156, 214, 0.2);
        border: 1px solid rgba(86, 156, 214, 0.5);
        pointer-events: none;
        z-index: 20;
        box-sizing: border-box;
    `;
    document.body.appendChild(selectionRect); // Append to body for precise positioning
    
    // Selection state variables
    let selecting = false;
    let selectionStart = null;
    
    // Mouse event handlers for selection
    overlayEl.addEventListener('mousedown', (e) => {
        const rect = overlayEl.getBoundingClientRect();
        const x = e.clientX - rect.left;
        
        // Only process if within bounds
        if (x < 0 || x > chartWidth) {
            return;
        }
        
        selecting = true;
        selectionStart = x;
        
        // Position selection rectangle with fixed positioning for precision
        selectionRect.style.display = 'block';
        selectionRect.style.left = `${e.clientX}px`;
        selectionRect.style.top = `${rect.top}px`;
        selectionRect.style.width = '0px';
        selectionRect.style.height = `${rect.height}px`;
    });
    
    overlayEl.addEventListener('mousemove', (e) => {
        if (selecting) {
            const rect = overlayEl.getBoundingClientRect();
            const x = e.clientX - rect.left;
            
            if (x < 0 || x > chartWidth) {
                return; // Don't process outside bounds
            }
            
            // Calculate left position based on initial selection
            const startX = rect.left + selectionStart;
            const width = Math.abs(e.clientX - startX);
            const left = Math.min(e.clientX, startX);
            
            selectionRect.style.left = `${left}px`;
            selectionRect.style.width = `${width}px`;
        }
    });
    
    // Handler for mouseup that also works when mouse leaves the overlay
    function handleMouseUp(e) {
        if (selecting) {
            selecting = false;
            selectionRect.style.display = 'none';
            
            const rect = overlayEl.getBoundingClientRect();
            const x = e.clientX - rect.left;
            
            // Only process if within bounds and selection has width
            if (x >= 0 && x <= chartWidth) {
                const width = Math.abs(x - selectionStart);
                
                // Only process if selection has reasonable width
                if (width > 5) {
                    const left = Math.min(selectionStart, x) / chartWidth;
                    const right = Math.max(selectionStart, x) / chartWidth;
                    
                    // Get current visible time range
                    const visibleStart = currentViewMin || timestamps[0];
                    const visibleEnd = currentViewMax || timestamps[timestamps.length - 1];
                    const visibleRange = visibleEnd - visibleStart;
                    
                    // Convert to timestamps within the visible range
                    const startTime = visibleStart + left * visibleRange;
                    const endTime = visibleStart + right * visibleRange;
                    
                    // Update our view
                    updateZoom(startTime, endTime);
                    
                    // If zoom controller exists, use it
                    if (typeof window.zoomController !== 'undefined') {
                        window.zoomController.syncZoom(chartObj, startTime, endTime);
                    }
                }
            }
        }
    }
    
    overlayEl.addEventListener('mouseup', handleMouseUp);
    overlayEl.addEventListener('mouseleave', handleMouseUp);
    
    // Double-click to reset zoom
    overlayEl.addEventListener('dblclick', () => {
        // Reset our view
        currentViewMin = null;
        currentViewMax = null;
        drawHeatmap();
        
        // If zoom controller exists, use it
        if (typeof window.zoomController !== 'undefined') {
            window.zoomController.resetZoom();
        }
    });
    
    // Create tooltip element
    const tooltipEl = document.createElement('div');
    tooltipEl.style.cssText = `
        position: fixed;
        display: none;
        background-color: rgba(40, 40, 40, 0.95);
        color: #eee;
        border: 1px solid #555;
        border-radius: 4px;
        padding: 8px 12px;
        pointer-events: none;
        z-index: 1000;
        font-size: 12px;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace;
        white-space: nowrap;
        box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
        backdrop-filter: blur(2px);
        max-width: 300px;
    `;
    document.body.appendChild(tooltipEl);
    
    // Handle mouse movement for tooltips
    overlayEl.addEventListener('mousemove', (e) => {
        const rect = overlayEl.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        if (x < 0 || x > chartWidth || y < 0 || y > chartHeight) {
            tooltipEl.style.display = 'none';
            return;
        }
        
        try {
            // Calculate which time point we're hovering over
            const visibleStart = currentViewMin || timestamps[0];
            const visibleEnd = currentViewMax || timestamps[timestamps.length - 1];
            const visibleRange = visibleEnd - visibleStart;
            
            const timePoint = visibleStart + (x / chartWidth) * visibleRange;
            
            // Find closest timestamp index
            let closestIdx = 0;
            let closestDist = Infinity;
            
            for (let i = 0; i < timestamps.length; i++) {
                const dist = Math.abs(timestamps[i] - timePoint);
                if (dist < closestDist) {
                    closestDist = dist;
                    closestIdx = i;
                }
            }
            
            // Calculate which CPU we're hovering over
            const cpuIndex = Math.floor(y / cellHeight);
            const actualCpuIdx = numCPUs - cpuIndex - 1;
            
            if (actualCpuIdx < 0 || actualCpuIdx >= numCPUs) {
                tooltipEl.style.display = 'none';
                return;
            }
            
            // Check if we have valid data
            if (!cpuData[actualCpuIdx] || typeof cpuData[actualCpuIdx][closestIdx] === 'undefined') {
                tooltipEl.style.display = 'none';
                return;
            }
            
            const value = cpuData[actualCpuIdx][closestIdx];
            
            // Format timestamp
            const date = new Date(timestamps[closestIdx] * 1000);
            const timeStr = date.toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                hour12: false
            });
            
            // Create tooltip content
            tooltipEl.innerHTML = `
                <div style="margin-bottom: 6px; font-weight: bold; color: #ddd; border-bottom: 1px solid #555; padding-bottom: 4px;">
                    ${timeStr}
                </div>
                <div style="display: flex; justify-content: space-between; margin: 3px 0; align-items: center;">
                    <span style="margin-right: 16px;">
                        CPU ${actualCpuIdx}:
                    </span>
                    <span style="font-weight: bold;">${value.toFixed(1)}${config.units}</span>
                </div>
            `;
            
            tooltipEl.style.display = 'block';
            
            // Position tooltip near cursor with fixed positioning
            let tooltipX = e.clientX + 10;
            let tooltipY = e.clientY + 10;
            
            // Adjust position to keep on screen
            const viewportWidth = window.innerWidth;
            const viewportHeight = window.innerHeight;
            
            const tooltipWidth = tooltipEl.offsetWidth || 200;
            const tooltipHeight = tooltipEl.offsetHeight || 80;
            
            if (tooltipX + tooltipWidth > viewportWidth - 10) {
                tooltipX = e.clientX - tooltipWidth - 10;
            }
            
            if (tooltipY + tooltipHeight > viewportHeight - 10) {
                tooltipY = e.clientY - tooltipHeight - 10;
            }
            
            tooltipEl.style.left = `${tooltipX}px`;
            tooltipEl.style.top = `${tooltipY}px`;
            
        } catch (err) {
            console.error("Error handling tooltip:", err);
            tooltipEl.style.display = 'none';
        }
    });
    
    // Hide tooltip when mouse leaves overlay
    overlayEl.addEventListener('mouseout', () => {
        tooltipEl.style.display = 'none';
    });
    
    // Create chart object with interface compatible with original
    const chartObj = {
        root: container,
        canvas: chartCanvas,
        over: overlayEl,
        width: originalWidth,
        height: originalHeight,
        _chart_type: 'heatmap',
        
        // Required methods for zoom controller integration
        setScale: function(axis, { min, max }) {
            if (axis === 'x') {
                updateZoom(min, max);
            }
        },
        
        updateZoom: function(min, max) {
            updateZoom(min, max);
        },
        
        setSize: function(dimensions) {
            if (dimensions.width) {
                const newWidth = dimensions.width;
                const oldWidth = this.width;
                
                // Only update if significant change
                if (Math.abs(newWidth - oldWidth) > 5) {
                    this.width = newWidth;
                    
                    // Update container
                    container.style.width = newWidth + 'px';
                    
                    // Calculate new chart width
                    const newChartWidth = newWidth - leftPadding;
                    
                    // Update canvas dimensions
                    chartCanvas.style.width = `${newChartWidth}px`;
                    chartCanvas.width = newChartWidth;
                    
                    // Update overlay dimensions
                    overlayEl.style.width = `${newChartWidth}px`;
                    
                    // Update time axis
                    timeAxisEl.style.width = `${newChartWidth}px`;
                    
                    // Update internal dims
                    this.bbox.width = newChartWidth;
                    
                    // Redraw with new dimensions
                    drawHeatmap();
                    
                    console.log(`Resized heatmap from ${oldWidth}px to ${newWidth}px`);
                }
            }
        },
        
        redraw: function() {
            drawHeatmap();
        },
        
        destroy: function() {
            // Remove tooltip and selection rect from body
            if (document.body.contains(tooltipEl)) {
                document.body.removeChild(tooltipEl);
            }
            if (document.body.contains(selectionRect)) {
                document.body.removeChild(selectionRect);
            }
            // Clear container
            container.innerHTML = '';
        },
        
        // Batch method for compatibility
        batch: function(callback) {
            callback();
            this.redraw();
        },
        
        // Empty setSelect for compatibility
        setSelect: function() {},
        
        // Store data for compatibility with other code
        data: [timestamps],
        _timestamps: timestamps,
        _cpuData: cpuData,
        _numCPUs: numCPUs,
        _colorScale: config.colorScale,
        _minValue: config.minValue,
        _maxValue: config.maxValue,
        
        // Required properties for interaction with zoom controller
        scales: {
            x: {
                min: timestamps[0],
                max: timestamps[timestamps.length - 1]
            }
        },
        
        // Used for positioning
        bbox: {
            left: chartLeft,
            top: chartTop,
            width: chartWidth,
            height: chartHeight
        },
        
        // For tooltips
        posToVal: function(pos, axis) {
            if (axis === 'x') {
                const visibleStart = currentViewMin || timestamps[0];
                const visibleEnd = currentViewMax || timestamps[timestamps.length - 1];
                const visibleRange = visibleEnd - visibleStart;
                
                // Convert pixel position to time value
                const pixelOffset = pos - chartLeft;
                return visibleStart + (pixelOffset / chartWidth) * visibleRange;
            }
            return 0;
        },
        
        valToPos: function(val, axis) {
            if (axis === 'x') {
                const visibleStart = currentViewMin || timestamps[0];
                const visibleEnd = currentViewMax || timestamps[timestamps.length - 1];
                const visibleRange = visibleEnd - visibleStart;
                
                // Convert time value to pixel position
                const timeOffset = val - visibleStart;
                return chartLeft + (timeOffset / visibleRange) * chartWidth;
            }
            return 0;
        },
        
        // For axis references
        axes: [
            {
                _min: timestamps[0],
                _max: timestamps[timestamps.length - 1]
            }
        ],
        
        // Empty hooks for compatibility
        hooks: {
            drawClear: [],
            draw: [],
            ready: [],
            setSize: [],
            setSelect: []
        }
    };
    
    return chartObj;
}

// Replace the original with our implementation
window.createCpuHeatmap = createNewCpuHeatmap;

// Add debug helper function to console
window.debugHeatmap = function() {
    // Find all heatmap charts
    const charts = [];
    if (window.zoomController && window.zoomController.plots) {
        window.zoomController.plots.forEach(plot => {
            if (plot._chart_type === 'heatmap') {
                charts.push(plot);
            }
        });
    }
    
    charts.forEach((chart, i) => {
        console.log(`Heatmap chart ${i}:`, {
            width: chart.width,
            height: chart.height,
            bbox: chart.bbox,
            canvas: chart.canvas,
            overlay: chart.over
        });
        
        // Add visual debug outline
        if (chart.canvas) {
            const canvas = chart.canvas;
            const wrapper = document.createElement('div');
            wrapper.style.cssText = `
                position: absolute;
                top: ${canvas.offsetTop}px;
                left: ${canvas.offsetLeft}px;
                width: ${canvas.offsetWidth}px;
                height: ${canvas.offsetHeight}px;
                border: 2px dashed red;
                z-index: 9999;
                pointer-events: none;
            `;
            chart.root.appendChild(wrapper);
            
            // Add label
            const label = document.createElement('div');
            label.textContent = 'Canvas';
            label.style.cssText = `
                position: absolute;
                top: ${canvas.offsetTop}px;
                left: ${canvas.offsetLeft}px;
                background: red;
                color: white;
                font-size: 10px;
                padding: 2px;
                z-index: 10000;
                pointer-events: none;
            `;
            chart.root.appendChild(label);
        }
        
        if (chart.over) {
            const overlay = chart.over;
            const wrapper = document.createElement('div');
            wrapper.style.cssText = `
                position: absolute;
                top: ${overlay.offsetTop}px;
                left: ${overlay.offsetLeft}px;
                width: ${overlay.offsetWidth}px;
                height: ${overlay.offsetHeight}px;
                border: 2px dashed blue;
                z-index: 9999;
                pointer-events: none;
            `;
            chart.root.appendChild(wrapper);
            
            // Add label
            const label = document.createElement('div');
            label.textContent = 'Overlay';
            label.style.cssText = `
                position: absolute;
                top: ${overlay.offsetTop}px;
                left: ${overlay.offsetLeft + 50}px;
                background: blue;
                color: white;
                font-size: 10px;
                padding: 2px;
                z-index: 10000;
                pointer-events: none;
            `;
            chart.root.appendChild(label);
        }
    });
    
    console.log("Debug visualization added to heatmap charts");
    
    return charts.length ? charts : "No heatmap charts found";
};