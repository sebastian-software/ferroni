"""Publication must reject incomplete or altered benchmark evidence."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("publication", Path(__file__).resolve().parents[1] / "publish-production.py")
publication = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publication)


class PublicationTests(unittest.TestCase):
    def test_homepage_summary_uses_production_pairs_without_probes_or_pending_cases(self):
        production_pair = {"engine": "c", "lane": "steady", "case_count": 1, "family_count": 1,
                           "family_weighted_ratio": 1.25,
                           "cases": [{"conclusion": "Ferroni faster in every run interval"}]}
        data = {
            "run": {"head_sha": "a" * 40, "html_url": "https://example.com/run", "created_at": "2026-10-14"},
            "cases": [
                {"pattern_origin": "production", "pending_reason": None, "input_count": 3, "family": "logs"},
                {"pattern_origin": "controlled", "pending_reason": None, "input_count": 4, "family": "literal"},
                {"pattern_origin": "production", "pending_reason": "capture mismatch", "input_count": 2, "family": "types"},
            ],
            "platforms": [{"profile": "macos-arm64", "context": {"repetitions": 3},
                           "production_comparisons": [production_pair],
                           "comparisons": [dict(production_pair, case_count=2, family_weighted_ratio=20)]}],
        }
        summary = publication.overview(data)
        self.assertEqual((summary["case_count"], summary["input_count"], summary["family_count"]), (1, 3, 1))
        row = summary["platforms"][0]["comparisons"][0]
        self.assertEqual((row["case_count"], row["family_weighted_ratio"]), (1, 1.25))
        self.assertEqual((row["ferroni_cases"], row["comparator_cases"], row["inconclusive_cases"]), (1, 0, 0))

    def test_last_bit_platform_math_differences_are_tolerated(self):
        self.assertTrue(publication.equivalent({"ratio": 1.0000000000000002}, {"ratio": 1.0}))
        self.assertFalse(publication.equivalent({"ratio": 1.001}, {"ratio": 1.0}))
        self.assertFalse(publication.equivalent({"count": 12}, {"count": 13}))

    def test_diagnostic_runs_cannot_be_published(self):
        with self.assertRaisesRegex(ValueError, "Diagnostic"):
            publication.verify_context({"diagnostic": True}, {}, {}, "macos-arm64")

    def test_dirty_builds_cannot_be_published(self):
        with self.assertRaisesRegex(ValueError, "clean"):
            publication.verify_context({"diagnostic": False, "git_status": " M src/regexec.rs"}, {}, {}, "macos-arm64")

    def test_failed_workflow_cannot_be_published(self):
        with self.assertRaisesRegex(ValueError, "successfully"):
            publication.validate_receipt({"conclusion": "failure"})

    def test_retained_estimate_must_match_raw_criterion_data(self):
        with tempfile.TemporaryDirectory() as scratch:
            raw = Path(scratch) / "new"
            raw.mkdir()
            (raw / "benchmark.json").write_text(json.dumps({"full_id": "production_regex/case/steady/c"}))
            (raw / "estimates.json").write_text(json.dumps({"mean": {"point_estimate": 10,
                "confidence_interval": {"confidence_level": 0.95, "lower_bound": 9, "upper_bound": 11}}}))
            with self.assertRaisesRegex(ValueError, "differs from raw"):
                publication.verify_raw(Path(scratch), {"raw": ".", "case": "case", "engine": "c"}, "steady",
                    {"raw_directory": "new", "ns": 20, "lower_ns": 9, "upper_ns": 11}, 30)

    def test_evidence_paths_cannot_escape_the_artifact(self):
        with self.assertRaisesRegex(ValueError, "Unsafe"):
            publication.verify_raw(Path("."), {"raw": "../elsewhere"}, "steady", {"raw_directory": "new"}, 30)

    def test_altered_raw_sample_cannot_keep_the_original_estimate(self):
        with tempfile.TemporaryDirectory() as scratch:
            raw = Path(scratch) / "new"
            raw.mkdir()
            (raw / "benchmark.json").write_text(json.dumps({"full_id": "production_regex/case/steady/c"}))
            (raw / "estimates.json").write_text(json.dumps({"mean": {"point_estimate": 10,
                "confidence_interval": {"confidence_level": 0.95, "lower_bound": 9, "upper_bound": 11}}}))
            sample = {"iters": [2] * 30, "times": [20] * 30}
            (raw / "sample.json").write_text(json.dumps(sample))
            record = {"raw": ".", "case": "case", "engine": "c"}
            estimate = {"raw_directory": "new", "ns": 10, "lower_ns": 9, "upper_ns": 11}
            publication.verify_raw(Path(scratch), record, "steady", estimate, 30)
            sample["times"][0] = 200
            (raw / "sample.json").write_text(json.dumps(sample))
            with self.assertRaisesRegex(ValueError, "normalized raw samples"):
                publication.verify_raw(Path(scratch), record, "steady", estimate, 30)


if __name__ == "__main__":
    unittest.main()
