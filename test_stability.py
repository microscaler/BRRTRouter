#!/usr/bin/env python3
"""
Test Stability Analysis Script
Runs cargo test 25 times and collects results to verify flakey test fixes
"""

import subprocess
import time
import json
from datetime import datetime

def run_test_iteration(iteration):
    """Run a single test iteration and return results"""
    print(f"Running test iteration {iteration}/25...")
    
    start_time = time.time()
    
    try:
        result = subprocess.run(
            ['cargo', 'test', '--quiet'],
            capture_output=True,
            text=True,
            timeout=300  # 5 minute timeout
        )
        
        end_time = time.time()
        duration = end_time - start_time
        
        # Parse the output to extract test counts
        output_lines = result.stdout.split('\n')
        stderr_lines = result.stderr.split('\n')
        
        # Look for test result summary
        passed = 0
        failed = 0
        ignored = 0
        
        for line in output_lines + stderr_lines:
            if 'test result:' in line and 'ok.' in line:
                # Parse line like: "test result: ok. 24 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.11s"
                parts = line.split(';')
                for part in parts:
                    part = part.strip()
                    if 'passed' in part:
                        passed += int(part.split()[0])
                    elif 'failed' in part:
                        failed += int(part.split()[0])
                    elif 'ignored' in part:
                        ignored += int(part.split()[0])
        
        return {
            'iteration': iteration,
            'success': result.returncode == 0,
            'duration': round(duration, 2),
            'passed': passed,
            'failed': failed,
            'ignored': ignored,
            'exit_code': result.returncode,
            'stdout_lines': len(output_lines),
            'stderr_lines': len(stderr_lines),
            'timestamp': datetime.now().isoformat()
        }
        
    except subprocess.TimeoutExpired:
        return {
            'iteration': iteration,
            'success': False,
            'duration': 300.0,
            'passed': 0,
            'failed': 0,
            'ignored': 0,
            'exit_code': -1,
            'stdout_lines': 0,
            'stderr_lines': 0,
            'error': 'TIMEOUT',
            'timestamp': datetime.now().isoformat()
        }
    except Exception as e:
        return {
            'iteration': iteration,
            'success': False,
            'duration': 0.0,
            'passed': 0,
            'failed': 0,
            'ignored': 0,
            'exit_code': -2,
            'stdout_lines': 0,
            'stderr_lines': 0,
            'error': str(e),
            'timestamp': datetime.now().isoformat()
        }

def main():
    """Run 25 test iterations and generate results table"""
    print("🧪 BRRTRouter Test Stability Analysis")
    print("=" * 50)
    print("Running 25 test iterations to verify flakey test fixes...")
    print()
    
    results = []
    start_time = time.time()
    
    for i in range(1, 26):
        result = run_test_iteration(i)
        results.append(result)
        
        # Print progress
        status = "✅ PASS" if result['success'] else "❌ FAIL"
        duration = result['duration']
        print(f"  {i:2d}/25: {status} - {duration:6.2f}s - P:{result['passed']} F:{result['failed']} I:{result['ignored']}")
    
    total_time = time.time() - start_time
    
    # Generate summary statistics
    successful_runs = sum(1 for r in results if r['success'])
    failed_runs = 25 - successful_runs
    avg_duration = sum(r['duration'] for r in results) / len(results)
    min_duration = min(r['duration'] for r in results)
    max_duration = max(r['duration'] for r in results)
    
    total_passed = sum(r['passed'] for r in results)
    total_failed = sum(r['failed'] for r in results)
    total_ignored = sum(r['ignored'] for r in results)
    
    print()
    print("📊 RESULTS SUMMARY")
    print("=" * 50)
    print(f"Total Runs:        25")
    print(f"Successful Runs:   {successful_runs} ({successful_runs/25*100:.1f}%)")
    print(f"Failed Runs:       {failed_runs} ({failed_runs/25*100:.1f}%)")
    print(f"Total Time:        {total_time:.1f}s")
    print(f"Average Duration:  {avg_duration:.2f}s")
    print(f"Min Duration:      {min_duration:.2f}s")
    print(f"Max Duration:      {max_duration:.2f}s")
    print()
    print(f"Total Tests Passed: {total_passed}")
    print(f"Total Tests Failed: {total_failed}")
    print(f"Total Tests Ignored: {total_ignored}")
    
    # Generate detailed results table
    print()
    print("📋 DETAILED RESULTS TABLE")
    print("=" * 80)
    print("| Run | Status | Duration | Passed | Failed | Ignored | Exit Code |")
    print("|-----|--------|----------|--------|--------|---------|-----------|")
    
    for result in results:
        status = "PASS" if result['success'] else "FAIL"
        duration = f"{result['duration']:.2f}s"
        exit_code = result['exit_code']
        
        print(f"| {result['iteration']:2d}  | {status:6s} | {duration:8s} | {result['passed']:6d} | {result['failed']:6d} | {result['ignored']:7d} | {exit_code:9d} |")
    
    # Check for any failures and report details
    if failed_runs > 0:
        print()
        print("❌ FAILURE DETAILS")
        print("=" * 50)
        for result in results:
            if not result['success']:
                print(f"Run {result['iteration']}: Exit Code {result['exit_code']}")
                if 'error' in result:
                    print(f"  Error: {result['error']}")
    
    # Save results to JSON file
    with open('test_stability_results.json', 'w') as f:
        json.dump({
            'summary': {
                'total_runs': 25,
                'successful_runs': successful_runs,
                'failed_runs': failed_runs,
                'success_rate': successful_runs/25*100,
                'total_time': total_time,
                'avg_duration': avg_duration,
                'min_duration': min_duration,
                'max_duration': max_duration,
                'total_passed': total_passed,
                'total_failed': total_failed,
                'total_ignored': total_ignored
            },
            'results': results
        }, f, indent=2)
    
    print()
    print("💾 Results saved to: test_stability_results.json")
    
    # Final assessment
    print()
    print("🎯 STABILITY ASSESSMENT")
    print("=" * 50)
    if successful_runs == 25:
        print("✅ EXCELLENT: All 25 test runs passed successfully!")
        print("   Flakey test fixes are working perfectly.")
    elif successful_runs >= 23:
        print("✅ GOOD: High success rate with minimal failures.")
        print("   Test stability significantly improved.")
    elif successful_runs >= 20:
        print("⚠️  MODERATE: Some instability detected.")
        print("   Further investigation may be needed.")
    else:
        print("❌ POOR: Significant test instability detected.")
        print("   Flakey tests may still be present.")

if __name__ == "__main__":
    main() 