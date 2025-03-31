/**
 * heatmap-core.js - Core heatmap functionality and initialization
 * Main entry point for the CPU heatmap visualization
 */

/**
 * Create a CPU heatmap chart with proper alignment and consistent sizing
 * @param {string} containerId - DOM element ID for the chart container
 * @param {Object} data - Data for the heatmap
 * @param {Object} options - Configuration options
 * @returns {Object} Plot object with interface for zoom controller
 */
function createCpuHeatmap(containerId, data, options = {}) {
    const container = document.getElementById(containerId);
    if (!container) {
        console.error(`Container element with ID ${containerId} not found`);
        return null;
    }
    
    // Get dimensions that match line chart exactly
    const originalWidth = options.width || container.clientWidth;
    // Use fixed height to ensure consistency with uPlot
    const originalHeight = options.height || 350;
    
    // Clear the container first
    container.innerHTML = '';
    
    // Set container class and style
    container.className = 'cpu-heatmap uplot'; // Add uplot class for CSS consistency
    container.style.position = 'relative';
    container.style.width = originalWidth + 'px';
    container.style.height = originalHeight + 'px';
    container.style.backgroundColor = '#1E1E1E';
    
    // Create layout elements with exact dimensions - matched to uPlot values
    const titleHeight = 40;            // Height of the title area (match uPlot)
    const timeAxisHeight = 30;         // Height of the bottom time axis
    const leftPadding = 60;            // Width of the left label area
    const rightPadding = 10;           // Right side padding
    const bottomPadding = 10;          // Bottom padding
    
    // Calculate the main chart area dimensions
    const chartTop = titleHeight;
    const chartLeft = leftPadding;
    const chartWidth = originalWidth - leftPadding - rightPadding;
    const chartHeight = originalHeight - titleHeight - timeAxisHeight - bottomPadding;
    
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
    
    // Create all UI elements
    const elements = createHeatmapElements(
        container,
        chartTop,
        chartLeft,
        chartWidth,
        chartHeight,
        leftPadding,
        bottomPadding,
        timeAxisHeight,
        config,
        numCPUs
    );
    
    // Current view state (for synchronized zooming)
    let currentViewMin = null;
    let currentViewMax = null;
    
    // Function to update view state and redraw
    function updateZoom(min, max) {
        currentViewMin = min;
        currentViewMax = max;
        
        // Get view range (all data if null)
        const viewStartTime = currentViewMin || timestamps[0];
        const viewEndTime = currentViewMax || timestamps[timestamps.length - 1];
        
        // Get canvas context
        const ctx = elements.chartCanvas.getContext('2d');
        
        // Redraw the heatmap
        redrawHeatmap(
            elements,
            ctx,
            chartWidth,
            chartHeight,
            timestamps,
            cpuData,
            numCPUs,
            viewStartTime,
            viewEndTime,
            config
        );
        
        // Update time axis labels and grid
        updateTimeAxisLabels(
            elements.timeAxisEl, 
            chartWidth, 
            viewStartTime, 
            viewEndTime
        );
        
        drawGridLines(
            elements.gridEl, 
            numCPUs, 
            chartWidth, 
            chartHeight
        );
        
        // Check alignment
        checkCanvasAlignment(elements.chartCanvas, elements.overlayEl);
    }
    
    // Set up event handlers
    const eventHandlers = setupEventHandlers(
        {
            overlayEl: elements.overlayEl,
            selectionRect: elements.selectionRect,
            tooltipEl: elements.tooltipEl,
            chartCanvas: elements.chartCanvas,
            currentViewMin: currentViewMin,
            currentViewMax: currentViewMax
        },
        chartTop,
        chartLeft,
        chartWidth,
        chartHeight,
        timestamps,
        cpuData,
        numCPUs,
        config,
        updateZoom
    );
    
    // Create chart object with interface compatible with uPlot for zoom controller
    const chartObj = createChartObject(
        container, 
        elements.chartCanvas, 
        elements.overlayEl, 
        originalWidth, 
        originalHeight, 
        chartTop, 
        chartLeft, 
        chartWidth, 
        chartHeight, 
        timestamps, 
        updateZoom,
        () => updateZoom(currentViewMin, currentViewMax),
        eventHandlers.handlers
    );
    
    // Add reference to updateZoom and current view for external access
    chartObj.updateZoom = updateZoom;
    chartObj.currentViewMin = () => currentViewMin;
    chartObj.currentViewMax = () => currentViewMax;
    
    // Initial render
    updateZoom(null, null);
    
    return chartObj;
}

/**
 * Helper function to prepare heatmap data from series data
 * @param {Object} seriesData - Data from the chart configuration
 * @returns {Object} Formatted data for the heatmap
 */
function prepareHeatmapData(seriesData) {
    // Extract CPU data from series
    const cpuData = seriesData.series.map(series => series.values);
    
    return {
        timestamps: seriesData.timestamps,
        cpuData: cpuData
    };
}