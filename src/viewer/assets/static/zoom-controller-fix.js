
// Enhanced ZoomController with better handling for heatmap and line chart synchronization
class EnhancedZoomController extends ZoomController {
    constructor() {
        super();
        
        // Add flag to track when we're updating zoom from controller vs user interaction
        this.controllerSync = false;
    }
    
    // Better handling for different chart types during sync
    syncZoom(sourcePlot, xMin, xMax) {
        // Same validation as parent class
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
        
        this.debug(`🔍 ZOOM: Plot ${sourcePlot._id} selected time range: ${xMin.toFixed(2)}-${xMax.toFixed(2)}`);
        this.debug(`📅 ZOOM: From ${this.formatTime(xMin)} to ${this.formatTime(xMax)}`);
        
        // Apply to all plots with type-specific handling
        this.plots.forEach(plot => {
            this.debug(`⚙️ Updating plot ${plot._id} to match time range ${xMin.toFixed(2)}-${xMax.toFixed(2)}`);
            
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
                    this.debug(`✅ Heatmap ${plot._id} updated via custom zoom method`);
                } else {
                    // Get the current min/max for validation
                    const oldMin = plot.scales.x.min;
                    const oldMax = plot.scales.x.max;
                    
                    // To ensure consistent tick alignment, use slightly adjusted min/max
                    // This forces ticks to align across all charts (both heatmap and line)
                    const interval = this.determineInterval(xMax - xMin);
                    
                    // Snap to interval boundary (same logic used in heatmap.js and time-axis-formatter.js)
                    const alignedMin = Math.floor(xMin / interval) * interval;
                    const adjustedMin = alignedMin < xMin ? alignedMin + interval : alignedMin;
                    
                    // IMPORTANT: Use batch updates to ensure all changes apply at once
                    plot.batch(() => {
                        // Force a complete redraw with new scales
                        plot.setScale("x", {
                            min: xMin,
                            max: xMax,
                            auto: false          // Disable auto-scaling
                        });
                        
                        // Override the axes explicitly
                        plot.axes[0]._min = xMin;
                        plot.axes[0]._max = xMax;
                    });
                    
                    // Verify the update worked
                    const newMin = plot.scales.x.min;
                    const newMax = plot.scales.x.max;
                    
                    if (Math.abs(newMin - xMin) > 0.01 || Math.abs(newMax - xMax) > 0.01) {
                        this.debug(`⚠️ Plot ${plot._id} scale update failed!`);
                        this.debug(`  Requested: ${xMin.toFixed(2)}-${xMax.toFixed(2)}`);
                        this.debug(`  Actual: ${newMin.toFixed(2)}-${newMax.toFixed(2)}`);
                        
                        // Force another update if not successful
                        plot.setScale("x", {
                            min: xMin,
                            max: xMax,
                            auto: false
                        });
                    } else {
                        this.debug(`✅ Plot ${plot._id} updated: ${oldMin.toFixed(2)}->${newMin.toFixed(2)}, ${oldMax.toFixed(2)}->${newMax.toFixed(2)}`);
                    }
                }
                
                // Force redraw to ensure all visual elements are updated
                plot.redraw();
            } catch (err) {
                this.debug(`❌ Error updating plot ${plot._id}: ${err.message}`);
                console.error(`Error updating plot ${plot._id}:`, err);
            }
        });
        
        // Release lock after a longer delay to ensure all plots have time to update
        setTimeout(() => {
            this.debug(`🔓 Sync lock released`);
            this.syncLock = false;
            this.controllerSync = false;
        }, 500);
    }
    
    // Enhanced reset zoom functionality
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
        this.debug(`📅 RESET: From ${this.formatTime(this.globalXMin)} to ${this.formatTime(this.globalXMax)}`);
        
        this.syncLock = true;
        this.controllerSync = true;
        
        this.plots.forEach(plot => {
            try {
                const oldMin = plot.scales.x.min;
                const oldMax = plot.scales.x.max;
                
                // Clear any existing selection first
                plot.setSelect({
                    width: 0,
                    height: 0
                });
                
                // Special handling for heatmap charts
                if (plot._chart_type === 'heatmap' && typeof plot.updateZoom === 'function') {
                    // Use the heatmap's special updateZoom method
                    plot.updateZoom(this.globalXMin, this.globalXMax);
                    this.debug(`✅ Heatmap ${plot._id} reset via custom zoom method`);
                } else {
                    // Use batch updates for consistency
                    plot.batch(() => {
                        plot.setScale('x', {
                            min: this.globalXMin,
                            max: this.globalXMax,
                            auto: false
                        });
                        
                        // Override the axes explicitly
                        plot.axes[0]._min = this.globalXMin;
                        plot.axes[0]._max = this.globalXMax;
                    });
                    
                    // Verify the update worked
                    const newMin = plot.scales.x.min;
                    const newMax = plot.scales.x.max;
                    
                    this.debug(`✅ Plot ${plot._id} reset: ${oldMin.toFixed(2)}->${newMin.toFixed(2)}, ${oldMax.toFixed(2)}->${newMax.toFixed(2)}`);
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
        }, 500); // Increased timeout for reset
    }
    
    // Improved resize handler
    handleResize() {
        if (this.syncLock) {
            this.debug(`⚠️ Resize attempted while sync lock active, deferring resize`);
            
            // Queue the resize for after the sync lock is released
            setTimeout(() => {
                if (!this.syncLock) {
                    this.handleResize();
                }
            }, 100);
            
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
            this.controllerSync = true;
            
            this.plots.forEach(plot => {
                try {
                    const container = plot.root.closest('.plot-container');
                    if (!container) return;
                    
                    const width = container.clientWidth;
                    
                    // Only resize if the width has changed significantly
                    if (Math.abs(plot.width - width) > 5) {
                        this.debug(`📐 Resizing plot ${plot._id} to width ${width}px`);
                        
                        // Special handling for heatmap charts
                        if (plot._chart_type === 'heatmap') {
                            plot.setSize({
                                width: width,
                                height: 350
                            });
                            
                            // Update zoom if needed
                            if (isZoomed) {
                                plot.updateZoom(currentMin, currentMax);
                            }
                        } else {
                            // For regular uPlot charts
                            plot.batch(() => {
                                // First resize the plot
                                plot.setSize({
                                    width: width,
                                    height: 350 // Fixed height for consistency
                                });
                                
                                // Then ensure the zoom state is maintained
                                if (isZoomed) {
                                    plot.setScale("x", {
                                        min: currentMin,
                                        max: currentMax,
                                        auto: false
                                    });
                                    
                                    // Override the axes explicitly
                                    plot.axes[0]._min = currentMin;
                                    plot.axes[0]._max = currentMax;
                                } else {
                                    // If not zoomed, maintain full data view
                                    plot.setScale("x", {
                                        min: this.globalXMin,
                                        max: this.globalXMax,
                                        auto: false
                                    });
                                    
                                    // Override the axes explicitly
                                    plot.axes[0]._min = this.globalXMin;
                                    plot.axes[0]._max = this.globalXMax;
                                }
                            });
                        }
                        
                        // Force a redraw to ensure everything is properly updated
                        plot.redraw();
                        
                        // Check if resize worked correctly
                        const newMin = plot.scales.x.min;
                        const newMax = plot.scales.x.max;
                        
                        this.debug(`✅ Plot ${plot._id} resize: x=${newMin.toFixed(2)}-${newMax.toFixed(2)}`);
                    }
                } catch (err) {
                    this.debug(`❌ Error during resize of plot ${plot._id}: ${err.message}`);
                    console.error(err);
                }
            });
            
            // Release the lock
            setTimeout(() => {
                this.syncLock = false;
                this.controllerSync = false;
                this.debug(`🔓 Resize complete, sync lock released`);
            }, 200);
        });
    }
    
    // Helper method to determine the appropriate time interval
    determineInterval(timeSpanSeconds) {
        // Define time intervals (same as in time-axis-formatter.js and heatmap.js)
        const TIME_INTERVALS = [
            { seconds: 1 },
            { seconds: 5 },
            { seconds: 15 },
            { seconds: 30 },
            { seconds: 60 },
            { seconds: 5 * 60 },
            { seconds: 15 * 60 },
            { seconds: 30 * 60 },
            { seconds: 60 * 60 },
            { seconds: 3 * 60 * 60 },
            { seconds: 6 * 60 * 60 },
            { seconds: 12 * 60 * 60 },
            { seconds: 24 * 60 * 60 }
        ];
        
        // Target ~6 labels on the axis for readability (same as other components)
        const targetDensity = timeSpanSeconds / 6;
        
        // Find the appropriate interval based on visible range
        let selectedInterval = TIME_INTERVALS[TIME_INTERVALS.length - 1].seconds;
        
        for (let i = 0; i < TIME_INTERVALS.length; i++) {
            if (TIME_INTERVALS[i].seconds >= targetDensity) {
                selectedInterval = TIME_INTERVALS[i].seconds;
                break;
            }
        }
        
        return selectedInterval;
    }
}