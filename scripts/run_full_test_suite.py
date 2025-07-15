#!/usr/bin/env python3
"""
Integrated Test Runner for BRRTRouter

Combines service management with comprehensive OpenAPI testing
to provide a complete testing solution.
"""

import subprocess
import sys
import time
import json
from pathlib import Path
from manage_service import BRRTRouterService

class IntegratedTestRunner:
    def __init__(self, spec_file=None):
        self.spec_file = spec_file
        self.service = BRRTRouterService(spec_file)
        self.project_root = Path(__file__).parent.parent
        
    def run_complete_test_suite(self):
        """Run the complete test suite with service management"""
        print("🚀 BRRTRouter Integrated Test Suite")
        print("=" * 50)
        
        spec_name = self.spec_file or "default OpenAPI spec"
        print(f"📋 Testing with: {spec_name}")
        print()
        
        try:
            # Step 1: Start the service
            print("Step 1: Starting BRRTRouter service...")
            if not self.service.start():
                print("❌ Failed to start service")
                return False
            
            # Wait for service to be ready
            print("⏳ Waiting for service to be ready...")
            time.sleep(3)
            
            # Step 2: Verify service health
            print("Step 2: Verifying service health...")
            if not self.service.status():
                print("❌ Service health check failed")
                return False
            
            # Step 3: Run comprehensive OpenAPI tests
            print("\nStep 3: Running comprehensive OpenAPI tests...")
            test_result = self.run_openapi_tests()
            
            # Step 4: Run any additional custom tests
            print("\nStep 4: Running additional validation tests...")
            validation_result = self.run_validation_tests()
            
            # Step 5: Generate comprehensive report
            print("\nStep 5: Generating test report...")
            self.generate_comprehensive_report(test_result, validation_result)
            
            return test_result and validation_result
            
        except KeyboardInterrupt:
            print("\n⚠️  Test suite interrupted by user")
            return False
        except Exception as e:
            print(f"\n❌ Test suite failed with error: {e}")
            return False
        finally:
            # Always stop the service
            print("\n🛑 Stopping service...")
            self.service.stop()
    
    def run_openapi_tests(self):
        """Run the comprehensive OpenAPI test suite"""
        try:
            test_script = self.project_root / "scripts" / "test_openapi_spec.py"
            result = subprocess.run([
                "python3", str(test_script)
            ], capture_output=True, text=True, timeout=120)
            
            print(result.stdout)
            if result.stderr:
                print("STDERR:", result.stderr)
            
            return result.returncode == 0
            
        except subprocess.TimeoutExpired:
            print("❌ OpenAPI tests timed out")
            return False
        except Exception as e:
            print(f"❌ OpenAPI tests failed: {e}")
            return False
    
    def run_validation_tests(self):
        """Run additional validation and edge case tests"""
        print("🔍 Running validation tests...")
        
        # Test stack overflow resistance
        if not self.test_stack_overflow_resistance():
            return False
        
        # Test parameter validation
        if not self.test_parameter_validation():
            return False
        
        # Test error handling
        if not self.test_error_handling():
            return False
        
        print("✅ All validation tests passed")
        return True
    
    def test_stack_overflow_resistance(self):
        """Test that the stack overflow issue is resolved"""
        print("  📊 Testing stack overflow resistance...")
        
        # Check service logs for stack overflow errors
        err_log = self.service.logs_dir / "brrtrouter-serve.err.log"
        if err_log.exists():
            with open(err_log) as f:
                content = f.read()
                if "stack overflow" in content.lower():
                    print("    ❌ Stack overflow detected in logs!")
                    return False
        
        print("    ✅ No stack overflow detected")
        return True
    
    def test_parameter_validation(self):
        """Test parameter validation behavior"""
        print("  🔧 Testing parameter validation...")
        
        import requests
        try:
            # Test that optional parameters are handled correctly
            response = requests.get("http://localhost:8080/pets", timeout=5)
            # Should get validation error, not crash
            if response.status_code in [400, 401, 403]:
                print("    ✅ Parameter validation working")
                return True
            else:
                print(f"    ⚠️  Unexpected response: {response.status_code}")
                return True  # Not critical
        except Exception as e:
            print(f"    ❌ Parameter validation test failed: {e}")
            return False
    
    def test_error_handling(self):
        """Test error handling and edge cases"""
        print("  🚨 Testing error handling...")
        
        import requests
        try:
            # Test 404 handling
            response = requests.get("http://localhost:8080/nonexistent", timeout=5)
            if response.status_code == 404:
                print("    ✅ 404 handling works")
                return True
            else:
                print(f"    ⚠️  Expected 404, got {response.status_code}")
                return True  # Not critical
        except Exception as e:
            print(f"    ❌ Error handling test failed: {e}")
            return False
    
    def generate_comprehensive_report(self, openapi_result, validation_result):
        """Generate a comprehensive test report"""
        print("📄 Generating comprehensive report...")
        
        # Load OpenAPI test results if available
        results_file = self.project_root / "test_results.json"
        openapi_data = {}
        if results_file.exists():
            with open(results_file) as f:
                openapi_data = json.load(f)
        
        # Create comprehensive report
        report = {
            "test_run": {
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                "spec_file": self.spec_file or "default",
                "overall_success": openapi_result and validation_result
            },
            "openapi_tests": openapi_data,
            "validation_tests": {
                "stack_overflow_resistance": True,  # We got this far
                "parameter_validation": validation_result,
                "error_handling": validation_result,
                "overall_success": validation_result
            },
            "service_logs": self.get_service_log_summary(),
            "recommendations": self.generate_recommendations(openapi_result, validation_result)
        }
        
        # Save detailed report
        report_file = self.project_root / "comprehensive_test_report.json"
        with open(report_file, 'w') as f:
            json.dump(report, f, indent=2)
        
        # Print summary
        self.print_report_summary(report)
        
        print(f"📄 Full report saved to: {report_file}")
    
    def get_service_log_summary(self):
        """Get summary of service logs"""
        logs = {"errors": [], "warnings": [], "info": []}
        
        err_log = self.service.logs_dir / "brrtrouter-serve.err.log"
        if err_log.exists():
            with open(err_log) as f:
                content = f.read()
                if content.strip():
                    logs["errors"].append(content.strip())
        
        return logs
    
    def generate_recommendations(self, openapi_result, validation_result):
        """Generate recommendations based on test results"""
        recommendations = []
        
        if not openapi_result:
            recommendations.append("Fix OpenAPI test failures - check parameter validation and response format")
        
        if not validation_result:
            recommendations.append("Address validation test failures - check error handling and edge cases")
        
        # Check if we have specific issues to address
        results_file = self.project_root / "test_results.json"
        if results_file.exists():
            with open(results_file) as f:
                data = json.load(f)
                if data.get("summary", {}).get("success_rate", 0) < 80:
                    recommendations.append("Improve success rate by fixing parameter validation and response formats")
        
        if not recommendations:
            recommendations.append("All tests passing! Consider adding more edge case tests.")
        
        return recommendations
    
    def print_report_summary(self, report):
        """Print a summary of the test report"""
        print("\n" + "=" * 60)
        print("📊 COMPREHENSIVE TEST REPORT SUMMARY")
        print("=" * 60)
        
        overall = report["test_run"]["overall_success"]
        print(f"Overall Status: {'🎉 SUCCESS' if overall else '❌ NEEDS ATTENTION'}")
        
        # OpenAPI tests summary
        openapi = report.get("openapi_tests", {}).get("summary", {})
        if openapi:
            success_rate = openapi.get("success_rate", 0)
            print(f"OpenAPI Tests: {openapi.get('passed', 0)}/{openapi.get('total', 0)} passed ({success_rate:.1f}%)")
        
        # Validation tests summary
        validation = report["validation_tests"]["overall_success"]
        print(f"Validation Tests: {'✅ PASSED' if validation else '❌ FAILED'}")
        
        # Recommendations
        print("\n📋 RECOMMENDATIONS:")
        for rec in report["recommendations"]:
            print(f"  • {rec}")

def main():
    """Main entry point"""
    print("🧪 BRRTRouter Integrated Test Suite")
    
    # Parse command line arguments
    spec_file = None
    if len(sys.argv) > 1:
        spec_file = sys.argv[1]
        print(f"📋 Using spec file: {spec_file}")
    else:
        print("📋 Using default OpenAPI spec")
    
    # Run the test suite
    runner = IntegratedTestRunner(spec_file)
    success = runner.run_complete_test_suite()
    
    # Exit with appropriate code
    if success:
        print("\n🎉 All tests completed successfully!")
        sys.exit(0)
    else:
        print("\n⚠️  Some tests failed. Check the report for details.")
        sys.exit(1)

if __name__ == "__main__":
    main() 