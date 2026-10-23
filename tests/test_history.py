import json
import os
import tempfile
import unittest

from src.history import History


class HistoryTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = os.path.join(self.dir.name, "sub", "history.json")

    def tearDown(self):
        self.dir.cleanup()

    def test_add_persists_newest_first(self):
        h = History(self.path)
        h.add("premier", 2.0, "fr")
        h.add("second")
        self.assertEqual([e["text"] for e in History(self.path).entries()], ["second", "premier"])

    def test_limit_keeps_most_recent(self):
        h = History(self.path, limit=2)
        for t in ("a", "b", "c"):
            h.add(t)
        self.assertEqual([e["text"] for e in h.entries()], ["c", "b"])
        h.set_limit(1)
        self.assertEqual([e["text"] for e in History(self.path).entries()], ["c"])

    def test_zero_limit_keeps_everything(self):
        h = History(self.path, limit=0)
        for i in range(5):
            h.add(str(i))
        self.assertEqual(len(h.entries()), 5)

    def test_get_delete_clear(self):
        h = History(self.path)
        e = h.add("à garder")
        other = h.add("à supprimer")
        self.assertEqual(h.get(e["id"])["text"], "à garder")
        self.assertTrue(h.delete(other["id"]))
        self.assertFalse(h.delete("inconnu"))
        self.assertEqual([x["text"] for x in History(self.path).entries()], ["à garder"])
        h.clear()
        self.assertEqual(History(self.path).entries(), [])

    def test_corrupt_file_gives_empty_history(self):
        os.makedirs(os.path.dirname(self.path))
        with open(self.path, "w", encoding="utf-8") as f:
            f.write("{pas du json")
        self.assertEqual(History(self.path).entries(), [])

    def test_ignores_malformed_entries(self):
        os.makedirs(os.path.dirname(self.path))
        with open(self.path, "w", encoding="utf-8") as f:
            json.dump([{"text": "ok", "id": "1"}, {"id": "2"}, "texte"], f)
        self.assertEqual([e["text"] for e in History(self.path).entries()], ["ok"])


if __name__ == "__main__":
    unittest.main()
