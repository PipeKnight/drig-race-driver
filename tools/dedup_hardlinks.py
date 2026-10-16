"""Replace byte-identical files under a directory with NTFS hard links.

Every path keeps existing and keeps its exact bytes, so citations, pinned hashes and
tools that read the captures keep working; only the duplicate storage is released.
Captures are write-once, which is what makes sharing storage safe: do not edit a file
in place afterwards (that would change every linked path). Deleting one path is safe.

Default is a dry run that only reports. --apply links, after a full byte comparison
of each pair, through a temporary link plus atomic rename so a failure never leaves a
path missing.

Usage:
  python -I tools/dedup_hardlinks.py local/behavior --report local/disk/dedup-behavior.tsv
  python -I tools/dedup_hardlinks.py local/behavior --apply --report local/disk/dedup-behavior-applied.tsv
"""
import argparse
import filecmp
import hashlib
import os
import sys
from collections import defaultdict

NTFS_LINK_LIMIT = 1023  # NTFS allows 1024 names per file; keep one spare.


def file_hash(path):
    h = hashlib.blake2b(digest_size=20)
    with open(path, "rb") as fh:
        while True:
            b = fh.read(1 << 20)
            if not b:
                return h.hexdigest()
            h.update(b)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("--apply", action="store_true", help="create hard links (default: dry run)")
    ap.add_argument("--min-size", type=int, default=64 * 1024, help="ignore files smaller than this")
    ap.add_argument("--report", help="TSV of planned/applied links")
    args = ap.parse_args()

    root = os.path.abspath(args.root)
    by_size = defaultdict(list)
    for d, _, files in os.walk(root):
        for f in files:
            p = os.path.join(d, f)
            try:
                st = os.lstat(p)
            except OSError:
                continue
            if st.st_size >= args.min_size and not os.path.islink(p):
                by_size[st.st_size].append(p)

    candidates = sum(len(v) for v in by_size.values() if len(v) > 1)
    print(f"size-collision files to hash: {candidates}", flush=True)

    plan = []  # (keep, dup, size)
    done = 0
    for size, paths in by_size.items():
        if len(paths) < 2:
            continue
        by_hash = defaultdict(list)
        for p in paths:
            by_hash[file_hash(p)].append(p)
            done += 1
            if done % 5000 == 0:
                print(f"hashed {done}/{candidates}", flush=True)
        for group in by_hash.values():
            if len(group) < 2:
                continue
            group.sort()
            # Choose as master the path that already has most links, then the first by name.
            group.sort(key=lambda p: -os.stat(p).st_nlink)
            keep = group[0]
            keep_st = os.stat(keep)
            links = keep_st.st_nlink
            for dup in group[1:]:
                st = os.stat(dup)
                if (st.st_dev, st.st_ino) == (keep_st.st_dev, keep_st.st_ino):
                    continue  # already the same file
                if links >= NTFS_LINK_LIMIT:
                    keep, keep_st, links = dup, st, st.st_nlink
                    continue
                plan.append((keep, dup, size))
                links += 1

    saved = sum(s for _, _, s in plan)
    print(f"duplicates: {len(plan)} files, {saved / (1 << 30):.2f} GiB reclaimable")

    applied = 0
    failed = 0
    rows = []
    for keep, dup, size in plan:
        status = "planned"
        if args.apply:
            try:
                if not filecmp.cmp(keep, dup, shallow=False):
                    status = "skipped-differs"
                else:
                    tmp = dup + ".dedup-tmp"
                    if os.path.exists(tmp):
                        os.remove(tmp)
                    os.link(keep, tmp)
                    # Preserve the duplicate's timestamps on the shared file is impossible
                    # (one inode); the master's timestamps win. Content is identical.
                    os.replace(tmp, dup)
                    status = "linked"
                    applied += size
            except OSError as e:
                status = f"error:{e.__class__.__name__}:{e}"
                failed += 1
        rows.append((status, size, os.path.relpath(keep, root), os.path.relpath(dup, root)))

    if args.report:
        os.makedirs(os.path.dirname(os.path.abspath(args.report)), exist_ok=True)
        with open(args.report, "w", encoding="utf-8") as w:
            w.write("status\tsize\tkeep\tlinked_path\n")
            for r in rows:
                w.write("\t".join(str(x) for x in r) + "\n")
        print(f"report: {args.report}")
    if args.apply:
        print(f"linked {applied / (1 << 30):.2f} GiB, errors {failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
