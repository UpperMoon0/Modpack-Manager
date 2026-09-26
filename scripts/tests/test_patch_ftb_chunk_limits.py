import os
import pathlib
import subprocess
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "patch-ftb-chunk-limits.py"
WRAPPER = SCRIPT.parent / "patch-server.sh"


class PatchFtbChunkLimitsTest(unittest.TestCase):
    def make_server(self, level_name="world"):
        tmp = tempfile.TemporaryDirectory()
        root = pathlib.Path(tmp.name)
        (root / "server.properties").write_text(f"motd=test\nlevel-name={level_name}\n", encoding="utf-8")
        serverconfig = root / level_name / "serverconfig"
        (serverconfig / "ftbranks").mkdir(parents=True)
        (serverconfig / "ftbchunks-world.snbt").write_text(
            "{\n\tmax_claimed_chunks: 500\n\tkeep_me: 17\n\tmax_force_loaded_chunks: 25\n}\n",
            encoding="utf-8",
        )
        (serverconfig / "ftbranks" / "ranks.snbt").write_text(
            "{\n member: {\n  ftbchunks.max_claimed: 50\n  ftbchunks.max_force_loaded: 5\n  untouched: true\n }\n admin: {\n  ftbchunks.max_claimed: 999\n  ftbchunks.max_force_loaded: 888\n }\n}\n",
            encoding="utf-8",
        )
        return tmp, root, serverconfig

    def run_script(self, root, *args):
        return subprocess.run(
            [str(SCRIPT), str(root), *args],
            text=True,
            capture_output=True,
            check=False,
        )

    def test_apply_updates_world_and_all_rank_overrides_preserving_other_content(self):
        tmp, root, serverconfig = self.make_server("custom-world")
        self.addCleanup(tmp.cleanup)
        result = self.run_script(root)
        self.assertEqual(result.returncode, 0, result.stderr)
        world = (serverconfig / "ftbchunks-world.snbt").read_text(encoding="utf-8")
        ranks = (serverconfig / "ftbranks" / "ranks.snbt").read_text(encoding="utf-8")
        self.assertIn("max_claimed_chunks: 1000000", world)
        self.assertIn("max_force_loaded_chunks: 1000000", world)
        self.assertIn("keep_me: 17", world)
        self.assertEqual(ranks.count("ftbchunks.max_claimed: 1000000"), 2)
        self.assertEqual(ranks.count("ftbchunks.max_force_loaded: 1000000"), 2)
        self.assertIn("untouched: true", ranks)

    def test_dry_run_reports_but_does_not_write(self):
        tmp, root, serverconfig = self.make_server()
        self.addCleanup(tmp.cleanup)
        before = (serverconfig / "ftbchunks-world.snbt").read_text(encoding="utf-8")
        result = self.run_script(root, "--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("500 -> 1000000", result.stdout)
        self.assertEqual((serverconfig / "ftbchunks-world.snbt").read_text(encoding="utf-8"), before)

    def test_explicit_limits_are_supported(self):
        tmp, root, serverconfig = self.make_server()
        self.addCleanup(tmp.cleanup)
        result = self.run_script(root, "--claimed", "123", "--force-loaded", "456")
        self.assertEqual(result.returncode, 0, result.stderr)
        world = (serverconfig / "ftbchunks-world.snbt").read_text(encoding="utf-8")
        self.assertIn("max_claimed_chunks: 123", world)
        self.assertIn("max_force_loaded_chunks: 456", world)

    def test_rejects_level_name_outside_server_root(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "server.properties").write_text("level-name=../elsewhere\n", encoding="utf-8")
            result = self.run_script(root)
            self.assertEqual(result.returncode, 2)
            self.assertIn("outside server root", result.stderr)


    def test_server_wrapper_applies_limits_after_modpack_apply(self):
        tmp, root, serverconfig = self.make_server()
        self.addCleanup(tmp.cleanup)
        mock = root / "modpackctl-mock"
        log = root / "modpackctl.log"
        mock.write_text(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' \"$*\" >> \"$MODPACKCTL_TEST_LOG\"\n",
            encoding="utf-8",
        )
        mock.chmod(0o755)
        env = os.environ.copy()
        env["MODPACKCTL"] = str(mock)
        env["MODPACKCTL_TEST_LOG"] = str(log)
        result = subprocess.run(
            [str(WRAPPER), str(root)],
            text=True,
            capture_output=True,
            env=env,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = log.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(calls), 2)
        self.assertTrue(calls[0].startswith("plan --target server"))
        self.assertTrue(calls[1].startswith("apply --target server"))
        world = (serverconfig / "ftbchunks-world.snbt").read_text(encoding="utf-8")
        self.assertIn("max_claimed_chunks: 1000000", world)
        self.assertIn("max_force_loaded_chunks: 1000000", world)

    def test_server_wrapper_dry_run_does_not_apply_or_write_limits(self):
        tmp, root, serverconfig = self.make_server()
        self.addCleanup(tmp.cleanup)
        mock = root / "modpackctl-mock"
        log = root / "modpackctl.log"
        mock.write_text(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' \"$*\" >> \"$MODPACKCTL_TEST_LOG\"\n",
            encoding="utf-8",
        )
        mock.chmod(0o755)
        before = (serverconfig / "ftbchunks-world.snbt").read_text(encoding="utf-8")
        env = os.environ.copy()
        env["MODPACKCTL"] = str(mock)
        env["MODPACKCTL_TEST_LOG"] = str(log)
        env["DRY_RUN"] = "1"
        result = subprocess.run(
            [str(WRAPPER), str(root)],
            text=True,
            capture_output=True,
            env=env,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = log.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(calls), 1)
        self.assertTrue(calls[0].startswith("plan --target server"))
        self.assertEqual((serverconfig / "ftbchunks-world.snbt").read_text(encoding="utf-8"), before)


if __name__ == "__main__":
    unittest.main()
