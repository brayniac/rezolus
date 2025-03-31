/**
 * heatmap-elements.js - UI elements creation and management
 * Creates and manages all DOM elements for the heatmap visualization
 */

/**
 * Create all UI elements for the heatmap
 * @param {Element} container - Container element
 * @param {number} chartTop - Top offset for chart
 * @param {number} chartLeft - Left offset for chart
 * @param {number} chartWidth - Width of chart
 * @param {number} chartHeight - Height of chart
 * @param {number} leftPadding - Left padding
 * @param {number} bottomPadding - Bottom padding
 * @param {number} timeAxisHeight - Height of time axis
 * @param {Object} config - Configuration options
 * @param {number} numCPUs - Number of CPUs to display
 * @returns {Object} Created elements
 */
function createHeatmapElements(container, chartTop, chartLeft, chartWidth, chartHeight, 
                              leftPadding, bottomPadding, timeAxisHeight, config, numCPUs) {
    // Create title element
    const titleEl = document.createElement('div');
    titleEl.className = 'u-title'; // Use same class as uPlot for consistency
    titleEl.textContent = config.title;
    titleEl.style.cssText = `
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: ${chartTop}px;
        line-height: ${chartTop}px;
        text-align: center;
        font-size: 16px;
        font-weight: 500;
        color: #CCCCCC;
        margin: 0;
        padding: 0;
    `;
    container.appendChild(titleEl);
    
    // Create legend container
    const legendContainer = document.createElement('div');
    legendContainer.className = 'heatmap-legend-container';
    container.appendChild(legendContainer);
    
    // Create the color scale legend
    createColorLegend(legendContainer, config.colorScale, config.minValue, config.maxValue, config.units);
    
    // Create wrapper for the heatmap canvas with exact positioning
    const canvasWrapper = document.createElement('div');
    canvasWrapper.className = 'cpu-heatmap-wrapper';
    canvasWrapper.style.cssText = `
        position: absolute;
        top: ${chartTop}px;
        left: ${chartLeft}px;
        width: ${chartWidth}px;
        height: ${chartHeight}px;
        overflow: hidden;
        z-index: 10;
    `;
    container.appendChild(canvasWrapper);
    
    // Create the main chart canvas with proper dimensions
    const chartCanvas = document.createElement('canvas');
    chartCanvas.className = 'cpu-heatmap-canvas';
    chartCanvas.width = chartWidth;
    chartCanvas.height = chartHeight;
    chartCanvas.style.cssText = `
        position: absolute;
        top: 0;
        left: 0;
        width: ${chartWidth}px;
        height: ${chartHeight}px;
        image-rendering: pixelated;
    `;
    canvasWrapper.appendChild(chartCanvas);
    
    // Calculate cell height
    const cellHeight = chartHeight / numCPUs;
    
    // Draw CPU labels with exact positioning to match Y-axis in uPlot
    for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
        // Reverse order (CPU0 at bottom)
        const reversedCpuIdx = numCPUs - cpuIdx - 1;
        
        // Calculate vertical position (center of cell)
        const labelY = chartTop + reversedCpuIdx * cellHeight + (cellHeight / 2);
        
        const labelEl = document.createElement('div');
        labelEl.textContent = `CPU ${cpuIdx}`;
        labelEl.className = 'cpu-label';
        labelEl.style.cssText = `
            position: absolute;
            top: ${labelY}px;
            left: 5px;
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
    
    // Create time axis with matching style to uPlot
    const timeAxisEl = document.createElement('div');
    timeAxisEl.className = 'time-axis';
    timeAxisEl.style.cssText = `
        position: absolute;
        bottom: ${bottomPadding}px;
        left: ${chartLeft}px;
        width: ${chartWidth}px;
        height: ${timeAxisHeight}px;
    `;
    container.appendChild(timeAxisEl);
    
    // Create the grid before the canvas for proper z-ordering
    const gridEl = document.createElement('div');
    gridEl.className = 'heatmap-grid';
    gridEl.style.cssText = `
        position: absolute;
        top: ${chartTop}px;
        left: ${chartLeft}px;
        width: ${chartWidth}px;
        height: ${chartHeight}px;
        z-index: 5;
        pointer-events: none;
    `;
    container.appendChild(gridEl);
    
    // Create overlay for mouse interactions that matches the canvas exactly
    const overlayEl = document.createElement('div');
    overlayEl.className = 'u-over'; // Same class as uPlot for consistency
    overlayEl.style.cssText = `
        position: absolute;
        top: ${chartTop}px;
        left: ${chartLeft}px;
        width: ${chartWidth}px;
        height: ${chartHeight}px;
        cursor: crosshair;
        z-index: 20;
        box-sizing: border-box;
    `;
    container.appendChild(overlayEl);
    
    // Create selection rectangle with fixed position
    const selectionRect = document.createElement('div');
    selectionRect.className = 'heatmap-selection';
    selectionRect.style.cssText = `
        position: absolute;
        display: none;
        background-color: rgba(86, 156, 214, 0.2);
        border: 1px solid rgba(86, 156, 214, 0.5);
        pointer-events: none;
        z-index: 20;
    `;
    container.appendChild(selectionRect);
    
    // Create tooltip element
    const tooltipEl = document.createElement('div');
    tooltipEl.className = 'heatmap-tooltip';
    tooltipEl.style.cssText = `
        position: absolute;
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
    container.appendChild(tooltipEl);
    
    return {
        titleEl,
        legendContainer,
        canvasWrapper,
        chartCanvas,
        timeAxisEl,
        gridEl,
        overlayEl,
        selectionRect,
        tooltipEl
    };
}

/**
 * Create the color legend element
 * @param {Element} container - Container element
 * @param {Array} colorScale - Array of color values
 * @param {number} minValue - Minimum value
 * @param {number} maxValue - Maximum value
 * @param {string} units - Units string
 */
function createColorLegend(container, colorScale, minValue, maxValue, units) {
    container.innerHTML = '';
    container.style.display = 'flex';
    container.style.flexDirection = 'row';
    container.style.alignItems = 'center';
    container.style.gap = '4px';
    
    // Add gradient bar
    const gradientBar = document.createElement('div');
    gradientBar.style.width = '100px';
    gradientBar.style.height = '8px';
    gradientBar.style.position = 'relative';
    
    // Create the gradient
    let gradient = 'linear-gradient(to right';
    colorScale.forEach((color, index) => {
        const percent = (index / (colorScale.length - 1)) * 100;
        gradient += `, ${color} ${percent}%`;
    });
    gradient += ')';
    
    gradientBar.style.background = gradient;
    gradientBar.style.borderRadius = '2px';
    
    container.appendChild(gradientBar);
    
    // Add min and max labels
    const minLabel = document.createElement('span');
    minLabel.textContent = `${minValue}${units}`;
    minLabel.style.fontSize = '10px';
    minLabel.style.color = '#AAAAAA';
    
    const maxLabel = document.createElement('span');
    maxLabel.textContent = `${maxValue}${units}`;
    maxLabel.style.fontSize = '10px';
    maxLabel.style.color = '#AAAAAA';
    
    container.appendChild(minLabel);
    container.appendChild(maxLabel);
}

/**
 * Update time axis labels based on current view
 * @param {Element} timeAxisEl - Time axis element
 * @param {number} chartWidth - Width of chart area
 * @param {number} viewStartTime - Start time of current view
 * @param {number} viewEndTime - End time of current view
 */
function updateTimeAxisLabels(timeAxisEl, chartWidth, viewStartTime, viewEndTime) {
    // Clear existing labels
    timeAxisEl.innerHTML = '';
    
    // Get the time span
    const timeSpan = viewEndTime - viewStartTime;
    
    // Determine how many labels based on width (similar to uPlot)
    const idealSpacing = 80; // pixels between labels
    const numLabels = Math.max(2, Math.floor(chartWidth / idealSpacing));
    
    for (let i = 0; i <= numLabels; i++) {
        const percent = i / numLabels;
        const time = viewStartTime + percent * timeSpan;
        const labelX = percent * chartWidth;
        
        // Format time using our consistent formatter
        const timeStr = formatTimeLabel(time, viewStartTime, viewEndTime);
        
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
}

/**
 * Draw grid lines to match uPlot styling
 * @param {Element} gridContainer - Grid container element
 * @param {number} numCPUs - Number of CPUs
 * @param {number} width - Width of grid area
 * @param {number} height - Height of grid area
 */
function drawGridLines(gridContainer, numCPUs, width, height) {
    gridContainer.innerHTML = '';
    
    // Add horizontal grid lines
    for (let i = 1; i < numCPUs; i++) {
        const y = (i / numCPUs) * height;
        
        const line = document.createElement('div');
        line.style.cssText = `
            position: absolute;
            left: 0;
            top: ${y}px;
            width: 100%;
            height: 1px;
            background-color: rgba(136, 136, 136, 0.1);
            pointer-events: none;
        `;
        gridContainer.appendChild(line);
    }
    
    // Add vertical grid lines (fewer, to match uPlot)
    const vertLines = 6; // Adjust to match uPlot
    for (let i = 1; i < vertLines; i++) {
        const x = (i / vertLines) * width;
        
        const line = document.createElement('div');
        line.style.cssText = `
            position: absolute;
            left: ${x}px;
            top: 0;
            width: 1px;
            height: 100%;
            background-color: rgba(136, 136, 136, 0.1);
            pointer-events: none;
        `;
        gridContainer.appendChild(line);
    }
}

/**
 * Redraw the heatmap with the current configuration
 * @param {Object} elements - All UI elements
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} chartWidth - Width of chart
 * @param {number} chartHeight - Height of chart
 * @param {Array} timestamps - Timestamp data
 * @param {Array} cpuData - CPU utilization data
 * @param {number} numCPUs - Number of CPUs
 * @param {number} viewStartTime - Start time of view range
 * @param {number} viewEndTime - End time of view range
 * @param {Object} config - Configuration options
 */
function redrawHeatmap(elements, ctx, chartWidth, chartHeight, /* other params */) {
    // Force canvas dimensions to match overlay before drawing
    const canvas = elements.chartCanvas;
    const overlay = elements.overlayEl;
    
    if (canvas && overlay) {
        // Check if dimensions match
        if (canvas.width !== overlay.offsetWidth || 
            canvas.height !== overlay.offsetHeight) {
            console.log('Fixing canvas dimensions before redraw');
            canvas.width = overlay.offsetWidth;
            canvas.height = overlay.offsetHeight;
            canvas.style.width = `${overlay.offsetWidth}px`;
            canvas.style.height = `${overlay.offsetHeight}px`;
        }
    }
    
    // Continue with existing clear and drawing operations
    ctx.clearRect(0, 0, chartWidth, chartHeight);
    
    // Calculate time to pixel conversion
    const timeToPixel = (time) => {
        const timeRange = viewEndTime - viewStartTime;
        const timeOffset = time - viewStartTime;
        return (timeOffset / timeRange) * chartWidth;
    };
    
    // Calculate cell height
    const cellHeight = chartHeight / numCPUs;
    
    // Draw each CPU row
    for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
        const reversedCpuIdx = numCPUs - cpuIdx - 1;
        const cpuValues = cpuData[cpuIdx];
        
        // Y position for this CPU
        const yPos = reversedCpuIdx * cellHeight;
        
        // Optimize by batching segments of similar colors
        let batchStart = null;
        let lastValue = null;
        let lastX = null;
        
        // Process all data points
        for (let i = 0; i < timestamps.length - 1; i++) {
            const time1 = timestamps[i];
            const time2 = timestamps[i + 1];
            const value = cpuValues[i];
            
            // Skip if outside view range
            if (time2 < viewStartTime || time1 > viewEndTime) {
                // If we were batching, draw the batch before skipping
                if (batchStart !== null) {
                    drawBatch(ctx, batchStart, lastX, lastValue, yPos, cellHeight, config);
                    batchStart = null;
                }
                continue;
            }
            
            // Get pixel positions
            const x1 = timeToPixel(Math.max(time1, viewStartTime));
            const x2 = timeToPixel(Math.min(time2, viewEndTime));
            
            // Skip if segment is too narrow
            if (x2 - x1 < 0.5) continue;
            
            // Start a new batch if needed
            if (batchStart === null || Math.abs(value - lastValue) > 2) {
                // Draw previous batch if exists
                if (batchStart !== null) {
                    drawBatch(ctx, batchStart, lastX, lastValue, yPos, cellHeight, config);
                }
                
                // Start new batch
                batchStart = x1;
                lastValue = value;
            }
            
            lastX = x2;
        }
        
        // Draw final batch if exists
        if (batchStart !== null) {
            drawBatch(ctx, batchStart, lastX, lastValue, yPos, cellHeight, config);
        }
    }
}

/**
 * Draw a batch of segments with the same color
 * @param {CanvasRenderingContext2D} ctx - Canvas context
 * @param {number} startX - Start X position
 * @param {number} endX - End X position
 * @param {number} value - Data value
 * @param {number} yPos - Y position
 * @param {number} height - Cell height
 * @param {Object} config - Configuration with color scale
 */
function drawBatch(ctx, startX, endX, value, yPos, height, config) {
    const color = getColor(value, config.minValue, config.maxValue, config.colorScale);
    ctx.fillStyle = color;
    ctx.fillRect(startX, yPos, endX - startX, height);
    
    // Add borders for larger batches
    if (endX - startX > 4) {
        ctx.strokeStyle = 'rgba(0,0,0,0.1)';
        ctx.lineWidth = 0.5;
        ctx.strokeRect(startX, yPos, endX - startX, height);
    }
}

/**
 * Check and log canvas alignment issues
 * @param {Element} canvasElement - Canvas element
 * @param {Element} overlayElement - Overlay element for interaction
 */
function checkCanvasAlignment(canvasElement, overlayElement) {
    if (!canvasElement || !overlayElement) return;
    
    const canvas = {
        width: canvasElement.width, 
        height: canvasElement.height,
        offsetLeft: canvasElement.offsetLeft,
        offsetTop: canvasElement.offsetTop
    };
    
    const overlay = {
        width: overlayElement.offsetWidth,
        height: overlayElement.offsetHeight,
        offsetLeft: overlayElement.offsetLeft,
        offsetTop: overlayElement.offsetTop
    };
    
    console.log('Canvas alignment check:');
    console.log('  Canvas:', canvas);
    console.log('  Overlay:', overlay);
    
    // Check for misalignment
    if (Math.abs(canvas.width - overlay.width) > 5 || 
        Math.abs(canvas.height - overlay.height) > 5) {
        console.warn('⚠️ Canvas size mismatch detected!');
        
        // Fix canvas dimensions
        canvasElement.width = overlay.width;
        canvasElement.height = overlay.height;
        canvasElement.style.width = `${overlay.width}px`;
        canvasElement.style.height = `${overlay.height}px`;
    }
}

/**
 * Add CSS styles for the heatmap
 */
function addHeatmapStyles() {
    // Check if styles already exist
    if (document.getElementById('heatmap-styles')) return;
    
    const styleEl = document.createElement('style');
    styleEl.id = 'heatmap-styles';
    styleEl.textContent = `
        .cpu-heatmap {
            height: 350px !important;
        }
        
        .heatmap-legend-container {
            position: absolute !important;
            bottom: 45px !important;
            left: 70px !important;
            z-index: 100 !important;
            background-color: rgba(30, 30, 30, 0.8);
            border: 1px solid #333;
            border-radius: 4px;
            padding: 5px;
        }
        
        .cpu-label {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif !important;
            font-size: 11px !important;
            color: #888888 !important;
        }
        
        .heatmap-tooltip {
            z-index: 1000 !important;
        }
        
        .heatmap-tooltip .header {
            margin-bottom: 6px;
            font-weight: bold;
            color: #ddd;
            border-bottom: 1px solid #555;
            padding-bottom: 4px;
        }
        
        .heatmap-tooltip .value-row {
            display: flex;
            justify-content: space-between;
            margin: 3px 0;
            align-items: center;
        }
        
        .heatmap-tooltip .label {
            margin-right: 16px;
            display: flex;
            align-items: center;
        }
        
        .heatmap-tooltip .color-dot {
            display: inline-block;
            width: 8px;
            height: 8px;
            border-radius: 50%;
            margin-right: 5px;
        }
        
        .heatmap-tooltip .value {
            font-weight: bold;
            font-variant-numeric: tabular-nums;
        }
        
        .cpu-heatmap-canvas {
            visibility: visible !important;
            opacity: 1 !important;
            display: block !important;
            image-rendering: pixelated;
        }
        
        .cpu-heatmap-wrapper {
            visibility: visible !important;
            opacity: 1 !important;
            display: block !important;
            overflow: visible;
        }
    `;
    document.head.appendChild(styleEl);
}