"""Synthetic reverse-Z visibility controls for analyze_temporal_audit history classification."""
import unittest
import numpy as np
import importlib.util
from pathlib import Path
spec = importlib.util.spec_from_file_location("temporal_audit", Path(__file__).with_name("analyze_temporal_audit.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
unproject_pixels, depth_history = module.unproject_pixels, module.depth_history

class HistoryTests(unittest.TestCase):
    def setUp(self):
        projection = np.array([[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,0.,.1],[0.,0.,1.,0.]])
        self.inverse = np.linalg.inv(projection)
        self.size = np.array([512,512])
        self.origin = np.array([240,240])
        self.xy = np.array([[250.25,250.25]])
        self.world, good = unproject_pixels(self.xy,np.array([.05]),self.inverse,self.size)
        self.assertTrue(good.all())
    def classify(self, depth, xy=None):
        return depth_history(self.world,self.xy if xy is None else xy,self.inverse,self.size,self.origin,depth)[0]
    def test_static_plane_agrees(self):
        c=self.classify(np.full((32,32),.05));self.assertTrue(c['stable'][0]);self.assertFalse(c['unavailable'][0])
    def test_previous_occluder_is_nearer(self):
        c=self.classify(np.full((32,32),.1));self.assertTrue(c['previous_nearer'][0]);self.assertFalse(c['stable'][0])
    def test_previous_surface_farther(self):
        c=self.classify(np.full((32,32),.025));self.assertTrue(c['previous_farther'][0]);self.assertFalse(c['stable'][0])
    def test_silhouette_taps_remain_ambiguous(self):
        a=np.full((32,32),.05);a[10,10]=.1;c=self.classify(a);self.assertTrue(c['mixed'][0]);self.assertFalse(c['previous_nearer'][0])
    def test_missing_depth_is_not_occlusion(self):
        c=self.classify(np.zeros((32,32)));self.assertTrue(c['unavailable'][0]);self.assertFalse(c['previous_nearer'][0])
    def test_outside_previous_roi_is_not_occlusion(self):
        c=self.classify(np.full((32,32),.1),np.array([[239.,250.]]));self.assertTrue(c['unavailable'][0]);self.assertFalse(c['previous_nearer'][0])

if __name__=='__main__': unittest.main()
