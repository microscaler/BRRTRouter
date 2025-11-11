# BRRTRouter Performance Profiling Dashboard

## Overview

A comprehensive Grafana dashboard for CPU and memory profiling of BRRTRouter applications, providing deep insights into performance characteristics, resource utilization, and efficiency metrics.

## Dashboard Structure

The dashboard is organized into 4 main sections with 13 panels total:

### 1. CPU Metrics Section

#### CPU Usage Gauge
- **Type**: Gauge
- **Metric**: `rate(process_cpu_seconds_total[1m]) * 100`
- **Purpose**: Real-time CPU usage percentage
- **Thresholds**: 
  - Green: 0-60%
  - Yellow: 60-80%
  - Orange: 80-90%
  - Red: >90%

#### CPU Usage Over Time
- **Type**: Time series
- **Metrics**: 1m, 5m, and 15m averages
- **Purpose**: Track CPU trends and spikes
- **Features**: Shows mean, max, min in legend

#### CPU Time Distribution
- **Type**: Donut chart
- **Metric**: `process_cpu_seconds_total`
- **Purpose**: Visualize total CPU time consumed
- **Display**: Shows both absolute time and percentages

#### CPU Efficiency
- **Type**: Stat
- **Metric**: `rate(brrtrouter_requests_total[1m]) / (rate(process_cpu_seconds_total[1m]) * 100)`
- **Purpose**: Requests processed per CPU percentage point
- **Unit**: Requests per CPU%

### 2. Memory Metrics Section

#### Memory Breakdown
- **Type**: Stacked area chart
- **Metrics**:
  - RSS (Resident Set Size) - Blue
  - Heap Memory - Green
  - Active Stack Memory - Yellow
  - VSS (Virtual Set Size) - Default
- **Purpose**: Comprehensive memory usage visualization
- **Features**: Shows current, mean, and max values

#### Memory Allocation Rate
- **Type**: Time series
- **Metrics**:
  - Heap allocation rate: `rate(process_memory_heap_bytes[1m])`
  - RSS growth rate: `rate(process_memory_rss_bytes[1m])`
- **Purpose**: Detect memory leaks and allocation patterns
- **Unit**: Bytes per second

#### Memory Efficiency
- **Type**: Stat
- **Metric**: `rate(brrtrouter_requests_total[1m]) / (process_memory_rss_bytes / 1024 / 1024)`
- **Purpose**: Requests processed per MB of memory
- **Thresholds**: Color-coded for efficiency levels

### 3. Coroutine Performance Section

#### Coroutine Stack Usage
- **Type**: Time series
- **Metrics**:
  - Configured stack size: `brrtrouter_coroutine_stack_bytes`
  - Actual stack used: `brrtrouter_coroutine_stack_used_bytes`
- **Purpose**: Monitor stack allocation efficiency
- **Insight**: Helps optimize stack size configuration

#### Stack Utilization Percentage
- **Type**: Gauge
- **Metric**: `(brrtrouter_coroutine_stack_used_bytes / brrtrouter_coroutine_stack_bytes) * 100`
- **Purpose**: Visual indicator of stack efficiency
- **Thresholds**:
  - Green: 0-50%
  - Yellow: 50-75%
  - Orange: 75-90%
  - Red: >90%

### 4. Request Performance Correlation Section

#### CPU vs Latency Correlation
- **Type**: Scatter plot overlay
- **Metrics**:
  - CPU usage (left axis): `rate(process_cpu_seconds_total[1m]) * 100`
  - Request latency (right axis): `brrtrouter_request_latency_seconds * 1000`
- **Purpose**: Identify correlation between CPU load and response times
- **Visualization**: Dual-axis for easy correlation analysis

#### Request Latency Percentiles
- **Type**: Time series
- **Metrics**: p50, p90, p95, p99 latencies
- **Calculation**: `histogram_quantile()` on request duration buckets
- **Purpose**: Track latency distribution and outliers
- **Unit**: Milliseconds

### 5. Resource Limits & Alerts Section

#### Resource Usage vs Limits
- **Type**: Bar gauge
- **Metrics**:
  - Memory usage percentage
  - CPU usage percentage
- **Purpose**: Quick visual on resource constraints
- **Display**: LCD-style bars with color thresholds

#### Health Indicators
- **Type**: Multi-stat panel
- **Metrics**:
  - Active requests: `brrtrouter_active_requests`
  - Error rate: 5xx responses / total responses
  - p99 latency: 99th percentile response time
- **Purpose**: At-a-glance health status
- **Thresholds**: Custom per metric for alerting

## Key Features

### 1. Multi-Timeframe Analysis
- 1-minute rates for real-time monitoring
- 5-minute averages for trend detection
- 15-minute averages for baseline establishment

### 2. Efficiency Metrics
- **CPU Efficiency**: Requests per CPU percentage
- **Memory Efficiency**: Requests per MB of RAM
- **Stack Efficiency**: Actual vs allocated stack usage

### 3. Correlation Analysis
- CPU vs Latency correlation
- Memory growth vs request rate
- Stack usage vs active requests

### 4. Performance Percentiles
- p50 (median) - typical performance
- p90 - most users experience
- p95 - performance SLA target
- p99 - worst-case scenarios

## Usage Patterns

### Identifying Memory Leaks
1. Check Memory Allocation Rate panel
2. Look for consistent positive slope in RSS growth
3. Correlate with request patterns
4. Compare heap vs RSS growth

### CPU Bottleneck Detection
1. Monitor CPU Usage gauge approaching red zone
2. Check CPU Efficiency metric dropping
3. Correlate with latency percentiles increasing
4. Review CPU vs Latency correlation

### Stack Optimization
1. Check Stack Utilization percentage
2. If consistently <50%, reduce `BRRTR_STACK_SIZE`
3. If >90%, increase stack size
4. Monitor for stack overflow errors

### Performance Degradation
1. Watch p99 latency trends
2. Check error rate increases
3. Monitor active request buildup
4. Correlate with resource usage

## Alert Recommendations

### Critical Alerts
- CPU Usage > 90% for 5 minutes
- Memory growth > 100MB/hour
- p99 latency > 1 second
- Error rate > 5%

### Warning Alerts
- CPU Usage > 70% for 10 minutes
- Stack utilization > 85%
- Active requests > 500
- Memory efficiency < 100 req/MB

### Info Alerts
- CPU efficiency dropping trend
- Memory allocation rate spike
- p95 latency > 500ms

## Dashboard Variables

The dashboard auto-refreshes every 5 seconds and shows the last 30 minutes by default. This can be adjusted based on needs:

- **Real-time debugging**: 5s refresh, last 5 minutes
- **Performance testing**: 10s refresh, last hour
- **Capacity planning**: 1m refresh, last 24 hours
- **Historical analysis**: No refresh, custom range

## Integration with Other Dashboards

This performance dashboard complements:

1. **Memory Monitoring Dashboard**: Detailed memory metrics and leak detection
2. **Connection Metrics Panel**: Client disconnect patterns
3. **Application Metrics**: Business-level KPIs
4. **Infrastructure Metrics**: Node/pod level resources

## Best Practices

1. **Baseline Establishment**: Run under normal load for 24 hours to establish baselines
2. **Load Testing**: Use during load tests to identify breaking points
3. **Production Monitoring**: Keep visible during deployments
4. **Capacity Planning**: Review weekly for trends
5. **Incident Response**: First dashboard to check during performance issues

## Deployment

To deploy this dashboard:

```bash
kubectl apply -f k8s/observability/grafana-dashboard-performance.yaml
```

The dashboard will automatically appear in Grafana under the "BRRTRouter" folder with tags: `performance`, `cpu`, `memory`, `profiling`.
