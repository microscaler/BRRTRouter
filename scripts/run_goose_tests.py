#!/usr/bin/env python3
"""
Consistent Goose performance test runner for BRRTRouter JSF improvements.

This script runs Goose load tests with consistent methodology, extracts metrics,
and compares against baselines to track JSF optimization progress.

The script runs each test 3 times and averages the results to reduce variance
and provide more reliable performance metrics.

Usage:
    python3 scripts/run_goose_tests.py --label jsf-p0-1 --users 2000 --run-time 60s
    python3 scripts/run_goose_tests.py --label jsf-p0-2 --baseline jsf-p0-1
"""

import argparse
import json
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Tuple


def parse_goose_output(output_file: Path) -> Dict:
    """Parse Goose test output file and extract key metrics."""
    return extract_metrics_from_output(output_file)


def run_goose_test(host: str, users: int, run_time: str, hatch_rate: int, 
                  output_file: Path) -> subprocess.CompletedProcess:
    """Run Goose load test and capture output."""
    print(f"🦆 Running Goose load test...")
    print(f"   Host: {host}")
    print(f"   Users: {users}")
    print(f"   Run Time: {run_time}")
    print(f"   Hatch Rate: {hatch_rate}/s")
    print(f"   Output: {output_file}\n")
    
    cmd = [
        'cargo', 'run', '--release', '--example', 'api_load_test',
        '--',
        '--host', host,
        '--users', str(users),
        '--run-time', run_time,
        '--hatch-rate', str(hatch_rate),
        '--no-reset-metrics',
    ]
    
    with open(output_file, 'w') as f:
        result = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, text=True)
    
    return result


def wait_for_server(host: str, max_wait: int = 30) -> bool:
    """Wait for server to be ready before running tests."""
    import urllib.request
    import urllib.error
    
    health_url = f"{host}/health"
    start_time = time.time()
    
    while time.time() - start_time < max_wait:
        try:
            urllib.request.urlopen(health_url, timeout=2)
            return True
        except (urllib.error.URLError, OSError):
            time.sleep(1)
    
    return False


def warmup_server(host: str, warmup_time: int = 10) -> None:
    """Run brief warmup to stabilize server state."""
    print(f"🔥 Warming up server for {warmup_time}s...")
    cmd = [
        'cargo', 'run', '--release', '--example', 'api_load_test',
        '--',
        '--host', host,
        '--users', '10',
        '--hatch-rate', '5',
        '--run-time', f'{warmup_time}s',
        '--no-reset-metrics',
    ]
    subprocess.run(cmd, capture_output=True, text=True)
    print("✅ Warmup complete\n")


def extract_metrics_from_output(output_file: Path) -> Dict:
    """Extract structured metrics from Goose output text."""
    with open(output_file, 'r') as f:
        content = f.read()
    
    metrics = {
        'timestamp': datetime.now().isoformat(),
        'output_file': str(output_file),
    }
    
    # Extract aggregated metrics
    aggregated_match = re.search(
        r'Aggregated\s+\|\s+(\d+(?:,\d+)*)\s+\|\s+(\d+)\s+\(([\d.]+)%\)\s+\|\s+([\d.]+)',
        content
    )
    if aggregated_match:
        metrics['total_requests'] = int(aggregated_match.group(1).replace(',', ''))
        metrics['total_failures'] = int(aggregated_match.group(2))
        metrics['failure_rate'] = float(aggregated_match.group(3))
        metrics['throughput_req_per_sec'] = float(aggregated_match.group(4))
    
    # Extract aggregated latency (Avg, Min, Max, Median) - look for PER TRANSACTION METRICS section
    # Format: Aggregated | 22.91 | 1 | 408 | 22
    latency_match = re.search(
        r'=== PER TRANSACTION METRICS ===.*?Name\s+\|\s+Avg \(ms\)\s+\|\s+Min\s+\|\s+Max\s+\|\s+Median.*?\n.*?Aggregated\s+\|\s+([\d.]+)\s+\|\s+(\d+)\s+\|\s+(\d+)\s+\|\s+(\d+)',
        content,
        re.DOTALL
    )
    if latency_match:
        metrics['latency_avg_ms'] = float(latency_match.group(1))
        metrics['latency_min_ms'] = int(latency_match.group(2))
        metrics['latency_max_ms'] = int(latency_match.group(3))
        metrics['latency_median_ms'] = int(latency_match.group(4))
    
    # Extract percentile latencies - look for "Slowest page load" section
    # Format: Aggregated | 22 | 31 | 54 | 60 | 130 | 340
    percentile_match = re.search(
        r'Slowest page load.*?\n.*?Name\s+\|\s+50%\s+\|\s+75%\s+\|\s+98%\s+\|\s+99%\s+\|\s+99\.9%\s+\|\s+99\.99%.*?\n.*?Aggregated\s+\|\s+(\d+)\s+\|\s+(\d+)\s+\|\s+(\d+)\s+\|\s+(\d+)\s+\|\s+(\d+)\s+\|\s+(\d+)',
        content,
        re.DOTALL
    )
    if percentile_match:
        metrics['latency_p50_ms'] = int(percentile_match.group(1))
        metrics['latency_p75_ms'] = int(percentile_match.group(2))
        metrics['latency_p98_ms'] = int(percentile_match.group(3))
        metrics['latency_p99_ms'] = int(percentile_match.group(4))
        metrics['latency_p99_9_ms'] = int(percentile_match.group(5))
        metrics['latency_p99_99_ms'] = int(percentile_match.group(6))
    
    return metrics


def average_metrics(metrics_list: List[Dict]) -> Dict:
    """Average metrics from multiple test runs."""
    if not metrics_list:
        return {}
    
    # Start with the first run's metadata (non-numeric fields)
    averaged = {
        'timestamp': datetime.now().isoformat(),
        'label': metrics_list[0].get('label', 'unknown'),
        'output_file': metrics_list[0].get('output_file', ''),
        'config': metrics_list[0].get('config', {}),
        'runs': len(metrics_list),
    }
    
    # Numeric metrics to average
    numeric_metrics = [
        'total_requests', 'total_failures', 'failure_rate',
        'throughput_req_per_sec', 'latency_avg_ms', 'latency_min_ms',
        'latency_max_ms', 'latency_median_ms', 'latency_p50_ms',
        'latency_p75_ms', 'latency_p98_ms', 'latency_p99_ms',
        'latency_p99_9_ms', 'latency_p99_99_ms'
    ]
    
    # Calculate averages for each metric
    for metric in numeric_metrics:
        values = []
        for run_metrics in metrics_list:
            if metric in run_metrics:
                values.append(run_metrics[metric])
        
        if values:
            if metric in ['total_requests', 'total_failures', 'latency_min_ms', 
                         'latency_max_ms', 'latency_median_ms', 'latency_p50_ms',
                         'latency_p75_ms', 'latency_p98_ms', 'latency_p99_ms',
                         'latency_p99_9_ms', 'latency_p99_99_ms']:
                # Integer metrics - round to nearest integer
                averaged[metric] = round(sum(values) / len(values))
            else:
                # Float metrics - keep precision
                averaged[metric] = sum(values) / len(values)
    
    return averaged


def save_metrics(metrics: Dict, label: str, output_dir: Path) -> Path:
    """Save metrics to JSON file with label."""
    output_dir.mkdir(parents=True, exist_ok=True)
    metrics_file = output_dir / f"{label}_metrics.json"
    
    with open(metrics_file, 'w') as f:
        json.dump(metrics, f, indent=2)
    
    return metrics_file


def load_baseline_metrics(baseline_label: str, output_dir: Path) -> Optional[Dict]:
    """Load baseline metrics for comparison."""
    baseline_file = output_dir / f"{baseline_label}_metrics.json"
    
    if not baseline_file.exists():
        return None
    
    with open(baseline_file, 'r') as f:
        return json.load(f)


def compare_metrics(baseline: Dict, current: Dict) -> Dict:
    """Compare current metrics against baseline."""
    comparison = {
        'baseline_label': baseline.get('label', 'unknown'),
        'current_label': current.get('label', 'unknown'),
        'improvements': [],
        'regressions': [],
        'changes': {},
    }
    
    # Metrics where higher is better
    higher_better = ['throughput_req_per_sec', 'total_requests']
    
    # Metrics where lower is better
    lower_better = [
        'latency_avg_ms', 'latency_p50_ms', 'latency_p75_ms', 
        'latency_p98_ms', 'latency_p99_ms', 'latency_p99_9_ms',
        'latency_p99_99_ms', 'latency_max_ms', 'failure_rate'
    ]
    
    for metric in higher_better + lower_better:
        if metric not in baseline or metric not in current:
            continue
        
        baseline_val = baseline[metric]
        current_val = current[metric]
        
        if baseline_val == 0:
            change_pct = 0.0 if current_val == 0 else float('inf')
        else:
            change_pct = ((current_val - baseline_val) / baseline_val) * 100.0
        
        comparison['changes'][metric] = {
            'baseline': baseline_val,
            'current': current_val,
            'change_pct': change_pct,
        }
        
        # Determine if improvement or regression
        is_improvement = False
        if metric in higher_better:
            is_improvement = change_pct > 0
        else:
            is_improvement = change_pct < 0
        
        change_info = {
            'metric': metric,
            'baseline': baseline_val,
            'current': current_val,
            'change_pct': change_pct,
        }
        
        if is_improvement:
            comparison['improvements'].append(change_info)
        elif abs(change_pct) > 1.0:  # Only report significant regressions (>1%)
            comparison['regressions'].append(change_info)
    
    return comparison


def format_comparison_report(comparison: Dict) -> str:
    """Format comparison results as human-readable report."""
    lines = []
    lines.append("\n" + "="*70)
    lines.append("PERFORMANCE COMPARISON REPORT")
    lines.append("="*70)
    lines.append(f"Baseline: {comparison['baseline_label']}")
    lines.append(f"Current:  {comparison['current_label']}")
    lines.append("")
    
    # Improvements
    if comparison['improvements']:
        lines.append("✅ IMPROVEMENTS:")
        for imp in comparison['improvements']:
            metric_name = imp['metric'].replace('_', ' ').title()
            lines.append(f"  • {metric_name}: {imp['baseline']:.2f} → {imp['current']:.2f} ({imp['change_pct']:+.1f}%)")
        lines.append("")
    
    # Regressions
    if comparison['regressions']:
        lines.append("⚠️  REGRESSIONS:")
        for reg in comparison['regressions']:
            metric_name = reg['metric'].replace('_', ' ').title()
            lines.append(f"  • {metric_name}: {reg['baseline']:.2f} → {reg['current']:.2f} ({reg['change_pct']:+.1f}%)")
        lines.append("")
    
    # Key metrics summary
    lines.append("KEY METRICS:")
    key_metrics = [
        ('throughput_req_per_sec', 'Throughput (req/s)'),
        ('latency_p50_ms', 'P50 Latency (ms)'),
        ('latency_p99_ms', 'P99 Latency (ms)'),
        ('failure_rate', 'Failure Rate (%)'),
    ]
    
    for metric_key, metric_name in key_metrics:
        if metric_key in comparison['changes']:
            change = comparison['changes'][metric_key]
            lines.append(f"  {metric_name:25} {change['baseline']:>10.2f} → {change['current']:>10.2f} ({change['change_pct']:>+6.1f}%)")
    
    lines.append("="*70)
    
    return "\n".join(lines)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description='Run consistent Goose performance tests for JSF improvements',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    
    parser.add_argument('--label', required=True,
                       help='Label for this test run (e.g., jsf-p0-1, jsf-p0-2)')
    parser.add_argument('--host', default='http://127.0.0.1:8080',
                       help='Target host (default: http://127.0.0.1:8080)')
    parser.add_argument('--users', type=int, default=2000,
                       help='Number of concurrent users (default: 2000)')
    parser.add_argument('--run-time', default='60s',
                       help='Test duration (default: 60s)')
    parser.add_argument('--hatch-rate', type=int, default=200,
                       help='Users to spawn per second (default: 200)')
    parser.add_argument('--warmup-time', type=int, default=10,
                       help='Warmup duration in seconds (default: 10)')
    parser.add_argument('--baseline', 
                       help='Baseline label to compare against')
    parser.add_argument('--output-dir', type=Path, default=Path('/tmp/goose_metrics'),
                       help='Directory for metrics storage (default: /tmp/goose_metrics)')
    parser.add_argument('--skip-warmup', action='store_true',
                       help='Skip server warmup phase')
    parser.add_argument('--skip-server-check', action='store_true',
                       help='Skip server health check')
    
    args = parser.parse_args()
    
    # Check server is ready
    if not args.skip_server_check:
        print("🔍 Checking server health...")
        if not wait_for_server(args.host):
            print(f"❌ Server not responding at {args.host}")
            print("   Start server with: just debug-petstore")
            sys.exit(1)
        print("✅ Server is ready\n")
    
    # Warmup (only once before all runs)
    if not args.skip_warmup:
        warmup_server(args.host, args.warmup_time)
    
    # Run test 3 times and collect metrics
    print("🔄 Running best-of-3 performance test...\n")
    all_metrics = []
    
    for run_num in range(1, 4):
        print(f"📊 Run {run_num}/3")
        output_file = Path(f"/tmp/goose_{args.label}_{args.users}users_run{run_num}.txt")
        
        result = run_goose_test(
            args.host, args.users, args.run_time, args.hatch_rate, output_file
        )
        
        if result.returncode != 0:
            print(f"❌ Goose test failed with exit code {result.returncode}")
            sys.exit(1)
        
        # Extract metrics from this run
        print(f"📊 Extracting metrics from run {run_num}...")
        run_metrics = extract_metrics_from_output(output_file)
        run_metrics['label'] = args.label
        run_metrics['config'] = {
            'host': args.host,
            'users': args.users,
            'run_time': args.run_time,
            'hatch_rate': args.hatch_rate,
        }
        run_metrics['run_number'] = run_num
        
        all_metrics.append(run_metrics)
        
        # Print quick summary for this run
        print(f"  Run {run_num}: {run_metrics.get('throughput_req_per_sec', 0):,.0f} req/s, "
              f"P50={run_metrics.get('latency_p50_ms', 0)}ms, "
              f"P99={run_metrics.get('latency_p99_ms', 0)}ms\n")
        
        # Small delay between runs to let system stabilize
        if run_num < 3:
            print("⏳ Waiting 5s before next run...\n")
            time.sleep(5)
    
    # Average all metrics
    print("📊 Averaging metrics from 3 runs...")
    metrics = average_metrics(all_metrics)
    
    # Save individual run metrics for reference
    for run_metrics in all_metrics:
        run_num = run_metrics['run_number']
        run_file = args.output_dir / f"{args.label}_run{run_num}_metrics.json"
        run_file.parent.mkdir(parents=True, exist_ok=True)
        with open(run_file, 'w') as f:
            json.dump(run_metrics, f, indent=2)
    
    # Save averaged metrics
    metrics_file = save_metrics(metrics, args.label, args.output_dir)
    print(f"✅ Averaged metrics saved to {metrics_file}\n")
    
    # Print current metrics
    print("📈 CURRENT METRICS:")
    print(f"  Throughput: {metrics.get('throughput_req_per_sec', 0):,.0f} req/s")
    print(f"  Total Requests: {metrics.get('total_requests', 0):,}")
    print(f"  Failures: {metrics.get('total_failures', 0)} ({metrics.get('failure_rate', 0):.2f}%)")
    print(f"  P50 Latency: {metrics.get('latency_p50_ms', 0)}ms")
    print(f"  P99 Latency: {metrics.get('latency_p99_ms', 0)}ms")
    print()
    
    # Compare with baseline if provided
    if args.baseline:
        print(f"📊 Comparing with baseline: {args.baseline}")
        baseline = load_baseline_metrics(args.baseline, args.output_dir)
        
        if not baseline:
            print(f"⚠️  Baseline '{args.baseline}' not found")
            print(f"   Available baselines in {args.output_dir}:")
            for f in args.output_dir.glob("*_metrics.json"):
                print(f"     - {f.stem.replace('_metrics', '')}")
        else:
            comparison = compare_metrics(baseline, metrics)
            report = format_comparison_report(comparison)
            print(report)
            
            # Save comparison
            comparison_file = args.output_dir / f"{args.label}_vs_{args.baseline}.json"
            with open(comparison_file, 'w') as f:
                json.dump(comparison, f, indent=2)
            print(f"📄 Comparison saved to {comparison_file}\n")
    
    print("✅ Test complete!")
    print(f"   Output: {output_file}")
    print(f"   Metrics: {metrics_file}")


if __name__ == '__main__':
    main()

