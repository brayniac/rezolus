/**
 * heatmap-utils.js - Utility functions for heatmap visualization
 * Contains color handling, time formatting, and other helper functions
 */

/**
 * Get a color for a specific value using the color scale
 * @param {number} value - The value to get color for
 * @param {number} minValue - Minimum value in scale
 * @param {number} maxValue - Maximum value in scale
 * @param {Array} colorScale - Array of colors
 * @returns {string} CSS color string
 */
function getColor(value, minValue, maxValue, colorScale) {
    // Normalize value to 0-1 range
    const normalized = Math.max(0, Math.min(1, (value - minValue) / (maxValue - minValue)));
    
    // Find position in color scale
    const position = normalized * (colorScale.length - 1);
    const index = Math.floor(position);
    const fraction = position - index;
    
    // If exact match to a color in scale
    if (fraction === 0 || index >= colorScale.length - 1) {
        return colorScale[Math.min(index, colorScale.length - 1)];
    }
    
    // Interpolate between two colors
    const color1 = parseColor(colorScale[index]);
    const color2 = parseColor(colorScale[index + 1]);
    
    // Blend the colors
    const r = Math.round(color1.r + fraction * (color2.r - color1.r));
    const g = Math.round(color1.g + fraction * (color2.g - color1.g));
    const b = Math.round(color1.b + fraction * (color2.b - color1.b));
    
    return `rgb(${r}, ${g}, ${b})`;
}

/**
 * Parse a color string to RGB components
 * @param {string} color - CSS color string
 * @returns {Object} Object with r, g, b components
 */
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

/**
 * Format a time label based on visible time range
 * @param {number} timestamp - Unix timestamp in seconds
 * @param {number} viewStartTime - Start of visible range
 * @param {number} viewEndTime - End of visible range
 * @returns {string} Formatted time string
 */
function formatTimeLabel(timestamp, viewStartTime, viewEndTime) {
    const timeSpanSeconds = viewEndTime - viewStartTime;
    
    // Define time intervals similar to time-axis-formatter.js
    const TIME_INTERVALS = [
        { seconds: 1, format: 'HH:mm:ss' },
        { seconds: 5, format: 'HH:mm:ss' },
        { seconds: 15, format: 'HH:mm:ss' },
        { seconds: 30, format: 'HH:mm:ss' },
        { seconds: 60, format: 'HH:mm' },
        { seconds: 5 * 60, format: 'HH:mm' },
        { seconds: 15 * 60, format: 'HH:mm' },
        { seconds: 30 * 60, format: 'HH:mm' },
        { seconds: 60 * 60, format: 'HH:mm' },
        { seconds: 3 * 60 * 60, format: 'HH:mm' },
        { seconds: 6 * 60 * 60, format: 'HH:mm' },
        { seconds: 12 * 60 * 60, format: 'HH:mm' },
        { seconds: 24 * 60 * 60, format: 'MM-DD HH:mm' }
    ];
    
    const targetDensity = timeSpanSeconds / 6;
    let selectedInterval = TIME_INTERVALS[TIME_INTERVALS.length - 1];
    
    for (let i = 0; i < TIME_INTERVALS.length; i++) {
        if (TIME_INTERVALS[i].seconds >= targetDensity) {
            selectedInterval = TIME_INTERVALS[i];
            break;
        }
    }
    
    const date = new Date(timestamp * 1000);
    
    // Handle different format strings
    if (selectedInterval.format === 'HH:mm:ss') {
        return date.toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
            hour12: false
        });
    } else if (selectedInterval.format === 'HH:mm') {
        return date.toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            hour12: false
        });
    } else if (selectedInterval.format === 'MM-DD HH:mm') {
        return `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')} ${date.toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            hour12: false
        })}`;
    }
    
    return date.toLocaleTimeString();
}

/**
 * Create a chart object that's compatible with uPlot
 * @param {Element} container - Container element
 * @param {Element} chartCanvas - Canvas element 
 * @param {Element} overlayEl - Overlay element
 * @param {number} width - Chart width
 * @param {number} height - Chart height
 * @param {number} chartTop - Top offset
 * @param {number} chartLeft - Left offset
 * @param {number} chartWidth - Chart area width
 * @param {number} chartHeight - Chart area height
 * @param {Array} timestamps - Array of timestamps
 * @param {Function} updateZoom - Function to update zoom
 * @param {Function} redraw - Function to redraw heatmap
 * @param {Object} handlers - Event handlers
 * @returns {Object} Chart object
 */
function createChartObject(container, chartCanvas, overlayEl, width, height, chartTop, chartLeft, 
                          chartWidth, chartHeight, timestamps, updateZoom, redraw, handlers) {
    return {
        root: container,
        canvas: chartCanvas,
        over: overlayEl,
        width: width,
        height: height,
        _chart_type: 'heatmap',
        
        // Required methods for zoom controller integration
        setScale: function(axis, { min, max }) {
            if (axis === 'x') {
                updateZoom(min, max);
            }
        },
        
        updateZoom: updateZoom,
        
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
                    const newChartWidth = newWidth - chartLeft - 10; // 10 is rightPadding
                    
                    // Update internal dimensions
                    this.bbox.width = newChartWidth;
                    
                    // Find and update elements
                    const canvasWrapper = container.querySelector('.cpu-heatmap-wrapper');
                    const canvas = container.querySelector('.cpu-heatmap-canvas');
                    const overlay = container.querySelector('.u-over');
                    const timeAxis = container.querySelector('.time-axis');
                    const grid = container.querySelector('.heatmap-grid');
                    
                    if (canvasWrapper) canvasWrapper.style.width = `${newChartWidth}px`;
                    if (canvas) {
                        canvas.width = newChartWidth;
                        canvas.style.width = `${newChartWidth}px`;
                    }
                    if (overlay) overlay.style.width = `${newChartWidth}px`;
                    if (timeAxis) timeAxis.style.width = `${newChartWidth}px`;
                    if (grid) grid.style.width = `${newChartWidth}px`;
                    
                    // Redraw with new dimensions
                    redraw();
                }
            }
        },
        
        redraw: redraw,
        
        destroy: function() {
            // Clean up event listeners
            Object.entries(handlers).forEach(([event, handler]) => {
                overlayEl.removeEventListener(event, handler);
            });
            
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
        
        // For tooltips and coordinates
        posToVal: function(pos, axis) {
            if (axis === 'x') {
                const visibleStart = this.currentViewMin || timestamps[0];
                const visibleEnd = this.currentViewMax || timestamps[timestamps.length - 1];
                const visibleRange = visibleEnd - visibleStart;
                
                // Convert pixel position to time value
                return visibleStart + ((pos - chartLeft) / chartWidth) * visibleRange;
            }
            return 0;
        },
        
        valToPos: function(val, axis) {
            if (axis === 'x') {
                const visibleStart = this.currentViewMin || timestamps[0];
                const visibleEnd = this.currentViewMax || timestamps[timestamps.length - 1];
                const visibleRange = visibleEnd - visibleStart;
                
                // Convert time value to pixel position
                return chartLeft + ((val - visibleStart) / visibleRange) * chartWidth;
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
            drawClear: [
                function() {
                    const ctx = chartCanvas.getContext('2d');
                    ctx.clearRect(0, 0, chartWidth, chartHeight);
                }
            ],
            draw: [],
            ready: [],
            setSize: [],
            setSelect: []
        }
    };
}

/**
 * Fixes for browser-specific rendering issues
 */
function applyBrowserHacks() {
    // Check if hacks have already been applied
    if (window._heatmapBrowserHacksApplied) return;
    
    const isFirefox = navigator.userAgent.toLowerCase().indexOf('firefox') > -1;
    const isSafari = /^((?!chrome|android).)*safari/i.test(navigator.userAgent);
    
    // Add extra styles for specific browsers
    const style = document.createElement('style');
    style.textContent = `
        /* Common fixes for all browsers */
        .cpu-heatmap-wrapper {
            will-change: transform;
            transform: translateZ(0);
        }
        
        .cpu-heatmap-canvas {
            will-change: transform;
            transform: translateZ(0);
        }
        
        /* Firefox-specific fixes */
        ${isFirefox ? `
        .cpu-heatmap-canvas {
            image-rendering: -moz-crisp-edges;
        }` : ''}
        
        /* Safari-specific fixes */
        ${isSafari ? `
        .cpu-heatmap-canvas {
            -webkit-backface-visibility: hidden;
            transform: translate3d(0, 0, 0);
        }` : ''}
    `;
    
    document.head.appendChild(style);
    window._heatmapBrowserHacksApplied = true;
}

// Apply browser hacks when loaded
applyBrowserHacks();