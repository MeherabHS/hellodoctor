"""Offline producer contracts. No sockets, backend, Redis or providers are used."""
import errno
import http.client
import importlib.util
import json
import os
import ssl
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location("attack_local", Path(__file__).with_name("attack-local.py"))
ATTACK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ATTACK)


class TransportTaxonomyTests(unittest.TestCase):
    def check(self, error, phase, category):
        result = ATTACK.transport_failure(error, phase)
        self.assertEqual(result["transport_error_type"], category)
        self.assertEqual(result["transport_phase"], phase)
        self.assertTrue(result["transport_error"])
        self.assertNotIn("message", result)
        self.assertNotIn("synthetic-private-token", json.dumps(result))

    def test_connect_timeout(self):
        self.check(TimeoutError("synthetic-private-token"), "connect", "CONNECT_TIMEOUT")

    def test_read_timeout(self):
        self.check(TimeoutError(), "response_headers", "READ_TIMEOUT")

    def test_write_timeout_is_not_mislabeled_read_timeout(self):
        self.check(TimeoutError(), "request_write", "WRITE_TIMEOUT")

    def test_connection_reset(self):
        self.check(ConnectionResetError(errno.ECONNRESET, "synthetic-private-token"), "response_headers", "CONNECTION_RESET")

    def test_connection_refused(self):
        self.check(ConnectionRefusedError(errno.ECONNREFUSED, "synthetic-private-token"), "connect", "CONNECTION_REFUSED")

    def test_broken_pipe(self):
        self.check(BrokenPipeError(errno.EPIPE, "synthetic-private-token"), "request_write", "BROKEN_PIPE")

    def test_eof_before_headers(self):
        self.check(http.client.RemoteDisconnected("synthetic-private-token"), "response_headers", "EOF_BEFORE_RESPONSE")

    def test_eof_during_body(self):
        self.check(http.client.IncompleteRead(b"synthetic-private-token", 40), "response_body", "EOF_DURING_RESPONSE")

    def test_tls_failure(self):
        self.check(ssl.SSLError("synthetic-private-token"), "connect", "TLS_ERROR_IF_APPLICABLE")

    def test_posix_resource_errors(self):
        for code in (errno.EMFILE, errno.ENFILE, errno.ENOBUFS, errno.ENOMEM, errno.EADDRNOTAVAIL, errno.EADDRINUSE):
            with self.subTest(code=code):
                self.check(OSError(code, "synthetic-private-token"), "connect", "CLIENT_RESOURCE_ERROR")

    def test_windows_resource_error_preserves_os_code(self):
        error = OSError("synthetic-private-token")
        error.winerror = 10055
        self.check(error, "connect", "CLIENT_RESOURCE_ERROR")
        self.assertEqual(ATTACK.transport_failure(error, "connect")["os_winerror"], 10055)

    def test_windows_timeout_and_reset_codes(self):
        for code, category in ((10060, "CONNECT_TIMEOUT"), (10054, "CONNECTION_RESET"), (10061, "CONNECTION_REFUSED")):
            with self.subTest(code=code):
                error = OSError("synthetic-private-token")
                error.winerror = code
                self.check(error, "connect", category)

    def test_unknown_is_not_attributed_to_server(self):
        self.check(http.client.BadStatusLine("synthetic-private-token"), "response_headers", "UNKNOWN_TRANSPORT_ERROR")


class BoundedSchedulerTests(unittest.TestCase):
    def test_all_30000_identities_once_and_at_most_32_outstanding(self):
        state = {"live": 0, "peak": 0}

        class Future:
            def __init__(self, function, index):
                self.function, self.index = function, index

            def result(self):
                state["live"] -= 1
                return self.function(self.index)

        class Executor:
            def __init__(self, max_workers):
                self.asserted_workers = max_workers

            def __enter__(self):
                return self

            def __exit__(self, *args):
                return False

            def submit(self, function, index):
                state["live"] += 1
                state["peak"] = max(state["peak"], state["live"])
                return Future(function, index)

        def one_completion(pending, return_when):
            future = next(iter(pending))
            return {future}, pending - {future}

        with patch.object(ATTACK.concurrent.futures, "ThreadPoolExecutor", Executor), patch.object(ATTACK.concurrent.futures, "wait", one_completion):
            results = list(ATTACK.bounded_results(lambda index: index, 30000, 32))
        self.assertEqual(len(results), 30000)
        self.assertEqual(set(results), set(range(30000)))
        self.assertEqual(state["live"], 0)
        self.assertEqual(state["peak"], 32)

    def test_fewer_requests_than_workers(self):
        self.assertEqual(list(ATTACK.bounded_results(lambda index: index, 1, 32)), [0])

    def test_worker_failure_is_not_swallowed(self):
        def broken(_):
            raise RuntimeError("synthetic worker failure")
        with self.assertRaisesRegex(RuntimeError, "synthetic worker failure"):
            list(ATTACK.bounded_results(broken, 1, 1))


class ProducerTelemetryTests(unittest.TestCase):
    def invoke(self, connection):
        with tempfile.TemporaryDirectory(dir=os.environ["TEMP"]) as directory:
            root = Path(directory)
            manifest, output = root / "manifest.json", root / "output.json"
            manifest.write_text(json.dumps({"synthetic_fixture": True, "base_url": "http://127.0.0.1:18080", "cases": {"test": {"path": "/api/v1/auth/patient/login", "body": {"email": "synthetic@example.invalid"}}}}), encoding="utf-8")
            arguments = ["attack-local.py", "--manifest", str(manifest), "--case", "test", "--count", "1", "--workers", "1", "--keys", "none", "--output", str(output)]
            with patch.object(sys, "argv", arguments), patch.object(ATTACK.http.client, "HTTPConnection", return_value=connection):
                ATTACK.main()
            return json.loads(output.read_text(encoding="utf-8"))

    def test_http_response_is_separate_from_completed_attempt(self):
        connection = Mock()
        connection.connect.side_effect = ConnectionRefusedError(errno.ECONNREFUSED, "synthetic-private-token")
        result = self.invoke(connection)
        self.assertEqual(result["request_count"], 1)
        self.assertEqual(result["http_response_count"], 0)
        self.assertEqual(result["http_responses_per_second"], 0)
        self.assertGreater(result["completed_attempts_per_second"], 0)
        self.assertEqual(result["transport_error_types"], {"CONNECTION_REFUSED": 1})
        self.assertEqual(result["requests"][0]["transport_phase"], "connect")
        self.assertIsNone(result["http_p95_ms"])
        self.assertNotIn("synthetic-private-token", json.dumps(result))
        connection.close.assert_called_once()

    def test_body_failure_preserves_received_status_without_claiming_completion(self):
        connection = Mock()
        response = connection.getresponse.return_value
        response.status = 503
        response.getheader.return_value = None
        response.read.side_effect = ConnectionResetError(errno.ECONNRESET, "synthetic-private-token")
        result = self.invoke(connection)
        self.assertEqual(result["http_statuses"], {"503": 1})
        self.assertEqual(result["http_response_count"], 1)
        self.assertEqual(result["completed_http_response_count"], 0)
        self.assertEqual(result["transport_error_count"], 1)
        self.assertEqual(result["requests"][0]["transport_phase"], "response_body")

    def test_complete_http_records_status_and_attempt_start(self):
        connection = Mock()
        response = connection.getresponse.return_value
        response.status = 429
        response.getheader.return_value = "synthetic-request-id"
        response.read.return_value = b""
        result = self.invoke(connection)
        self.assertEqual(result["completed_http_response_count"], 1)
        self.assertEqual(result["transport_error_count"], 0)
        self.assertEqual(result["requests"][0]["request_id"], "synthetic-request-id")
        self.assertGreaterEqual(result["requests"][0]["started_ms"], 0)
        response.read.assert_called_once_with(64 * 1024)
        self.assertEqual(result["worker_concurrency"], 1)
        self.assertIsNone(result["offered_rps"])

    def test_limits_reject_excess_count_before_any_network_or_manifest_read(self):
        arguments = ["attack-local.py", "--manifest", "absent.json", "--case", "test", "--count", "30001", "--output", "absent-output.json"]
        with patch.object(sys, "argv", arguments), patch.object(ATTACK.http.client, "HTTPConnection") as connection, patch("sys.stderr"):
            with self.assertRaises(SystemExit) as raised:
                ATTACK.main()
            self.assertEqual(raised.exception.code, 2)
            connection.assert_not_called()


if __name__ == "__main__":
    unittest.main(verbosity=2)
