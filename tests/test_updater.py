import unittest

from src import updater


def release(*names, **extra):
    data = {"tag_name": "v1.2.0", "body": "Nouveautés", "assets": [
        {"name": n, "size": 10, "browser_download_url": f"https://github.com/MrSKONK/Murmure/releases/download/v1.2.0/{n}"}
        for n in names]}
    data.update(extra)
    return data


class VersionTest(unittest.TestCase):
    def test_compare(self):
        self.assertTrue(updater.is_newer("1.2.0", "1.1.9"))
        self.assertTrue(updater.is_newer("v1.10.0", "1.9.3"))
        self.assertFalse(updater.is_newer("1.2.0", "1.2.0"))
        self.assertFalse(updater.is_newer("1.0.0", "1.2.0"))


class PickReleaseTest(unittest.TestCase):
    def test_installer_with_digest(self):
        r = updater.pick_release(release("Murmure-Setup-1.2.0.exe", "Murmure-Setup-1.2.0.exe.sha256"))
        self.assertEqual(r["version"], "1.2.0")
        self.assertTrue(r["url"].endswith("Murmure-Setup-1.2.0.exe"))
        self.assertTrue(r["sha256_url"].endswith(".sha256"))
        self.assertEqual(r["notes"], "Nouveautés")

    def test_refused_without_digest(self):
        self.assertIsNone(updater.pick_release(release("Murmure-Setup-1.2.0.exe")))

    def test_refused_draft_prerelease_or_foreign_host(self):
        names = ("Murmure-Setup-1.2.0.exe", "Murmure-Setup-1.2.0.exe.sha256")
        self.assertIsNone(updater.pick_release(release(*names, draft=True)))
        self.assertIsNone(updater.pick_release(release(*names, prerelease=True)))
        evil = release(*names)
        evil["assets"][0]["browser_download_url"] = "https://evil.example/Murmure-Setup-1.2.0.exe"
        self.assertIsNone(updater.pick_release(evil))

    def test_ignores_other_files(self):
        self.assertIsNone(updater.pick_release(release("notes.txt", "Murmure-portable.zip")))
        self.assertIsNone(updater.pick_release({"message": "Not Found"}))


if __name__ == "__main__":
    unittest.main()
