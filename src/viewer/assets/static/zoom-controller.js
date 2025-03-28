// ZoomController: Manages synchronized zooming across multiple uPlot charts
class ZoomController {
    constructor() {
        this.plots = [];
        this.globalXMin = null;
        this.globalXMax = null;
        this.syncLock = false;
        this.initialRangeSet = false;
    }

    // Register a plot with the controller
    registerPlot(plot) {
        // Add ID if not present
        if (!plot._id) {
            plot._id = this.plots.length + 1;
        }
        
        this.plots.push(plot);
        this.debug(`🔄 Registered plot ${plot._id}`);
        
        // Track selection events
        this.setupPlotListeners(plot);
        
        // Update global time range
        const timestamps = plot.data[0];
        if (timestamps && timestamps.length > 0) {
            if (this.globalXMin === null || timestamps[0] < this.globalXMin) {
                this.globalXMin = timestamps[0];
            }
            if (this.globalXMax === null || timestamps[timestamps.length - 1] > this.globalXMax) {
                this.globalXMax = timestamps[timestamps.length - 1];
            }
        }
        
        return plot;
    }
    
    // Set up selection and zoom listeners for a plot
    setupPlotListeners(plot) {
        // Check if hooks property exists and initialize it if needed
        if (!plot.hooks) {
            this.debug(`⚠️ Plot ${plot._id} missing hooks property, skipping hook setup`);
        } else if (!plot.hooks.setSelect) {
            this.debug(`⚠️ Plot ${plot._id} missing hooks.setSelect, skipping hook setup`);
        } else {
            // Set hook for built-in selection handler
            plot.hooks.setSelect.push((u) => {
                // When uPlot itself completes a selection, this hook is called
                const selEl = u.root.querySelector(".u-select");
                
                if (selEl && selEl.style.display !== "none" && parseInt(selEl.style.width, 10) > 5) {
                    const selLeft = parseInt(selEl.style.left, 10);
                    const selWidth = parseInt(selEl.style.width, 10);
                    
                    const minX = u.posToVal(selLeft, 'x');
                    const maxX = u.posToVal(selLeft + selWidth, 'x');
                    
                    this.debug(`🎣 Plot ${u._id} selection hook triggered: ${minX.toFixed(2)}-${maxX.toFixed(2)}`);
                    
                    // Only trigger if we're not already in a sync process
                    if (!this.syncLock && selWidth > 5) {
                        this.syncZoom(u, minX, maxX);
                    }
                }
            });
        }
        
        // Track selection state
        let selectionActive = false;
        let selectionStartX = null;
        
        // Track when selection starts
        plot.over.addEventListener('mousedown', (e) => {
            // Only track left button
            if (e.button !== 0) return;
            
            const rect = plot.over.getBoundingClientRect();
            const xPos = e.clientX - rect.left;
            const xVal = plot.posToVal(xPos, 'x');
            
            selectionActive = true;
            selectionStartX = xVal;
            
            this.debug(`🖱️ Plot ${plot._id} selection started at x=${xVal.toFixed(2)} (${this.formatTime(xVal)})`);
        });
        
        // Track when selection ends
        plot.over.addEventListener('mouseup', (e) => {
            // Only track left button
            if (e.button !== 0 || !selectionActive) return;
            
            // Skip if sync is already in progress
            if (this.syncLock) {
                this.debug(`⚠️ Plot ${plot._id} selection ended but sync lock active, ignoring`);
                selectionActive = false;
                return;
            }
            
            try {
                const rect = plot.over.getBoundingClientRect();
                const xPos = e.clientX - rect.left;
                const xVal = plot.posToVal(xPos, 'x');
                
                this.debug(`🖱️ Plot ${plot._id} selection ended at x=${xVal.toFixed(2)} (${this.formatTime(xVal)})`);
                
                // Get the select element from uPlot
                const selectElem = plot.root.querySelector(".u-select");
                
                if (selectElem && selectElem.style.display !== "none") {
                    // Selection is visible, get its position
                    const selectLeft = parseInt(selectElem.style.left, 10);
                    const selectWidth = parseInt(selectElem.style.width, 10);
                    
                    // Convert selection pixels to values
                    const minX = plot.posToVal(selectLeft, 'x');
                    const maxX = plot.posToVal(selectLeft + selectWidth, 'x');
                    
                    this.debug(`🔍 Plot ${plot._id} selection detected: ${minX.toFixed(2)}-${maxX.toFixed(2)} (width: ${selectWidth}px)`);
                    this.debug(`📅 Time range: ${this.formatTime(minX)} to ${this.formatTime(maxX)}`);
                    
                    // Check if selection is meaningful (not too small)
                    if (selectWidth > 5 && Math.abs(maxX - minX) > 1) {
                        // This happens before uPlot automatically zooms the plot
                        // We need to manually sync all plots to this range
                        this.syncZoom(plot, minX, maxX);
                        
                        // Also update this plot to make sure it gets the exact same range
                        plot.setScale('x', {
                            min: minX,
                            max: maxX
                        });
                        
                        // Clear the selection
                        plot.setSelect({
                            width: 0,
                            height: 0
                        });
                    } else {
                        this.debug(`ℹ️ Plot ${plot._id} selection too small, ignoring`);
                    }
                } else {
                    // No visible selection, might be a click without drag
                    this.debug(`ℹ️ Plot ${plot._id} no visible selection detected`);
                }
            } catch (err) {
                this.debug(`❌ Error in selection handler for plot ${plot._id}: ${err.message}`);
                console.error(err);
            } finally {
                selectionActive = false;
            }
        });
        
        // Double-click to reset
        plot.over.addEventListener('dblclick', () => {
            this.debug(`👆 Double-click on plot ${plot._id}, requesting reset`);
            this.resetZoom();
        });
    }

    // Function to sync all plots to a specific range
    syncZoom(sourcePlot, xMin, xMax) {
        if (this.syncLock) {
            this.debug(`⚠️ Sync lock active, skipping sync from plot ${sourcePlot._id}`);
            return;
        }
        
        this.syncLock = true;
        
        this.debug(`🔍 ZOOM: Plot ${sourcePlot._id} selected time range: ${xMin.toFixed(2)}-${xMax.toFixed(2)}`);
        this.debug(`📅 ZOOM: From ${this.formatTime(xMin)} to ${this.formatTime(xMax)}`);
        
        // Apply to all plots including source plot to ensure consistency
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
                
                // Get the current min/max for validation
                const oldMin = plot.scales.x.min;
                const oldMax = plot.scales.x.max;
                
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
                    
                    // Extreme fallback - recreate the plot with the desired range
                    const opts = Object.assign({}, plot.opts, {
                        scales: {
                            x: {
                                min: xMin,
                                max: xMax,
                                auto: false
                            },
                            y: {
                                min: plot.scales.y.min !== null ? plot.scales.y.min : undefined,
                                max: plot.scales.y.max !== null ? plot.scales.y.max : undefined,
                                auto: plot.scales.y.min === null && plot.scales.y.max === null,
                            }
                        }
                    });
                    
                    // Capture the parent element
                    const parent = plot.root.parentElement;
                    const id = parent.id;
                    this.debug(`🔄 Last resort: Recreating plot ${plot._id} in ${id}`);
                    
                    // Clean up old plot
                    plot.destroy();
                    
                    // Create a new plot
                    const newPlot = new uPlot(opts, plot.data, document.getElementById(id));
                    newPlot._id = plot._id;
                    
                    // Replace in the array
                    const idx = this.plots.indexOf(plot);
                    if (idx !== -1) {
                        this.plots[idx] = newPlot;
                    }
                    
                    this.debug(`✅ Plot ${plot._id} recreated with fixed bounds`);
                } else {
                    this.debug(`✅ Plot ${plot._id} updated: ${oldMin.toFixed(2)}->${newMin.toFixed(2)}, ${oldMax.toFixed(2)}->${newMax.toFixed(2)}`);
                }
            } catch (err) {
                this.debug(`❌ Error updating plot ${plot._id}: ${err.message}`);
                console.error(`Error updating plot ${plot._id}:`, err);
            }
        });
        
        // Release lock after a longer delay to ensure all plots have time to update
        setTimeout(() => {
            this.debug(`🔓 Sync lock released`);
            this.syncLock = false;
        }, 500);
    }
    
    // Reset zoom function
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
        
        this.plots.forEach(plot => {
            try {
                const oldMin = plot.scales.x.min;
                const oldMax = plot.scales.x.max;
                
                // Clear any existing selection first
                plot.setSelect({
                    width: 0,
                    height: 0
                });
                
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
        }, 500); // Increased timeout for reset
    }
    
    // Handle resize for all plots
    handleResize() {
        this.debug(`📐 Window resize detected, updating plot dimensions`);
        this.plots.forEach(plot => {
            try {
                const container = plot.root.closest('.plot-container');
                const width = container.clientWidth;
                this.debug(`📐 Resizing plot ${plot._id} to width ${width}px`);
                plot.setSize({
                    width: width,
                    height: 400
                });
            } catch (err) {
                this.debug(`❌ Error during resize of plot ${plot._id}: ${err.message}`);
            }
        });
    }
    
    // Format time for display
    formatTime(timestamp) {
        const date = new Date(timestamp * 1000);
        return date.toLocaleTimeString([], {
            hour: '2-digit', 
            minute: '2-digit', 
            second: '2-digit', 
            hour12: false
        });
    }
    
    // Debug logging
    debug(message) {
        const timestamp = new Date().toISOString().substr(11, 12);
        const formattedMessage = `[${timestamp}] ${message}`;
        
        console.log(formattedMessage);
    }
    
    // Mark initialization complete
    setInitialized() {
        this.initialRangeSet = true;
        this.debug(`🌎 Global time range set: ${this.globalXMin.toFixed(2)}-${this.globalXMax.toFixed(2)}`);
        this.debug(`📅 Global time range: ${this.formatTime(this.globalXMin)} to ${this.formatTime(this.globalXMax)}`);
    }
}