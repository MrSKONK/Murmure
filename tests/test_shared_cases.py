"""Cas partagés avec la version 2 en Rust (v2/core/tests/shared_cases.rs) : les deux corrections doivent
donner exactement le même texte. Régénérer le fichier si un test de test_cleanup.py change."""
import json
import os
import unittest

from src.cleanup import clean, fit_to_context

CASES = os.path.join(os.path.dirname(__file__), "cleanup_cases.json")


class SharedCasesTest(unittest.TestCase):
    def test_cases(self):
        with open(CASES, encoding="utf-8") as f:
            cases = json.load(f)
        for case in cases:
            with self.subTest(case=case):
                if case["fn"] == "clean":
                    got = clean(case["text"], **case["opts"])
                else:
                    got = fit_to_context(case["text"], case["before"], tuple(case["keep_case"]))
                self.assertEqual(got, case["expected"])


if __name__ == "__main__":
    unittest.main()
