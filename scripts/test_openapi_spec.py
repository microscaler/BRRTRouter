#!/usr/bin/env python3
"""
Dynamic OpenAPI Specification Testing Framework

Reads the OpenAPI specification and dynamically generates tests
based on the actual spec definition rather than hardcoded expectations.
"""

import requests
import time
import sys
import json
import yaml
from pathlib import Path
from typing import Dict, List, Optional, Any, Set
from dataclasses import dataclass
from enum import Enum

class TestResult(Enum):
    PASS = "✅ PASS"
    FAIL = "❌ FAIL"
    SKIP = "⏭️  SKIP"
    WARNING = "⚠️  WARN"

@dataclass
class TestCase:
    name: str
    method: str
    path: str
    headers: Optional[Dict[str, str]] = None
    params: Optional[Dict[str, str]] = None
    body: Optional[Dict[str, Any]] = None
    expected_status: int = 200
    description: str = ""
    category: str = "general"
    spec_source: str = ""  # Where this test came from in the spec

@dataclass
class TestSummary:
    total: int = 0
    passed: int = 0
    failed: int = 0
    skipped: int = 0
    warnings: int = 0

class OpenAPISpecParser:
    """Parse OpenAPI specification and extract testable information"""
    
    def __init__(self, spec_path: str = "examples/openapi.yaml"):
        self.spec_path = spec_path
        self.spec = self.load_spec()
        
    def load_spec(self) -> Dict[str, Any]:
        """Load and parse the OpenAPI specification"""
        try:
            with open(self.spec_path, 'r') as f:
                return yaml.safe_load(f)
        except Exception as e:
            print(f"❌ Failed to load OpenAPI spec from {self.spec_path}: {e}")
            sys.exit(1)
    
    def get_paths(self) -> Dict[str, Any]:
        """Get all paths from the spec"""
        return self.spec.get('paths', {})
    
    def get_components(self) -> Dict[str, Any]:
        """Get components (schemas, etc.) from the spec"""
        return self.spec.get('components', {})
    
    def get_servers(self) -> List[Dict[str, Any]]:
        """Get server definitions"""
        return self.spec.get('servers', [])
    
    def get_security_schemes(self) -> Dict[str, Any]:
        """Get security scheme definitions"""
        return self.get_components().get('securitySchemes', {})
    
    def extract_parameters(self, operation: Dict[str, Any]) -> Dict[str, List[Dict[str, Any]]]:
        """Extract parameters by location from an operation"""
        parameters = operation.get('parameters', [])
        by_location = {'query': [], 'header': [], 'path': [], 'cookie': []}
        
        for param in parameters:
            location = param.get('in', 'query')
            if location in by_location:
                by_location[location].append(param)
        
        return by_location
    
    def extract_request_body_schema(self, operation: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Extract request body schema if present"""
        request_body = operation.get('requestBody', {})
        content = request_body.get('content', {})
        
        # Look for JSON content
        for content_type in ['application/json', '*/*']:
            if content_type in content:
                return content[content_type].get('schema')
        
        return None
    
    def extract_response_schemas(self, operation: Dict[str, Any]) -> Dict[str, Dict[str, Any]]:
        """Extract response schemas by status code"""
        responses = operation.get('responses', {})
        schemas = {}
        
        for status_code, response in responses.items():
            content = response.get('content', {})
            for content_type in ['application/json', '*/*']:
                if content_type in content:
                    schemas[status_code] = content[content_type].get('schema')
                    break
        
        return schemas
    
    def generate_example_data(self, schema: Dict[str, Any]) -> Any:
        """Generate example data based on schema"""
        if not schema:
            return None
            
        # Use explicit example if available
        if 'example' in schema:
            return schema['example']
        
        # Handle different schema types
        schema_type = schema.get('type', 'object')
        
        if schema_type == 'object':
            properties = schema.get('properties', {})
            example = {}
            for prop_name, prop_schema in properties.items():
                example[prop_name] = self.generate_example_data(prop_schema)
            return example
        elif schema_type == 'array':
            items_schema = schema.get('items', {})
            return [self.generate_example_data(items_schema)]
        elif schema_type == 'string':
            return schema.get('default', 'test_string')
        elif schema_type == 'integer':
            return schema.get('default', 42)
        elif schema_type == 'number':
            return schema.get('default', 42.0)
        elif schema_type == 'boolean':
            return schema.get('default', True)
        else:
            return None

class DynamicOpenAPITester:
    def __init__(self, base_url: str = "http://localhost:8080", spec_path: str = "examples/openapi.yaml"):
        self.base_url = base_url
        self.timeout = 10
        self.session = requests.Session()
        self.results: List[tuple] = []
        self.summary = TestSummary()
        self.parser = OpenAPISpecParser(spec_path)
        
    def get_api_key(self) -> str:
        """Get API key for authentication"""
        return "test123"  # Default test API key
        
    def run_test(self, test: TestCase) -> TestResult:
        """Execute a single test case"""
        try:
            url = f"{self.base_url}{test.path}"
            
            # Prepare headers with authentication (unless it's an auth test)
            if test.category == "authentication":
                # For auth tests, use exactly the headers specified (may be empty or invalid)
                headers = test.headers or {}
            else:
                # For all other tests, include API key
                headers = {"X-API-Key": self.get_api_key()}
                if test.headers:
                    headers.update(test.headers)
            
            # Prepare request
            kwargs = {
                'method': test.method,
                'url': url,
                'headers': headers,
                'params': test.params or {},
            }
            
            if test.body:
                kwargs['json'] = test.body
                kwargs['headers']['Content-Type'] = 'application/json'
            
            # Execute request
            response = self.session.request(timeout=self.timeout, **kwargs)
            
            # Evaluate result based on expected status codes from spec
            if response.status_code == test.expected_status:
                result = TestResult.PASS
                details = f"Status: {response.status_code}"
            else:
                result = TestResult.FAIL
                details = f"Expected: {test.expected_status}, Got: {response.status_code}\nResponse: {response.text[:200]}"
            
            self.results.append((test, result, details))
            
            # Update summary
            self.summary.total += 1
            if result == TestResult.PASS:
                self.summary.passed += 1
            elif result == TestResult.FAIL:
                self.summary.failed += 1
            elif result == TestResult.SKIP:
                self.summary.skipped += 1
            elif result == TestResult.WARNING:
                self.summary.warnings += 1
                
            return result
            
        except requests.exceptions.RequestException as e:
            result = TestResult.FAIL
            details = f"Request failed: {str(e)}"
            self.results.append((test, result, details))
            self.summary.total += 1
            self.summary.failed += 1
            return result
    
    def print_result(self, test: TestCase, result: TestResult, details: str):
        """Print test result"""
        status = result.value
        print(f"{status} [{test.category}] {test.name}")
        if result in [TestResult.FAIL, TestResult.WARNING]:
            print(f"    {details}")
        if test.description:
            print(f"    📝 {test.description}")
        if test.spec_source:
            print(f"    🔗 From: {test.spec_source}")
    
    def generate_dynamic_test_cases(self) -> List[TestCase]:
        """Generate test cases dynamically from the OpenAPI specification"""
        tests = []
        
        # === INFRASTRUCTURE TESTS ===
        tests.extend(self.generate_infrastructure_tests())
        
        # === DYNAMIC PATH-BASED TESTS ===
        paths = self.parser.get_paths()
        for path_pattern, path_obj in paths.items():
            tests.extend(self.generate_path_tests(path_pattern, path_obj))
        
        # === AUTHENTICATION TESTS ===
        tests.extend(self.generate_authentication_tests())
        
        # === VALIDATION TESTS ===
        tests.extend(self.generate_validation_tests())
        
        # === ERROR HANDLING TESTS ===
        tests.extend(self.generate_error_tests())
        
        return tests
    
    def generate_infrastructure_tests(self) -> List[TestCase]:
        """Generate infrastructure-related tests"""
        return [
            TestCase(
                name="Health Check",
                method="GET",
                path="/health",
                expected_status=200,
                description="Basic health endpoint should always respond",
                category="infrastructure",
                spec_source="Built-in endpoint"
            ),
            TestCase(
                name="OpenAPI Spec",
                method="GET", 
                path="/openapi.yaml",
                expected_status=200,
                description="OpenAPI specification should be accessible",
                category="infrastructure",
                spec_source="Built-in endpoint"
            ),
            TestCase(
                name="Swagger UI",
                method="GET",
                path="/docs",
                expected_status=200,
                description="Swagger UI documentation should be available",
                category="infrastructure",
                spec_source="Built-in endpoint"
            ),
        ]
    
    def generate_authentication_tests(self) -> List[TestCase]:
        """Generate authentication-related tests"""
        auth_tests = []
        
        # Test without API key (should get 401)
        auth_tests.append(TestCase(
            name="Request without API key",
            method="GET",
            path="/pets",
            headers={},  # No API key
            expected_status=401,
            description="Should return 401 when no API key is provided",
            category="authentication",
            spec_source="Security validation test"
        ))
        
        # Test with invalid API key (should get 401)
        auth_tests.append(TestCase(
            name="Request with invalid API key",
            method="GET",
            path="/pets",
            headers={"X-API-Key": "invalid_key_12345"},
            expected_status=401,
            description="Should return 401 when invalid API key is provided",
            category="authentication",
            spec_source="Security validation test"
        ))
        
        return auth_tests
    
    def generate_path_tests(self, path_pattern: str, path_obj: Dict[str, Any]) -> List[TestCase]:
        """Generate tests for a specific path from the OpenAPI spec"""
        tests = []
        
        for method, operation in path_obj.items():
            if method.upper() not in ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS']:
                continue
                
            operation_id = operation.get('operationId', f"{method}_{path_pattern}")
            summary = operation.get('summary', f"{method.upper()} {path_pattern}")
            
            # Extract parameters
            params_by_location = self.parser.extract_parameters(operation)
            
            # Extract expected response status codes
            responses = operation.get('responses', {})
            success_status = self.determine_success_status(responses)
            
            # Generate basic test with minimal parameters
            test = self.create_basic_path_test(
                path_pattern, method.upper(), operation_id, summary, 
                params_by_location, operation, success_status
            )
            if test:
                tests.append(test)
            
            # Generate parameter validation tests
            tests.extend(self.generate_parameter_tests(
                path_pattern, method.upper(), operation_id, params_by_location, operation
            ))
            
            # Generate request body tests for POST/PUT/PATCH
            if method.upper() in ['POST', 'PUT', 'PATCH']:
                tests.extend(self.generate_request_body_tests(
                    path_pattern, method.upper(), operation_id, operation, success_status
                ))
        
        return tests
    
    def determine_success_status(self, responses: Dict[str, Any]) -> int:
        """Determine the expected success status code from responses"""
        # Look for success status codes in order of preference
        for status in ['200', '201', '202', '204']:
            if status in responses:
                return int(status)
        
        # Default to 200
        return 200
    
    def create_basic_path_test(self, path_pattern: str, method: str, operation_id: str, 
                              summary: str, params_by_location: Dict[str, List[Dict]], 
                              operation: Dict[str, Any], success_status: int) -> Optional[TestCase]:
        """Create a basic test for a path with minimal required parameters"""
        
        # Convert path pattern to actual path with example values
        actual_path = self.resolve_path_parameters(path_pattern, params_by_location['path'])
        if not actual_path:
            return None
        
        # Build required query parameters
        query_params = {}
        for param in params_by_location['query']:
            if param.get('required', False):
                example_value = self.get_parameter_example(param)
                if example_value is not None:
                    query_params[param['name']] = str(example_value)
        
        # Build required headers
        headers = {}
        for param in params_by_location['header']:
            if param.get('required', False):
                example_value = self.get_parameter_example(param)
                if example_value is not None:
                    headers[param['name']] = str(example_value)
        
        # Determine expected status based on whether we have all required params
        has_all_required = self.has_all_required_parameters(params_by_location, query_params, headers)
        expected_status = success_status if has_all_required else 400
        
        category = self.categorize_path(path_pattern)
        
        return TestCase(
            name=f"{summary} - Basic Test",
            method=method,
            path=actual_path,
            headers=headers if headers else None,
            params=query_params if query_params else None,
            expected_status=expected_status,
            description=f"Test {operation_id} with minimal required parameters",
            category=category,
            spec_source=f"{method} {path_pattern}"
        )
    
    def resolve_path_parameters(self, path_pattern: str, path_params: List[Dict[str, Any]]) -> Optional[str]:
        """Convert path pattern with {param} to actual path with example values"""
        actual_path = path_pattern
        
        for param in path_params:
            param_name = param['name']
            placeholder = f"{{{param_name}}}"
            
            if placeholder in actual_path:
                example_value = self.get_parameter_example(param)
                if example_value is None:
                    # Can't resolve this path parameter
                    return None
                actual_path = actual_path.replace(placeholder, str(example_value))
        
        return actual_path
    
    def get_parameter_example(self, param: Dict[str, Any]) -> Any:
        """Get an example value for a parameter"""
        # Use explicit example if available
        if 'example' in param:
            return param['example']
        
        # Use schema example
        schema = param.get('schema', {})
        if 'example' in schema:
            return schema['example']
        
        # Generate based on type
        param_type = schema.get('type', 'string')
        if param_type == 'string':
            # Special handling for common parameter names
            param_name = param.get('name', '').lower()
            if 'id' in param_name:
                return "123"
            elif 'email' in param_name:
                return "test@example.com"
            elif 'name' in param_name:
                return "test_name"
            else:
                return "test_value"
        elif param_type == 'integer':
            return 123
        elif param_type == 'number':
            return 123.45
        elif param_type == 'boolean':
            return True
        
        return None
    
    def has_all_required_parameters(self, params_by_location: Dict[str, List[Dict]], 
                                   query_params: Dict[str, str], headers: Dict[str, str]) -> bool:
        """Check if we have all required parameters"""
        for param in params_by_location['query']:
            if param.get('required', False) and param['name'] not in query_params:
                return False
        
        for param in params_by_location['header']:
            if param.get('required', False) and param['name'] not in headers:
                return False
        
        # Path parameters are handled in resolve_path_parameters
        return True
    
    def categorize_path(self, path_pattern: str) -> str:
        """Categorize a path for test organization"""
        if '/pets' in path_pattern:
            return 'pets'
        elif '/users' in path_pattern:
            return 'users'
        elif '/health' in path_pattern:
            return 'infrastructure'
        else:
            return 'general'
    
    def generate_parameter_tests(self, path_pattern: str, method: str, operation_id: str,
                                params_by_location: Dict[str, List[Dict]], operation: Dict[str, Any]) -> List[TestCase]:
        """Generate parameter validation tests"""
        tests = []
        category = self.categorize_path(path_pattern)
        
        # Test missing required parameters
        for location, params in params_by_location.items():
            for param in params:
                if param.get('required', False):
                    # Test what happens when this required parameter is missing
                    test_name = f"{operation_id} - Missing Required {location.title()} Parameter '{param['name']}'"
                    
                    # Build minimal path
                    actual_path = self.resolve_path_parameters(path_pattern, params_by_location['path'])
                    if not actual_path:
                        continue
                    
                    tests.append(TestCase(
                        name=test_name,
                        method=method,
                        path=actual_path,
                        expected_status=400,
                        description=f"Should return 400 when required {location} parameter '{param['name']}' is missing",
                        category=category,
                        spec_source=f"{method} {path_pattern} - parameter validation"
                    ))
        
        return tests
    
    def generate_request_body_tests(self, path_pattern: str, method: str, operation_id: str,
                                   operation: Dict[str, Any], success_status: int) -> List[TestCase]:
        """Generate request body tests for POST/PUT/PATCH operations"""
        tests = []
        category = self.categorize_path(path_pattern)
        
        # Resolve path parameters
        params_by_location = self.parser.extract_parameters(operation)
        actual_path = self.resolve_path_parameters(path_pattern, params_by_location['path'])
        if not actual_path:
            return tests
        
        # Get request body schema
        body_schema = self.parser.extract_request_body_schema(operation)
        if body_schema:
            # Test with valid body
            valid_body = self.parser.generate_example_data(body_schema)
            if valid_body:
                tests.append(TestCase(
                    name=f"{operation_id} - Valid Request Body",
                    method=method,
                    path=actual_path,
                    body=valid_body,
                    expected_status=success_status,
                    description=f"Test {operation_id} with valid request body",
                    category=category,
                    spec_source=f"{method} {path_pattern} - request body"
                ))
            
            # Test with invalid body (empty)
            tests.append(TestCase(
                name=f"{operation_id} - Invalid Request Body",
                method=method,
                path=actual_path,
                body={},
                expected_status=400,
                description=f"Should return 400 with invalid request body",
                category=category,
                spec_source=f"{method} {path_pattern} - request body validation"
            ))
        
        return tests
    
    def generate_validation_tests(self) -> List[TestCase]:
        """Generate validation edge case tests"""
        tests = []
        
        # Get some paths for validation testing
        paths = self.parser.get_paths()
        if paths:
            first_path = list(paths.keys())[0]
            tests.append(TestCase(
                name="Invalid Path Parameter Types",
                method="GET",
                path=first_path.replace('{id}', 'invalid-id') if '{id}' in first_path else f"{first_path}/invalid",
                expected_status=404,  # Might be 400 or 404 depending on implementation
                description="Test with invalid path parameter types",
                category="validation",
                spec_source="Generated validation test"
            ))
        
        return tests
    
    def generate_error_tests(self) -> List[TestCase]:
        """Generate error handling tests"""
        return [
            TestCase(
                name="Not Found Endpoint",
                method="GET",
                path="/nonexistent",
                expected_status=404,
                description="Should return 404 for unknown endpoints",
                category="errors",
                spec_source="Generated error test"
            ),
        ]
    
    def run_all_tests(self):
        """Run all test cases"""
        print("🧪 Starting Dynamic OpenAPI Testing")
        print(f"📋 Reading specification from: {self.parser.spec_path}")
        print("=" * 60)
        
        tests = self.generate_dynamic_test_cases()
        
        # Group tests by category
        categories = {}
        for test in tests:
            if test.category not in categories:
                categories[test.category] = []
            categories[test.category].append(test)
        
        # Run tests by category
        for category, category_tests in categories.items():
            print(f"\n📋 {category.upper()} TESTS")
            print("-" * 40)
            
            for test in category_tests:
                result = self.run_test(test)
                details = self.results[-1][2]
                self.print_result(test, result, details)
                
                # Small delay between tests
                time.sleep(0.1)
        
        # Print summary
        self.print_summary()
    
    def print_summary(self):
        """Print test execution summary"""
        print("\n" + "=" * 60)
        print("📊 TEST SUMMARY")
        print("=" * 60)
        
        s = self.summary
        total = s.total
        
        print(f"Total Tests:     {total}")
        print(f"✅ Passed:       {s.passed} ({s.passed/total*100:.1f}%)" if total > 0 else "✅ Passed:       0")
        print(f"❌ Failed:       {s.failed} ({s.failed/total*100:.1f}%)" if total > 0 else "❌ Failed:       0") 
        print(f"⏭️  Skipped:      {s.skipped} ({s.skipped/total*100:.1f}%)" if total > 0 else "⏭️  Skipped:      0")
        print(f"⚠️  Warnings:     {s.warnings} ({s.warnings/total*100:.1f}%)" if total > 0 else "⚠️  Warnings:     0")
        
        if s.failed > 0:
            print(f"\n🔍 FAILED TESTS:")
            for test, result, details in self.results:
                if result == TestResult.FAIL:
                    print(f"  • {test.name}: {details.split('Response:')[0].strip()}")
        
        success_rate = (s.passed / total * 100) if total > 0 else 0
        if success_rate >= 80:
            print(f"\n🎉 Overall Status: GOOD ({success_rate:.1f}% success rate)")
        elif success_rate >= 60:
            print(f"\n⚠️  Overall Status: NEEDS IMPROVEMENT ({success_rate:.1f}% success rate)")
        else:
            print(f"\n🚨 Overall Status: CRITICAL ISSUES ({success_rate:.1f}% success rate)")
    
    def export_results(self, filename: str = "test_results.json"):
        """Export detailed test results to JSON"""
        results_data = {
            "summary": {
                "total": self.summary.total,
                "passed": self.summary.passed,
                "failed": self.summary.failed,
                "skipped": self.summary.skipped,
                "warnings": self.summary.warnings,
                "success_rate": (self.summary.passed / self.summary.total * 100) if self.summary.total > 0 else 0
            },
            "spec_info": {
                "path": self.parser.spec_path,
                "title": self.parser.spec.get('info', {}).get('title', 'Unknown'),
                "version": self.parser.spec.get('info', {}).get('version', 'Unknown'),
                "paths_count": len(self.parser.get_paths()),
                "components_count": len(self.parser.get_components())
            },
            "results": []
        }
        
        for test, result, details in self.results:
            results_data["results"].append({
                "name": test.name,
                "category": test.category,
                "method": test.method,
                "path": test.path,
                "expected_status": test.expected_status,
                "result": result.name,
                "details": details,
                "description": test.description,
                "spec_source": test.spec_source
            })
        
        with open(filename, 'w') as f:
            json.dump(results_data, f, indent=2)
        
        print(f"\n📄 Detailed results exported to {filename}")

def main():
    """Main test execution"""
    base_url = "http://localhost:8080"
    spec_path = "examples/openapi.yaml"
    
    if len(sys.argv) > 1:
        base_url = sys.argv[1]
    if len(sys.argv) > 2:
        spec_path = sys.argv[2]
    
    print(f"🎯 Testing BRRTRouter at {base_url}")
    print(f"📋 Using OpenAPI spec: {spec_path}")
    
    # Check if server is running
    try:
        response = requests.get(f"{base_url}/health", timeout=5)
        if response.status_code != 200:
            print(f"❌ Server health check failed: {response.status_code}")
            sys.exit(1)
    except requests.exceptions.RequestException as e:
        print(f"❌ Cannot connect to server at {base_url}: {e}")
        print("💡 Make sure the server is running with: python3 scripts/manage_service.py start")
        sys.exit(1)
    
    print("✅ Server is running, starting tests...\n")
    
    # Run tests
    tester = DynamicOpenAPITester(base_url, spec_path)
    tester.run_all_tests()
    
    # Export results
    tester.export_results()
    
    # Exit with appropriate code
    if tester.summary.failed > 0:
        sys.exit(1)
    else:
        sys.exit(0)

if __name__ == "__main__":
    main() 