"""Throwaway web server for the e2e run: answers every GET with the given HTTP status and a small
HTML page, like a local program's error page (400) or a short-lived web page (200).
Usage: python3 status-page.py <port> <status>"""
import http.server
import sys

STATUS = int(sys.argv[2])


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = f"<!doctype html><html><head><title>Status {STATUS}</title></head><body><h1>{STATUS}</h1></body></html>".encode()
        self.send_response(STATUS)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
