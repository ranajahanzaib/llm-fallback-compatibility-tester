import unittest
import importlib.util
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread

from fallbackcontract_adapters.protocol import PROTOCOL_VERSION, handle
from fallbackcontract_adapters.ollama import chat


class ProtocolTests(unittest.TestCase):
    def test_handshake_echoes_correlation_id(self):
        response = handle({"id": "handshake-1", "type": "hello"})
        self.assertEqual(response["id"], "handshake-1")
        self.assertEqual(response["protocol_version"], PROTOCOL_VERSION)

    def test_rejects_unknown_provider(self):
        with self.assertRaisesRegex(ValueError, "unsupported Python provider"):
            handle({"id": "r1", "provider": "unknown"})

    def test_shutdown_is_acknowledged(self):
        response = handle({"id": "bye", "type": "shutdown"})
        self.assertTrue(response["shutdown"])
        self.assertEqual(response["id"], "bye")

    @unittest.skipUnless(importlib.util.find_spec("httpx"), "install package dependencies to exercise HTTP adapter")
    def test_ollama_adapter_normalizes_a_local_chat_response(self):
        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                payload=b'{"model":"local-test","message":{"role":"assistant","content":"ok","tool_calls":[]},"prompt_eval_count":4,"eval_count":1,"done_reason":"stop"}'
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            def log_message(self, *_args):
                pass

        server=ThreadingHTTPServer(("127.0.0.1",0),Handler)
        Thread(target=server.serve_forever,daemon=True).start()
        try:
            result=chat({"id":"ollama-1","provider":"ollama","model":"local-test","base_url":f"http://127.0.0.1:{server.server_port}","messages":[{"role":"user","content":"hello"}]})
        finally:
            server.shutdown()
        self.assertEqual(result["id"],"ollama-1")
        self.assertEqual(result["content"],"ok")
        self.assertEqual(result["usage"],{"input_tokens":4,"output_tokens":1})


if __name__ == "__main__":
    unittest.main()
