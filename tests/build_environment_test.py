import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from scripts.build_environment import build_environment, prune_temporary


class TemporaryStorage(unittest.TestCase):
    def test_stale_runs_are_removed_while_active_runs_and_sources_survive(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            (target / 'tmp').mkdir()
            stale = target / 'tmp/oikonomos-e2e-abandoned'
            source = target / 'tmp/source'
            stale.mkdir()
            source.mkdir()
            os.utime(stale, (1, 1))
            os.utime(source, (1, 1))
            with build_environment(target, 'oikonomos-rebuild-') as environment:
                active = Path(environment['TMPDIR'])
                os.utime(active, (1, 1))
                prune_temporary(target)
                self.assertFalse(stale.exists())
                self.assertTrue(active.exists())
                self.assertTrue(source.exists())
            self.assertFalse(active.exists())

    def test_an_active_child_keeps_its_scratch_after_its_parent_loses_the_lease(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            scratch = target / 'tmp/oikonomos-e2e-live-child'
            scratch.mkdir(parents=True)
            os.utime(scratch, (1, 1))
            child = subprocess.Popen(['sleep', '30'], env=os.environ | {'OIKONOMOS_BUILD_SCRATCH': str(scratch)})
            try:
                prune_temporary(target)
                self.assertTrue(scratch.exists())
            finally:
                child.terminate()
                child.wait(timeout=3)
            os.utime(scratch, (1, 1))
            prune_temporary(target)
            self.assertFalse(scratch.exists())

    def test_symlinks_never_make_external_directories_disposable(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / 'target'
            (target / 'tmp').mkdir(parents=True)
            source = Path(directory) / 'source'
            source.mkdir()
            (source / 'keep').touch()
            (target / 'tmp/oikonomos-e2e-link').symlink_to(source, target_is_directory=True)
            os.utime(source, (1, 1))
            prune_temporary(target)
            self.assertTrue((source / 'keep').exists())


if __name__ == '__main__':
    unittest.main()
