#!/usr/bin/env python3
"""
Detailed Test Stability Analysis Script
Runs cargo test 25 times and tracks individual test results to identify flakey tests
"""

import subprocess
import time
import json
import re
from datetime import datetime
from collections import defaultdict

def parse_test_results(output):
    """Parse cargo test output to extract individual test results"""
    tests = []
    current_suite = None
    
    lines = output.split('\n')
    for line in lines:
        line = line.strip()
        
        # Match test suite start
        if line.startswith('running ') and 'tests' in line:
            match = re.match(r'running (\d+) tests', line)
            if match:
                current_suite = f"suite_{len(tests)}"
        
        # Match individual test results
        test_match = re.match(r'test (.+?) \.\.\. (ok|FAILED|ignored)', line)
        if test_match:
            test_name = test_match.group(1)
            result = test_match.group(2)
            tests.append({
                'name': test_name,
                'result': result,
                'suite': current_suite
            })
    
    return tests

def run_test_iteration(iteration):
    """Run a single test iteration and return detailed results"""
    print(f"Running test iteration {iteration}/25...")
    
    start_time = time.time()
    
    try:
        result = subprocess.run(
            ['cargo', 'test', '--', '--nocapture'],
            capture_output=True,
            text=True,
            timeout=300  # 5 minute timeout
        )
        
        end_time = time.time()
        duration = end_time - start_time
        
        # Parse individual test results
        individual_tests = parse_test_results(result.stdout)
        
        # Count results
        passed = sum(1 for t in individual_tests if t['result'] == 'ok')
        failed = sum(1 for t in individual_tests if t['result'] == 'FAILED')
        ignored = sum(1 for t in individual_tests if t['result'] == 'ignored')
        
        # Extract failed test names
        failed_tests = [t['name'] for t in individual_tests if t['result'] == 'FAILED']
        
        return {
            'iteration': iteration,
            'success': result.returncode == 0,
            'duration': round(duration, 2),
            'passed': passed,
            'failed': failed,
            'ignored': ignored,
            'exit_code': result.returncode,
            'failed_tests': failed_tests,
            'individual_tests': individual_tests,
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
            'failed_tests': [],
            'individual_tests': [],
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
            'failed_tests': [],
            'individual_tests': [],
            'error': str(e),
            'timestamp': datetime.now().isoformat()
        }

def analyze_flakey_tests(results):
    """Analyze results to identify flakey tests"""
    test_results = defaultdict(list)
    
    # Collect all results for each test
    for result in results:
        for test in result['individual_tests']:
            test_results[test['name']].append({
                'iteration': result['iteration'],
                'result': test['result'],
                'success': test['result'] == 'ok'
            })
    
    # Identify flakey tests (tests that sometimes pass and sometimes fail)
    flakey_tests = {}
    stable_failing_tests = {}
    stable_passing_tests = {}
    
    for test_name, test_runs in test_results.items():
        if not test_runs:  # Skip if no results
            continue
            
        success_count = sum(1 for run in test_runs if run['success'])
        failure_count = len(test_runs) - success_count
        success_rate = success_count / len(test_runs) * 100
        
        if success_count > 0 and failure_count > 0:
            # Flakey test - sometimes passes, sometimes fails
            flakey_tests[test_name] = {
                'total_runs': len(test_runs),
                'successes': success_count,
                'failures': failure_count,
                'success_rate': success_rate,
                'failed_iterations': [run['iteration'] for run in test_runs if not run['success']]
            }
        elif failure_count == len(test_runs):
            # Consistently failing test
            stable_failing_tests[test_name] = {
                'total_runs': len(test_runs),
                'success_rate': 0.0
            }
        else:
            # Consistently passing test
            stable_passing_tests[test_name] = {
                'total_runs': len(test_runs),
                'success_rate': 100.0
            }
    
    return flakey_tests, stable_failing_tests, stable_passing_tests

def main():
    """Run 25 test iterations and generate detailed analysis"""
    print("🧪 BRRTRouter Detailed Test Stability Analysis")
    print("=" * 60)
    print("Running 25 test iterations to identify flakey tests...")
    print()
    
    results = []
    start_time = time.time()
    
    for i in range(1, 26):
        result = run_test_iteration(i)
        results.append(result)
        
        # Print progress
        status = "✅ PASS" if result['success'] else "❌ FAIL"
        duration = result['duration']
        failed_count = len(result['failed_tests'])
        
        print(f"  {i:2d}/25: {status} - {duration:6.2f}s - P:{result['passed']} F:{result['failed']} I:{result['ignored']}")
        
        if failed_count > 0:
            print(f"        Failed tests: {', '.join(result['failed_tests'])}")
    
    total_time = time.time() - start_time
    
    # Analyze for flakey tests
    flakey_tests, stable_failing, stable_passing = analyze_flakey_tests(results)
    
    # Generate summary statistics
    successful_runs = sum(1 for r in results if r['success'])
    failed_runs = 25 - successful_runs
    avg_duration = sum(r['duration'] for r in results) / len(results)
    min_duration = min(r['duration'] for r in results)
    max_duration = max(r['duration'] for r in results)
    
    print()
    print("📊 RESULTS SUMMARY")
    print("=" * 60)
    print(f"Total Runs:        25")
    print(f"Successful Runs:   {successful_runs} ({successful_runs/25*100:.1f}%)")
    print(f"Failed Runs:       {failed_runs} ({failed_runs/25*100:.1f}%)")
    print(f"Total Time:        {total_time:.1f}s")
    print(f"Average Duration:  {avg_duration:.2f}s")
    print(f"Min Duration:      {min_duration:.2f}s")
    print(f"Max Duration:      {max_duration:.2f}s")
    
    # Detailed results table
    print()
    print("📋 DETAILED RESULTS TABLE")
    print("=" * 90)
    print("| Run | Status | Duration | Passed | Failed | Ignored | Failed Tests")
    print("|-----|--------|----------|--------|--------|---------|-------------")
    
    for result in results:
        status = "PASS" if result['success'] else "FAIL"
        duration = f"{result['duration']:.2f}s"
        failed_tests_str = ', '.join(result['failed_tests'][:3])  # Show first 3
        if len(result['failed_tests']) > 3:
            failed_tests_str += f" (+{len(result['failed_tests'])-3} more)"
        
        print(f"| {result['iteration']:2d}  | {status:6s} | {duration:8s} | {result['passed']:6d} | {result['failed']:6d} | {result['ignored']:7d} | {failed_tests_str}")
    
    # Flakey tests analysis
    print()
    print("🔍 FLAKEY TESTS ANALYSIS")
    print("=" * 60)
    
    if flakey_tests:
        print("❌ FLAKEY TESTS DETECTED:")
        print("   These tests sometimes pass and sometimes fail:")
        print()
        for test_name, stats in sorted(flakey_tests.items(), key=lambda x: x[1]['success_rate']):
            print(f"   • {test_name}")
            print(f"     Success Rate: {stats['success_rate']:.1f}% ({stats['successes']}/{stats['total_runs']})")
            print(f"     Failed in iterations: {stats['failed_iterations']}")
            print()
    else:
        print("✅ NO FLAKEY TESTS DETECTED!")
        print("   All tests are consistently passing or failing.")
    
    # Consistently failing tests
    if stable_failing:
        print()
        print("⚠️  CONSISTENTLY FAILING TESTS:")
        for test_name in sorted(stable_failing.keys()):
            print(f"   • {test_name} (failed in all {stable_failing[test_name]['total_runs']} runs)")
    
    # Test stability summary
    print()
    print("📈 TEST STABILITY SUMMARY")
    print("=" * 60)
    print(f"Total Unique Tests: {len(stable_passing) + len(stable_failing) + len(flakey_tests)}")
    print(f"Stable Passing:     {len(stable_passing)}")
    print(f"Stable Failing:     {len(stable_failing)}")
    print(f"Flakey Tests:       {len(flakey_tests)}")
    
    # Save detailed results
    detailed_results = {
        'summary': {
            'total_runs': 25,
            'successful_runs': successful_runs,
            'failed_runs': failed_runs,
            'success_rate': successful_runs/25*100,
            'total_time': total_time,
            'avg_duration': avg_duration,
            'min_duration': min_duration,
            'max_duration': max_duration
        },
        'flakey_tests': flakey_tests,
        'stable_failing_tests': stable_failing,
        'stable_passing_tests': stable_passing,
        'detailed_results': results
    }
    
    with open('detailed_test_stability_results.json', 'w') as f:
        json.dump(detailed_results, f, indent=2)
    
    print()
    print("💾 Detailed results saved to: detailed_test_stability_results.json")
    
    # Final assessment
    print()
    print("🎯 STABILITY ASSESSMENT")
    print("=" * 60)
    if len(flakey_tests) == 0 and successful_runs == 25:
        print("✅ EXCELLENT: All tests are stable and passing!")
        print("   No flakey tests detected.")
    elif len(flakey_tests) == 0 and successful_runs >= 20:
        print("✅ GOOD: No flakey tests, but some consistent failures.")
        print("   Test stability is good, focus on fixing failing tests.")
    elif len(flakey_tests) <= 2:
        print("⚠️  MODERATE: Few flakey tests detected.")
        print("   Address the identified flakey tests above.")
    else:
        print("❌ POOR: Multiple flakey tests detected.")
        print("   Significant test instability requires investigation.")

if __name__ == "__main__":
    main() 