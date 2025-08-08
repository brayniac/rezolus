import {
    createAxisLabelFormatter,
} from './util/units.js';
import {
    getBaseOption,
    getBaseYAxisOption,
    getTooltipFormatter,
} from './base.js';

/**
 * Configures the Chart based on Chart.spec
 * Responsible for calling setOption on the echart instance, and for setting up any
 * chart-specific dynamic behavior.
 * @param {import('./chart.js').Chart} chart - the chart to configure
 */
export function configureLineChart(chart) {
    const {
        data,
        opts
    } = chart.spec;

    const baseOption = getBaseOption(opts.title, (val) => val);

    if (!data || data.length < 2) {
        // Show empty chart with "No data" message
        const emptyOption = {
            ...baseOption,
            yAxis: getBaseYAxisOption(false, undefined, undefined, opts.format?.unit_system),
            graphic: {
                type: 'text',
                left: 'center',
                top: 'middle',
                style: {
                    text: 'No data',
                    fontSize: 14,
                    fill: '#999'
                }
            }
        };
        chart.echart.setOption(emptyOption);
        return;
    }

    const [timeData, valueData] = data;
    
    // Check if arrays are empty
    if (!timeData || timeData.length === 0 || !valueData || valueData.length === 0) {
        // Show empty chart with "No data" message
        const emptyOption = {
            ...baseOption,
            yAxis: getBaseYAxisOption(false, undefined, undefined, opts.format?.unit_system),
            graphic: {
                type: 'text',
                left: 'center',
                top: 'middle',
                style: {
                    text: 'No data',
                    fontSize: 14,
                    fill: '#999'
                }
            }
        };
        chart.echart.setOption(emptyOption);
        return;
    }

    const zippedData = timeData.map((t, i) => [t * 1000, valueData[i]]);

    // Access format properties using snake_case naming to match Rust serialization
    const format = opts.format || {};
    const unitSystem = format.unit_system;
    // const yAxisLabel = format.y_axis_label || format.axis_label;
    // const valueLabel = format.value_label;
    const logScale = format.log_scale;
    const minValue = format.min;
    const maxValue = format.max;

    const option = {
        ...baseOption,
        yAxis: getBaseYAxisOption(logScale, minValue, maxValue, unitSystem),
        tooltip: {
            ...baseOption.tooltip,
            formatter: getTooltipFormatter(unitSystem ?
                createAxisLabelFormatter(unitSystem) :
                val => val),
        },
        series: [{
            data: zippedData,
            type: 'line',
            name: opts.title,
            showSymbol: false,
            emphasis: {
                focus: 'series'
            },
            step: 'start',
            lineStyle: {
                width: 2
            },
            animationDuration: 0, // Don't animate the line in
        }]
    };

    chart.echart.setOption(option);
}