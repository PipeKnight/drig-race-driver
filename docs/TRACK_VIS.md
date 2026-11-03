# track.vis: the per-route visibility file

Status: 2026-11-03, Claude Code (session fervent-napier). Asked for by backlog row 102 (raised
ground-cover grass on Okutama routes 1, 2, 4 and 5): does the original hide grass per route?
Static reading of the shipped files only; no reader in the port yet.

## Layout [V: Okutama routes 0-5, Donington, Washington]

Little-endian. Offsets are absolute file offsets.

- Header: u32 version 2, then the string "visgen test data (x360?)".
- Offset 40: seven u32 counts `c0..c6`. Okutama route 0: 6569, 2077, 97, 943, 1878, 1805, 318.
- Offset 112: `c2 + c3 + c4 + c5 + c6` boxes of 32 bytes (min xyz w=1, max xyz w=1), grouped
  in that order. The `c3` group is the track's `grass.grs` patches: all 943 Okutama boxes equal
  the patch bounds exactly. The other groups were not identified.
- Then a kd-tree of `c0` nodes, 48 bytes each: 7 floats (box, split value), then 5 u32
  (axis, child 0 offset, child 1 offset, `u3`, `u4`).
  - A leaf has both child offsets 0; `u3` is the offset of its bitset, `u4` its index.
  - Okutama route 0 has 3285 leaves. A bitset holds `c1` bits (272 bytes on Okutama route 0;
    the stride differs per route).
- Then a bounding-volume tree of `c1` nodes, 128 bytes each: a 32-byte box, 16 u32 list slots
  and children at u32 16 and 17. A non-zero slot is the offset of (u32 count, u32 pointer) to
  `count` u16 item indices.
  - Slot 1 lists grass patches: each of the 943 patches appears in exactly one node.

Reading [I, consistent with every check below]: a leaf's bit `n` set means node `n` is visible
from that leaf. The camera's leaf is found by walking the kd-tree with its position. From a
patch's own centre, 930 of 943 patch nodes (sampled across the track) and 943 of 943 (route 0
leaves along the road) test visible, so set means visible, not hidden.

Not traced: the original's reader, and who fills the view's patch list that ground cover
scatters (`0x0096C250` calls `0x00959200` with it; GROUND_EFFECTS_RESEARCH.md). So "the original
culls grass with track.vis" is still an inference from the data.

## Per-route grass on Okutama (F709)

`local/r42/vis_test.py` (private, fervent-napier worktree): the leaves along each route's gate
line (`progress_track.xml` gate centres + 1.5 m), the union of their bitsets, and each patch's
node in that set. "Raised" patches are the float audit's `grass_patch` rows (painted vertices
more than 0.25 m above every surface). Output: `local/r43/vis_test.txt`.

| route | leaves on the line | patches visible | raised | raised and hidden | raised within 150 m of the route |
|---|---|---|---|---|---|
| 0 | 477 | 943 | 191 | 0 | 176 |
| 1 | 178 | 866 | 529 | 40 | 0 |
| 2 | 317 | 809 | 540 | 72 | 0 |
| 3 | 469 | 943 | 191 | 0 | 176 |
| 4 | 211 | 873 | 529 | 37 | 0 |
| 5 | 310 | 819 | 540 | 65 | 0 |

- The visibility set hides only 37-72 of each short route's raised patches: it does not
  explain them.
- None of them is within 150 m of its route. `ground_cover.xml`'s farthest draw distance is
  about 145 m, and both the original (`0x00959200`, the patch centre within the draw distance
  of the eye) and the port (`ground_detail.rs`, per instance) scatter only within it. So the
  raised grass of routes 1, 2, 4 and 5 is never drawn while racing those routes, in either.
  It lies over land those routes do not load (the shared `grass.grs` covers the whole
  mountain).
- So no per-route patch set is needed to match the original during a race; the port draws the
  same patches the draw distance admits. Applying `track.vis` as a culling step would only skip
  some patches that are within reach but occluded, a performance step, not a look change.

Routes 0 and 3 (the full course) keep 176 patches with raised vertices near the road, mostly
1-8 edge vertices each; 110 of them have nothing below at all. Patch 702 (-62.7, 78.1, -84.7,
private render `local/r42/p702.png`) shows grass clumps at the road edge over the drop beside
it. The original scatters each triangle of a kept patch at the triangle's own height with no
ground test, so it draws these vertices the same way [I]: they are the original's data, not a
port defect.

## Audit change (F709)

The float audit's `grass_patch` rows now add two columns, the nearest raised vertex's
distance to the route's gate line and how many raised vertices lie within the farthest
ground-cover draw distance of it, and the summary line adds `grass_raised_in_reach`. Rows with
no vertex in reach are grass the route never draws.
