/**
 * heatmap-integration.js
 * Main script to integrate all heatmap components and fix alignment issues
 * 
 * This file should be included in your project AFTER loading:
 * - heatmap-utils.js
 * - heatmap-elements.js
 * - heatmap-events.js
 * - heatmap-core.js
 */

function fixCanvasRendering() {
    const canvases = document.querySelectorAll('.cpu-heatmap-canvas');
    
    canvases.forEach((canvas, idx) => {
        // Find parent heatmap and related elements
        const heatmap = canvas.closest('.cpu-heatmap');
        if (!heatmap) return;
        
        const over = heatmap.querySelector('.u-over');
        const wrapper = canvas.closest('.cpu-heatmap-wrapper');
        
        if (!over || !wrapper) return;
        
        console.log(`Fixing canvas rendering for heatmap #${idx}`);
        
        // Ensure wrapper is properly positioned
        wrapper.style.position = 'absolute';
        wrapper.style.top = `${over.offsetTop}px`;
        wrapper.style.left = `${over.offsetLeft}px`;
        wrapper.style.width = `${over.offsetWidth}px`;
        wrapper.style.height = `${over.offsetHeight}px`;
        wrapper.style.zIndex = '10';
        wrapper.style.overflow = 'visible';
        
        // Position canvas precisely at 0,0 within wrapper
        canvas.style.position = 'absolute';
        canvas.style.top = '0px';
        canvas.style.left = '0px';
        canvas.style.width = `${over.offsetWidth}px`;
        canvas.style.height = `${over.offsetHeight}px`;
        
        // Fix the actual dimensions to match
        canvas.width = over.offsetWidth;
        canvas.height = over.offsetHeight;
        
        // Set high visibility
        canvas.style.visibility = 'visible';
        canvas.style.opacity = '1';
        canvas.style.display = 'block';
        
        // Force redraw directly on the canvas with a visible pattern to check if rendering works
        const ctx = canvas.getContext('2d');
        
        // Clear the canvas first
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        
        // Find plot object for this canvas to get data
        let plotObj = null;
        if (window.zoomController && window.zoomController.plots) {
            plotObj = window.zoomController.plots.find(p => 
                p._chart_type === 'heatmap' && p.canvas === canvas
            );
        }
        
        if (plotObj) {
            console.log(`Found plot data for heatmap #${idx}, forcing redraw`);
            
            // Redraw with the plot's data
            if (typeof plotObj.redraw === 'function') {
                plotObj.redraw();
            }
        } else {
            // Draw a test pattern to check if canvas is rendering at all
            console.log(`No plot data found for heatmap #${idx}, drawing test pattern`);
            
            // Draw a colorful grid
            const cellHeight = canvas.height / 8; // Assuming 8 CPUs
            
            for (let i = 0; i < 8; i++) { // 8 CPUs
                const yPos = i * cellHeight;
                
                // Draw alternating color bands
                for (let j = 0; j < 20; j++) {
                    const xPos = j * (canvas.width / 20);
                    const width = canvas.width / 20;
                    
                    // Create a gradient of colors
                    const hue = (i * 30 + j * 15) % 360;
                    ctx.fillStyle = `hsl(${hue}, 80%, 60%)`;
                    ctx.fillRect(xPos, yPos, width, cellHeight);
                    
                    // Add border
                    ctx.strokeStyle = 'rgba(0,0,0,0.2)';
                    ctx.lineWidth = 0.5;
                    ctx.strokeRect(xPos, yPos, width, cellHeight);
                }
            }
            
            // Add CPU labels directly on canvas
            ctx.fillStyle = 'white';
            ctx.font = '12px sans-serif';
            
            for (let i = 0; i < 8; i++) {
                const yPos = i * cellHeight + cellHeight/2;
                ctx.fillText(`CPU ${7-i}`, 10, yPos);
            }
        }
        
        console.log(`Canvas rendering fix applied for heatmap #${idx}`);
    });
}

/**
 * Enhanced implementation to fix the very specific issue
 */
function fixHeatmapPositioning() {
    // Find all heatmap containers
    const heatmaps = document.querySelectorAll('.cpu-heatmap');
    
    heatmaps.forEach((heatmap, idx) => {
        // Get all the key elements
        const canvas = heatmap.querySelector('.cpu-heatmap-canvas');
        const wrapper = heatmap.querySelector('.cpu-heatmap-wrapper');
        const over = heatmap.querySelector('.u-over');
        
        if (!canvas || !over) return;
        
        console.log(`[fixHeatmapPositioning] Fixing heatmap #${idx} positioning`);
        
        // Force correct wrapper and canvas positioning
        if (wrapper) {
            // Position wrapper at exact overlay position
            wrapper.style.position = 'absolute';
            wrapper.style.top = `${over.offsetTop}px`;
            wrapper.style.left = `${over.offsetLeft}px`;
            wrapper.style.width = `${over.offsetWidth}px`;
            wrapper.style.height = `${over.offsetHeight}px`;
            wrapper.style.zIndex = '10';
            wrapper.style.backgroundColor = 'transparent';
            wrapper.style.overflow = 'visible';
            wrapper.style.visibility = 'visible';
            wrapper.style.display = 'block';
        } else {
            // Create wrapper if it doesn't exist
            const newWrapper = document.createElement('div');
            newWrapper.className = 'cpu-heatmap-wrapper';
            newWrapper.style.cssText = `
                position: absolute;
                top: ${over.offsetTop}px;
                left: ${over.offsetLeft}px;
                width: ${over.offsetWidth}px;
                height: ${over.offsetHeight}px;
                z-index: 10;
                background-color: transparent;
                overflow: visible;
                visibility: visible;
                display: block;
            `;
            
            // Move canvas into new wrapper
            canvas.parentElement.insertBefore(newWrapper, canvas);
            newWrapper.appendChild(canvas);
        }
        
        // Ensure canvas is positioned and sized correctly
        canvas.style.position = 'absolute';
        canvas.style.top = '0px';
        canvas.style.left = '0px';
        canvas.style.width = `${over.offsetWidth}px`;
        canvas.style.height = `${over.offsetHeight}px`;
        canvas.width = over.offsetWidth;
        canvas.height = over.offsetHeight;
        canvas.style.visibility = 'visible';
        canvas.style.opacity = '1';
        canvas.style.display = 'block';
        
        // Also fix any potential CSS issues
        const css = `
            #${heatmap.id} .cpu-heatmap-canvas {
                visibility: visible !important;
                opacity: 1 !important;
                display: block !important;
                z-index: 10 !important;
            }
            
            #${heatmap.id} .cpu-heatmap-wrapper {
                visibility: visible !important;
                opacity: 1 !important;
                display: block !important;
                overflow: visible !important;
                z-index: 10 !important;
            }
        `;
        
        // Add chart-specific CSS
        const styleId = `heatmap-fix-${heatmap.id}`;
        let styleEl = document.getElementById(styleId);
        
        if (!styleEl) {
            styleEl = document.createElement('style');
            styleEl.id = styleId;
            document.head.appendChild(styleEl);
        }
        
        styleEl.textContent = css;
        
        console.log(`[fixHeatmapPositioning] Positioning fixed for heatmap #${idx}`);
    });
}

/**
 * Function to manually redraw a heatmap
 * @param {HTMLElement} heatmap - The heatmap container element
 * @param {Array} data - The data to render [Optional]
 */
function forceHeatmapRedraw(heatmap, data) {
    const canvas = heatmap.querySelector('.cpu-heatmap-canvas');
    if (!canvas) return;
    
    const ctx = canvas.getContext('2d');
    
    // Clear the canvas first
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    
    // Try to find the plot object for this heatmap
    let plotObj = null;
    if (window.zoomController && window.zoomController.plots) {
        plotObj = window.zoomController.plots.find(p => 
            p._chart_type === 'heatmap' && p.root === heatmap
        );
    }
    
    if (plotObj && typeof plotObj.redraw === 'function') {
        console.log(`Found plot object, forcing redraw`);
        plotObj.redraw();
        return;
    }
    
    // If no plot object or we have explicit data, draw the data
    const cpuData = data || generateDemoData(canvas.width, canvas.height);
    
    // Draw the data
    drawHeatmapData(ctx, cpuData, canvas.width, canvas.height);
}

// Generate some demo data if needed
function generateDemoData(width, height) {
    const numCPUs = 8;
    const numTimePoints = 60;
    
    const cpuData = [];
    
    // Generate data for each CPU
    for (let i = 0; i < numCPUs; i++) {
        const cpuValues = [];
        
        // Base pattern is a sine wave with different phase per CPU
        const phase = i * 0.5;
        const amplitude = 25 + i * 5;
        const offset = 30 + i * 5;
        
        for (let t = 0; t < numTimePoints; t++) {
            const time = t / 10;
            const value = offset + amplitude * Math.sin(time * 0.5 + phase);
            cpuValues.push(value);
        }
        
        cpuData.push(cpuValues);
    }
    
    return {
        cpuData,
        numCPUs,
        colorScale: ["#100060", "#4000A0", "#8000C0", "#A000E0", "#C000FF", "#FF2000", "#FF6000", "#FFA000", "#FFE000"],
        minValue: 0,
        maxValue: 100
    };
}

// Draw heatmap data directly to canvas
function drawHeatmapData(ctx, data, width, height) {
    const { cpuData, numCPUs, colorScale, minValue, maxValue } = data;
    
    // Calculate cell dimensions
    const cellHeight = height / numCPUs;
    const cellWidth = width / cpuData[0].length;
    
    // Draw each CPU row
    for (let cpuIdx = 0; cpuIdx < numCPUs; cpuIdx++) {
        const reversedCpuIdx = numCPUs - cpuIdx - 1;
        const cpuValues = cpuData[cpuIdx];
        
        // Y position for this CPU
        const yPos = reversedCpuIdx * cellHeight;
        
        // Draw each time segment
        for (let i = 0; i < cpuValues.length; i++) {
            const xPos = i * cellWidth;
            const value = cpuValues[i];
            
            // Get color based on value
            const normalizedValue = (value - minValue) / (maxValue - minValue);
            const colorIndex = Math.floor(normalizedValue * (colorScale.length - 1));
            const color = colorScale[Math.min(colorIndex, colorScale.length - 1)];
            
            // Draw the cell
            ctx.fillStyle = color;
            ctx.fillRect(xPos, yPos, cellWidth, cellHeight);
            
            // Add a subtle border
            ctx.strokeStyle = 'rgba(0,0,0,0.1)';
            ctx.lineWidth = 0.5;
            ctx.strokeRect(xPos, yPos, cellWidth, cellHeight);
        }
    }
}

// Add call to these functions after DOM is loaded and on resize
document.addEventListener('DOMContentLoaded', () => {
    // Initial fixes
    setTimeout(() => {
        fixHeatmapPositioning();
        setTimeout(fixCanvasRendering, 100);
    }, 500);
    
    // Run again after everything is definitely loaded
    setTimeout(() => {
        fixHeatmapPositioning();
        setTimeout(fixCanvasRendering, 100);
    }, 1500);
});

// Also add to resize handler
window.addEventListener('resize', () => {
    // Wait until resize is complete
    setTimeout(() => {
        fixHeatmapPositioning();
        setTimeout(fixCanvasRendering, 100);
    }, 300);
});

// Enhanced ZoomController for better heatmap support
class EnhancedZoomController extends ZoomController {
    constructor() {
        super();
        // Add flag to track when we're updating zoom from controller vs user interaction
        this.controllerSync = false;
    }
    
    // Better handling for different chart types during sync
    syncZoom(sourcePlot, xMin, xMax) {
        // Validate input parameters
        if (isNaN(xMin) || isNaN(xMax)) {
            this.debug(`❌ Invalid zoom range: min=${xMin}, max=${xMax}`);
            return;
        }
        
        if (this.syncLock) {
            this.debug(`⚠️ Sync lock active, skipping sync from plot ${sourcePlot._id}`);
            return;
        }
        
        this.syncLock = true;
        this.controllerSync = true;
        
        this.debug(`🔍 ZOOM: Selected time range: ${xMin.toFixed(2)}-${xMax.toFixed(2)}`);
        this.debug(`📅 ZOOM: From ${this.formatTime(xMin)} to ${this.formatTime(xMax)}`);
        
        // Apply to all plots with type-specific handling
        this.plots.forEach(plot => {
            try {
                // Clear any existing selection first to avoid conflicts
                if (plot.select && typeof plot.select.width === 'number') {
                    plot.setSelect({
                        width: 0,
                        height: 0
                    });
                }
                
                // Special handling for heatmap charts
                if (plot._chart_type === 'heatmap' && typeof plot.updateZoom === 'function') {
                    // Use the heatmap's special updateZoom method
                    plot.updateZoom(xMin, xMax);
                } else {
                    // IMPORTANT: Use batch updates to ensure all changes apply at once
                    plot.batch(() => {
                        // Force a complete redraw with new scales
                        plot.setScale("x", {
                            min: xMin,
                            max: xMax,
                            auto: false
                        });
                        
                        // Override the axes explicitly
                        plot.axes[0]._min = xMin;
                        plot.axes[0]._max = xMax;
                    });
                }
                
                // Force redraw to ensure all visual elements are updated
                plot.redraw();
            } catch (err) {
                this.debug(`❌ Error updating plot ${plot._id}: ${err.message}`);
                console.error(`Error updating plot ${plot._id}:`, err);
            }
        });
        
        // Release lock after a shorter delay (300ms is enough)
        setTimeout(() => {
            this.debug(`🔓 Sync lock released`);
            this.syncLock = false;
            this.controllerSync = false;
        }, 300);
    }
    
    // Improved reset zoom functionality
    resetZoom() {
        if (!this.initialRangeSet) {
            this.debug(`⚠️ RESET: Cannot reset zoom, initial range not set yet`);
            return;
        }
        
        if (this.syncLock) {
            this.debug(`⚠️ RESET: Cannot reset zoom while sync lock is active`);
            return;
        }
        
        this.debug(`🔄 RESET: Resetting all plots to global range: ${this.globalXMin.toFixed(2)}-${this.globalXMax.toFixed(2)}`);
        
        this.syncLock = true;
        this.controllerSync = true;
        
        this.plots.forEach(plot => {
            try {
                // Clear any existing selection first
                plot.setSelect({
                    width: 0,
                    height: 0
                });
                
                // Special handling for heatmap charts
                if (plot._chart_type === 'heatmap' && typeof plot.updateZoom === 'function') {
                    // Use the heatmap's special updateZoom method
                    plot.updateZoom(this.globalXMin, this.globalXMax);
                } else {
                    // Use batch updates for consistency
                    plot.batch(() => {
                        plot.setScale('x', {
                            min: this.globalXMin,
                            max: this.globalXMax,
                            auto: false
                        });
                    });
                }
                
                // Force redraw
                plot.redraw();
            } catch (err) {
                this.debug(`❌ Error resetting plot ${plot._id}: ${err.message}`);
                console.error(`Error resetting plot ${plot._id}:`, err);
            }
        });
        
        setTimeout(() => {
            this.debug(`🔓 Reset complete, sync lock released`);
            this.syncLock = false;
            this.controllerSync = false;
        }, 300);
    }
    
    // Improved resize handler with fixed height for consistency
    handleResize() {
        if (this.syncLock) {
            this.debug(`⚠️ Resize attempted while sync lock active, deferring resize`);
            return;
        }
        
        this.debug(`📐 Window resize detected, updating plots`);
        
        // Use requestAnimationFrame for smoother resize handling
        if (this.resizeTimeout) {
            cancelAnimationFrame(this.resizeTimeout);
        }
        
        this.resizeTimeout = requestAnimationFrame(() => {
            // Get current global X range before resize
            const currentMin = this.plots.length > 0 ? this.plots[0].scales.x.min : null;
            const currentMax = this.plots.length > 0 ? this.plots[0].scales.x.max : null;
            const isZoomed = currentMin !== this.globalXMin || currentMax !== this.globalXMax;
            
            this.syncLock = true;
            
            this.plots.forEach(plot => {
                try {
                    const container = plot.root.closest('.plot-container');
                    if (!container) return;
                    
                    const width = container.clientWidth;
                    
                    // Only resize if the width has changed significantly
                    if (Math.abs(plot.width - width) > 5) {
                        this.debug(`📐 Resizing plot ${plot._id} to width ${width}px`);
                        
                        plot.setSize({
                            width: width,
                            height: 350 // Fixed height for consistency
                        });
                        
                        // Maintain zoom state
                        if (isZoomed) {
                            if (plot._chart_type === 'heatmap' && typeof plot.updateZoom === 'function') {
                                plot.updateZoom(currentMin, currentMax);
                            } else {
                                plot.setScale("x", {
                                    min: currentMin,
                                    max: currentMax,
                                    auto: false
                                });
                            }
                        }
                        
                        // Force redraw
                        plot.redraw();
                    }
                } catch (err) {
                    this.debug(`❌ Error during resize of plot ${plot._id}: ${err.message}`);
                    console.error(err);
                }
            });
            
            // Release the lock
            setTimeout(() => {
                this.syncLock = false;
                this.debug(`🔓 Resize complete, sync lock released`);
                
                // Check alignment after resize
                setTimeout(fixHeatmapAlignment, 200);
            }, 200);
        });
    }
}

// Add CSS to improve chart layout in the dashboard
function addDashboardStyles() {
    // Check if already added
    if (document.getElementById('dashboard-alignment-styles')) return;
    
    const styleEl = document.createElement('style');
    styleEl.id = 'dashboard-alignment-styles';
    styleEl.textContent = `
        /* Ensure consistent chart height and padding */
        .cpu-heatmap,
        .uplot {
            height: 350px !important;
            box-sizing: border-box !important;
        }
        
        /* Fix alignment of charts in the CPU group */
        #group-1 .group-content {
            display: flex;
            flex-wrap: wrap;
            gap: 20px;
            align-items: flex-start;
        }
        
        #group-1 .plot-container {
            flex: 1 1 calc(50% - 20px);
            min-width: 400px;
            max-width: calc(50% - 10px);
        }
        
        #group-1 .plot-container.full-width {
            flex: 1 1 calc(50% - 20px);
            min-width: 400px;
            max-width: calc(50% - 10px);
            width: calc(50% - 10px) !important;
        }
        
        /* Remove padding on left side that causes misalignment */
        .cpu-heatmap {
            padding-left: 0 !important;
        }
        
        /* Fix for smaller screens */
        @media (max-width: 992px) {
            #group-1 .plot-container,
            #group-1 .plot-container.full-width {
                flex: 1 1 100%;
                max-width: 100%;
                width: 100% !important;
            }
        }
    `;
    document.head.appendChild(styleEl);
}

// Function to check and fix heatmap canvas alignment
function fixHeatmapAlignment() {
    // Find all heatmap containers
    const heatmaps = document.querySelectorAll('.cpu-heatmap');
    let fixesApplied = false;
    
    heatmaps.forEach((heatmap, idx) => {
        const canvas = heatmap.querySelector('.cpu-heatmap-canvas');
        const wrapper = heatmap.querySelector('.cpu-heatmap-wrapper');
        const over = heatmap.querySelector('.u-over');
        
        if (canvas && over) {
            console.log(`Checking heatmap #${idx} alignment...`);
            
            // Check canvas dimensions
            const canvasBounds = {
                width: canvas.width,
                height: canvas.height,
                styleWidth: parseInt(canvas.style.width, 10) || canvas.width,
                styleHeight: parseInt(canvas.style.height, 10) || canvas.height,
                offsetLeft: wrapper ? wrapper.offsetLeft : canvas.offsetLeft,
                offsetTop: wrapper ? wrapper.offsetTop : canvas.offsetTop
            };
            
            // Check interaction area
            const overBounds = {
                width: over.offsetWidth,
                height: over.offsetHeight,
                offsetLeft: over.offsetLeft,
                offsetTop: over.offsetTop
            };
            
            // Check for ANY misalignment
            const hasMismatch = 
                Math.abs(canvasBounds.width - overBounds.width) > 5 || 
                Math.abs(canvasBounds.height - overBounds.height) > 5 ||
                Math.abs(canvasBounds.styleWidth - overBounds.width) > 5 ||
                Math.abs(canvasBounds.styleHeight - overBounds.height) > 5 ||
                Math.abs(canvasBounds.offsetLeft - overBounds.offsetLeft) > 5 ||
                Math.abs(canvasBounds.offsetTop - overBounds.offsetTop) > 5;
            
            if (hasMismatch) {
                console.warn(`⚠️ Fixing alignment issues in heatmap #${idx}:`);
                console.warn(`   Canvas: ${JSON.stringify(canvasBounds)}`);
                console.warn(`   Overlay: ${JSON.stringify(overBounds)}`);
                
                // Fix canvas dimensions (BOTH .width and style.width)
                canvas.width = overBounds.width;
                canvas.height = overBounds.height;
                canvas.style.width = `${overBounds.width}px`;
                canvas.style.height = `${overBounds.height}px`;
                
                // Ensure wrapper is correctly positioned
                if (wrapper) {
                    wrapper.style.left = `${overBounds.offsetLeft}px`;
                    wrapper.style.top = `${overBounds.offsetTop}px`;
                    wrapper.style.width = `${overBounds.width}px`;
                    wrapper.style.height = `${overBounds.height}px`;
                } else {
                    // Direct positioning if no wrapper
                    canvas.style.left = `${overBounds.offsetLeft}px`;
                    canvas.style.top = `${overBounds.offsetTop}px`;
                }
                
                fixesApplied = true;
            } else {
                console.log(`  ✅ Heatmap #${idx} alignment looks good`);
            }
        }
    });
    
    // Force a redraw if fixes were applied
    if (fixesApplied) {
        console.log('Forcing redraw after alignment fixes');
        if (window.zoomController) {
            window.zoomController.plots.forEach(plot => {
                if (plot._chart_type === 'heatmap' && typeof plot.redraw === 'function') {
                    plot.redraw();
                }
            });
        }
    }
    
    return fixesApplied;
}

// Enhanced check chart alignment function with heatmap-specific fixes
function enhancedCheckChartAlignment() {
    // First run the regular alignment check
    if (typeof window.checkChartAlignment === 'function') {
        window.checkChartAlignment();
    }
    
    // Then run the heatmap-specific fixes
    setTimeout(() => {
        fixHeatmapAlignment();
    }, 100);
}

// Function to replace the existing heatmap chart creation
function enhanceCreateCpuHeatmap() {
    // Keep reference to the original function
    if (!window._originalCreateCpuHeatmap && typeof window.createCpuHeatmap === 'function') {
        window._originalCreateCpuHeatmap = window.createCpuHeatmap;
        
        // Replace with our improved version
        window.createCpuHeatmap = function(containerId, data, options) {
            console.log(`Creating enhanced heatmap for ${containerId}`);
            
            // Call our implementation
            const result = window._originalCreateCpuHeatmap(containerId, data, options);
            
            // Check alignment immediately after creation
            setTimeout(() => {
                fixHeatmapAlignment();
            }, 50);
            
            return result;
        };
    }
}

// Function to enhance the existing zoom controller
function enhanceZoomController() {
    if (typeof window.zoomController !== 'undefined') {
        // Store reference to original functions
        const originalSyncZoom = window.zoomController.syncZoom;
        const originalResetZoom = window.zoomController.resetZoom;
        const originalHandleResize = window.zoomController.handleResize;
        
        // Create enhanced controller
        const enhancedController = new EnhancedZoomController();
        
        // Copy over important state
        enhancedController.plots = window.zoomController.plots || [];
        enhancedController.globalXMin = window.zoomController.globalXMin;
        enhancedController.globalXMax = window.zoomController.globalXMax;
        enhancedController.initialRangeSet = window.zoomController.initialRangeSet;
        
        // Replace the global controller
        window.zoomController = enhancedController;
        
        console.log('✅ Enhanced zoom controller activated');
    }
}

function forceCanvasAlignment() {
    // Find all heatmap canvases
    const canvases = document.querySelectorAll('.cpu-heatmap-canvas');
    
    canvases.forEach((canvas, idx) => {
        // Only proceed if we haven't already patched this canvas
        if (canvas._dimensionsPatched) return;
        
        // Get the overlay element (interaction area)
        const heatmap = canvas.closest('.cpu-heatmap');
        if (!heatmap) return;
        
        const overlay = heatmap.querySelector('.u-over');
        if (!overlay) return;
        
        console.log(`Patching canvas #${idx} to maintain alignment`);
        
        // Store the correct dimensions
        const correctWidth = overlay.offsetWidth;
        const correctHeight = overlay.offsetHeight;
        
        // Create wrapper if needed
        let wrapper = canvas.parentElement;
        if (!wrapper.classList.contains('cpu-heatmap-wrapper')) {
            // Need to create a wrapper
            wrapper = document.createElement('div');
            wrapper.className = 'cpu-heatmap-wrapper';
            wrapper.style.cssText = `
                position: absolute;
                top: ${overlay.offsetTop}px;
                left: ${overlay.offsetLeft}px;
                width: ${correctWidth}px;
                height: ${correctHeight}px;
                overflow: hidden;
                z-index: 10;
            `;
            
            // Move canvas into wrapper
            canvas.parentElement.insertBefore(wrapper, canvas);
            wrapper.appendChild(canvas);
        }
        
        // Mark the canvas as patched
        canvas._dimensionsPatched = true;
        
        // Override the width and height properties
        const originalWidthDescriptor = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, 'width');
        const originalHeightDescriptor = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, 'height');
        
        // Force dimensions using getters and setters
        Object.defineProperties(canvas, {
            'width': {
                get: function() {
                    return correctWidth;
                },
                set: function(val) {
                    console.log(`Intercepted attempt to set canvas width to ${val}, using ${correctWidth} instead`);
                    originalWidthDescriptor.set.call(this, correctWidth);
                }
            },
            'height': {
                get: function() {
                    return correctHeight;
                },
                set: function(val) {
                    console.log(`Intercepted attempt to set canvas height to ${val}, using ${correctHeight} instead`);
                    originalHeightDescriptor.set.call(this, correctHeight);
                }
            }
        });
        
        // Also enforce style dimensions
        canvas.style.width = `${correctWidth}px`;
        canvas.style.height = `${correctHeight}px`;
        canvas.style.top = '0';
        canvas.style.left = '0';
        
        // Set the actual dimensions one last time
        originalWidthDescriptor.set.call(canvas, correctWidth);
        originalHeightDescriptor.set.call(canvas, correctHeight);
        
        // Watch for style changes with MutationObserver
        const observer = new MutationObserver(mutations => {
            mutations.forEach(mutation => {
                if (mutation.attributeName === 'style') {
                    // Reset style if needed
                    if (canvas.style.width !== `${correctWidth}px` || 
                        canvas.style.height !== `${correctHeight}px`) {
                        console.log('Fixing canvas style dimensions after mutation');
                        canvas.style.width = `${correctWidth}px`;
                        canvas.style.height = `${correctHeight}px`;
                    }
                }
            });
        });
        
        // Start observing style changes
        observer.observe(canvas, { attributes: true, attributeFilter: ['style'] });
        
        console.log(`Canvas #${idx} patched successfully`);
    });
}

// Call this function after DOM is loaded and whenever a resize occurs
document.addEventListener('DOMContentLoaded', () => {
    // Run on a delay to ensure elements are created
    setTimeout(forceCanvasAlignment, 500);
    setTimeout(forceCanvasAlignment, 1500);
});

// Also intercept any resize operations
window.addEventListener('resize', () => {
    // Wait until after resize completes
    setTimeout(forceCanvasAlignment, 500);
});

// Initialize everything when the page loads
document.addEventListener('DOMContentLoaded', function() {
    console.log('🚀 Initializing heatmap integration');
    
    // Add dashboard styles
    addDashboardStyles();
    
    // Enhance any existing components
    enhanceCreateCpuHeatmap();
    enhanceZoomController();
    
    // Add additional CSS to ensure canvas visibility
    const extraStyle = document.createElement('style');
    extraStyle.textContent = `
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
    document.head.appendChild(extraStyle);
    
    // Run fixes after initial charts are created
    setTimeout(fixHeatmapAlignment, 500);
    setTimeout(fixHeatmapAlignment, 1500);
    
    // If the original checkChartAlignment function exists, replace it
    if (typeof window.checkChartAlignment !== 'undefined') {
        window.checkChartAlignment = enhancedCheckChartAlignment;
    }
    
    console.log('✅ Heatmap integration complete');
});

// Update any existing charts that are already created
if (typeof window.zoomController !== 'undefined' && window.zoomController.plots) {
    console.log('📊 Updating existing charts...');
    
    setTimeout(() => {
        enhanceZoomController();
        fixHeatmapAlignment();
    }, 0);
}

/**
 * Function to fix axis labels for both uPlot charts and heatmaps
 */
function fixAllAxisLabels() {
    console.log("Fixing axis labels for all chart types...");
    
    // Fix uPlot charts (line charts)
    const uplotCharts = document.querySelectorAll('.uplot');
    uplotCharts.forEach((chart, idx) => {
        console.log(`Examining uPlot chart #${idx}`);
        
        // Find the bottom axis elements in uPlot SVGs
        const svg = chart.querySelector('svg');
        if (svg) {
            // Create a new axis if needed
            const container = chart.closest('.plot-container');
            if (container) {
                const width = container.clientWidth;
                const timeAxisEl = document.createElement('div');
                timeAxisEl.className = 'manual-time-axis';
                timeAxisEl.style.cssText = `
                    position: absolute;
                    bottom: 0;
                    left: 60px;
                    width: ${width - 70}px;
                    height: 30px;
                    z-index: 100;
                `;
                container.appendChild(timeAxisEl);
                
                // Find plot object for this chart
                let plotObj = null;
                if (window.zoomController && window.zoomController.plots) {
                    plotObj = window.zoomController.plots.find(p => p.root === chart);
                }
                
                if (plotObj && plotObj.data && plotObj.data[0]) {
                    // Get time data
                    const timestamps = plotObj.data[0];
                    if (timestamps.length > 0) {
                        // Add time labels
                        const numLabels = 6;
                        const minTime = plotObj.scales.x.min || timestamps[0];
                        const maxTime = plotObj.scales.x.max || timestamps[timestamps.length - 1];
                        const timeRange = maxTime - minTime;
                        
                        for (let i = 0; i < numLabels; i++) {
                            const percent = i / (numLabels - 1);
                            const time = minTime + percent * timeRange;
                            const labelX = percent * (width - 70);
                            
                            // Format time
                            const date = new Date(time * 1000);
                            const timeStr = date.toLocaleTimeString([], {
                                hour: '2-digit', 
                                minute: '2-digit', 
                                second: '2-digit', 
                                hour12: false
                            });
                            
                            // Create label
                            const label = document.createElement('div');
                            label.textContent = timeStr;
                            label.style.cssText = `
                                position: absolute;
                                bottom: 5px;
                                left: ${labelX}px;
                                transform: translateX(-50%);
                                font-size: 11px;
                                color: #888888;
                                pointer-events: none;
                            `;
                            
                            timeAxisEl.appendChild(label);
                        }
                    }
                }
            }
        }
    });
    
    // Fix heatmap charts
    const heatmaps = document.querySelectorAll('.cpu-heatmap');
    heatmaps.forEach((heatmap, idx) => {
        console.log(`Examining heatmap #${idx}`);
        
        // Find or create time axis element
        let timeAxisEl = heatmap.querySelector('.time-axis');
        if (!timeAxisEl) {
            const over = heatmap.querySelector('.u-over');
            if (over) {
                timeAxisEl = document.createElement('div');
                timeAxisEl.className = 'time-axis';
                timeAxisEl.style.cssText = `
                    position: absolute;
                    bottom: 10px;
                    left: ${over.offsetLeft}px;
                    width: ${over.offsetWidth}px;
                    height: 30px;
                    z-index: 100;
                `;
                heatmap.appendChild(timeAxisEl);
            }
        }
        
        if (timeAxisEl) {
            // Find plot object for this heatmap
            let plotObj = null;
            if (window.zoomController && window.zoomController.plots) {
                plotObj = window.zoomController.plots.find(p => p._chart_type === 'heatmap' && p.root === heatmap);
            }
            
            if (plotObj && plotObj.data && plotObj.data[0]) {
                // Get time data
                const timestamps = plotObj.data[0];
                if (timestamps.length > 0) {
                    // Clear existing labels
                    timeAxisEl.innerHTML = '';
                    
                    // Add time labels
                    const numLabels = 6;
                    const minTime = plotObj.scales.x.min || timestamps[0];
                    const maxTime = plotObj.scales.x.max || timestamps[timestamps.length - 1];
                    const timeRange = maxTime - minTime;
                    
                    for (let i = 0; i < numLabels; i++) {
                        const percent = i / (numLabels - 1);
                        const time = minTime + percent * timeRange;
                        const labelX = percent * timeAxisEl.offsetWidth;
                        
                        // Format time
                        const date = new Date(time * 1000);
                        const timeStr = date.toLocaleTimeString([], {
                            hour: '2-digit', 
                            minute: '2-digit', 
                            second: '2-digit', 
                            hour12: false
                        });
                        
                        // Create label
                        const label = document.createElement('div');
                        label.textContent = timeStr;
                        label.style.cssText = `
                            position: absolute;
                            bottom: 5px;
                            left: ${labelX}px;
                            transform: translateX(-50%);
                            font-size: 11px;
                            color: #888888;
                            pointer-events: none;
                        `;
                        
                        timeAxisEl.appendChild(label);
                    }
                }
            }
        }
    });
    
    // Add CSS to ensure axis visibility
    const style = document.createElement('style');
    style.id = 'axis-label-fixes';
    style.textContent = `
        .manual-time-axis,
        .time-axis {
            visibility: visible !important;
            opacity: 1 !important;
            display: block !important;
            z-index: 100 !important;
        }
        
        .manual-time-axis div,
        .time-axis div {
            visibility: visible !important;
            opacity: 1 !important;
            display: block !important;
            color: #888888 !important;
            z-index: 101 !important;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif !important;
            font-size: 11px !important;
        }
    `;
    
    document.head.appendChild(style);
}

// Call the function immediately if document is loaded
if (document.readyState === 'complete') {
    fixAllAxisLabels();
} else {
    // Otherwise wait for load
    document.addEventListener('DOMContentLoaded', () => {
        setTimeout(fixAllAxisLabels, 500);
    });
}

// Also fix on resize
window.addEventListener('resize', () => {
    setTimeout(fixAllAxisLabels, 500);
});

// Additional call for existing plots
setTimeout(fixAllAxisLabels, 1000);

// Listen for window resize events to maintain alignment
window.addEventListener('resize', function() {
    // Debounce resize checks
    if (window.resizeTimer) clearTimeout(window.resizeTimer);
    window.resizeTimer = setTimeout(fixHeatmapAlignment, 250);
});

// Ensure alignment fix is always applied last
let lastFixTimeout;
window.addEventListener('resize', function() {
    if (lastFixTimeout) clearTimeout(lastFixTimeout);
    lastFixTimeout = setTimeout(fixHeatmapAlignment, 500); // Run after all other handlers
});