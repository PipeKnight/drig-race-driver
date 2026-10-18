"""Synthetic colour-space/packing guards for the surface edge preparation tool."""
import struct
import unittest
import numpy as np
from PIL import Image
import io
from prepare_surface_edge_candidate import derive


def dds(rgb, alpha=255):
    h, w = rgb.shape[:2]
    header = bytearray(128)
    header[:4] = b"DDS "
    levels = max(w, h).bit_length()
    for p, v in [(4,124),(8,0x2100f),(12,h),(16,w),(20,w*4),(28,levels),
                 (76,32),(80,0x41),(88,32),(92,0xff0000),(96,0xff00),
                 (100,0xff),(104,0xff000000),(108,0x401008)]:
        struct.pack_into("<I",header,p,v)
    parts = []
    for _ in range(levels):
        rgba = np.concatenate((rgb, np.full((h,w,1), alpha, np.uint8)), axis=2)
        parts.append(rgba[:,:,[2,1,0,3]].tobytes())
        w, h = max(1,w//2), max(1,h//2)
        rgb = np.zeros((h,w,3),np.uint8)
    return bytes(header)+b"".join(parts)


class EdgeTests(unittest.TestCase):
    def test_linear_transition_and_complete_mips(self):
        carrier=dds(np.zeros((4,4,3),np.uint8))
        donor=dds(np.full((2,2,3),255,np.uint8))
        before=bytes(carrier)
        out,base,record=derive(carrier,donor,.25,.75,.5,1)
        np.testing.assert_allclose(base[0,:,0],[0,110,237,255],atol=1)
        self.assertEqual(carrier,before)
        self.assertEqual(len(out),128+(16+4+1)*4)
        self.assertEqual(record["unchanged_base_columns"],1)
        self.assertTrue(np.all(np.frombuffer(out[128:],np.uint8)[3::4]==255))
        self.assertLessEqual(abs(out[-4]-188),1)
        self.assertEqual(np.asarray(Image.open(io.BytesIO(out))).shape[:2],(4,4))

    def test_periodic_donor_interpolates_in_linear_light(self):
        carrier=dds(np.zeros((4,4,3),np.uint8))
        donor=dds(np.array([[[255,0,0],[0,255,0]]],np.uint8))
        _,base,_=derive(carrier,donor,0,.01,.885,1)
        np.testing.assert_allclose(base[0,0],[188,188,0],atol=1)

    def test_packed_alpha_and_truncated_chain_rejected(self):
        donor=dds(np.full((2,2,3),255,np.uint8))
        carrier=dds(np.zeros((4,4,3),np.uint8),alpha=128)
        with self.assertRaisesRegex(ValueError,"alpha"):
            derive(carrier,donor,.25,.75,.5,1)
        with self.assertRaisesRegex(ValueError,"mip chain"):
            derive(carrier[:-4],donor,.25,.75,.5,1)

    def test_nonfinite_and_invalid_mapping_rejected(self):
        carrier=dds(np.zeros((4,4,3),np.uint8))
        for args in [(float("nan"),.75,.5,1),(.75,.25,.5,1),(.25,.75,.5,0)]:
            with self.assertRaises(ValueError):
                derive(carrier,carrier,*args)


if __name__=="__main__":
    unittest.main()
