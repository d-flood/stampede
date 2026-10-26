import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import fcntl

from scripts.build_environment import build_environment, prune_build, prune_temporary


class TemporaryStorage(unittest.TestCase):
    def test_stale_runs_are_removed_while_active_runs_and_sources_survive(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            (target / 'tmp').mkdir()
            stale = target / 'tmp/stampede-e2e-abandoned'
            source = target / 'tmp/source'
            stale.mkdir()
            source.mkdir()
            os.utime(stale, (1, 1))
            os.utime(source, (1, 1))
            with build_environment(target, 'stampede-rebuild-') as environment:
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
            scratch = target / 'tmp/stampede-e2e-live-child'
            scratch.mkdir(parents=True)
            os.utime(scratch, (1, 1))
            child = subprocess.Popen(['sleep', '30'], env=os.environ | {'STAMPEDE_BUILD_SCRATCH': str(scratch)})
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
            (target / 'tmp/stampede-e2e-link').symlink_to(source, target_is_directory=True)
            os.utime(source, (1, 1))
            prune_temporary(target)
            self.assertTrue((source / 'keep').exists())


def make(path, old=False):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.touch()
    if old:
        os.utime(path, (1, 1))
    return path


class BuildStorage(unittest.TestCase):
    def test_units_unused_for_a_week_are_removed_whole_while_used_ones_survive(self):
        with tempfile.TemporaryDirectory() as directory:
            profile = Path(directory) / 'debug'
            stale = [
                make(profile / 'deps/libold-aaaaaaaaaaaaaaaa.rlib', old=True),
                make(profile / '.fingerprint/old-aaaaaaaaaaaaaaaa/lib-old', old=True),
                make(profile / 'build/old-aaaaaaaaaaaaaaaa/output', old=True),
                make(profile / 'incremental/old-1cm7ce2qlkj48/s-old', old=True),
                make(profile / 'oikonomos', old=True),
                make(profile / 'deps/new-bbbbbbbbbbbbbbbb.tmp1a2b', old=True),
            ]
            kept = [
                make(profile / 'deps/libnew-bbbbbbbbbbbbbbbb.rlib'),
                make(profile / '.fingerprint/new-bbbbbbbbbbbbbbbb/lib-new', old=True),
                make(profile / 'incremental/new-25eedo906ch0z/s-new'),
                make(profile / 'stampede'),
            ]
            for path in [profile / '.fingerprint/old-aaaaaaaaaaaaaaaa', profile / 'build/old-aaaaaaaaaaaaaaaa', profile / 'incremental/old-1cm7ce2qlkj48']:
                os.utime(path, (1, 1))
            prune_build(Path(directory))
            self.assertEqual([path for path in stale if path.exists()], [])
            self.assertFalse((profile / '.fingerprint/old-aaaaaaaaaaaaaaaa').exists())
            self.assertEqual([path for path in kept if not path.exists()], [])

    def test_a_profile_cargo_is_building_is_left_alone(self):
        with tempfile.TemporaryDirectory() as directory:
            profile = Path(directory) / 'release'
            stale = make(profile / 'deps/libold-aaaaaaaaaaaaaaaa.rlib', old=True)
            (profile / '.fingerprint').mkdir()
            with (profile / '.cargo-lock').open('a') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                prune_build(Path(directory))
            self.assertTrue(stale.exists())


if __name__ == '__main__':
    unittest.main()
