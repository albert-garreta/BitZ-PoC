#!/usr/bin/env python3
"""Offline synthetic parser tests: no subprocesses, executables or proving."""

import importlib.util
import json
from pathlib import Path
import unittest


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("forest_profile", HERE / "forest-profile.py")
PROFILE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROFILE)
PARSER = PROFILE.load_module("saved_forest_test_parser", PROFILE.PARSER)


def stage(name, span_id, ancestors, path, elapsed):
    return json.dumps(dict(event="stage", name=name, span_id=span_id,
                           ancestor_ids=ancestors, path=path, elapsed_ms=elapsed, fields={}))


def block(index, *, retained=False, drop=(), extra=()):
    # Independent fixture following the source's execution order, rather than
    # constructing the log from the parser's expected-label set.
    labels = ["nibble rows", "L3 value tables", "levels 5..13 build", "levels ≥4"]
    labels += [f"level {level}" for level in range(12, 4, -1)]
    labels += ["L4 retained" if retained else "L4 rebuild", "level 4"]
    for level in [3, 2, 1, 0]:
        if level == 2:
            labels.append("L2 bit round 1")
        elif level < 2:
            labels.append(f"L{level} one-pass bit rounds")
        labels += [f"L{level} {suffix}" for suffix in ["tables", "jit round", "jit fold", "dense tail"]]
        labels += list(extra) if level == 0 else []
        labels.append(f"level {level}")
    value = 100 if index == 0 else 1
    lines = [f"bitz: {label:<24} {value}ms  0.0 MB faulted" for label in labels if label not in drop]
    root = index * 10 + 1
    lines += [stage(PARSER.FOREST, root + 2, [root, root + 1, root + 2],
                    [PARSER.PROVE, "falcon_algebraic:bitz", PARSER.FOREST], value * 50),
              stage(PARSER.PROVE, root, [root], [PARSER.PROVE], value * 60),
              # A verifier descendant with a large duration must not be included.
              stage("falcon_bridge:integer_folds", root + 4, [root + 3, root + 4],
                    ["falcon_algebraic:verify", "falcon_bridge:integer_folds"], 9999),
              stage("falcon_algebraic:verify", root + 3, [root + 3],
                    ["falcon_algebraic:verify"], 10000)]
    return "\n".join(lines)


class ForestProfileTests(unittest.TestCase):
    def setUp(self):
        self.job = dict(id="synthetic", degree=512, protocol="canonical", seed=42, warmup=1, iterations=1)
        self.trials = [dict(trial="warmup"), dict(trial="sample")]
        self.manifest = dict(batch=1024, security=128)

    def parse(self, text, *, job=None, manifest=None, overrides=None):
        return PROFILE.parse_complete_trace(text, job or self.job, self.trials,
                                            manifest or self.manifest, overrides or {}, PARSER)

    def test_complete_degrees_exclude_warmup_and_verifier(self):
        text = block(0) + "\n" + block(1)
        for degree in [512, 1024]:
            samples = self.parse(text, job=dict(self.job, degree=degree))
            self.assertEqual(len(samples), 1)
            self.assertEqual(samples[0]["root_span_id"], 11)
            self.assertEqual(samples[0]["derived_ms"]["jit_folds_ms"], 4)
            self.assertEqual(samples[0]["trace_ms"]["nibble rows"], 1)
            self.assertFalse(any("verify" in path for path in samples[0]["stage_ms"]))

    def test_dropped_l2_jit_fold_is_rejected(self):
        text = block(0) + "\n" + block(1, drop={"L2 jit fold"})
        with self.assertRaisesRegex(ValueError, "missing=.*L2 jit fold"):
            self.parse(text)

    def test_dropped_warmup_timer_is_also_rejected(self):
        text = block(0, drop={"L2 jit fold"}) + "\n" + block(1)
        with self.assertRaisesRegex(ValueError, "forest block 0 label mismatch"):
            self.parse(text)

    def test_dropped_required_labels_never_become_zero(self):
        for label in ["nibble rows", "L3 value tables", "levels 5..13 build", "levels ≥4",
                      "level 12", "level 4", "L4 rebuild", "L3 tables", "L0 jit round",
                      "L1 dense tail", "L2 bit round 1", "L1 one-pass bit rounds",
                      "L0 one-pass bit rounds"]:
            with self.subTest(label=label), self.assertRaisesRegex(ValueError, "label mismatch"):
                self.parse(block(0) + "\n" + block(1, drop={label}))

    def test_level_zero_alone_is_not_a_complete_profile(self):
        root = 11
        text = block(0) + "\n" + "\n".join([
            "bitz: level 0 1ms  0 MB faulted",
            stage(PARSER.FOREST, 13, [root, 12, 13], [PARSER.PROVE, "falcon_algebraic:bitz", PARSER.FOREST], 2),
            stage(PARSER.PROVE, root, [root], [PARSER.PROVE], 3),
        ])
        with self.assertRaisesRegex(ValueError, "label mismatch"):
            self.parse(text)

    def test_retained_policy_requires_exact_alternative(self):
        text = block(0, retained=True) + "\n" + block(1, retained=True)
        samples = self.parse(text, overrides={"BITZ_FOREST_L4": "retain"})
        self.assertEqual(samples[0]["derived_ms"]["l4_retained_ms"], 1)
        self.assertEqual(samples[0]["derived_ms"]["l4_rebuild_ms"], 0)
        with self.assertRaisesRegex(ValueError, "label mismatch"):
            self.parse(text)
        with self.assertRaisesRegex(ValueError, "label mismatch"):
            self.parse(block(0) + "\n" + block(1, extra=["L4 retained"]))

    def test_wrong_geometry_or_new_label_fails_closed(self):
        text = block(0) + "\n" + block(1)
        for changes in [dict(batch=513), dict(security=100)]:
            with self.assertRaisesRegex(ValueError, "unsupported forest geometry"):
                self.parse(text, manifest=dict(self.manifest, **changes))
        for changes in [dict(degree=2048), dict(protocol="factored")]:
            with self.assertRaisesRegex(ValueError, "unsupported forest geometry"):
                self.parse(text, job=dict(self.job, **changes))
        for label in ["levels 5..12 build", "L0 bit round 1", "unrecognized new timer"]:
            with self.subTest(label=label), self.assertRaisesRegex(ValueError, "label mismatch"):
                self.parse(block(0) + "\n" + block(1, extra=[label]))

    def test_legacy_duplicate_and_missing_root_checks_are_retained(self):
        with self.assertRaisesRegex(ValueError, "duplicate forest label"):
            self.parse(block(0) + "\n" + block(1, extra=["L2 jit fold"]))
        with self.assertRaisesRegex(ValueError, "proof/forest count mismatch"):
            self.parse(block(0))


if __name__ == "__main__":
    unittest.main()
