#!/usr/bin/env python3
"""
Lightweight development HTTP server for Web Flasher with zero-caching headers.
Ensures Chrome/Edge always fetch the latest HTML/JS/CSS assets immediately.
"""

import http.server
import socketserver
import os
import sys

PORT = 8888

class NoCacheHTTPRequestHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-cache, no-store, must-revalidate")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

def run():
    web_dir = os.path.dirname(os.path.abspath(__file__))
    os.chdir(web_dir)
    handler = NoCacheHTTPRequestHandler
    
    # Allow address reuse to avoid OSError 48
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("", PORT), handler) as httpd:
        print(f"Web Flasher server running at http://localhost:{PORT}")
        print("Press Ctrl+C to stop.")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nServer stopped.")

if __name__ == "__main__":
    run()
