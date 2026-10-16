"""Admission, merge and capture contracts; no timing assertions."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load("scanner_clients", ROOT / "scripts/compare-scanner-clients.py")
capture = load("capture_clients", ROOT / "benches/scanner_clients/capture.py")


class Reporting(unittest.TestCase):
    def setUp(self):
        self.catalog = runner.corpus()
        self.records = [dict(case=c["id"], mode=m, repetition=1, status="measured",
                             lanes={l: dict(ns=10, interval=dict(confidence_level=.95,
                                      lower_bound=9, upper_bound=11), raw=l + "/new") for l in c["lanes"]})
                        for c in self.catalog["cases"] for m in c["modes"]]

    def test_committed_provenance_and_exact_cell_set(self):
        runner.admit(self.records, self.catalog, [1])
        self.assertEqual(sum(len(r["lanes"]) for r in self.records), 9)

    def test_missing_duplicate_failed_or_missing_lane_is_rejected(self):
        candidates = [self.records[:-1], self.records + [self.records[0]]]
        failed, incomplete = copy.deepcopy(self.records), copy.deepcopy(self.records)
        failed[0]["status"] = "timeout"
        incomplete[0]["lanes"].pop("steady")
        for rows in candidates + [failed, incomplete]:
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                runner.admit(rows, self.catalog, [1])

    def test_nonfinite_times_and_inverted_intervals_are_rejected(self):
        for value in (float("nan"), float("inf"), 0, -1):
            rows = copy.deepcopy(self.records)
            rows[0]["lanes"]["compile"]["ns"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                runner.admit(rows, self.catalog, [1])
        self.records[0]["lanes"]["compile"]["interval"]["upper_bound"] = 8
        with self.assertRaises(ValueError):
            runner.admit(self.records, self.catalog, [1])

    def shards(self, base):
        shards = []
        for repetition in (1, 2, 3):
            shard = base / str(repetition)
            shard.mkdir()
            runner.write(shard / "catalog.json", self.catalog)
            context = {key: "identical" for key in ("commit", "platform", "machine", "binary_sha256", "source_sha256", "native_source_sha256",
                       "cargo_lock_sha256", "rustc", "profile", "profile_env", "rustflags", "samples", "seconds", "warmup", "diagnostic")}
            context.update(only_run=repetition, catalog_sha256=runner.digest(shard / "catalog.json"))
            runner.write(shard / "context.json", context)
            rows = copy.deepcopy(self.records)
            for row in rows:
                row["repetition"] = repetition
                for lane, value in row["lanes"].items():
                    directory = shard / "raw" / row["case"] / row["mode"] / value["raw"]
                    directory.mkdir(parents=True)
                    runner.write(directory / "benchmark.json", dict(full_id=f"scanner_clients/{row['case']}/{lane}/{row['mode']}"))
                    runner.write(directory / "estimates.json", dict(mean=dict(point_estimate=value["ns"], confidence_interval=value["interval"])))
            runner.write(shard / "measurements.json", rows)
            runner.write(shard / "eligibility.json", [dict(case=r["case"], mode=r["mode"], status="eligible", c_oracle=True) for r in rows])
            shards.append(shard)
        return shards

    def test_merge_retains_all_evidence_without_an_aggregate(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            runner.merge(base / "merged", self.shards(base))
            self.assertEqual(len(json.loads((base / "merged/summary.json").read_text())), 9)
            self.assertTrue((base / "merged/run-3/raw/related/uncached/compile/new/estimates.json").is_file())

    def test_merge_rejects_modified_raw_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            shards = self.shards(base)
            path = shards[0] / "raw/tsx/recorded/compile/new/estimates.json"
            row = json.loads(path.read_text())
            row["mean"]["point_estimate"] = 10.5
            runner.write(path, row)
            with self.assertRaisesRegex(ValueError, "retained Criterion"):
                runner.merge(base / "merged", shards)

    def test_merge_rejects_mixed_architectures_and_repetitions(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            shards = self.shards(base)
            with self.assertRaisesRegex(ValueError, "repetitions"):
                runner.merge(base / "merged", [shards[0]] * 3)
            path = shards[1] / "context.json"
            context = json.loads(path.read_text())
            context["machine"] = "other architecture"
            runner.write(path, context)
            with self.assertRaisesRegex(ValueError, "machine"):
                runner.merge(base / "merged", shards)


class Capture(unittest.TestCase):
    def trace(self, rows):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "raw.jsonl"
            path.write_text("\n".join(json.dumps(r) for r in rows))
            return capture.compact(path)

    def rows(self):
        return [dict(kind="create", scanner=9, cache=7, patterns=["x"], syntax="Oniguruma", options=256, prefilter=True, warmup=32),
                *[dict(kind="find", scanner=9, subject_id=i, subject="xx", start=0, options=0, expected=None) for i in (40, 41, 40)],
                dict(kind="drop", scanner=9), dict(kind="drop_cache", cache=7)]

    def test_equal_subjects_keep_their_original_identities(self):
        trace = self.trace(self.rows())
        self.assertEqual(trace["subjects"], ["xx", "xx"])
        self.assertEqual([r[2] for r in trace["events"] if r[0] == "find"], [0, 1, 0])
        self.assertEqual(trace["events"][-2:], [["drop", 0], ["drop_cache", 0]])

    def test_incomplete_lifetimes_and_changed_configuration_are_rejected(self):
        rows = self.rows()
        with self.assertRaisesRegex(ValueError, "destruction"):
            self.trace(rows[:-2])
        rows[0]["warmup"] = 0
        with self.assertRaisesRegex(ValueError, "configuration"):
            self.trace(rows)


if __name__ == "__main__":
    unittest.main()
