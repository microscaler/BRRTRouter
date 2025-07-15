#!/bin/bash
# BRRTRouter API Testing Tool
# Simple entry point for testing the BRRTRouter API

cd "$(dirname "$0")"
python3 scripts/test_api.py "$@" 