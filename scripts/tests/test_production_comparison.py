"""Regression tests for comparison admission and interpretation."""

import importlib.util
import math
from pathlib import Path
import unittest


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).resolve().parents[1] / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


comparison = load("production_comparison", "compare-production.py")


def estimate(ns, low=None, high=None):
    return {"steady": {"ns": ns, "lower_ns": low or ns * 0.99, "upper_ns": high or ns * 1.01}}


class ComparisonTests(unittest.TestCase):
    def test_timed_environments_share_one_size_per_repetition(self):
        short = {"PATH": "/usr/bin", "PRODUCTION_CASE": "uuid_v4", "PRODUCTION_ENGINE": "c"}
        long = dict(short, PRODUCTION_CASE="ferrolink_attribute_split", PRODUCTION_ENGINE="fancy_regex_seek")
        sizes = []
        for repetition in range(len(comparison.ALIGNMENT_OFFSETS)):
            size = comparison.aligned_size([short, long], repetition)
            padded = [comparison.aligned_environment(env, size) for env in (short, long)]
            self.assertEqual({comparison.environment_size(env) for env in padded}, {size})
            self.assertEqual(padded[1]["PRODUCTION_ENGINE"], "fancy_regex_seek")
            sizes.append(size)
        self.assertEqual([size - sizes[0] for size in sizes], list(comparison.ALIGNMENT_OFFSETS))
        self.assertEqual(sizes[0] % comparison.ALIGNMENT_ROUNDING, 0)
        with self.assertRaises(ValueError):
            comparison.aligned_environment(long, comparison.environment_size(short))

    def test_ferroni_oracle_mismatch_stops_timings(self):
        records = [{"case": "case", "engine": "rust", "status": "semantic_difference"}]
        self.assertEqual(comparison.fatal_preflight(records), records)

    def test_fallible_ferroni_profile_mismatch_also_stops_timings(self):
        records = [{"case": "case", "engine": "rust_erroraware", "status": "semantic_difference"}]
        self.assertEqual(comparison.fatal_preflight(records), records)

    def test_execution_errors_and_timeouts_stop_timings_for_any_engine(self):
        records = [{"case": "one", "engine": "c", "status": "timeout"},
                   {"case": "two", "engine": "regex", "status": "execution_error"}]
        self.assertEqual(comparison.fatal_preflight(records), records)

    def test_comparator_capability_gaps_remain_reportable(self):
        records = [{"case": "one", "engine": "regex", "status": "syntax_rejected"},
                   {"case": "two", "engine": "onigmo", "status": "semantic_difference"},
                   {"case": "three", "engine": "rust", "status": "pending"}]
        self.assertEqual(comparison.fatal_preflight(records), [])

    def test_missing_repetition_excludes_the_task(self):
        cases = [{"id": "complete", "family": "one"}, {"id": "incomplete", "family": "two"}]
        values = {}
        for rep in range(3):
            for case in cases:
                values[(case["id"], "rust", rep)] = estimate(10)
                if case["id"] != "incomplete" or rep != 2:
                    values[(case["id"], "c", rep)] = estimate(20)
        summary = comparison.summarize_pair(cases, values, "c", 3, "steady")
        self.assertEqual([row["case"] for row in summary["cases"]], ["complete"])

    def test_family_weight_does_not_follow_variant_count(self):
        cases = [{"id": str(n), "family": "many"} for n in range(4)] + [{"id": "single", "family": "other"}]
        values = {}
        for rep in range(3):
            for case in cases:
                values[(case["id"], "rust", rep)] = estimate(10)
                values[(case["id"], "c", rep)] = estimate(40 if case["family"] == "many" else 10)
        summary = comparison.summarize_pair(cases, values, "c", 3, "steady")
        self.assertTrue(math.isclose(summary["family_weighted_ratio"], 2.0))

    def test_reversed_runs_are_inconclusive(self):
        cases = [{"id": "case", "family": "one"}]
        values = {}
        for rep, ns in enumerate((20, 20, 5)):
            values[("case", "rust", rep)] = estimate(10)
            values[("case", "c", rep)] = estimate(ns)
        result = comparison.summarize_pair(cases, values, "c", 3, "steady")["cases"][0]
        self.assertEqual(result["conclusion"], "inconclusive")

    def test_overlapping_intervals_are_inconclusive(self):
        values = {}
        for rep in range(3):
            values[("case", "rust", rep)] = estimate(10, 9, 11)
            values[("case", "c", rep)] = estimate(11, 10, 12)
        result = comparison.summarize_pair([{"id": "case", "family": "one"}], values, "c", 3, "steady")["cases"][0]
        self.assertEqual(result["conclusion"], "inconclusive")

    def test_one_run_cannot_establish_a_direction(self):
        values = {("case", "rust", 0): estimate(10), ("case", "c", 0): estimate(200)}
        result = comparison.summarize_pair([{"id": "case", "family": "one"}], values, "c", 1, "steady")["cases"][0]
        self.assertEqual(result["conclusion"], "inconclusive")

    def test_balanced_order_is_repeatable_and_covers_each_pair(self):
        engines = ("rust", "c", "regex")
        first = list(comparison.ordered_pairs(["a", "b"], engines, 0, 12))
        self.assertEqual(first, list(comparison.ordered_pairs(["a", "b"], engines, 0, 12)))
        self.assertEqual(set(first), {(case, engine) for case in ("a", "b") for engine in engines})
        self.assertNotEqual(first, list(comparison.ordered_pairs(["a", "b"], engines, 1, 12)))


if __name__ == "__main__":
    unittest.main()
