/**
 * heatmap-events.js - Event handling for heatmap interactions
 * Manages all mouse interactions and event handlers
 */

/**
 * Set up event handlers for the heatmap
 * @param {Object} elements - UI elements
 * @param {number} chartTop - Top offset of chart
 * @param {number} chartLeft - Left offset of chart
 * @param {number} chartWidth - Width of chart
 * @param {number} chartHeight - Height of chart
 * @param {Array} timestamps - Array of timestamps
 * @param {Array} cpuData - CPU data array
 * @param {number} numCPUs - Number of CPUs
 * @param {Object} config - Configuration object
 * @param {Function} updateZoom - Function to update zoom
 * @returns {Object} Event handlers and API
 */
function setupEventHandlers(elements, chartTop, chartLeft, chartWidth, chartHeight, 
                          timestamps, cpuData, numCPUs, config, updateZoom) {
    // Selection state variables
    let selecting = false;
    let selectionStart = null;
    
    // Cell height for tooltip calculations
    const cellHeight = chartHeight / numCPUs;
    
    // Store current view bounds for access in handlers
    let currentViewMin = elements.currentViewMin || null;
    let currentViewMax = elements.currentViewMax || null;
    
    // Update the reference to current view boundaries
    function setCurrentView(min, max) {
        currentViewMin = min;
        currentViewMax = max;
    }
    
    // Handler for mouse down (start selection)
    function handleMouseDown(e) {
        const rect = elements.overlayEl.getBoundingClientRect();
        const x = e.clientX - rect.left;
        
        // Only process if within bounds
        if (x < 0 || x > chartWidth) {
            return;
        }
        
        selecting = true;
        selectionStart = x;
        
        // Position selection rectangle within chart bounds
        elements.selectionRect.style.display = 'block';
        elements.selectionRect.style.left = `${chartLeft + x}px`;
        elements.selectionRect.style.top = `${chartTop}px`;
        elements.selectionRect.style.width = '0px';
        elements.selectionRect.style.height = `${chartHeight}px`;
    }
    
    // Handler for mouse move (update selection or show tooltip)
    function handleMouseMove(e) {
        const rect = elements.overlayEl.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        if (selecting) {
            if (x < 0 || x > chartWidth) {
                return; // Don't process outside bounds
            }
            
            // Calculate width and position relative to chart
            const width = Math.abs(x - selectionStart);
            const left = Math.min(x, selectionStart);
            
            elements.selectionRect.style.left = `${chartLeft + left}px`;
            elements.selectionRect.style.width = `${width}px`;
        } else {
            // Handle tooltips when not selecting
            handleTooltip(e, rect, x, y);
        }
    }
    
    // Handler for tooltip display
    function handleTooltip(e, rect, x, y) {
        if (x < 0 || x > chartWidth || y < 0 || y > chartHeight) {
            elements.tooltipEl.style.display = 'none';
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
                elements.tooltipEl.style.display = 'none';
                return;
            }
            
            // Check if we have valid data
            if (!cpuData[actualCpuIdx] || typeof cpuData[actualCpuIdx][closestIdx] === 'undefined') {
                elements.tooltipEl.style.display = 'none';
                return;
            }
            
            const value = cpuData[actualCpuIdx][closestIdx];
            
            // Format timestamp
            const formattedDate = formatDateTimeISO(timestamps[closestIdx]);
            
            // Create tooltip content
            elements.tooltipEl.innerHTML = `
                <div class="header">${formattedDate}</div>
                <div class="value-row">
                    <span class="label">
                        <span class="color-dot" style="background-color: ${getColor(value, config.minValue, config.maxValue, config.colorScale)};"></span>
                        CPU ${actualCpuIdx}:
                    </span>
                    <span class="value">${value.toFixed(1)}${config.units}</span>
                </div>
            `;
            
            elements.tooltipEl.style.display = 'block';
            
            // Position tooltip near cursor
            let tooltipX = x + 10;
            let tooltipY = y + 10;
            
            // Adjust position to keep tooltip on screen
            const tooltipWidth = elements.tooltipEl.offsetWidth || 200;
            const tooltipHeight = elements.tooltipEl.offsetHeight || 80;
            
            if (tooltipX + tooltipWidth > chartWidth - 10) {
                tooltipX = x - tooltipWidth - 10;
            }
            
            if (tooltipY + tooltipHeight > chartHeight - 10) {
                tooltipY = y - tooltipHeight - 10;
            }
            
            elements.tooltipEl.style.left = `${tooltipX}px`;
            elements.tooltipEl.style.top = `${tooltipY}px`;
            
        } catch (err) {
            console.error("Error handling tooltip:", err);
            elements.tooltipEl.style.display = 'none';
        }
    }
    
    // Handler for mouse up (end selection)
    function handleMouseUp(e) {
        if (selecting) {
            selecting = false;
            elements.selectionRect.style.display = 'none';
            
            const rect = elements.overlayEl.getBoundingClientRect();
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
                    setCurrentView(startTime, endTime);
                    updateZoom(startTime, endTime);
                    
                    // If zoom controller exists, use it
                    if (typeof window.zoomController !== 'undefined') {
                        window.zoomController.syncZoom({_id: 'heatmap'}, startTime, endTime);
                    }
                }
            }
        }
    }
    
    // Double-click to reset zoom
    function handleDoubleClick() {
        // Reset our view
        setCurrentView(null, null);
        updateZoom(null, null);
        
        // If zoom controller exists, use it
        if (typeof window.zoomController !== 'undefined') {
            window.zoomController.resetZoom();
        }
    }
    
    // Hide tooltip when mouse leaves overlay
    function handleMouseOut() {
        elements.tooltipEl.style.display = 'none';
    }
    
    // Add event listeners
    elements.overlayEl.addEventListener('mousedown', handleMouseDown);
    elements.overlayEl.addEventListener('mousemove', handleMouseMove);
    elements.overlayEl.addEventListener('mouseup', handleMouseUp);
    elements.overlayEl.addEventListener('mouseleave', handleMouseUp);
    elements.overlayEl.addEventListener('dblclick', handleDoubleClick);
    elements.overlayEl.addEventListener('mouseout', handleMouseOut);
    
    // Return handlers for cleanup and an API for external access
    return {
        handlers: {
            mousedown: handleMouseDown,
            mousemove: handleMouseMove,
            mouseup: handleMouseUp,
            mouseleave: handleMouseUp,
            dblclick: handleDoubleClick,
            mouseout: handleMouseOut
        },
        setCurrentView: setCurrentView,
        getCurrentView: () => ({ min: currentViewMin, max: currentViewMax })
    };
}

/**
 * Format a date/time as ISO string
 * @param {number} timestamp - Unix timestamp (seconds)
 * @returns {string} Formatted date string
 */
function formatDateTimeISO(timestamp) {
    const date = new Date(timestamp * 1000);
    
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    const hours = String(date.getHours()).padStart(2, '0');
    const minutes = String(date.getMinutes()).padStart(2, '0');
    const seconds = String(date.getSeconds()).padStart(2, '0');
    
    return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
}

/**
 * Create an API for synchronizing zoom with external controllers
 * @param {Object} elements - The UI elements
 * @param {Object} eventHandlers - The event handler functions
 * @param {Function} updateZoom - The function to call to update zoom
 * @param {Function} redraw - The function to redraw the chart
 * @returns {Object} API object with zoom methods
 */
function createZoomAPI(elements, eventHandlers, updateZoom, redraw) {
    return {
        // Set the zoom range
        setZoom: function(min, max) {
            eventHandlers.setCurrentView(min, max);
            updateZoom(min, max);
            redraw();
        },
        
        // Reset zoom to show all data
        resetZoom: function() {
            eventHandlers.setCurrentView(null, null);
            updateZoom(null, null);
            redraw();
        },
        
        // Get current zoom state
        getZoom: function() {
            return eventHandlers.getCurrentView();
        }
    };
}