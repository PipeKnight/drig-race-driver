"""Print a grid-binxml TSV (hex-encoded names/values) as an indented tree."""
import sys


def unhex(s):
    return bytes.fromhex(s).decode("utf-8", "replace") if s else ""


def main(path):
    elems, attrs = {}, {}
    for line in open(path, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        if f[0] == "E":
            elems[int(f[1])] = (unhex(f[2]), unhex(f[3]), f[4], int(f[5]), int(f[6]))
            attrs.setdefault(int(f[1]), [])
        else:
            attrs.setdefault(int(f[1]), []).append((unhex(f[2]), unhex(f[3])))

    def show(i, d):
        name, value, _, start, end = elems[i]
        a = " ".join(f"{k}={v}" for k, v in attrs[i])
        print("  " * d + name + (f" '{value}'" if value else "") + (f" [{a}]" if a else ""))
        for c in range(start, end):
            show(c, d + 1)

    for i in sorted(elems):
        if not elems[i][2]:
            show(i, 0)


if __name__ == "__main__":
    main(sys.argv[1])
