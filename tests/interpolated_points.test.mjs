// Points a producer never observed the interval of.
//
// `rate()` spans a hole in a series — the total across it is known even though
// its distribution inside is not — but such a point carries no uncertainty
// band, because the honest bound on an unobserved interval is not a number.
// The `interpolated` flag is the only thing distinguishing it from a measured
// point, and these cover the path from the wire response to the echarts series
// that draws it differently.
import test from 'node:test';
import assert from 'node:assert/strict';

// The chart modules read CSS custom properties at load; stub the DOM so their
// palettes fall back to literals (same shim the other chart tests use).
globalThis.document = globalThis.document || { documentElement: {} };
globalThis.getComputedStyle =
    globalThis.getComputedStyle || (() => ({ getPropertyValue: () => '' }));

const { parseIntervals, parseInterpolated } = await import('../src/viewer/assets/lib/data.js');

// The engine's response for a counter that reads at 3s/4s/5s, goes null, and
// returns at 8s/9s. `bands` is null exactly where nobody observed the interval.
const gapped = {
    values: [[4, 100], [5, 100], [6, 133.33], [7, 133.33], [8, 133.33], [9, 100]],
    bands: [[95.24, 105.26], [95.24, 105.26], null, null, null, [95.24, 105.26]],
    interpolated: [false, false, true, true, true, false],
};

test('bands survive the points that have them when others do not', () => {
    const iv = parseIntervals(gapped);
    assert.equal(iv.length, 6, 'parallel to values');
    assert.deepEqual(
        iv.map((p) => p !== null),
        [true, true, false, false, false, true],
        'a hole nulls its own entry and leaves the rest alone',
    );
    assert.deepEqual(iv[0], [95.24, 105.26]);
});

test('bands is preferred over the all-or-nothing intervals field', () => {
    // A response carrying both: `intervals` is the lossy legacy view and goes
    // absent for the whole series as soon as one point lacks a band, so a
    // reader that trusts it loses bands it could have drawn.
    const both = { ...gapped, intervals: undefined };
    assert.ok(parseIntervals(both), 'bands alone is enough');

    // And an older response with only `intervals` still parses, unchanged.
    const legacy = {
        values: [[1, 5], [2, 6]],
        intervals: [[4, 6], [5, 7]],
    };
    assert.deepEqual(parseIntervals(legacy), [[4, 6], [5, 7]]);
});

test('interpolated parses to a boolean array, or null when nothing is', () => {
    assert.deepEqual(parseInterpolated(gapped), [false, false, true, true, true, false]);
    assert.equal(
        parseInterpolated({ interpolated: [false, false] }),
        null,
        'all-false is the same as absent — nothing to draw',
    );
    assert.equal(parseInterpolated({}), null, 'absent for non-rate queries');
    assert.equal(parseInterpolated({ interpolated: 'nope' }), null, 'malformed');
});

const { buildInterpolatedSeries } = await import('../src/viewer/assets/lib/charts/line.js');

const series = (flags) => ({
    name: 'probe',
    color: '#2E5BFF',
    timeData: [4, 5, 6, 7, 8, 9],
    valueData: [100, 100, 133.33, 133.33, 133.33, 100],
    interpolated: flags,
});

test('the overlay covers the hole and anchors to the measured line', () => {
    const [s] = buildInterpolatedSeries(series(gapped.interpolated), null);
    assert.ok(s, 'an overlay is produced');

    const drawn = s.data.map(([, v]) => v !== null);
    // Indices 2,3,4 are interpolated; 1 and 5 are their measured neighbours and
    // must be drawn too, or the dashed segment floats detached from the line.
    assert.deepEqual(drawn, [false, true, true, true, true, true]);
    assert.equal(s.lineStyle.type, 'dashed');
    assert.ok(s.lineStyle.opacity < 1, 'desaturated relative to the nominal');
    assert.equal(s.silent, true, 'the nominal line owns the tooltip');
    assert.equal(s.color ?? s.lineStyle.color, '#2E5BFF', 'keeps the series identity');
});

test('no overlay when nothing is interpolated', () => {
    assert.deepEqual(buildInterpolatedSeries(series([false, false, false, false, false, false]), null), []);
    assert.deepEqual(buildInterpolatedSeries(series(null), null), []);
    assert.deepEqual(
        buildInterpolatedSeries(series([true, true]), null),
        [],
        'a flags array that does not match the data length is ignored',
    );
});
