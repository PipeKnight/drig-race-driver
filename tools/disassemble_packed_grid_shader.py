"""Read-only selected shaderpack program disassembly; all evidence stays private."""
import argparse
import hashlib
import json
from pathlib import Path

from disassemble_grid_shader import disassemble
from survey_pssg_payloads import survey


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--program", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    private = (Path(__file__).resolve().parent.parent / "local").resolve()
    output = args.output.resolve()
    metadata = output.with_suffix(output.suffix + ".json")
    if not output.is_relative_to(private):
        parser.error("original shader evidence must stay under ignored local/")
    if output.exists() or metadata.exists():
        raise FileExistsError("refusing existing assembly or metadata")
    if args.source.stat().st_size > 256 * 1024 * 1024:
        raise ValueError("shaderpack exceeds survey bound")
    report = survey(args.source)
    data = args.source.read_bytes()
    if hashlib.sha256(data).hexdigest() != report["sha256"]:
        raise ValueError("shaderpack changed during survey")
    nodes = report["nodes"]

    def attributes(node):
        return {k: v.get("u32be", v.get("string")) for k, v in node["attributes"].items()}

    programs = [i for i, n in enumerate(nodes) if n["name"] == "SHADERPROGRAM"
                and attributes(n).get("id") == args.program]
    if len(programs) != 1:
        raise ValueError("expected one exact program match")
    codes = [i for i, n in enumerate(nodes) if n["parent"] == programs[0]
             and n["name"] == "SHADERPROGRAMCODE"]
    if len(codes) != 1:
        raise ValueError("expected one selected program code")
    code = attributes(nodes[codes[0]])
    if code.get("codeType") != "HLSLBinary":
        raise ValueError("unsupported program code type")
    blocks = [n for n in nodes if n["parent"] == codes[0]
              and n["name"] == "SHADERPROGRAMBINARYCODEBLOCK"]
    if len(blocks) != 1 or blocks[0]["payload_size"] != code.get("binaryCodeSize"):
        raise ValueError("binary program count/size mismatch")
    block = blocks[0]
    binary = data[block["payload_offset"]:block["payload_offset"] + block["payload_size"]]
    assembly = disassemble(binary)
    params = [attributes(n) for n in nodes if n["parent"] == codes[0]
              and n["name"] == "SHADERINPUTDEFINITION"]
    result = dict(source=str(args.source), source_sha256=report["sha256"],
                  program=args.program, code=code, parameters=params,
                  binary_sha256=hashlib.sha256(binary).hexdigest(),
                  assembly_sha256=hashlib.sha256(assembly).hexdigest(),
                  scope="static selected variant, not observed runtime selection")
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as stream:
        stream.write(assembly)
    with metadata.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2, allow_nan=False)
    print(json.dumps(dict(program=args.program, bytes=len(binary),
                          assembly_bytes=len(assembly), output=str(output))))


if __name__ == "__main__":
    main()
