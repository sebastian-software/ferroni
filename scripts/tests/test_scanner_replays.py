"""Regression tests for the scanner replay lane's admission, merge and interpretation."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).resolve().parents[1] / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


replays = load("scanner_replays", "compare-scanner-replays.py")

CASE = "json_scanner/document"


def log(*lines):
    return "\n".join(lines) + "\n"


def note(kind, benchmark, engine, **extra):
    return f"{kind} " + json.dumps(dict({"id": benchmark, "engine": engine, "calls": 7}, **extra))


def estimate(ns, low=None, high=None):
    return {"ns": ns, "lower_ns": low or ns * 0.99, "upper_ns": high or ns * 1.01}


class EligibilityTests(unittest.TestCase):
    def record(self, text, engine):
        return replays.eligibility_record(CASE, engine, replays.parse_notes(text), replays.tested(text))

    def test_validated_and_tested_replay_is_timed(self):
        text = log(note("VALIDATED", f"{CASE}_c", "c"), f"Testing {CASE}_c", "Success")
        self.assertEqual(self.record(text, "c")["status"], "eligible")

    def test_validated_replay_whose_test_iteration_failed_is_an_error(self):
        text = log(note("VALIDATED", f"{CASE}_c", "c"), f"Testing {CASE}_c")
        self.assertEqual(self.record(text, "c")["status"], "execution_error")

    def test_rejected_pattern_is_reported_with_its_reason(self):
        text = log(note("UNSUPPORTED", f"{CASE}_pcre2", "pcre2", reason="PCRE2 at offset 6"))
        record = self.record(text, "pcre2")
        self.assertEqual((record["status"], record["reason"]), ("unsupported", "PCRE2 at offset 6"))

    def test_empty_capture_difference_is_not_timed(self):
        text = log(note("EQUIVALENT", f"{CASE}_onigmo", "onigmo", empty_capture_differences=3),
                   f"Testing {CASE}_onigmo", "Success")
        self.assertEqual(self.record(text, "onigmo")["status"], "empty_capture_difference")

    def test_silent_bench_is_a_harness_gap(self):
        self.assertEqual(self.record(log(), "fancy_regex")["status"], "missing")

    def test_duplicate_setup_records_are_rejected(self):
        line = note("VALIDATED", CASE, "rust")
        with self.assertRaises(RuntimeError):
            replays.parse_notes(log(line, line))

    def test_oniguruma_mismatch_and_harness_gaps_stop_timings(self):
        records = [{"case": CASE, "engine": "c", "status": "unsupported"},
                   {"case": CASE, "engine": "rust", "status": "empty_capture_difference"},
                   {"case": CASE, "engine": "pcre2", "status": "missing"}]
        self.assertEqual(replays.fatal_preflight(records), records)

    def test_comparator_gaps_remain_reportable(self):
        records = [{"case": CASE, "engine": "pcre2_jit", "status": "unsupported"},
                   {"case": CASE, "engine": "onigmo", "status": "empty_capture_difference"},
                   {"case": CASE, "engine": "rust", "status": "eligible"}]
        self.assertEqual(replays.fatal_preflight(records), [])


class SummaryTests(unittest.TestCase):
    def measurements(self, rust, other, engine="c"):
        rows = []
        for rep, (base, value) in enumerate(zip(rust, other)):
            rows.append({"case": CASE, "engine": "rust", "repetition": rep, "status": "measured", "estimate": base})
            rows.append({"case": CASE, "engine": engine, "repetition": rep, "status": "measured", "estimate": value})
        return rows

    def test_ratios_divide_within_each_repetition(self):
        rows = self.measurements([estimate(10), estimate(20), estimate(10)],
                                 [estimate(30), estimate(60), estimate(30)])
        ratio = replays.summarize([CASE], ["rust", "c"], rows, [0, 1, 2])[0]["ratios"]["c"]
        self.assertEqual((ratio["median"], ratio["range"]), (3, [3, 3]))
        self.assertEqual(ratio["conclusion"], "Ferroni faster in every run interval")

    def test_overlapping_intervals_are_inconclusive(self):
        rows = self.measurements([estimate(10, 9, 11)] * 3, [estimate(11, 10, 12)] * 3)
        ratio = replays.summarize([CASE], ["rust", "c"], rows, [0, 1, 2])[0]["ratios"]["c"]
        self.assertEqual(ratio["conclusion"], "inconclusive")

    def test_one_repetition_cannot_establish_a_direction(self):
        rows = self.measurements([estimate(10)], [estimate(200)])
        ratio = replays.summarize([CASE], ["rust", "c"], rows, [0])[0]["ratios"]["c"]
        self.assertEqual(ratio["conclusion"], "inconclusive")

    def test_missing_repetition_leaves_the_engine_out(self):
        rows = self.measurements([estimate(10)] * 3, [estimate(30)] * 3)
        rows = [row for row in rows if not (row["engine"] == "c" and row["repetition"] == 2)]
        self.assertEqual(replays.summarize([CASE], ["rust", "c"], rows, [0, 1, 2])[0]["ratios"], {})


class MergeTests(unittest.TestCase):
    ENGINES = ["rust", "c"]

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def shard(self, run, skip=(), **changes):
        directory = self.root / f"shard-{run}"
        directory.mkdir()
        context = {key: None for key in replays.SHARED_CONTEXT}
        context.update({"format_version": 1, "commit": "0" * 40, "git_status": "", "rustc": "rustc 1.94.0",
                        "cc": "cc", "build_profile": "bench", "engines": self.ENGINES,
                        "profiles": {engine: replays.PROFILES[engine] for engine in self.ENGINES},
                        "cases": [CASE], "repetitions": 3, "diagnostic": True, "machine": "arm64",
                        "only_run": run, "platform": "test", "host_note": "test", "binaries": {"bench": "1"}})
        context.update(changes)
        (directory / "context.json").write_text(json.dumps(context))
        (directory / "eligibility.json").write_text(json.dumps(
            [{"case": CASE, "engine": engine, "status": "eligible", "calls": 7} for engine in self.ENGINES]))
        for name in ("Cargo.lock", "source-snapshot.tar.gz"):
            (directory / name).write_text("same")
        (directory / "validation").mkdir()
        measurements = []
        for engine in self.ENGINES:
            if engine in skip:
                continue
            relative = f"raw/run-{run}/{CASE}/{engine}"
            criterion = directory / relative / "group" / "new"
            criterion.mkdir(parents=True)
            (criterion / "benchmark.json").write_text(json.dumps({"full_id": replays.bench_id(CASE, engine)}))
            ns = 10.0 if engine == "rust" else 20.0
            (criterion / "estimates.json").write_text(json.dumps(
                {"mean": {"point_estimate": ns, "confidence_interval": {"lower_bound": ns - 1, "upper_bound": ns + 1}}}))
            (criterion / "sample.json").write_text(json.dumps({"sampling_mode": "Linear", "iters": [1, 2], "times": [1, 2]}))
            measurements.append({"case": CASE, "engine": engine, "repetition": run - 1, "raw": relative,
                                 "status": "measured", "estimate": replays.estimate(directory / relative,
                                                                                    replays.bench_id(CASE, engine))})
        (directory / "measurements.json").write_text(json.dumps(measurements))
        (directory / "order.json").write_text("[]")
        return directory

    def test_complete_shards_merge_into_one_report(self):
        shards = [self.shard(run) for run in (3, 1, 2)]
        replays.merge(self.root / "merged", shards)
        summary = json.loads((self.root / "merged" / "summary.json").read_text())
        self.assertEqual(summary["cases"][0]["ratios"]["c"]["median"], 2)

    def test_missing_cell_rejects_the_merge(self):
        shards = [self.shard(1), self.shard(2, skip=("c",)), self.shard(3)]
        with self.assertRaises(SystemExit):
            replays.merge(self.root / "merged", shards)
        self.assertFalse((self.root / "merged").exists())

    def test_different_toolchain_rejects_the_merge(self):
        shards = [self.shard(1), self.shard(2, rustc="rustc 1.95.0"), self.shard(3)]
        with self.assertRaisesRegex(SystemExit, "rustc"):
            replays.merge(self.root / "merged", shards)

    def test_missing_repetition_rejects_the_merge(self):
        with self.assertRaises(SystemExit):
            replays.merge(self.root / "merged", [self.shard(1), self.shard(3)])

    def test_edited_estimate_rejects_the_merge(self):
        shards = [self.shard(run) for run in (1, 2, 3)]
        path = shards[1] / "measurements.json"
        rows = json.loads(path.read_text())
        rows[1]["estimate"]["ns"] = 20.5
        path.write_text(json.dumps(rows))
        with self.assertRaisesRegex(SystemExit, "retained Criterion"):
            replays.merge(self.root / "merged", shards)


class CaseSetTests(unittest.TestCase):
    def test_every_replay_of_the_lane_runs_both_lanes(self):
        cases = replays.lane_cases()
        self.assertEqual(len(cases), 2 * len(replays.REPLAYS))
        self.assertIn("tsx_scanner/cold_document", cases)
        self.assertEqual(replays.bench_id("tsx_scanner/cold_document", "c"), "tsx_scanner/cold_document_c")
        self.assertEqual(replays.bench_id("json_scanner/document", "rust"), "json_scanner/document")

    def test_a_lane_the_bench_does_not_register_stops_timings(self):
        notes = replays.parse_notes(log(note("VALIDATED", CASE, "rust"), f"Testing {CASE}", "Success"))
        records = [replays.eligibility_record("json_scanner/cold_document", engine, notes, set())
                   for engine in ("rust", "c")]
        self.assertEqual(replays.fatal_preflight(records), records)


if __name__ == "__main__":
    unittest.main()
