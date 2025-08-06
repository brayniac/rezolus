// multi.js - Multi-series chart configuration with deterministic cgroup colors

import {
    createAxisLabelFormatter,
} from './util/units.js';
import {
    getBaseOption,
    getBaseYAxisOption,
    getTooltipFormatter,
} from './base.js';
import globalColorMapper from './util/colormap.js';

/**
 * Configures the Chart based on Chart.spec
 * Responsible for calling setOption on the echart instance, and for setting up any
 * chart-specific dynamic behavior.
 * @param {import('./chart.js').Chart} chart - the chart to configure
 */
export function configureMultiSeriesChart(chart) {
    const {
        data,
        opts,
    } = chart.spec;

    const baseOption = getBaseOption(opts.title);

    if (!data || data.length < 2) {
        return baseOption;
    }


    // For multi-series charts, the first row contains timestamps, subsequent rows are series data
    const timeData = data[0];
    const lineCount = data.length - 1;

    let seriesNames = chart.spec.series_names;
    if (!seriesNames || seriesNames.length !== lineCount) {
        console.log("series_names is missing or wrong length", seriesNames);
        seriesNames = Array.from(Array(lineCount).keys()).map(i => `Series ${i + 1}`);
    }

    // Access format properties using snake_case naming to match Rust serialization
    const format = opts.format || {};
    const unitSystem = format.unit_system;
    // const yAxisLabel = format.y_axis_label || format.axis_label;
    // const valueLabel = format.value_label;
    const logScale = format.log_scale;
    const minValue = format.min;
    const maxValue = format.max;

    // Create series configurations for each data series
    const series = [];

    // Determine if this is a cgroup chart (where consistent colors matter)
    const isCgroupChart = opts.id && opts.id.includes('cgroup');
    
    // Get colors - use deterministic colors for cgroups, index-based for others
    let colors;
    if (isCgroupChart) {
        colors = globalColorMapper.getColors(seriesNames);
    } else {
        // Use index-based colors for better distinction
        colors = seriesNames.map((_, index) => globalColorMapper.getColorByIndex(index));
    }

    for (let i = 1; i < data.length; i++) {
        const name = seriesNames[i - 1];
        const isOtherCategory = name === "Other";
        const color = (i <= colors.length) ? colors[i - 1] : undefined;

        const zippedData = timeData.map((t, j) => [t * 1000, data[i][j]]);

        series.push({
            name: name,
            type: 'line',
            data: zippedData,
            itemStyle: {
                color,
            },
            step: 'start',
            lineStyle: {
                color,
                width: 2,
            },
            // Add symbol for "Other" category to make it more distinguishable
            showSymbol: isOtherCategory,
            symbolSize: isOtherCategory ? 4 : 0,
            // Ensure "Other" appears behind other lines
            z: isOtherCategory ? 1 : 2,
            emphasis: {
                focus: 'series'
            },
            animationDuration: 0
        });
    }

    // Ensure "Other" category is the last in the series array so it appears in the legend last.
    const otherIndex = series.findIndex(s => s.name === "Other");
    if (otherIndex !== -1 && otherIndex !== series.length - 1) {
        const otherSeries = series.splice(otherIndex, 1)[0];
        series.push(otherSeries);
    }

    const option = {
        ...baseOption,
        yAxis: getBaseYAxisOption(logScale, minValue, maxValue, unitSystem),
        tooltip: {
            ...baseOption.tooltip,
            formatter: getTooltipFormatter(unitSystem ?
                createAxisLabelFormatter(unitSystem) :
                val => val),
        },
        series: series,
        // Use our custom color palette
        color: colors,
    };

    chart.echart.setOption(option);
}