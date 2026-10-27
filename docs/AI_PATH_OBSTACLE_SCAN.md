# Path-obstacle scan 0x78B560 (static spec, 2026-10-26; port in ai_driver.rs `path_obstacle_scan`)

Static analysis only (game not run). Image: `local/compatibility/grid-unpacked.exe`
(sha256 aac46a64…4514, base 0x400000). Evidence: `local/ai-overtaking/dis_78b560.txt`
(full disassembly), the capstone helper `local/ai-overtaking/scripts/pe.py`, and the Ghidra
output `local/re-cache/0078B560.c`.

Tags: **[V]** means verified from the disassembly, with stack offsets tracked through every push
and pop. **[I]** means inferred. **[G]** marks a place where Ghidra's output differs from the
disassembly.

Vector notes:
- All vectors are 4-lane SSE (x, y, z, w).
- `dot4(a,b)` is the 4-lane dot product. The code always computes it with the
  rotate/add pattern (a·b + rot1(a)·rot1(b), then a horizontal add), which equals
  `a.x*b.x+a.y*b.y+a.z*b.z+a.w*b.w`.
- Direction vectors are assumed to have w = 0, and positions have equal w [I]. Under that
  assumption `dot4` is a 3-D dot.
- `flat(v)` means "y lane set to 0". The mask 0xCAF410 is the **y** lane only
  (00000000,FFFFFFFF,00000000,00000000).
- `normalize(v)` = `v * rsqrt'(dot4(v,v))`, where rsqrt' is `rsqrtps` plus one Newton step
  `r + 0.5*r*(1 - x*r*r)`. Constants: 0.5 from 0xCAF88C, 1.0 from 0xCAF888.
- `normalize` has no zero guard. The only zero handling is the 0.0001 case in step 9.
- `length(v)` is `x*r` with the same Newton step. It returns exactly 0 when `x == 0`
  (cmpeqps against the 0 vector at 0xE58BE0) [V].

---

## 1. Inputs, constants, config

### 1.1 Call and context
- Caller: 0x78F690 (thiscall, ECX = path-obstacle object `PO`, one argument `ctx`). It zeroes
  the following, then calls `0x78B560(ctx)` with ECX = PO [V]:
  - `PO+0x00` and `PO+0x10` (vectors);
  - `+0x20`, `+0x24` and `+0x28`;
  - the obstacle-list count `+0x38`;
  - the contact-list count `+0x168`.
- 0x78B560 is `__thiscall(PO, ctx)` and ends with `ret 4` [V].
- It does **not** clear anything itself. Its best-time tracker is a local [V].

| Read | Meaning |
|---|---|
| ctx+0x08 → cfg | driver AI config (fields below) [V] |
| ctx+0x0C → list | observed-vehicle list. vtable +0x1C = `count()`, called again after every candidate; vtable +0x10 = `get(i)`, returns a record pointer or NULL. Loop `i = 0; while (i < count())`, unsigned compare [V] |
| ctx+0x1C → own | own car state [V] |
| ctx+0x3C → req | request; `byte req+0xAF` = reverse flag [V] |
| ctx (whole) | passed through to 0x783B20, which does not read it [V] |
| ctx+0x28 | **not read** by 0x78B560 or by its helpers [V] |

Own car-state fields read:

| Field | Meaning |
|---|---|
| +0x20 | centre |
| +0x30 | forward (box axis) |
| +0x50 | right |
| +0xA0 | velocity |
| +0xB0 | heading |
| +0xD0 | speed |
| +0xF0 | front point |
| +0x100 | rear point |
| +0x110 | half width |
| +0x114 | length [I name; it is the divisor of the length ratios in 0x783B20] |

Record fields read:

| Field | Meaning |
|---|---|
| +0x10 | speed |
| +0x20 | centre |
| +0x30 | forward |
| +0x40 | right |
| +0x90 | velocity |
| +0xA0 | heading |
| +0xD0 | front point |
| +0xE0 | rear point |
| +0xF0 | half width |
| +0xFC | motion state |

The front/rear naming follows AI_OVERTAKING_RESEARCH §5.1 [I].

### 1.2 Config (name → offset, verified in the constructor near 0x786C00–0x7873D0)

Each entry is `{?, name ptr, value, …}`. The value sits 4 bytes after the name pointer [V].

| cfg offset | Name (string VA) | Shipped value |
|---|---|---|
| +0x8D8 | avStoppedSpeed (0xCF8404) | 1.0 (built-in) |
| +0x928 | avBufferZone (0xCF83B4) | 2.0. A second section in the decoded XML has 5.0 |
| +0x93C | avBufferZoneBaseSpeed (0xCF839C) | 10.0 |
| +0x9A0 | avSimilarDirectionMaxAngle (0xCF831C) | 0.5 rad |
| +0xBE4 | contactZone (0xCF8110) | 0.1 |

The shipped values come from `local/binxml/captured-dependencies/decoded-009.xml` and the
docs. The +0xBE4 row in AI_MISTAKES_PACING_RESEARCH (`removal_distance_track`) belongs to a
different object, the choreographer manager. In the driver cfg, +0xBE4 is contactZone [V].

### 1.3 Constants (read from the image) [V]

| VA | Value | Use |
|---|---|---|
| 0xCAF460 | all-ones mask | with 0xCAF410 builds the "clear y" mask |
| 0xCAF410 | y-lane mask | flat() |
| 0xCAF470 | sign bits ×4 | negate (xorps) |
| 0xCAFE40 | 0x7FFFFFFF in lane 0 | fabsf |
| 0xCAF89C | 3.4028235e38 (FLT_MAX) | initial best time; initial hit distance |
| 0xCAF88C / 0xCAF888 | 0.5 / 1.0 | Newton step |
| 0xCBA1F0 | 0.0001 | zero relative-velocity substitute |
| 0xCB82C0 | 500.0 | sweep length L |
| 0xCB46B4 / 0xCC0A20 | 1e-5 / −1e-5 | parallel threshold in 0x7792C0 |
| 0xCB0F38 | 0.33 | 0x783B20 single side contact |
| 0xCF7F18 | 0.35 | 0x783B20 front two-corner case |
| 0xCF7F14 | 0.45 | 0x783B20 rear two-corner case |

### 1.4 Path-obstacle object layout (constructor 0x78F610) [V]

| Offset | Content |
|---|---|
| +0x00 | relative velocity (vec4) |
| +0x10 | hit point (vec4) |
| +0x20 | record pointer |
| +0x24 | distance |
| +0x28 | time |
| +0x30 / +0x34 / +0x38 | obstacle list: pointer (= PO+0x40), capacity **6**, count. Entries are 0x30 bytes, laid out exactly like PO+0x00..0x2B |
| +0x160 / +0x164 / +0x168 | contact list: pointer (= PO+0x170), capacity **6**, count. Entries are 8 bytes: {record pointer, int contact type} |

---

## 2. Algorithm (exact)

```
fn scan(PO, ctx):
  cfg = ctx.cfg; own = ctx.own; list = ctx.list
  # --- per-call setup ----------------------------------------------------------[V]
  base = cfg.avBufferZoneBaseSpeed
  ratio = (base == 0.0) ? 0.0 : own.speed / base     # ucomiss: equal → 0. NaN is not "equal" → divide
  raw   = ratio * cfg.avBufferZone
  buffer = clamp_between(raw, 0, cfg.avBufferZone)
      # exact branches (b = avBufferZone):
      #   if b > 0:  raw < 0 → 0;  raw > b → b;  else raw
      #   else:      b > raw → b;  raw > 0 → 0;  else raw
  fwdN = normalize(flat(own.forward(+0x30)))           # w is kept, y = 0
  ownFrontB = own.front(+0xF0) + buffer*fwdN
  ownRearB  = own.rear(+0x100) - buffer*fwdN
  ownC      = own.centre(+0x20);  ownH = own.heading(+0xB0)
  bestTime  = FLT_MAX

  i = 0
  while i < list.count():                                # count() is re-queried every iteration
    rec = list.get(i); i += 1                            # the increment happens at the loop bottom
    if rec == NULL: continue

    # (a) contact classification. Its side effect appends to PO+0x160 ---------------[V]
    ownBoxCZ = own_corners(own, cfg.contactZone)         # 0x77B400
    recBoxCZ = rec_corners(rec, cfg.contactZone)         # 0x77F9F0
    contact  = contact_test(PO, ctx, own, rec, ownBoxCZ, recBoxCZ)   # 0x783B20, ALWAYS returns 0

    # (b) both stopped → skip ---------------------------------------------------------[V]
    if !(own.speed > cfg.avStoppedSpeed) and !(rec.speed > cfg.avStoppedSpeed): continue

    cosT = fabsf( (float) cos( (double) cfg.avSimilarDirectionMaxAngle ) )   # 0xBF523A = SSE2 cos
    recDir = (rec.speed > cfg.avStoppedSpeed) ? rec.heading(+0xA0) : rec.forward(+0x30)
    hdot = dot4(recDir, ownH)
    if hdot > cosT:  P = (rec.state(+0xFC) == 0) ? rec.front(+0xD0) : rec.rear(+0xE0)
    else:            P = rec.centre(+0x20)

    # (c) must be ahead -------------------------------------------------------------[V]
    if !(dot4(P - ownC, ownH) > 0): continue

    # (d) must be closing ---------------------------------------------------------------[V]
    relVel = flat(own.vel(+0xA0) - rec.vel(+0x90))
    if !(dot4(rec.centre - ownC, relVel) > 0): continue

    # (e) gap vector --------------------------------------------------------------------[V]
    recFrontB = rec.front(+0xD0) + buffer*rec.forward(+0x30)    # rec.forward is NOT normalised or flattened
    recRearB  = rec.rear(+0xE0)  - buffer*rec.forward(+0x30)
    if req.reverse(+0xAF) != 0:  gap = recFrontB - ownRearB
    elif hdot < 0:               gap = recFrontB - ownFrontB    # "0 > dot", the same hdot as above
    else:                        gap = recRearB  - ownFrontB
    gapAhead = dot4(gap, ownH) > 0
    if !contact and gapAhead and !(dot4(ownH, relVel) > 0): continue
        # contact is always 0 (see 0x783B20), so the rule is:
        # a gap ahead with no closing speed along the heading → skip

    # (f) lateral overlap of the swept corridor -------------------------------------[V]
    recBox = rec_corners(rec, buffer)                    # 0x77F9F0, corners 0..3
    ri = nearest_order(4, &ownC, recBox)                 # 0x774B00 → ri[0] nearest .. ri[3] farthest
    if dot4(relVel, relVel) == 0:                        # ordered equal; dead in practice because (d) ⇒ relVel ≠ 0
        relVel = flat(normalize(gap) * 0.0001)
    vN   = normalize(relVel)
    perp = (-vN.z, 0, vN.x, 0)                           # 0x7A2EA0
    a = [ -dot4(recBox[ri[0]],perp), -dot4(recBox[ri[1]],perp),
          -dot4(recBox[ri[2]],perp), -dot4(recBox[ri[3]],perp) ]
    lo = min(a); hi = max(a)                             # argmin/argmax with strict compares
    ownBox0 = own_corners(own, 0.0)                      # 0x77B400, unbuffered
    overlap = false
    for k in 0..3:                                       # corner order 0,1,2,3
      d = dot4(perp, ownBox0[k])
      if 0 >= (d + hi) * (lo + d): overlap = true; break # NaN → no
    if !overlap: continue
        # Meaning: some own corner's lateral coordinate lies inside the rec box's lateral
        # interval [min dot, max dot]. It is NOT a full interval-overlap test: if the own
        # interval strictly contains the rec interval, no corner lies inside and the
        # candidate is dropped.

    # (g) distance by 4 segment intersections -----------------------------------------[V]
    ownBox = own_corners(own, buffer)                    # 0x77B400 (buffered)
    cr0 = recBox[ri[0]]
    oi  = nearest_order(4, &cr0, ownBox)                 # own corners ordered by distance to cr0
    co0 = ownBox[oi[0]]
    L = 500.0
    vN2 = normalize(relVel)                              # recomputed; same value as vN
    Aown = co0 + L*vN2                                   # own nearest corner swept forward
    Arec = cr0 - L*vN2                                   # rec nearest corner swept backward
    best = FLT_MAX; point = cr0; kind = 2
    # call 1
    r = seg_x(&out,&s,&t, co0, Aown, cr0, recBox[ri[1]])        # 0x7792C0
    if r == 1 or r == 2:  best = s*L; point = out; kind = r
    # call 2
    r = seg_x(&out,&s,&t, co0, Aown, cr0, recBox[ri[2]])
    if (r == 1 or (r == 2 and kind == 2)) and (s*L < best or (r == 1 and kind == 2)):
         best = s*L; point = cr0 (!); kind = r
    # call 3
    r = seg_x(&out,&s,&t, cr0, Arec, co0, ownBox[oi[1]])
    if (same condition): best = s*L; point = cr0 (!); kind = r
    # call 4
    r = seg_x(&out,&s,&t, cr0, Arec, co0, ownBox[oi[2]])
    if (same condition): best = s*L; point = out          # kind is not updated (unused afterwards)

    # (h) time and entry ----------------------------------------------------------------[V]
    time = best / length(relVel)                          # IEEE division; no guard
    entry = { relVel, point, rec, best, (time > 0) ? time : 0 }   # 0x7A3200 zeroes it first; NaN → 0
    if PO.list.count < PO.list.cap(6): push_back(PO.list, entry)  # 0x8EC720
    if bestTime > time:                                   # the UNCLAMPED time; strict → the earlier record wins ties
        bestTime = time
        copy entry → PO+0x00..0x2B                        # 0x7A3380
```

### 2.1 Notes on the algorithm

**Contact flag [V].** 0x783B20 ends with `xor al,al` on its only return path
(0x78452E). The value kept in BL is therefore always 0. The `contact` test in step (e) never
lets a candidate through. **[G]** Ghidra shows `cVar16 != '\0' || …` as if the flag mattered.

**Segment-test result codes and the point [V].** Result 2 from 0x7792C0 means "the infinite
lines cross outside the segments". It is accepted when nothing better was found. So:
- `s` can be **negative or above 1**. In that case `best`, and with it `time`, can be negative.
- A negative time is stored clamped to 0 at +0x28.
- The comparison against `bestTime` uses the **unclamped** value, so a negative time wins.

The `point = cr0` in calls 2 and 3 is real. The update reads stack slot body+0x70 (cr0), not
the output slot body+0x80. Ghidra agrees: `fStack_344 = fStack_364` [V][G-agrees]. Calls 1
and 4 copy the output point.

**Default results.** If no call returns 1 or 2, then `best = FLT_MAX` and `point = cr0`. In
that case `time = FLT_MAX/|relVel|`:
- if `|relVel| > 1`, the time is finite and the record can still become the kept obstacle;
- otherwise the time is +inf (or FLT_MAX at exactly 1) and the record can never beat the
  initial `bestTime` [V arithmetic, I relevance].

Calls 1 and 2 use perpendicular edges of the rec box, so both lines are parallel to `vN` only
if `vN` is degenerate. In practice a result 1 or 2 is always found [I].

**Order of effects per candidate [V].** The contact append (a) happens for **every non-NULL
record**, before the speed, ahead and closing filters.

**Stored fields [V].** +0x00 holds the raw (unnormalised, y = 0) relative velocity. Only in
the dead branch of (f) is it replaced by the 0.0001 vector. +0x24 holds `best` and +0x28 holds
the clamped time.

**[G] Ghidra stack-variable errors.** Ghidra uses the uninitialised `iStack_3b8` as "own" and
`iStack_3bc` as the record. In the disassembly, own is EDI = ctx+0x1C, held at body+0x1C, and
the record is ESI = `get(i)`, held at body+0x18. The FLT_MAX initialiser `local_3a4` is the
best time (body+0x2C), and a second FLT_MAX is `best` (body+0x10). The control flow above
follows the disassembly.

---

## 3. Helpers

### 0x77B400 `own_corners(this = own state, out vec4[4], float m)`
`__thiscall`, `ret 8` [V].
```
F = own.front(+0xF0) + m*own.fwd(+0x30);  R = own.rear(+0x100) - m*own.fwd
S = (own.halfW(+0x110) + m) * own.right(+0x50)
out[0]=flat(F+S)  out[1]=flat(F-S)  out[2]=flat(R+S)  out[3]=flat(R-S)
```
The forward vector is not normalised. EDX is not changed, so the caller's
`mov eax, edx` after the call passes on the same box pointer it put in EDX before the call [V].

### 0x77F9F0 `rec_corners(this = record, out vec4[4], float m)`
Identical to 0x77B400, but with the record offsets: front +0xD0, rear +0xE0, forward +0x30,
right +0x40, half width +0xF0 [V].

Corner order for both builders: 0 = front+right, 1 = front−right, 2 = rear+right,
3 = rear−right [V]. In the docs this is FR, FL, RR, RL, with "right" = the +0x50/+0x40 axis [I].

### 0x774B00 `nearest_order(int out[n], int n, vec4* P, vec4* pts)`
`__cdecl` [V].
```
memset(out,0,4n); dist[n] = 0
for i in 0..n-1: out[i] = i; dist[i] = dot4(pts[i]-*P, pts[i]-*P)
for i in 0..n-1:                       # single pass (the binary unrolls it ×4, remainder loop after)
  d = dist[i]
  last = out[n-1]; if d > dist[last]:  out[i] = last;  out[n-1] = i
  first = out[0];  if dist[first] > d: out[i] = first; out[0] = i
```
Port this exactly. It is **not** a full sort.

Simulation of all 4-element orderings, including ties:
- the result is always a permutation;
- `out[0]` is the nearest (first minimum);
- `out[3]` is the farthest;
- `out[1]` and `out[2]` are the remaining two in an arbitrary order (unsorted in 12 of 24 orderings).

For a rectangle and an outside point, `out[1]` and `out[2]` are the two corners adjacent to
`out[0]` [I geometry].

Distances are 4-lane. The corners have y = 0 while `P` (own centre) keeps its y. This adds the
same constant to every distance, so the order is unchanged.

### 0x7A2EA0 `perp(out, const vec4* v)`
`__cdecl`. Writes `(-v.z, 0, v.x, 0)` to `out` and returns `out` [V].

### 0x7792C0 `seg_x(vec4* out, float* s, float* t, A0*, A1*, B0*, B1*)`
`__cdecl`, 7 arguments. Uses the xz plane only [V].
```
d = A1-A0, e = B1-B0, w = A0-B0           (x,z components)
den = d.x*e.z - d.z*e.x
if -1e-5 < den < 1e-5:                     # strict on both sides
   ns = w.z*e.x - w.x*e.z
   r = (-1e-5 < ns < 1e-5) ? -2 /*collinear*/ : -1 /*parallel*/
   *s = *t = 0
else:
   *s = (w.z*e.x - w.x*e.z) * (1/den)     # computed as reciprocal, then multiply
   *t = (d.x*w.z - d.z*w.x) * (1/den)
   r = (0<=*s<=1 && 0<=*t<=1) ? 1 : 2     # NaN → 2
*out = A0 + (*s)*(A1-A0)                   # 4-lane
return r
```
The output satisfies `A0 + s·d = B0 + t·e`.

### 0x783B20 `contact_test(this = PO, ctx, own, rec, ownBox, recBox)`
`__thiscall`, `ret 0x14`. **Always returns 0** [V].

All dot products below are 4-lane. Corners are flat [V].

**Loop 1: own corner i (0..3) inside the rec box.**
```
a1 = dot(rec.fwd, recBox[0]-ownBox[i]),   b1 = dot(rec.fwd, recBox[3]-ownBox[i])
a2 = dot(rec.right, recBox[0]-ownBox[i]), b2 = dot(rec.right, recBox[3]-ownBox[i])
if 0 > a1*b1 and 0 > a2*b2:
    if n == 0: t1 = i+1; P1 = ownBox[i]
    else:      t2 = i+1; P2 = ownBox[i]
    n++
```
`t1` and `t2` start at 0. P1 and P2 start as the zero vector.

**Loop 2: rec corner c inside the own box.** Skipped when `n == 2`. Runs for each rec corner
`c = recBox[j]`, j = 0..3:
```
p1 = dot(own.fwd, ownBox[0]-c),   q1 = dot(own.fwd, ownBox[3]-c)
p2 = dot(own.right, ownBox[0]-c), q2 = dot(own.right, ownBox[3]-c)
if 0 > p1*q1 and 0 > p2*q2:
    (lat, side) = (|q2| > |p2|) ? (|p2|, 7) : (|q2|, 8)
    if |q1| > |p1|: lon = |p1|; face = 5       # front
    else:           lon = |q1|; face = 6       # rear
    typ = (lat > lon) ? face : side            # a tie → side
    if n == 0: t1 = typ; P1 = c  else: t2 = typ; P2 = c
    n++
```
`own.fwd` = +0x30 and `own.right` = +0x50. Only own+0x30 and +0x50 are used for the own
axes, and rec+0x30 and +0x40 for the rec axes.

**Resolve.** `len = own+0x114`.
```
n == 0 → return 0 (nothing appended)
n == 1 → t1 in {1..6}: type = t1
         t1 == 7: r = dot(own.fwd, P1-ownBox[2])/len; type = (0.33 > r) ? 3 : 7
         t1 == 8: same r;                               type = (0.33 > r) ? 4 : 8
         (both use ownBox[2], the rear-right corner)
n == 2 → switch t1 (jump table 0x784538):
    1: t2==7 ? (r=dot(fwd,P1-P2)/len; r > 0.35 ? 7 : 1) : (t2==5||t2==2 ? 5 : 7)
    2: t2==8 ? (r=dot(fwd,P1-P2)/len; r > 0.35 ? 8 : 2) : (t2==5||t2==1 ? 5 : 8)
    3: t2==7 ? (r=dot(fwd,P2-P1)/len; r > 0.45 ? 7 : 3) : (t2==6||t2==4 ? 6 : 7)
    4: t2==8 ? (r=dot(fwd,P2-P1)/len; r > 0.45 ? 8 : 4) : (t2==6||t2==3 ? 6 : 8)
    5..8: type = t1
n >= 3 → type = t1
append {rec, type} via 0x7A45D0 to PO+0x160
```
0x7A45D0 appends only when `count < cap`. Otherwise the call does nothing.

### 0x7A3200 `entry_clear(this)`
Zeroes +0x00 and +0x10 (vec4), +0x20 (int) and +0x24/+0x28 (float) [V].

### 0x7A3380 `entry_assign(this, const entry*)`
Copies the vec4s at +0x00 and +0x10 and the int at +0x20. Copies +0x24 and +0x28 through x87
`fld`/`fstp`, which is bit-exact for normal floats. `ret 4` [V].

### 0x8EC720 `vec48_push_back(this = {ptr, cap, count}, const entry*)`
Appends only when `count < cap`. Copies 0x30 bytes to `ptr + 0x30*count`, then increments
`count`. Returns 1, or 0 when full [V]. The caller also checks `count < cap` [V].

### 0x7A45D0 `vec8_push_back(this = {ptr, cap, count}, const {u32, u32}*)`
The same pattern with 8-byte entries [V].

### Other calls
- **0xBF523A**: SSE2 libm `cos(double)`, argument and result in xmm0. Established earlier in
  ENGINE_AUDIO_CONTROL.md [V there].
- **0x40AC50 / 0xA35670**: the compiler's `eh vector constructor iterator` (stdcall,
  `ret 0x10`) with a trivial vec4 constructor (`mov eax, ecx; ret`). It is used once on the
  4-corner buffer at body+0x2B0 and has no effect [V].

---

## 4. Points to keep when porting (all [V])
1. `buffer` uses the own speed only. The same `buffer` widens both bumper extensions and
   both boxes in (f) and (g). contactZone is used only for the contact test.
2. In (e) and (f), the rec forward is the raw +0x30, not normalised. The own forward used for
   the bumper extension is flattened and normalised.
3. The heading test is `hdot > |cos(angle)|`, without abs on `hdot`. Front/rear is chosen
   by `rec+0xFC == 0`.
4. The lateral-overlap test in (f) uses the **unbuffered** own box. The segment tests in (g)
   use the **buffered** own box.
5. The obstacle list receives every candidate that reaches (h), in record order, up to 6.
   The kept obstacle has the smallest unclamped time, and the first one wins ties.

## 5. Proposed symbols (docs/re/symbols.tsv)

| Address | Name |
|---|---|
| 0x78B560 | `ai_path_obstacle_scan` |
| 0x78F690 | `ai_path_obstacle_reset_and_scan` |
| 0x78F610 | `ai_path_obstacle_ctor` |
| 0x77B400 | `car_state_box_corners` (existing row "corners") |
| 0x77F9F0 | `observed_record_box_corners` |
| 0x783B20 | `ai_contact_classify` (always returns false) |
| 0x774B00 | `points_nearest_farthest_order` |
| 0x7A2EA0 | `vec_perp_xz` |
| 0x7792C0 | `segment_intersect_xz` |
| 0x7A3200 | `path_obstacle_entry_clear` |
| 0x7A3380 | `path_obstacle_entry_assign` |
| 0x8EC720 | `fixed_vec48_push_back` |
| 0x7A45D0 | `fixed_vec8_push_back` |
| 0x40AC50 | `eh_vector_constructor_iterator` |
| 0xA35670 | `vec4_trivial_ctor` |
| 0xBF523A | `libm_sse2_cos` |
