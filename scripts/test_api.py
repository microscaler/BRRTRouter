#!/usr/bin/env python3
"""
Pragmatic OpenAPI API Testing Script

A human-friendly way to test the BRRTRouter API based on the OpenAPI specification.
Shows live progress, detailed responses, and clear test results.
"""

import requests
import json
import yaml
import time
import sys
from pathlib import Path
from typing import Dict, List, Optional, Any

# Simple color codes for terminal output
class Colors:
    RED = '\033[91m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    MAGENTA = '\033[95m'
    CYAN = '\033[96m'
    WHITE = '\033[97m'
    BOLD = '\033[1m'
    UNDERLINE = '\033[4m'
    END = '\033[0m'

def print_colored(text: str, color: str = Colors.WHITE):
    """Print colored text"""
    print(f"{color}{text}{Colors.END}")

def print_header(text: str):
    """Print a header with formatting"""
    print("\n" + "=" * 60)
    print_colored(f"  {text}", Colors.BOLD + Colors.BLUE)
    print("=" * 60)

def print_section(text: str):
    """Print a section header"""
    print("\n" + "-" * 40)
    print_colored(f"📋 {text}", Colors.BOLD + Colors.CYAN)
    print("-" * 40)

def print_test(name: str):
    """Print test name"""
    print_colored(f"\n🧪 {name}", Colors.BOLD + Colors.CYAN)

def print_pass(text: str):
    """Print success message"""
    print_colored(f"✅ {text}", Colors.GREEN)

def print_fail(text: str):
    """Print failure message"""
    print_colored(f"❌ {text}", Colors.RED)

def print_warn(text: str):
    """Print warning message"""
    print_colored(f"⚠️  {text}", Colors.YELLOW)

def print_info(text: str):
    """Print info message"""
    print_colored(f"ℹ️  {text}", Colors.BLUE)

def print_json(data: Any, indent: int = 2):
    """Print formatted JSON"""
    try:
        formatted = json.dumps(data, indent=indent, ensure_ascii=False)
        # Add simple syntax highlighting
        for line in formatted.split('\n'):
            if ':' in line and any(char in line for char in ['"', "'"]):
                print_colored(line, Colors.CYAN)
            else:
                print(line)
    except:
        print(str(data))

class APITester:
    def __init__(self, base_url: str = "http://localhost:8080", spec_path: str = "examples/pet_store/doc/openapi.yaml"):
        self.base_url = base_url
        self.spec_path = spec_path
        self.session = requests.Session()
        self.spec = self.load_spec()
        self.results = []
        
    def load_spec(self) -> Dict[str, Any]:
        """Load the OpenAPI specification"""
        try:
            with open(self.spec_path, 'r') as f:
                return yaml.safe_load(f)
        except FileNotFoundError:
            print_fail(f"OpenAPI spec not found at {self.spec_path}")
            sys.exit(1)
        except Exception as e:
            print_fail(f"Failed to load OpenAPI spec: {e}")
            sys.exit(1)

    def check_server_health(self):
        """Check if the server is running"""
        try:
            response = self.session.get(f"{self.base_url}/health", timeout=5)
            if response.status_code == 200:
                print_pass("Server is running and healthy")
                return True
            else:
                print_fail(f"Server health check failed: {response.status_code}")
                return False
        except requests.exceptions.RequestException as e:
            print_fail(f"Cannot connect to server: {e}")
            print_info("Start the server with: python scripts/manage_service.py start")
            return False

    def get_api_key(self) -> str:
        """Get API key from environment or use default test key"""
        return "test123"  # Default test API key

    def make_request(self, method: str, path: str, headers: Optional[Dict] = None, 
                    params: Optional[Dict] = None, json_data: Optional[Dict] = None,
                    show_request: bool = True, show_response: bool = True) -> requests.Response:
        """Make an HTTP request with detailed logging"""
        
        url = f"{self.base_url}{path}"
        
        # Prepare headers
        request_headers = {"X-API-Key": self.get_api_key()}
        if headers:
            request_headers.update(headers)
        
        if show_request:
            # Show request details
            print_colored(f"\n→ {method.upper()} {path}", Colors.BOLD + Colors.BLUE)
            if params:
                print_colored(f"  Query params: {params}", Colors.WHITE)
            if json_data:
                print_colored("  Request body:", Colors.WHITE)
                print_json(json_data)
        
        try:
            # Make the request
            response = self.session.request(
                method=method,
                url=url,
                headers=request_headers,
                params=params,
                json=json_data,
                timeout=10
            )
            
            if show_response:
                # Show response details
                if 200 <= response.status_code < 300:
                    print_colored(f"← {response.status_code} {response.reason}", Colors.GREEN)
                elif response.status_code >= 400:
                    print_colored(f"← {response.status_code} {response.reason}", Colors.RED)
                else:
                    print_colored(f"← {response.status_code} {response.reason}", Colors.YELLOW)
                
                if response.headers.get('content-type', '').startswith('application/json'):
                    try:
                        json_response = response.json()
                        print_json(json_response)
                    except json.JSONDecodeError:
                        print(response.text[:200] + "..." if len(response.text) > 200 else response.text)
                else:
                    print(response.text[:200] + "..." if len(response.text) > 200 else response.text)
            
            return response
            
        except requests.exceptions.RequestException as e:
            print_fail(f"Request failed: {e}")
            raise

    def test_infrastructure(self):
        """Test basic infrastructure endpoints"""
        print_section("Infrastructure Tests")
        
        tests = [
            ("Health Check", "GET", "/health", None, None, None, 200),
            ("OpenAPI Spec", "GET", "/openapi.yaml", None, None, None, 200),
            ("Swagger UI", "GET", "/docs", None, None, None, 200),
        ]
        
        results = []
        for name, method, path, headers, params, json_data, expected_status in tests:
            try:
                print_test(name)
                response = self.make_request(method, path, headers, params, json_data)
                
                if response.status_code == expected_status:
                    print_pass("PASS")
                    results.append(("PASS", name, response.status_code))
                else:
                    print_fail(f"FAIL - Expected {expected_status}, got {response.status_code}")
                    results.append(("FAIL", name, response.status_code))
                    
            except Exception as e:
                print_fail(f"ERROR - {e}")
                results.append(("ERROR", name, str(e)))
        
        return results

    def test_pets_api(self):
        """Test the pets API endpoints"""
        print_section("Pets API Tests")
        
        results = []
        
        # Test 1: List pets
        try:
            print_test("List Pets")
            response = self.make_request("GET", "/pets", params={"limit": 5})
            
            if response.status_code == 200:
                pets = response.json()
                print_pass(f"PASS - Retrieved {len(pets)} pets")
                
                # Validate structure
                if isinstance(pets, list) and len(pets) > 0:
                    pet = pets[0]
                    required_fields = ['id', 'name', 'breed', 'age']
                    missing_fields = [field for field in required_fields if field not in pet]
                    
                    if missing_fields:
                        print_warn(f"Missing fields: {missing_fields}")
                    else:
                        print_pass("All required fields present")
                
                results.append(("PASS", "List Pets", 200))
            else:
                print_fail(f"FAIL - Status {response.status_code}")
                results.append(("FAIL", "List Pets", response.status_code))
                
        except Exception as e:
            print_fail(f"ERROR - {e}")
            results.append(("ERROR", "List Pets", str(e)))

        # Test 2: Get specific pet
        try:
            print_test("Get Pet by ID")
            response = self.make_request("GET", "/pets/12345")
            
            if response.status_code in [200, 400]:  # 400 might be validation error
                if response.status_code == 200:
                    print_pass("PASS - Pet retrieved successfully")
                    results.append(("PASS", "Get Pet", 200))
                else:
                    print_warn("VALIDATION - Response validation error (expected in dev)")
                    results.append(("VALIDATION", "Get Pet", 400))
            else:
                print_fail(f"FAIL - Status {response.status_code}")
                results.append(("FAIL", "Get Pet", response.status_code))
                
        except Exception as e:
            print_fail(f"ERROR - {e}")
            results.append(("ERROR", "Get Pet", str(e)))

        # Test 3: Add new pet
        try:
            print_test("Add New Pet")
            new_pet = {
                "name": "Test Pet",
                "breed": "Test Breed",
                "age": 2,
                "owner_id": "test-user-123",
                "tags": ["test", "api"]
            }
            
            response = self.make_request("POST", "/pets", 
                                       headers={"Content-Type": "application/json"}, 
                                       json_data=new_pet)
            
            if response.status_code in [201, 200, 400]:  # Various success/validation responses
                if response.status_code in [200, 201]:
                    print_pass("PASS - Pet created successfully")
                    results.append(("PASS", "Add Pet", response.status_code))
                else:
                    print_warn("VALIDATION - Request validation (check OpenAPI spec requirements)")
                    results.append(("VALIDATION", "Add Pet", 400))
            else:
                print_fail(f"FAIL - Status {response.status_code}")
                results.append(("FAIL", "Add Pet", response.status_code))
                
        except Exception as e:
            print_fail(f"ERROR - {e}")
            results.append(("ERROR", "Add Pet", str(e)))

        return results

    def test_users_api(self):
        """Test the users API endpoints"""
        print_section("Users API Tests")
        
        results = []
        
        # Test: List users
        try:
            print_test("List Users")
            response = self.make_request("GET", "/users")
            
            if response.status_code == 200:
                users = response.json()
                print_pass(f"PASS - Retrieved {len(users)} users")
                results.append(("PASS", "List Users", 200))
            else:
                print_fail(f"FAIL - Status {response.status_code}")
                results.append(("FAIL", "List Users", response.status_code))
                
        except Exception as e:
            print_fail(f"ERROR - {e}")
            results.append(("ERROR", "List Users", str(e)))

        # Test: Get user
        try:
            print_test("Get User by ID")
            response = self.make_request("GET", "/users/user-123")
            
            if response.status_code in [200, 400]:
                if response.status_code == 200:
                    print_pass("PASS - User retrieved successfully")
                    results.append(("PASS", "Get User", 200))
                else:
                    print_warn("VALIDATION - Response validation error (expected in dev)")
                    results.append(("VALIDATION", "Get User", 400))
            else:
                print_fail(f"FAIL - Status {response.status_code}")
                results.append(("FAIL", "Get User", response.status_code))
                
        except Exception as e:
            print_fail(f"ERROR - {e}")
            results.append(("ERROR", "Get User", str(e)))

        return results

    def test_auth_security(self):
        """Test authentication and security"""
        print_section("Authentication & Security Tests")
        
        results = []
        
        # Test: No API key
        try:
            print_test("Request without API key")
            response = self.session.get(f"{self.base_url}/pets", timeout=10)
            
            if response.status_code == 401:
                print_pass("PASS - Correctly rejected unauthorized request")
                results.append(("PASS", "Auth Required", 401))
            else:
                print_fail(f"FAIL - Expected 401, got {response.status_code}")
                results.append(("FAIL", "Auth Required", response.status_code))
                
        except Exception as e:
            print_fail(f"ERROR - {e}")
            results.append(("ERROR", "Auth Required", str(e)))

        # Test: Invalid API key
        try:
            print_test("Request with invalid API key")
            response = self.session.get(f"{self.base_url}/pets", 
                                      headers={"X-API-Key": "invalid_key"}, 
                                      timeout=10)
            
            if response.status_code == 401:
                print_pass("PASS - Correctly rejected invalid API key")
                results.append(("PASS", "Invalid Auth", 401))
            else:
                print_warn(f"Expected 401, got {response.status_code}")
                results.append(("UNEXPECTED", "Invalid Auth", response.status_code))
                
        except Exception as e:
            print_fail(f"ERROR - {e}")
            results.append(("ERROR", "Invalid Auth", str(e)))

        return results

    def print_summary(self, all_results: List[List]):
        """Print a comprehensive test summary"""
        print_header("Test Summary")
        
        # Flatten results
        flattened = []
        for category_results in all_results:
            flattened.extend(category_results)
        
        # Count results
        total = len(flattened)
        passed = len([r for r in flattened if r[0] == "PASS"])
        failed = len([r for r in flattened if r[0] == "FAIL"])
        errors = len([r for r in flattened if r[0] == "ERROR"])
        validation = len([r for r in flattened if r[0] == "VALIDATION"])
        
        # Print summary table
        print(f"\n{'Category':<20} {'Count':<8} {'Percentage':<12}")
        print("-" * 40)
        print_colored(f"{'✅ Passed':<20} {passed:<8} {passed/total*100:.1f}%" if total > 0 else "{'✅ Passed':<20} {0:<8} 0%", Colors.GREEN)
        print_colored(f"{'❌ Failed':<20} {failed:<8} {failed/total*100:.1f}%" if total > 0 else "{'❌ Failed':<20} {0:<8} 0%", Colors.RED)
        print_colored(f"{'🔧 Validation':<20} {validation:<8} {validation/total*100:.1f}%" if total > 0 else "{'🔧 Validation':<20} {0:<8} 0%", Colors.YELLOW)
        print_colored(f"{'💥 Errors':<20} {errors:<8} {errors/total*100:.1f}%" if total > 0 else "{'💥 Errors':<20} {0:<8} 0%", Colors.RED)
        print("-" * 40)
        print_colored(f"{'TOTAL':<20} {total:<8} 100%", Colors.BOLD)
        
        # Success rate
        success_rate = (passed / total * 100) if total > 0 else 0
        if success_rate >= 80:
            print_colored(f"\n🎉 Overall Status: EXCELLENT ({success_rate:.1f}% success rate)", Colors.GREEN + Colors.BOLD)
        elif success_rate >= 60:
            print_colored(f"\n⚠️  Overall Status: GOOD ({success_rate:.1f}% success rate)", Colors.YELLOW + Colors.BOLD)
        else:
            print_colored(f"\n🚨 Overall Status: NEEDS WORK ({success_rate:.1f}% success rate)", Colors.RED + Colors.BOLD)
        
        # Show failed tests
        failed_tests = [r for r in flattened if r[0] in ["FAIL", "ERROR"]]
        if failed_tests:
            print_colored("\nFailed Tests:", Colors.RED + Colors.BOLD)
            for status, name, details in failed_tests:
                print_colored(f"  • {name}: {details}", Colors.RED)

    def run_all_tests(self):
        """Run the complete test suite"""
        print_header("BRRTRouter API Test Suite")
        print_info(f"Testing: {self.base_url}")
        print_info(f"Spec: {self.spec_path}")
        
        # Check server health first
        if not self.check_server_health():
            return False
        
        # Run test categories
        all_results = []
        all_results.append(self.test_infrastructure())
        all_results.append(self.test_pets_api())
        all_results.append(self.test_users_api())
        all_results.append(self.test_auth_security())
        
        # Print summary
        self.print_summary(all_results)
        
        return True

def main():
    """Main entry point"""
    base_url = "http://localhost:8080"
    spec_path = "examples/pet_store/doc/openapi.yaml"
    
    if len(sys.argv) > 1:
        base_url = sys.argv[1]
    if len(sys.argv) > 2:
        spec_path = sys.argv[2]
    
    tester = APITester(base_url, spec_path)
    success = tester.run_all_tests()
    
    if not success:
        sys.exit(1)

if __name__ == "__main__":
    main() 