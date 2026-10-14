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
