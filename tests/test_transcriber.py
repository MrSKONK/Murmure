import unittest

from src.transcriber import cache_folder_name, is_repairable


class RepairTest(unittest.TestCase):
    def test_cache_folder_name(self):
        self.assertEqual(cache_folder_name("Systran/faster-whisper-medium"), "models--Systran--faster-whisper-medium")

    def test_file_errors_are_repairable(self):
        err = RuntimeError("Unable to open file 'model.bin' in model 'C:\\\\Users\\\\x\\\\.cache\\\\huggingface\\\\hub\\\\"
                           "models--Systran--faster-whisper-medium\\\\snapshots\\\\08e1'")
        self.assertTrue(is_repairable(err))

    def test_hardware_errors_are_not(self):
        for msg in ("CUDA failed with error out of memory", "Requested int8 compute type, but the target device "
                    "or backend do not support efficient int8 computation.", "cuDNN failed"):
            self.assertFalse(is_repairable(RuntimeError(msg)), msg)


if __name__ == "__main__":
    unittest.main()
