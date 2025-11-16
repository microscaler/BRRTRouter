#!/usr/bin/env python3
"""
Compare two performance metrics JSON files from performance_metrics_load_test.

Usage:
    python3 scripts/compare_metrics.py baseline.json current.json
    
    Or with thresholds:
    python3 scripts/compare_metrics.py baseline.json current.json \
        --max-regression 10 --fail-on-regression
"""

import json
import sys
import argparse
from typing import Dict, Any, Tuple


def load_metrics(filename: str) -> Dict[str, Any]:
    """Load metrics from JSON file."""
    with open(filename, 'r') as f:
        return json.load(f)


def calculate_change_percent(baseline: float, current: float) -> float:
    """Calculate percentage change from baseline to current."""
    if baseline == 0:
        return 0.0 if current == 0 else float('inf')
    return ((current - baseline) / baseline) * 100.0


def format_change(baseline: float, current: float, unit: str = "", lower_is_better: bool = True) -> str:
    """Format the change with color indicators."""
    change_percent = calculate_change_percent(baseline, current)
    
    if abs(change_percent) < 1.0:
        indicator = "≈"  # No significant change
        color = ""
    elif (lower_is_better and change_percent < 0) or (not lower_is_better and change_percent > 0):
        indicator = "✅"  # Improvement
        color = "\033[92m"  # Green
    else:
        indicator = "⚠️"  # Regression
        color = "\033[91m"  # Red
    
    reset = "\033[0m" if color else ""
    
    return f"{indicator} {baseline:.2f}{unit} → {current:.2f}{unit} ({color}{change_percent:+.1f}%{reset})"


def compare_metrics(baseline: Dict[str, Any], current: Dict[str, Any]) -> Tuple[bool, Dict[str, float]]:
    """
    Compare two metric sets and return whether there are regressions.
    
    Returns:
        (has_regressions, regression_dict)
    """
    print("\n╔════════════════════════════════════════════════════════════════╗")
    print("║            PERFORMANCE METRICS COMPARISON                     ║")
    print("╚════════════════════════════════════════════════════════════════╝\n")
    
    regressions = {}
    
    # Route Matching Metrics
    print("📊 Route Matching Latency:")
    for metric in ['avg_route_match_latency_us', 'p50_route_match_latency_us', 
                   'p95_route_match_latency_us', 'p99_route_match_latency_us',
                   'max_route_match_latency_us']:
        b_val = baseline.get(metric, 0)
        c_val = current.get(metric, 0)
        change = calculate_change_percent(b_val, c_val)
        
        label = metric.replace('_route_match_latency_us', '').replace('_', ' ').title()
        print(f"  ├─ {label}: {format_change(b_val, c_val, 'µs')}")
        
        if change > 10:  # More than 10% regression
            regressions[metric] = change
    
    # Match Success Rate
    print("\n🎯 Match Success Rate:")
    b_error = baseline.get('match_error_rate', 0)
    c_error = current.get('match_error_rate', 0)
    print(f"  ├─ Error Rate: {format_change(b_error, c_error, '%')}")
    if calculate_change_percent(b_error, c_error) > 50 and c_error > 1.0:
        regressions['match_error_rate'] = calculate_change_percent(b_error, c_error)
    
    b_total = baseline.get('total_requests', 0)
    c_total = current.get('total_requests', 0)
    print(f"  └─ Total Requests: {format_change(b_total, c_total, '', lower_is_better=False)}")
    
    # Handler Dispatch Metrics
    print("\n⚡ Handler Dispatch Latency:")
    for metric in ['avg_dispatch_latency_us', 'p95_dispatch_latency_us', 'p99_dispatch_latency_us']:
        b_val = baseline.get(metric, 0)
        c_val = current.get(metric, 0)
        change = calculate_change_percent(b_val, c_val)
        
        label = metric.replace('_dispatch_latency_us', '').replace('_', ' ').title()
        print(f"  ├─ {label}: {format_change(b_val, c_val, 'µs')}")
        
        if change > 10:  # More than 10% regression
            regressions[metric] = change
    
    # Lock Contention Metrics
    print("\n🔒 Lock Contention:")
    b_lock_avg = baseline.get('avg_lock_acquisition_us', 0)
    c_lock_avg = current.get('avg_lock_acquisition_us', 0)
    print(f"  ├─ Avg Lock Acquisition: {format_change(b_lock_avg, c_lock_avg, 'µs')}")
    
    b_lock_p99 = baseline.get('p99_lock_acquisition_us', 0)
    c_lock_p99 = current.get('p99_lock_acquisition_us', 0)
    print(f"  ├─ P99 Lock Acquisition: {format_change(b_lock_p99, c_lock_p99, 'µs')}")
    
    b_contentions = baseline.get('lock_contentions', 0)
    c_contentions = current.get('lock_contentions', 0)
    print(f"  └─ Contentions: {format_change(b_contentions, c_contentions)}")
    
    if calculate_change_percent(b_lock_p99, c_lock_p99) > 20:
        regressions['p99_lock_acquisition_us'] = calculate_change_percent(b_lock_p99, c_lock_p99)
    
    # Memory & GC Metrics
    print("\n💾 Memory & GC:")
    b_mem_avg = baseline.get('avg_memory_bytes', 0) / 1_048_576
    c_mem_avg = current.get('avg_memory_bytes', 0) / 1_048_576
    print(f"  ├─ Avg Memory: {format_change(b_mem_avg, c_mem_avg, 'MB')}")
    
    b_mem_max = baseline.get('max_memory_bytes', 0) / 1_048_576
    c_mem_max = current.get('max_memory_bytes', 0) / 1_048_576
    print(f"  ├─ Max Memory: {format_change(b_mem_max, c_mem_max, 'MB')}")
    
    b_gc = baseline.get('gc_delays_detected', 0)
    c_gc = current.get('gc_delays_detected', 0)
    print(f"  └─ GC Delays: {format_change(b_gc, c_gc)}")
    
    if calculate_change_percent(b_mem_max, c_mem_max) > 20:
        regressions['max_memory_bytes'] = calculate_change_percent(b_mem_max, c_mem_max)
    
    # Summary
    print("\n" + "="*66)
    if regressions:
        print("\n⚠️  PERFORMANCE REGRESSIONS DETECTED:\n")
        for metric, change in regressions.items():
            print(f"  • {metric}: {change:+.1f}%")
        return False, regressions
    else:
        print("\n✅ No significant performance regressions detected!")
        return True, {}


def main():
    parser = argparse.ArgumentParser(description='Compare performance metrics from two test runs')
    parser.add_argument('baseline', help='Baseline metrics JSON file')
    parser.add_argument('current', help='Current metrics JSON file')
    parser.add_argument('--max-regression', type=float, default=10.0,
                       help='Maximum allowed regression percentage (default: 10%%)')
    parser.add_argument('--fail-on-regression', action='store_true',
                       help='Exit with error code if regressions detected')
    
    args = parser.parse_args()
    
    try:
        baseline = load_metrics(args.baseline)
        current = load_metrics(args.current)
    except FileNotFoundError as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON: {e}", file=sys.stderr)
        sys.exit(1)
    
    no_regressions, regressions = compare_metrics(baseline, current)
    
    if args.fail_on_regression and not no_regressions:
        # Check if any regression exceeds threshold
        max_regression = max(regressions.values()) if regressions else 0
        if max_regression > args.max_regression:
            print(f"\n❌ Maximum regression ({max_regression:.1f}%) exceeds threshold ({args.max_regression}%)")
            sys.exit(1)
    
    print()


if __name__ == '__main__':
    main()
