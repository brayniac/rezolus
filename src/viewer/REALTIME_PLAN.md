# Realtime Viewer Implementation Plan

## Overview
Transform the Rezolus viewer from static pre-computed dashboards to a dynamic realtime monitoring system.

## Architecture Changes

### Backend (Rust)

1. **API Layer** (`api.rs`)
   - [x] Create REST API endpoints for querying metrics
   - [x] Dashboard configuration endpoint (structure without data)
   - [x] Metric query endpoint with time range support
   - [x] Plot-specific endpoints for complex visualizations
   - [x] SSE/WebSocket endpoints for streaming updates

2. **Realtime Module** (`realtime.rs`)
   - [x] New viewer mode that keeps TSDB in memory
   - [x] Agent connector for fetching live data
   - [x] WebSocket handler for pushing updates
   - [ ] Data ingestion and time-series management

3. **TSDB Enhancements**
   - [ ] Add time range query support
   - [ ] Implement sliding window for memory management
   - [ ] Add metric discovery/listing
   - [ ] Support incremental updates

4. **Data Sources**
   - [ ] Connect to Rezolus agent msgpack endpoint
   - [ ] Support for multiple data sources
   - [ ] Data aggregation and downsampling

### Frontend (JavaScript/TypeScript)

1. **Data Fetching Layer**
   - [ ] Replace static JSON loading with API calls
   - [ ] Implement time range selector
   - [ ] Add refresh interval control
   - [ ] Cache management for fetched data

2. **Chart Updates**
   - [ ] Modify ECharts integration for dynamic data
   - [ ] Implement incremental chart updates
   - [ ] Add loading states and error handling
   - [ ] Smooth transitions for live updates

3. **UI Components**
   - [ ] Time range picker (last 5m, 15m, 1h, 6h, 24h, custom)
   - [ ] Refresh rate selector (1s, 5s, 10s, 30s, 1m)
   - [ ] Live/pause toggle
   - [ ] Connection status indicator

4. **WebSocket Integration**
   - [ ] Establish WebSocket connection for live mode
   - [ ] Handle reconnection logic
   - [ ] Process incremental updates
   - [ ] Update charts without full redraw

## Implementation Steps

### Phase 1: Backend API
1. Complete TSDB query methods
2. Implement data ingestion from agent
3. Add time-based data management
4. Create comprehensive API endpoints

### Phase 2: Frontend Refactor
1. Create API client module
2. Update dashboard components to fetch data
3. Implement time range controls
4. Add loading and error states

### Phase 3: Live Updates
1. Implement WebSocket connection
2. Add incremental update logic
3. Optimize chart rendering
4. Add connection management

### Phase 4: Polish
1. Add data caching and optimization
2. Implement proper error handling
3. Add configuration options
4. Performance tuning

## Benefits

1. **Real-time Monitoring**: See metrics as they happen
2. **Historical Analysis**: Query any time range
3. **Lower Memory Usage**: Don't pre-compute all dashboards
4. **Flexibility**: Easy to add new visualizations
5. **Scalability**: Can handle continuous data streams

## Challenges

1. **Memory Management**: Need to limit historical data retention
2. **Performance**: Efficient queries over large datasets
3. **Network**: Handle connection issues gracefully
4. **UI Complexity**: More interactive components needed

## Migration Path

1. Keep existing static mode as default
2. Add `--realtime` flag for new mode
3. Gradually migrate features
4. Eventually deprecate static mode

## API Endpoints

```
GET /api/metrics              - List available metrics
GET /api/dashboard/:section   - Get dashboard configuration
GET /api/query                - Query metric data
GET /api/plot/:section/:id    - Get specific plot data
GET /api/stream               - SSE endpoint for updates
WS  /ws                       - WebSocket for bidirectional updates
```

## Query Parameters

```
?metric=cpu_usage             - Metric name
&labels={"state":"user"}      - Label filters (JSON)
&start=1234567890            - Start timestamp
&end=1234567899              - End timestamp
&refresh=5                   - Refresh interval (seconds)
```