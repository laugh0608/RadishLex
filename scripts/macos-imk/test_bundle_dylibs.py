import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("bundle_dylibs.py")
SPEC = importlib.util.spec_from_file_location("bundle_dylibs", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class BundleDylibsTests(unittest.TestCase):
    def test_system_dependency_classification(self):
        self.assertTrue(MODULE.is_system_dependency("/System/Library/Frameworks/AppKit.framework/AppKit"))
        self.assertTrue(MODULE.is_system_dependency("/usr/lib/libSystem.B.dylib"))
        self.assertFalse(MODULE.is_system_dependency("/opt/homebrew/lib/librime.1.dylib"))
        self.assertFalse(MODULE.is_system_dependency("@rpath/librime.1.dylib"))

    def test_hash_is_stable(self):
        self.assertEqual(
            MODULE.hashlib.sha256(b"radishlex").hexdigest(),
            "f0c81261ac1a77eb6f95df13398f8764452b1b4b51bceaa1bc49251c0d867488",
        )


if __name__ == "__main__":
    unittest.main()
