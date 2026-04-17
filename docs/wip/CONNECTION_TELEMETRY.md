# Connection Telemetry Implementation

## Overview

Added comprehensive connection telemetry to BRRTRouter to track client disconnections and connection errors separately from normal requests.

## Implementation

### 1. Metrics Middleware Enhancement

Added three new metrics to `MetricsMiddleware`:

```rust
/// Connection close events (client disconnects, timeouts, etc.)
connection_closes: AtomicUsize,
/// Connection errors (broken pipe, reset, etc.)
connection_errors: AtomicUsize,
```

### 2. New Metrics Methods

```rust
pub fn inc_connection_close(&self) // Increment connection close counter
pub fn connection_closes(&self) -> usize // Get total connection closes
pub fn inc_connection_error(&self) // Increment connection error counter  
pub fn connection_errors(&self) -> usize // Get total connection errors
pub fn connection_health_ratio(&self) -> f64 // Calculate health ratio
```

### 3. Connection Health Ratio

The health ratio provides a single metric showing the proportion of successful requests:

```rust
total_requests / (total_requests + total_issues)
```

Where `total_issues = connection_closes + connection_errors`

A ratio of 1.0 means perfect health, lower values indicate connection problems.

## Prometheus Metrics

Three new metrics are exported at `/metrics`:

```prometheus
# HELP brrtrouter_connection_closes_total Total number of connection close events (client disconnects)
# TYPE brrtrouter_connection_closes_total counter
brrtrouter_connection_closes_total 42

# HELP brrtrouter_connection_errors_total Total number of connection errors (broken pipe, reset, etc.)
# TYPE brrtrouter_connection_errors_total counter
brrtrouter_connection_errors_total 3

# HELP brrtrouter_connection_health_ratio Ratio of successful requests to total connection events
# TYPE brrtrouter_connection_health_ratio gauge
brrtrouter_connection_health_ratio 0.9567
```

## Grafana Dashboard

Added a new "Connection Metrics" panel to the BRRTRouter Memory Monitoring dashboard showing:

1. **Connection Closes Rate** (yellow line) - Rate of client disconnections per minute
2. **Connection Errors Rate** (red line) - Rate of connection errors per minute  
3. **Connection Health Ratio** (green line) - Overall connection health as a percentage

The panel uses:
- Time series visualization
- 1-minute rate calculations for closes/errors
- Percentage display for health ratio
- Color coding for quick visual assessment

## Usage

### Manual Tracking

To track connection events in custom code:

```rust
// When detecting a client disconnect
if let Err(e) = stream.read() {
    match e.kind() {
        io::ErrorKind::BrokenPipe | 
        io::ErrorKind::UnexpectedEof => {
            metrics.inc_connection_close();
        }
        io::ErrorKind::ConnectionAborted |
        io::ErrorKind::ConnectionReset => {
            metrics.inc_connection_error();
        }
        _ => {}
    }
}
```

### Automatic Tracking

Currently, connection events need to be manually tracked as may_minihttp logs them but doesn't expose hooks.

Future work could include:
1. PR to may_minihttp to add connection event callbacks
2. Custom tracing layer to intercept may_minihttp logs
3. Wrapper around TcpStream to detect disconnections

## Monitoring Best Practices

### Healthy Ratios
- `> 0.99` - Excellent, minimal connection issues
- `0.95 - 0.99` - Good, some client disconnects
- `0.90 - 0.95` - Fair, investigate if unexpected
- `< 0.90` - Poor, likely infrastructure issues

### Common Causes of Connection Closes
- Client timeouts
- Browser navigation away
- Mobile network switches
- Keep-alive expiry
- Load balancer health checks

### Common Causes of Connection Errors
- Network interruptions
- Proxy/firewall issues
- Resource exhaustion
- DDoS attempts
- Infrastructure problems

## Benefits

1. **Visibility**: Distinguish between normal disconnects and errors
2. **Alerting**: Set thresholds on health ratio for proactive monitoring
3. **Debugging**: Correlate connection issues with other metrics
4. **Capacity Planning**: Understand connection patterns and scaling needs
5. **SLA Monitoring**: Track connection reliability for SLA compliance

## Future Enhancements

1. **Per-endpoint tracking**: Track connection metrics per API endpoint
2. **Client identification**: Group metrics by client IP/user agent
3. **Geo-location**: Track connection quality by region
4. **Time-of-day patterns**: Identify peak connection issue times
5. **Automatic mitigation**: Circuit breakers based on connection health
