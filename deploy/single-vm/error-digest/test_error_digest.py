"""Unit tests for error_digest.py (no database, docker or network needed).

Run: python -m unittest test_error_digest.py
"""
import unittest
from unittest import mock

import error_digest as d


class MaskTests(unittest.TestCase):
    def test_personal_data_and_secrets_are_masked(self):
        line = (
            "send to jane.doe@example.com failed for +254 711 330 002 and 0711330002 "
            "token=abc123secret Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sflKxwRJSMeKKF2QT4fwpM "
            "id 12345678901"
        )
        out = d.mask(line, limit=1000)
        for leaked in ("jane.doe", "711 330 002", "0711330002", "abc123secret", "eyJhbGci", "12345678901"):
            self.assertNotIn(leaked, out)
        for marker in ("[email]", "[phone]", "[hidden]", "[number]"):
            self.assertIn(marker, out)

    def test_long_text_is_capped_and_whitespace_collapsed(self):
        out = d.mask("a  b\n\nc" + "x" * 500, limit=20)
        self.assertTrue(out.startswith("a b c"))
        self.assertEqual(len(out), 20)


class ReportTests(unittest.TestCase):
    def test_all_clear_subject(self):
        ok = d.Section("Database", lines=["Reachable."])
        subject, text, body = d.build_report([ok])
        self.assertIn("all clear", subject)
        self.assertIn("Reachable.", text)
        self.assertIn("<li>Reachable.</li>", body)

    def test_problems_are_counted_and_html_is_escaped(self):
        bad = d.Section("Backend and app errors")
        bad.flag("error", "<script>x</script> failed")
        warn = d.Section("VM")
        warn.flag("warn", "Disk /: 85% used")
        subject, _, body = d.build_report([bad, warn, d.Section("Database")])
        self.assertIn("2 area(s) need attention", subject)
        self.assertTrue(subject.startswith(d.ICON["error"]))
        self.assertNotIn("<script>", body)

    def test_a_failing_source_does_not_stop_the_report(self):
        def boom():
            raise RuntimeError("cannot reach jane@example.com")

        with mock.patch.object(d, "app_errors", boom), \
             mock.patch.object(d, "psql", return_value=None), \
             mock.patch.object(d, "containers", lambda: d.Section("Containers")), \
             mock.patch.object(d, "vm_health", lambda: d.Section("VM")):
            sections = d.collect()
        self.assertEqual(len(sections), 7)
        failed = sections[0]
        self.assertEqual(failed.status, "unavailable")
        self.assertIn("[email]", failed.lines[0])

    def test_unreachable_database_is_reported_as_an_error(self):
        with mock.patch.object(d, "psql", return_value=None):
            self.assertEqual(d.database().status, "error")
            self.assertEqual(d.app_errors().status, "unavailable")


if __name__ == "__main__":
    unittest.main()
