#!/usr/bin/env python3
"""
BRRTRouter Service Manager

Manages the BRRTRouter serve service using launchctl for testing purposes.
"""

import subprocess
import sys
import os
import time
from pathlib import Path

class BRRTRouterService:
    def __init__(self, spec_file=None):
        self.project_root = Path(__file__).parent.parent
        self.plist_path = self.project_root / "scripts" / "com.brrtrouter.serve.plist"
        self.logs_dir = self.project_root / "logs"
        self.service_name = "com.brrtrouter.serve"
        self.examples_dir = self.project_root / "examples"
        self.openapi_yaml = self.examples_dir / "openapi.yaml"
        self.backup_yaml = self.examples_dir / "openapi.yaml.backup"
        self.current_spec = spec_file
        
    def setup_logs_dir(self):
        """Create logs directory if it doesn't exist"""
        self.logs_dir.mkdir(exist_ok=True)
        
    def backup_original_spec(self):
        """Backup the original openapi.yaml if it exists"""
        if self.openapi_yaml.exists() and not self.backup_yaml.exists():
            print(f"📋 Backing up original openapi.yaml to openapi.yaml.backup")
            self.openapi_yaml.rename(self.backup_yaml)
            
    def restore_original_spec(self):
        """Restore the original openapi.yaml from backup"""
        if self.backup_yaml.exists():
            if self.openapi_yaml.exists():
                self.openapi_yaml.unlink()  # Remove current file
            self.backup_yaml.rename(self.openapi_yaml)
            print(f"📋 Restored original openapi.yaml from backup")
            
    def setup_spec_file(self):
        """Setup the correct spec file for testing"""
        if self.current_spec:
            spec_path = self.examples_dir / self.current_spec
            if not spec_path.exists():
                print(f"❌ Spec file not found: {spec_path}")
                return False
                
            # Backup original if needed
            self.backup_original_spec()
            
            # Copy the test spec to openapi.yaml
            print(f"📋 Using spec file: {self.current_spec}")
            if self.openapi_yaml.exists():
                self.openapi_yaml.unlink()
            
            import shutil
            shutil.copy2(spec_path, self.openapi_yaml)
            print(f"📋 Copied {self.current_spec} to openapi.yaml")
            
        return True
        
    def start(self):
        """Start the BRRTRouter service"""
        self.setup_logs_dir()
        
        # Setup the spec file if specified
        if not self.setup_spec_file():
            return False
        
        # First, ensure any existing service is stopped
        self.stop()
        
        print(f"🚀 Starting BRRTRouter service...")
        print(f"📁 Working directory: {self.project_root}")
        print(f"📄 Plist: {self.plist_path}")
        print(f"📝 Logs: {self.logs_dir}")
        
        try:
            # Load the service
            result = subprocess.run([
                "launchctl", "load", str(self.plist_path)
            ], capture_output=True, text=True)
            
            if result.returncode != 0:
                print(f"❌ Failed to load service: {result.stderr}")
                return False
                
            # Start the service
            result = subprocess.run([
                "launchctl", "start", self.service_name
            ], capture_output=True, text=True)
            
            if result.returncode != 0:
                print(f"❌ Failed to start service: {result.stderr}")
                return False
                
            print("✅ Service started successfully")
            print("📋 Use 'python scripts/manage_service.py status' to check status")
            print("📋 Use 'python scripts/manage_service.py logs' to view logs")
            return True
            
        except Exception as e:
            print(f"❌ Error starting service: {e}")
            return False
    
    def stop(self):
        """Stop the BRRTRouter service"""
        print("🛑 Stopping BRRTRouter service...")
        
        try:
            # Stop the service
            subprocess.run([
                "launchctl", "stop", self.service_name
            ], capture_output=True, text=True)
            
            # Unload the service
            subprocess.run([
                "launchctl", "unload", str(self.plist_path)
            ], capture_output=True, text=True)
            
            # Note: We don't restore the spec here to allow testing
            # Call restore_original_spec() manually when done testing
            
            print("✅ Service stopped")
            return True
            
        except Exception as e:
            print(f"❌ Error stopping service: {e}")
            return False
    
    def status(self):
        """Check the status of the BRRTRouter service"""
        try:
            result = subprocess.run([
                "launchctl", "list", self.service_name
            ], capture_output=True, text=True)
            
            if result.returncode == 0:
                print("✅ Service is running")
                print(result.stdout)
                return True
            else:
                print("❌ Service is not running")
                return False
                
        except Exception as e:
            print(f"❌ Error checking status: {e}")
            return False
    
    def logs(self, follow=False):
        """View service logs"""
        out_log = self.logs_dir / "brrtrouter-serve.out.log"
        err_log = self.logs_dir / "brrtrouter-serve.err.log"
        
        print(f"📄 Output log: {out_log}")
        print(f"📄 Error log: {err_log}")
        print("-" * 50)
        
        if follow:
            print("👀 Following logs (Ctrl+C to stop)...")
            try:
                subprocess.run(["tail", "-f", str(out_log), str(err_log)])
            except KeyboardInterrupt:
                print("\n📋 Stopped following logs")
        else:
            if out_log.exists():
                print("📤 STDOUT:")
                with open(out_log) as f:
                    print(f.read())
                    
            if err_log.exists():
                print("\n📤 STDERR:")
                with open(err_log) as f:
                    print(f.read())
    
    def test_stack_overflow(self):
        """Test the service to trigger stack overflow"""
        print("🧪 Testing for stack overflow...")
        
        # Wait a moment for service to be ready
        time.sleep(2)
        
        try:
            # Make a test request
            result = subprocess.run([
                "curl", "-X", "GET", "http://localhost:8080/pets"
            ], capture_output=True, text=True, timeout=10)
            
            print(f"📡 Request result: {result.returncode}")
            if result.stdout:
                print(f"📤 Response: {result.stdout}")
            if result.stderr:
                print(f"📤 Error: {result.stderr}")
                
        except subprocess.TimeoutExpired:
            print("⏰ Request timed out - possible stack overflow")
        except Exception as e:
            print(f"❌ Request failed: {e}")
        
        # Check logs for stack overflow
        print("\n🔍 Checking logs for stack overflow...")
        self.check_logs_for_errors()
    
    def check_logs_for_errors(self):
        """Check logs for stack overflow and other errors"""
        err_log = self.logs_dir / "brrtrouter-serve.err.log"
        
        if err_log.exists():
            with open(err_log) as f:
                content = f.read()
                if "stack overflow" in content.lower():
                    print("🚨 STACK OVERFLOW DETECTED!")
                    print(content)
                elif "panic" in content.lower():
                    print("🚨 PANIC DETECTED!")
                    print(content)
                elif content.strip():
                    print("⚠️  Errors in log:")
                    print(content)
                else:
                    print("✅ No errors in logs")
        else:
            print("📄 No error log found")

def main():
    # Parse arguments
    if len(sys.argv) < 2:
        print("Usage: python scripts/manage_service.py [start|stop|status|logs|test|follow-logs|restart] [spec_file]")
        print("  spec_file: Optional OpenAPI spec file (e.g., simple_test.yaml)")
        print("  Examples:")
        print("    python scripts/manage_service.py start")
        print("    python scripts/manage_service.py start simple_test.yaml")
        print("    python scripts/manage_service.py test simple_test.yaml")
        sys.exit(1)
    
    command = sys.argv[1]
    spec_file = sys.argv[2] if len(sys.argv) > 2 else None
    
    service = BRRTRouterService(spec_file)
    
    if command == "start":
        service.start()
    elif command == "stop":
        service.stop()
    elif command == "status":
        service.status()
    elif command == "logs":
        service.logs()
    elif command == "follow-logs":
        service.logs(follow=True)
    elif command == "test":
        service.test_stack_overflow()
    elif command == "restart":
        service.stop()
        time.sleep(1)
        service.start()
    elif command == "restore":
        service.restore_original_spec()
        print("✅ Original spec restored")
    else:
        print(f"Unknown command: {command}")
        print("Available commands: start, stop, status, logs, test, follow-logs, restart, restore")
        sys.exit(1)

if __name__ == "__main__":
    main() 