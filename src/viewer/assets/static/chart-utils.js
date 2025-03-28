// --------------------------------------------------------
// Chart Utility Functions
// --------------------------------------------------------

// Setup group toggle functionality
function setupGroupToggles() {
    const toggleButtons = document.querySelectorAll('.group-toggle');
    
    toggleButtons.forEach(button => {
        button.addEventListener('click', () => {
            const groupId = button.getAttribute('data-group');
            const groupElement = document.getElementById(`group-${groupId}`);
            
            groupElement.classList.toggle('collapsed');
            
            // Resize plots in group after toggling with a delay
            setTimeout(() => {
                if (!groupElement.classList.contains('collapsed')) {
                    const groupIndex = groupId - 1; // Convert to 0-based index
                    resizePlotsInGroup(groupIndex);
                    
                    // Force resize after a slight delay to ensure proper layout
                    setTimeout(() => {
                        window.dispatchEvent(new Event('resize'));
                    }, 100);
                }
            }, 50);
        });
    });
    
    // Toggle all groups button
    document.getElementById('toggle-all-groups').addEventListener('click', () => {
        const groups = document.querySelectorAll('.metric-group');
        const allCollapsed = Array.from(groups).every(g => g.classList.contains('collapsed'));
        
        groups.forEach(group => {
            if (allCollapsed) {
                group.classList.remove('collapsed');
            } else {
                group.classList.add('collapsed');
            }
        });
        
        // Resize plots in all groups after toggling
        if (allCollapsed) {
            setTimeout(() => {
                resizeAllPlots();
                
                // Force resize after a slight delay to ensure proper layout
                setTimeout(() => {
                    window.dispatchEvent(new Event('resize'));
                }, 100);
            }, 50);
        }
    });
}

function resizePlotsInGroup(groupIndex) {
    // Find all plots belonging to this group
    const plots = [];
    
    zoomController.plots.forEach(plot => {
        if (plot._group === groupIndex) {
            plots.push(plot);
        }
    });
        
    plots.forEach(plot => {
        const container = plot.root.parentElement;
        if (!container) return;
        
        const newWidth = container.clientWidth;
        if (Math.abs(plot.width - newWidth) > 5) {
            plot.setSize({width: newWidth, height: plot.height});
        }
    });
}

function resizeAllPlots() {
    zoomController.plots.forEach(plot => {
        const container = plot.root.parentElement;
        if (!container) return;
        
        const newWidth = container.clientWidth;
        if (Math.abs(plot.width - newWidth) > 5) {
            plot.setSize({width: newWidth, height: plot.height});
        }
    });
}

// Create the data array for uPlot
function createDataArray(timestamps, seriesArray) {
    // First series is timestamps
    const data = [timestamps];
    
    // Add each data series values
    seriesArray.forEach(s => {
        data.push(s.values);
    });
    
    return data;
}

// Prepare heatmap data from series data
function prepareHeatmapData(seriesData) {
    // Extract CPU data from series
    const cpuData = seriesData.series.map(series => series.values);
    
    return {
        timestamps: seriesData.timestamps,
        cpuData: cpuData
    };
}

// --------------------------------------------------------
// Formatting Utility Functions
// --------------------------------------------------------

function debug(message) {
    const timestamp = new Date().toISOString().substr(11, 12);
    const formattedMessage = `[${timestamp}] ${message}`;
    
    console.log(formattedMessage);
}

function formatTime(timestamp) {
    const date = new Date(timestamp * 1000);
    return date.toLocaleTimeString([], {
        hour: '2-digit', 
        minute: '2-digit', 
        second: '2-digit', 
        hour12: false
    });
}

// Time-only formatting for axis labels (HH:MM:SS)
function formatTimeOnly(timestamp) {
    const date = new Date(timestamp * 1000);
    return date.toLocaleTimeString([], {
        hour: '2-digit', 
        minute: '2-digit', 
        second: '2-digit', 
        hour12: false
    });
}

// Format date in YYYY-MM-DD HH:MM:SS format
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

// Convert hex color to rgb for opacity support
function hexToRgb(hex) {
    // Remove # if present
    hex = hex.replace('#', '');
    
    // Parse the hex values
    const r = parseInt(hex.substring(0, 2), 16);
    const g = parseInt(hex.substring(2, 4), 16);
    const b = parseInt(hex.substring(4, 6), 16);
    
    return `${r}, ${g}, ${b}`;
}

// Interpolate between two colors
function interpolateColor(color1, color2, factor) {
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
    
    // Convert back to hex
    return `rgb(${r}, ${g}, ${b})`;
}