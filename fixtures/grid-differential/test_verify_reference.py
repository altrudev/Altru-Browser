#!/usr/bin/env python3
import copy
import unittest
from verify_reference import verify

BASE = [
    {"id": "fr-auto", "template": "210px 80px", "width": 300,
     "children": [{"x": 0, "width": 210, "height": 24}, {"x": 220, "width": 80, "height": 24}]},
    {"id": "auto-fr", "template": "30px 260px", "width": 300,
     "children": [{"x": 0, "width": 30, "height": 24}, {"x": 40, "width": 260, "height": 24}]},
    {"id": "fr-fr", "template": "145px 145px", "width": 300,
     "children": [{"x": 0, "width": 145, "height": 24}, {"x": 155, "width": 145, "height": 24}]},
]

class ReferenceGateTests(unittest.TestCase):
    def test_distinct_valid_reference(self):
        self.assertEqual(len(verify(copy.deepcopy(BASE))), 3)

    def test_reject_equalized_auto(self):
        cases = copy.deepcopy(BASE)
        cases[0]["children"] = copy.deepcopy(cases[2]["children"])
        with self.assertRaises(ValueError):
            verify(cases)

    def test_reject_overflow_and_wrong_gap(self):
        for value in (170, float("nan"), -2):
            cases = copy.deepcopy(BASE)
            cases[0]["children"][1]["x"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                verify(cases)

    def test_reject_missing_case(self):
        with self.assertRaises(ValueError):
            verify(copy.deepcopy(BASE[:2]))

if __name__ == "__main__":
    unittest.main()
