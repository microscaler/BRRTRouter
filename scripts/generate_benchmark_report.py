#!/usr/bin/env python3
"""
Generate a comprehensive benchmark report for BRRTRouter performance tests.

Usage:
    python3 scripts/generate_benchmark_report.py [--output-dir DIR] [--users N] [--run-time TIME]
    
Example:
    # Basic usage
    python3 scripts/generate_benchmark_report.py
    
    # With custom configuration
    python3 scripts/generate_benchmark_report.py --output-dir ./reports --users 500 --run-time 10m
    
This script:
1. Runs the performance metrics load test
2. Collects metrics from the test
3. Generates a markdown report
4. Optionally compares with baseline
"""

import argparse
import json
import os
import platform
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Dict, Any, Optional


def get_system_info() -> Dict[str, str]:
    """Collect system information for the report."""
    info = {
        'os': f"{platform.system()} {platform.release()}",
        'machine': platform.machine(),
        'processor': platform.processor() or 'Unknown',
        'python': platform.python_version(),
    }
    
    # Try to get CPU count
    try:
        import multiprocessing
        info['cpu_cores'] = str(multiprocessing.cpu_count())
    except:
        info['cpu_cores'] = 'Unknown'
    
    # Try to get Rust version
    try:
        result = subprocess.run(['rustc', '--version'], capture_output=True, text=True)
        if result.returncode == 0:
            info['rust'] = result.stdout.strip()
    except:
        info['rust'] = 'Unknown'
    
    # Try to get Cargo version
    try:
        result = subprocess.run(['cargo', '--version'], capture_output=True, text=True)
        if result.returncode == 0:
            info['cargo'] = result.stdout.strip()
    except:
        info['cargo'] = 'Unknown'
    
    return info


def run_performance_test(host: str, users: int, run_time: str, hatch_rate: int, 
                        report_file: str) -> subprocess.CompletedProcess:
    """Run the performance metrics load test."""
    print("🚀 Running performance metrics load test...")
    print(f"   Host: {host}")
    print(f"   Users: {users}")
    print(f"   Run Time: {run_time}")
    print(f"   Hatch Rate: {hatch_rate}")
    print()
    
    cmd = [
        'cargo', 'run', '--release', '--example', 'performance_metrics_load_test',
        '--',
        '--host', host,
        '--users', str(users),
        '--run-time', run_time,
        '--hatch-rate', str(hatch_rate),
        '--report-file', report_file,
    ]
    
    return subprocess.run(cmd, capture_output=False, text=True)


def find_latest_metrics_file() -> Optional[Path]:
    """Find the most recently created metrics JSON file."""
    metrics_files = list(Path('.').glob('metrics-*.json'))
    if not metrics_files:
        return None
    return max(metrics_files, key=lambda p: p.stat().st_mtime)


def load_metrics(filepath: Path) -> Dict[str, Any]:
    """Load metrics from JSON file."""
    with open(filepath, 'r') as f:
        return json.load(f)


def generate_system_info_markdown(system_info: Dict[str, str], config: Dict[str, Any]) -> str:
    """Generate system information markdown."""
    md = ["# System Information", ""]
    md.append("## Hardware")
    md.append("```")
    md.append(f"CPU: {system_info.get('processor', 'Unknown')}")
    md.append(f"CPU Cores: {system_info.get('cpu_cores', 'Unknown')}")
    md.append(f"Machine: {system_info.get('machine', 'Unknown')}")
    md.append("```")
    md.append("")
    
    md.append("## Software")
    md.append("```")
    md.append(f"OS: {system_info.get('os', 'Unknown')}")
    md.append(f"Python: {system_info.get('python', 'Unknown')}")
    md.append(f"Rust: {system_info.get('rust', 'Unknown')}")
    md.append(f"Cargo: {system_info.get('cargo', 'Unknown')}")
    md.append("```")
    md.append("")
    
    md.append("## Test Configuration")
    md.append("```")
    md.append(f"Host: {config['host']}")
    md.append(f"Concurrent Users: {config['users']}")
    md.append(f"Run Time: {config['run_time']}")
    md.append(f"Hatch Rate: {config['hatch_rate']} users/sec")
    md.append(f"Timestamp: {config['timestamp']}")
    md.append("```")
    
    return "\n".join(md)


def generate_report_markdown(metrics: Dict[str, Any], config: Dict[str, Any], 
                            report_dir: Path, baseline_path: Optional[Path] = None) -> str:
    """Generate the main benchmark report markdown."""
    md = ["# BRRTRouter Performance Benchmark Report", ""]
    md.append(f"**Generated**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    md.append("")
    
    # Executive Summary
    md.append("## Executive Summary")
    md.append("")
    
    avg_route = metrics.get('avg_route_match_latency_us', 0)
    p99_route = metrics.get('p99_route_match_latency_us', 0)
    error_rate = metrics.get('match_error_rate', 0)
    total_req = metrics.get('total_requests', 0)
    
    md.append(f"- **Total Requests**: {total_req:,}")
    md.append(f"- **Average Route Match Latency**: {avg_route:.2f}µs")
    md.append(f"- **P99 Route Match Latency**: {p99_route}µs")
    md.append(f"- **Error Rate**: {error_rate:.2f}%")
    md.append("")
    
    # Performance assessment
    if p99_route < 100:
        md.append("✅ **Performance Rating**: Excellent (P99 < 100µs)")
    elif p99_route < 1000:
        md.append("⚠️ **Performance Rating**: Good (P99 < 1ms)")
    else:
        md.append("❌ **Performance Rating**: Needs Improvement (P99 > 1ms)")
    
    md.append("")
    
    # Detailed Metrics
    md.append("## Detailed Metrics")
    md.append("")
    
    md.append("### Route Matching Performance")
    md.append("")
    md.append("| Metric | Value |")
    md.append("|--------|-------|")
    md.append(f"| Average | {metrics.get('avg_route_match_latency_us', 0):.2f}µs |")
    md.append(f"| P50 (Median) | {metrics.get('p50_route_match_latency_us', 0)}µs |")
    md.append(f"| P95 | {metrics.get('p95_route_match_latency_us', 0)}µs |")
    md.append(f"| P99 | {metrics.get('p99_route_match_latency_us', 0)}µs |")
    md.append(f"| Max | {metrics.get('max_route_match_latency_us', 0)}µs |")
    md.append("")
    
    md.append("### Handler Dispatch Performance")
    md.append("")
    md.append("| Metric | Value |")
    md.append("|--------|-------|")
    md.append(f"| Average | {metrics.get('avg_dispatch_latency_us', 0):.2f}µs |")
    md.append(f"| P95 | {metrics.get('p95_dispatch_latency_us', 0)}µs |")
    md.append(f"| P99 | {metrics.get('p99_dispatch_latency_us', 0)}µs |")
    md.append("")
    
    md.append("### Lock Contention")
    md.append("")
    md.append("| Metric | Value |")
    md.append("|--------|-------|")
    md.append(f"| Average Lock Time | {metrics.get('avg_lock_acquisition_us', 0):.2f}µs |")
    md.append(f"| P99 Lock Time | {metrics.get('p99_lock_acquisition_us', 0)}µs |")
    md.append(f"| Contentions | {metrics.get('lock_contentions', 0)} |")
    md.append("")
    
    md.append("### Memory & GC")
    md.append("")
    avg_mem_mb = metrics.get('avg_memory_bytes', 0) / 1_048_576
    max_mem_mb = metrics.get('max_memory_bytes', 0) / 1_048_576
    md.append("| Metric | Value |")
    md.append("|--------|-------|")
    md.append(f"| Average Memory | {avg_mem_mb:.2f}MB |")
    md.append(f"| Max Memory | {max_mem_mb:.2f}MB |")
    md.append(f"| GC Delays | {metrics.get('gc_delays_detected', 0)} |")
    md.append("")
    
    # Files section
    md.append("## Files")
    md.append("")
    md.append(f"- [Full Metrics (JSON)](metrics.json)")
    md.append(f"- [Goose HTML Report](goose-report.html)")
    md.append(f"- [System Information](system-info.md)")
    md.append("")
    
    # Comparison with baseline
    md.append("## Comparison with Baseline")
    md.append("")
    
    if baseline_path and baseline_path.exists():
        md.append("Comparing with baseline...")
        md.append("")
        md.append("```")
        # Run comparison script
        try:
            result = subprocess.run(
                ['python3', 'scripts/compare_metrics.py', str(baseline_path), 
                 str(report_dir / 'metrics.json')],
                capture_output=True, text=True
            )
            md.append(result.stdout)
        except Exception as e:
            md.append(f"Error running comparison: {e}")
        md.append("```")
    else:
        md.append("_No baseline metrics found. Save current metrics as baseline:_")
        md.append("```bash")
        md.append("cp " + str(report_dir / 'metrics.json') + " baseline-metrics.json")
        md.append("```")
    
    return "\n".join(md)


def main():
    parser = argparse.ArgumentParser(
        description='Generate comprehensive benchmark report for BRRTRouter',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    
    parser.add_argument('--output-dir', default='benchmark-reports',
                       help='Output directory for reports (default: benchmark-reports)')
    parser.add_argument('--host', default=os.environ.get('GOOSE_HOST', 'http://localhost:8080'),
                       help='Host to test (default: http://localhost:8080)')
    parser.add_argument('--users', type=int, default=int(os.environ.get('USERS', '100')),
                       help='Number of concurrent users (default: 100)')
    parser.add_argument('--run-time', default=os.environ.get('RUN_TIME', '5m'),
                       help='Test duration (default: 5m)')
    parser.add_argument('--hatch-rate', type=int, default=int(os.environ.get('HATCH_RATE', '10')),
                       help='Users to spawn per second (default: 10)')
    parser.add_argument('--baseline', type=Path, default=Path('baseline-metrics.json'),
                       help='Baseline metrics file for comparison (default: baseline-metrics.json)')
    
    args = parser.parse_args()
    
    # Setup
    timestamp = datetime.now().strftime('%Y%m%d-%H%M%S')
    report_dir = Path(args.output_dir) / timestamp
    report_dir.mkdir(parents=True, exist_ok=True)
    
    print("╔════════════════════════════════════════════════════════════════╗")
    print("║        BRRTRouter Benchmark Report Generator                  ║")
    print("╚════════════════════════════════════════════════════════════════╝")
    print()
    
    config = {
        'host': args.host,
        'users': args.users,
        'run_time': args.run_time,
        'hatch_rate': args.hatch_rate,
        'timestamp': timestamp,
    }
    
    # Collect system information
    print("📋 Collecting system information...")
    system_info = get_system_info()
    
    # Save system info
    system_info_md = generate_system_info_markdown(system_info, config)
    with open(report_dir / 'system-info.md', 'w') as f:
        f.write(system_info_md)
    
    # Run performance test
    goose_report = str(report_dir / 'goose-report.html')
    result = run_performance_test(
        args.host, args.users, args.run_time, args.hatch_rate, goose_report
    )
    
    if result.returncode != 0:
        print("❌ Error: Performance test failed")
        sys.exit(1)
    
    # Find and move metrics file
    print()
    print("📊 Collecting metrics...")
    metrics_file = find_latest_metrics_file()
    
    if not metrics_file:
        print("❌ Error: No metrics file found")
        sys.exit(1)
    
    # Move metrics to report directory
    metrics_dest = report_dir / 'metrics.json'
    metrics_file.rename(metrics_dest)
    
    # Load metrics
    metrics = load_metrics(metrics_dest)
    
    # Generate main report
    print("📝 Generating report...")
    report_md = generate_report_markdown(metrics, config, report_dir, args.baseline)
    with open(report_dir / 'README.md', 'w') as f:
        f.write(report_md)
    
    # Success message
    print()
    print("✅ Benchmark report generated successfully!")
    print()
    print(f"📁 Report Location: {report_dir}")
    print()
    print("📄 View report:")
    print(f"   cat {report_dir / 'README.md'}")
    print()
    print("🌐 View HTML report:")
    print(f"   open {report_dir / 'goose-report.html'}")
    print()
    
    # Offer to set as baseline
    if not args.baseline.exists():
        print("💡 Tip: Set this as your baseline for future comparisons:")
        print(f"   cp {metrics_dest} baseline-metrics.json")
        print()


if __name__ == '__main__':
    main()
