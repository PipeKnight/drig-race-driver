"""Read-only alpha-test coverage survey of validated private BC1/BC3 DDS mips.

This measures texel statistics, not screen-space visibility or temporal quality.
No source image or alpha channel is rewritten.
"""
import argparse
import hashlib
import io
import json
import struct
from pathlib import Path

import numpy as np
from PIL import Image


def coverage(alpha, threshold=128):
    hist = np.bincount(alpha.reshape(-1), minlength=256)
    covered = np.cumsum(hist[::-1])[::-1] / alpha.size
    return hist, covered, float(covered[threshold])


def inspect(path):
    if not 128 <= path.stat().st_size <= 64 * 1024 * 1024:
        raise ValueError('DDS size outside 128 bytes..64MiB')
    raw = path.read_bytes()
    if raw[:4] != b'DDS ' or struct.unpack_from('<I', raw, 4)[0] != 124:
        raise ValueError('expected legacy DDS header')
    height, width = struct.unpack_from('<II', raw, 12)
    levels = max(1, struct.unpack_from('<I', raw, 28)[0])
    fmt = raw[84:88]
    if fmt not in (b'DXT1', b'DXT5') or not 1 <= width <= 4096 or not 1 <= height <= 4096:
        raise ValueError('only bounded 2D BC1/BC3 supported')
    if not 1 <= levels <= max(width, height).bit_length() or struct.unpack_from('<I',raw,112)[0]:
        raise ValueError('invalid levels or non-2D resource')
    block = 8 if fmt == b'DXT1' else 16
    rows = []; offset = 128; target = None
    for level in range(levels):
        w, h = max(1, width >> level), max(1, height >> level)
        size = ((w+3)//4) * ((h+3)//4) * block
        if offset+size > len(raw):
            raise ValueError('truncated mip data')
        header = bytearray(raw[:128])
        struct.pack_into('<II',header,12,h,w)
        struct.pack_into('<I',header,20,size)
        struct.pack_into('<I',header,28,1)
        with Image.open(io.BytesIO(header+raw[offset:offset+size])) as image:
            alpha = np.asarray(image.convert('RGBA'))[:,:,3]
        hist, tail, value = coverage(alpha)
        if target is None:
            target = value
        # Discrete threshold alternatives describe attainable coverage. They do
        # not propose an unbounded alpha scale, nor account for bilinear sampling.
        candidates = np.arange(1,256)
        error = np.abs(tail[1:]-target)
        best = candidates[error == error.min()]
        chosen = int(best[np.argmin(np.abs(best-128))])
        rows.append(dict(level=level,width=w,height=h,coverage_at_128=value,
                         mean_alpha=float(alpha.mean()/255),nonzero=float(1-hist[0]/alpha.size),
                         best_discrete_threshold=chosen,closest_coverage=float(tail[chosen]),
                         closest_absolute_error=float(abs(tail[chosen]-target))))
        offset += size
    if offset != len(raw):
        raise ValueError('DDS contains unaccounted trailing bytes')
    return dict(path=str(path),sha256=hashlib.sha256(raw).hexdigest(),format=fmt.decode(),
                base_coverage=target,mips=rows,limits='texel counts only; no bilinear, perspective, subpixel geometry, MSAA or temporal acceptance')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dds',type=Path,nargs='+')
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    private=(Path(__file__).resolve().parent.parent/'local').resolve()
    if not 1 <= len(args.dds) <= 64 or not args.output.resolve().is_relative_to(private):
        parser.error('1..64 inputs and a report under this checkout local/ required')
    result=[inspect(p) for p in args.dds]
    with args.output.open('x',encoding='utf8') as f:
        json.dump(result,f,indent=2,allow_nan=False)
    for r in result:
        print(Path(r['path']).name,' '.join(f"{m['width']}x{m['height']}:{m['coverage_at_128']:.3f}" for m in r['mips']))


if __name__=='__main__':
    main()
