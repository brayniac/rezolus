// time-axis-formatter.js
// Adaptive time axis formatter for uPlot charts
// Automatically adjusts time labels based on zoom level using natural time intervals

// Define our time intervals in seconds
const TIME_INTERVALS = [
    { seconds: 1, format: 'HH:mm:ss' },       // 1 second
    { seconds: 5, format: 'HH:mm:ss' },       // 5 seconds
    { seconds: 15, format: 'HH:mm:ss' },      // 15 seconds
    { seconds: 30, format: 'HH:mm:ss' },      // 30 seconds
    { seconds: 60, format: 'HH:mm' },         // 1 minute
    { seconds: 5 * 60, format: 'HH:mm' },     // 5 minutes
    { seconds: 15 * 60, format: 'HH:mm' },    // 15 minutes
    { seconds: 30 * 60, format: 'HH:mm' },    // 30 minutes
    { seconds: 60 * 60, format: 'HH:mm' },    // 1 hour
    { seconds: 3 * 60 * 60, format: 'HH:mm' }, // 3 hours
    { seconds: 6 * 60 * 60, format: 'HH:mm' }, // 6 hours
    { seconds: 12 * 60 * 60, format: 'HH:mm' }, // 12 hours
    { seconds: 24 * 60 * 60, format: 'MM-DD HH:mm' }, // 1 day
];

/**
 * Format a timestamp based on specified format
 * @param {number} timestamp - Unix timestamp in seconds
 * @param {string} format - Format string ('HH:mm:ss', 'HH:mm', etc)
 * @returns {string} Formatted time string
 */
function formatTimeByPattern(timestamp, format) {
    const date = new Date(timestamp * 1000);
    
    // Handle different format strings
    if (format === 'HH:mm:ss') {
        return date.toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
            hour12: false
        });
    } else if (format === 'HH:mm') {
        return date.toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            hour12: false
        });
    } else if (format === 'MM-DD HH:mm') {
        return `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')} ${date.toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
            hour12: false
        })}`;
    }
    
    return date.toLocaleTimeString();
}

/**
 * Get appropriate interval based on visible time range
 * @param {number} timeSpanSeconds - Visible time range in seconds 
 * @returns {Object} Selected interval information
 */
function getInterval(timeSpanSeconds) {
    // Target ~5-8 labels on the axis for readability
    const targetDensity = timeSpanSeconds / 6;
    
    // Find the appropriate interval based on visible range
    let selectedInterval = TIME_INTERVALS[TIME_INTERVALS.length - 1];
    
    for (let i = 0; i < TIME_INTERVALS.length; i++) {
        if (TIME_INTERVALS[i].seconds >= targetDensity) {
            selectedInterval = TIME_INTERVALS[i];
            break;
        }
    }
    
    return selectedInterval;
}

/**
 * Snap a timestamp to the nearest interval boundary
 * @param {number} timestamp - Unix timestamp in seconds
 * @param {number} intervalSeconds - Interval in seconds
 * @returns {number} Snapped timestamp
 */
function snapToInterval(timestamp, intervalSeconds) {
    return Math.floor(timestamp / intervalSeconds) * intervalSeconds;
}

/**
 * Generate values for axis ticks aligned to natural time boundaries
 * @param {number} scaleMin - Minimum visible timestamp
 * @param {number} scaleMax - Maximum visible timestamp
 * @param {Object} interval - Time interval information
 * @returns {Array<number>} Aligned tick values
 */
function generateAlignedTicks(scaleMin, scaleMax, interval) {
    const ticks = [];
    
    // Find the first tick (aligned to interval)
    let tick = snapToInterval(scaleMin, interval.seconds);
    
    // If first tick is before visible range, move to next interval
    if (tick < scaleMin) {
        tick += interval.seconds;
    }
    
    // Generate ticks within visible range
    while (tick <= scaleMax) {
        ticks.push(tick);
        tick += interval.seconds;
    }
    
    return ticks;
}

/**
 * Creates a time axis formatter that displays natural time intervals
 * @returns {Function} Formatter function for uPlot axes
 */
function createTimeAxisFormatter() {
    return function(u, axisVals) {
        if (!u.scales.x) return axisVals.map(v => formatTimeByPattern(v, 'HH:mm:ss'));
        
        const timeSpanSeconds = u.scales.x.max - u.scales.x.min;
        const interval = getInterval(timeSpanSeconds);
        
        return axisVals.map(v => formatTimeByPattern(v, interval.format));
    };
}

/**
 * Creates a splits function for custom tick placement
 * @returns {Function} Splits function for uPlot axes
 */
function createTimeSplitsFn() {
    return function(u, axisIdx, scaleMin, scaleMax, foundIncr, foundSpace) {
        if (!u || !u.scales || !u.scales.x) return null;
        
        const timeSpanSeconds = u.scales.x.max - u.scales.x.min;
        const interval = getInterval(timeSpanSeconds);
        
        return generateAlignedTicks(scaleMin, scaleMax, interval);
    };
}