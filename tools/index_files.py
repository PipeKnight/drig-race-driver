"""Generate tools/README.md and docs/INDEX.md: one line per tool and per document.

Run with `py -I tools/index_files.py` after adding, renaming or retiring a file in tools/ or
docs/, and commit the regenerated indexes with it. `--check` only reports files the indexes
miss (exit 1); `tools/log_entry.py check` reports the same as notes.

Descriptions come from each file's header: a Python docstring, a PowerShell .SYNOPSIS or the
leading comment lines (`#`, `//`, `/* */`, `rem`, `;`), first sentence only. A tool's "doc" is
the document under docs/ that names the file most often. The grouping and the retire list are
the hand-edited part: the GROUPS, FAMILY_TOPICS, DOC_GROUPS and RETIRE tables below.
"""

import argparse
import ast
import collections
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "tools"
DOCS = ROOT / "docs"
TOOLS_INDEX = TOOLS / "README.md"
DOCS_INDEX = DOCS / "INDEX.md"

# ---- hand-edited tables -----------------------------------------------------------------

# Probe families: files that share a topic across these name patterns form one row.
FAMILY_PATTERNS = [
    (r"probe_(.+)\.c", "probe"),
    (r"build_(.+?)_probes?\.cmd", "build"),
    (r"build_(.+?)_regression\.cmd", "build regression"),
    (r"validate_(.+)\.py", "validator"),
    (r"audit_(.+)\.py", "audit"),
    (r"replay_(.+?)(?:_regression)?\.py", "replay"),
    (r"(.+?)_probe\.h", "header"),
]

# Probe-family subsections: first keyword found in the topic wins.
FAMILY_TOPICS = [
    ("Engine audio", ("audio", "engine_audio", "engine_mixer", "continuous_engine")),
    ("Steering", ("steering", "assisted")),
    ("Suspension", ("suspension",)),
    ("Tyres and wheels", ("tyre", "wheel", "grip", "radius")),
    ("Vehicle step and driver", ("vehicle", "car_job", "car_manager", "finalize", "pose", "setup", "preload",
                                 "force", "dynamics", "handling", "propulsion", "gearing", "chassis")),
    ("Deformation", ("deform", "volume", "settle")),
    ("Impacts and body events", ("impact", "body", "collision_damage", "damage")),
    ("Packed world, grids and track queries", ("packed", "grid", "leaf", "rectangle", "material_key",
                                               "track_contact", "query", "surface")),
    ("Triangle meshes and manifolds", ("triangle", "manifold", "mesh", "cqtc", "oriented")),
    ("Contacts and constraints", ("contact", "owner", "serial", "callback", "response", "joint", "bias")),
    ("Physical world and scheduling", ("physical", "world", "tick", "frame", "geometric", "carried",
                                       "shaped", "grouped", "standard", "constructor", "properties")),
]

# Groups for single files, in order; the first rule whose pattern matches the path wins.
GROUPS = [
    ("Process: log, indexes, pre-push, release, builds and disk", [
        r"tools/(log_entry|split_project_log|index_files)\.py", r"tools/prepush\.ps1", r"tools/release_playable",
        r"tools/update_prototype", r"tools/build_gate", r"tools/build_seed", r"tools/seed_build_dir",
        r"tools/prune_build_dirs", r"tools/dedup_hardlinks", r"tools/disk_report", r"tools/time_build",
        r"tools/install_", r"tools/identity\.tsv", r"tools/build_lock", r"tools/.*lock.*\.ps1"]),
    ("Android", [r"android", r"tools/push_android"]),
    ("Capture and recording of the original (live process, macros, debugger)", [
        r"tools/capture_", r"tools/start_race_capture", r"tools/restart_grid_reference", r"tools/resume_reference",
        r"tools/read_reference_", r"tools/read_grid_", r"tools/find_reference_", r"tools/trace_grid_",
        r"tools/run_grid_macro", r"tools/grid_macros", r"tools/probe_debugger", r"tools/check_debugger",
        r"tools/debugger_smoke", r"tools/build_debugger_smoke", r"tools/check_owner_capture",
        r"tools/check_reference_preservation", r"tools/inspect_live_", r"tools/check_wheel_force_capture",
        r"tools/check_.*_controls", r"tools/build_grid_audio_capture", r"tools/windows/grid_timed_input",
        r"tools/summarize_live_", r"tools/summarize_deformation_capture", r"tools/export_runtime_image"]),
    ("Startup compatibility and installation (Phase 0)", [
        r"tools/collect_machine", r"tools/probe_affinity", r"tools/probe_memory_fix", r"tools/download_memory_fixes",
        r"tools/verify_compatibility", r"tools/set_copy_codepage", r"tools/probe_document_codepages",
        r"tools/windows/document_codepages", r"tools/make_import_manifest", r"tools/isolated_", r"tools/inspect_installation",
        r"tools/inspect_minidump", r"tools/inspect_dump_threads", r"tools/summarize_procmon", r"tools/probe_ctf_library",
        r"tools/probe_refresh"]),
    ("Reverse engineering: Ghidra, symbols, static scans", [
        r"tools/ghidra/", r"tools/re/", r"tools/run_ghidra_", r"tools/inspect_private_", r"tools/scan_",
        r"tools/inspect_numeric_kernels", r"tools/inspect_save_path_code", r"tools/inspect_behavior_fields",
        r"tools/inspect_reference_outer", r"tools/extract_", r"tools/inspect_handling_damage",
        r"tools/inspect_deformation_edges", r"tools/inspect_steering", r"tools/dump_grid_car",
        r"tools/prepare_", r"tools/classify_behavior_captures", r"tools/inspect_grid_engine_trace",
        r"tools/render_grid_engine_trace"]),
    ("Graphics, textures and meshes", [
        r"alpha_mips", r"mip_tails", r"shader", r"inventory_graphics", r"prepare_texture", r"preview_mesh",
        r"pssg", r"hdr_probe", r"object_lods", r"light_math", r"particle"]),
    ("Resource formats and data readers", [
        r"binxml", r"resource_readers", r"validate_progress", r"surface_xml", r"vehicle_tuning",
        r"vehicle_source_gearing", r"entity_descriptors", r"body_tuning"]),
    ("Driving, AI and race analysis (port runs and recordings)", [
        r"tools/analyze_", r"tools/summarize_driving", r"tools/replay_driving", r"tools/compare_driving",
        r"tools/inspect_lap", r"tools/route_survey", r"tools/inspect_gameplay_video", r"tools/sample_profile",
        r"tools/chrome_trace_summary", r"tools/avoid_leak", r"tools/replay_damage_freeze", r"tools/audit_lap_replay",
        r"tools/validate_grid_driving", r"tools/validate_live_", r"tools/validate_spawn", r"tools/validate_manifold_turns",
        r"tools/audit_motion_sequence"]),
    ("Audio", [r"audio", r"engine_mixer", r"continuous_engine"]),
]
OTHER_GROUP = "Other validators and comparators"

# Documents: first rule whose pattern matches the file name wins.
DOC_GROUPS = [
    ("Project process, tooling and builds", [
        r"TOOLCHAIN", r"DISK_USAGE", r"PROTOTYPE_BUILD", r"TRY_THE_PROTOTYPE", r"GITHUB", r"PROJECT_RELOCATION",
        r"IMPLEMENTATION_PLAN", r"NEXT_MILESTONES", r"PROGRESS_ASSESSMENT", r"FIDELITY_BACKLOG", r"CAPTURE_BACKLOG",
        r"SOAK_TESTING", r"RUNTIME_LIMITS", r"ROBUSTNESS", r"PERFORMANCE", r"TICK_PERFORMANCE", r"^re/",
        r"^reference/", r"^templates/", r"INDEX"]),
    ("Original reference, captures and owner sessions", [
        r"PHASE0", r"COMPATIBILITY", r"LOADER_RESEARCH", r"REFERENCE_", r"LIVE_SPAWN", r"MACRO_SETUP",
        r"GAME_CONTROL", r"TIMED_INPUT", r"AUTOSAVE", r"OWNER_", r"PLAYTEST", r"GAMEPLAY_RECORDING",
        r"DONINGTON_RACE_VIDEO", r"AUTONOMOUS_PRACTICE", r"BRAKING_REFERENCE"]),
    ("Resource formats and data", [
        r"RESOURCE_FORMATS", r"BINARY_XML", r"PSSG", r"PROGRESS_AND_SPAWN", r"VEHICLE_TUNING", r"ENTITY_COMPONENTS",
        r"TRACK_READERS", r"STATIC_VIEWER", r"ORNAMENT", r"TRACK_COLLISION", r"CAR_COLLISION", r"BODY_PARAMETERS",
        r"STANDARD_PROPERTIES", r"SURFACE_MECHANICS", r"HANDLING_CONFIGURATION"]),
    ("Audio", [r"AUDIO", r"ENGINE_", r"CONTINUOUS_ENGINE"]),
    ("AI drivers", [r"^AI_", r"FIRST_CORNER", r"ROUTE_PROPS"]),
    ("Events, race rules, career and series", [
        r"RACE_", r"CAREER", r"SERIES", r"DISCIPLINES", r"DEMOLITION", r"TOUGE", r"TIME_TRIAL", r"EVENT_MENU",
        r"FLASHBACK", r"REPLAY", r"PAUSE_MENU", r"LE_MANS", r"LAUNCH_AND_GRID", r"PRACTICE_LAP",
        r"MULTI_CAR", r"RESET_AND_RESPAWN", r"CAMERA", r"HUD", r"SETTINGS"]),
    ("Tracks and circuits", [
        r"TRACK_", r"ISTANBUL", r"JARAMA", r"LONG_BEACH", r"SPA", r"STREET_CIRCUITS", r"DONINGTON_SCENE",
        r"DONINGTON_SURFACE", r"SKY", r"YOKOHAMA", r"DONINGTON_ATMOSPHERE", r"CAR_GENERALIZATION"]),
    ("Rendering and graphics", [
        r"GRAPHICS", r"TEXTURE", r"LIVERY", r"SHADER", r"CLEARCOAT", r"REFLECTIONS", r"GRASS", r"VEGETATION",
        r"SCENE_HDR", r"IMAGE_FORMATION", r"LIGHTING", r"LACETTI_GRAPHICS", r"TREAD_NORMAL", r"TYRE_VISUAL",
        r"TYRE_SMOKE", r"BODY_DEFORMATION_PRESENTATION", r"VISUAL_DAMAGE", r"GROUND_EFFECTS", r"DYNAMIC_PROPS"]),
    ("Input, platforms and Android", [
        r"ANDROID", r"PLATFORMS", r"PAD_PROBE", r"DRIVER_AIDS", r"DRIVING_ASSISTS", r"VEHICLE_CONTROLS"]),
    ("Damage and deformation", [
        r"DAMAGE", r"DEFORM", r"WHEEL_DETACHMENT", r"IMPACT", r"BODY_IMPACT", r"COLLISION_DAMAGE",
        r"MECHANICAL_DAMAGE", r"HANDLING_DAMAGE", r"CAR_TO_CAR"]),
    ("Collisions, contacts and the physical world", [
        r"CONTACT", r"COLLISION", r"MESH", r"MANIFOLD", r"TRIANGLE", r"PACKED", r"GRID_POPULATION", r"CANDIDATE_GRID",
        r"PRODUCTION_GRID", r"LEAF_QUERIES", r"RECTANGLE_CULL", r"SOURCE_WORLD", r"PHYSICAL", r"SERIAL",
        r"CQTC", r"BOX_CONTACTS", r"JOINTS", r"OWNER_COLLECTION", r"RESPONSE_CONSUMPTION", r"REGISTERED_POOL",
        r"CARRIED", r"GROUPED", r"GEOMETRIC", r"SCHEDULED", r"HEAVY_CONTACT", r"REPEATED_CONTACT", r"GROUND_",
        r"ADVANCED_GEOMETRY", r"WORLD"]),
    ("Vehicle physics: suspension, tyres, steering, drivetrain", [
        r"VEHICLE", r"SUSPENSION", r"TYRE", r"WHEEL", r"STEERING", r"GRIP", r"SURFACE", r"FORCE", r"COUPLED",
        r"HANDLING", r"SOURCE_", r"STANDARD_CAR", r"MUTABLE", r"CAR_JOB", r"CAR_MANAGER", r"SOURCE_DRIVING"]),
]

# Candidates to retire, by hand: {tools-relative path: reason}. Byte-identical copies are added
# automatically.
RETIRE = {
    "windows/grid_timed_input.rs": "Phase 1 SendInput helper never validated against GRID (docs/TIMED_INPUT.md); "
                                   "the AutoHotkey macros grid_macros.ahk / run_grid_macro.ps1 are the verified input "
                                   "path (docs/MACRO_SETUP.md)",
    "probe_memory_fix.ps1": "Phase 0 memory-patch experiment: every patch still crashed (F016); the startup remedy is "
                            "the process-codepage control (H003, docs/COMPATIBILITY_MEMORY_PATCHES.md)",
    "download_memory_fixes.ps1": "downloads the Phase 0 memory patches of probe_memory_fix.ps1; same closed question (F016)",
    "probe_affinity.ps1": "Phase 0 affinity experiment, recorded as an insufficient remedy (F008)",
}

# ---- headers ----------------------------------------------------------------------------

def first_sentence(text, limit=170):
    text = " ".join(text.split())
    text = re.sub(r"^(SYNOPSIS|\.SYNOPSIS)\s*", "", text)
    m = re.search(r"(?<!\b[A-Z])(?<!e\.g)(?<!i\.e)[.!?](\s|$)", text)
    s = text[: m.start() + 1] if m else text
    if len(s) > limit:
        s = s[:limit].rsplit(" ", 1)[0] + " …"
    return s.replace("|", "\\|")


def header(path):
    try:
        text = path.read_bytes().decode("utf-8", "replace").replace("\r\n", "\n")
    except OSError:
        return ""
    ext = path.suffix.lower()
    if ext == ".py":
        try:
            doc = ast.get_docstring(ast.parse(text))
        except (SyntaxError, ValueError):
            doc = None
        if doc:
            return first_sentence(doc.split("\n\n")[0])
    if ext == ".ps1":
        m = re.search(r"\.SYNOPSIS\s*\n(.+?)(\n\s*\n|\n\.)", text, re.S)
        if m:
            return first_sentence(m.group(1))
    lines = text.split("\n")
    out = []
    in_block = False
    for line in lines[:40]:
        s = line.strip()
        if not out and (s.startswith("#!") or s.startswith("#Requires") or s.startswith("#Single")
                        or s.lower() in ("@echo off", "setlocal") or s.startswith("<#") or s == ""):
            if s.startswith("<#"):
                in_block = True
            continue
        if in_block:
            if s.startswith("#>"):
                break
            out.append(s)
            continue
        if s.startswith("/*"):
            body = s[2:]
            if "*/" in body:
                out.append(body.split("*/")[0])
                break
            in_block = True
            out.append(body)
            continue
        m = re.match(r"^(#|//|;|::|@?rem\b)\s?(.*)", s, re.I)
        if m and not s.startswith("#include") and not s.startswith("#define") and not s.startswith("#pragma"):
            out.append(m.group(2))
            continue
        break
    joined = " ".join(o.strip(" */") for o in out).strip()
    return first_sentence(joined) if joined else ""


# ---- references from docs -----------------------------------------------------------------

def doc_mentions():
    """{file name: Counter(doc path)} for tool-like file names written in docs."""
    pat = re.compile(r"[A-Za-z0-9_.-]+\.(?:py|ps1|cmd|c|h|cpp|rs|java|ahk|sh|tsv|ini)\b")
    out = collections.defaultdict(collections.Counter)
    for d in sorted(DOCS.rglob("*.md")):
        if d == DOCS_INDEX:
            continue
        rel = d.relative_to(DOCS).as_posix()
        for name in pat.findall(d.read_text(encoding="utf-8", errors="replace")):
            out[name.rsplit("/", 1)[-1]][rel] += 1
    return out


def best_doc(names, mentions, topic=None):
    c = collections.Counter()
    for n in names:
        c.update(mentions.get(n, {}))
    if c:
        return c.most_common(1)[0][0]
    # No document names the files: fall back to a document named after the topic.
    if topic and (DOCS / (topic.upper() + ".md")).exists():
        return topic.upper() + ".md"
    return ""


# ---- tools index --------------------------------------------------------------------------

def tool_files():
    skip = {TOOLS_INDEX}
    return sorted(p for p in TOOLS.rglob("*") if p.is_file() and p not in skip and "__pycache__" not in p.parts)


def compiled_sources(p):
    return list(dict.fromkeys(re.findall(r"probe_([A-Za-z0-9_]+)\.c", p.read_text(encoding="utf-8", errors="replace"))))


def families(files):
    topic_of = {}
    for p in files:
        if p.parent != TOOLS:
            continue
        if p.suffix == ".cmd" and p.name.startswith("build_"):
            srcs = compiled_sources(p)
            if len(srcs) == 1:
                # A build script belongs to the probe it compiles (some build variants with defines).
                topic_of[p] = (srcs[0], "build" if "regression" not in p.name else "build regression")
                continue
            if len(srcs) > 1:
                topic_of[p] = (re.sub(r"^build_|_probes?$|\.cmd$", "", p.stem.replace("_regression", " (regression)")),
                               "build several")
                continue
        for pat, role in FAMILY_PATTERNS:
            m = re.fullmatch(pat, p.name)
            if m:
                topic_of[p] = (m.group(1), role)
                break
    by_topic = collections.defaultdict(list)
    for p, (t, role) in topic_of.items():
        by_topic[t].append((role, p))
    # A family needs a native part (probe, build or header) and at least two files.
    return {t: sorted(v, key=lambda rp: rp[1].name) for t, v in by_topic.items()
            if any(r in ("probe", "build", "header", "build regression", "build several") for r, _ in v)
            and (len(v) >= 2 or v[0][0] == "build several")}


def group_of(rel):
    for name, pats in GROUPS:
        if any(re.search(p, rel) for p in pats):
            return name
    return OTHER_GROUP


def family_topic(t):
    for name, keys in FAMILY_TOPICS:
        if any(k in t for k in keys):
            return name
    return "Other probe families"


def link(rel_from_tools):
    return "[%s](%s)" % (rel_from_tools, rel_from_tools)


def doc_link(doc, prefix="../docs/"):
    return "[%s](%s%s)" % (doc[:-3] if doc.endswith(".md") else doc, prefix, doc) if doc else ""


def retire_candidates(files):
    out = dict(RETIRE)
    by_hash = collections.defaultdict(list)
    for p in files:
        by_hash[hashlib.sha256(p.read_bytes().replace(b"\r\n", b"\n")).hexdigest()].append(p)
    for ps in by_hash.values():
        if len(ps) > 1:
            keep = ps[0].relative_to(TOOLS).as_posix()
            for p in ps[1:]:
                rel = p.relative_to(TOOLS).as_posix()
                out.setdefault(rel, "byte-identical to %s" % keep)
    return out


def render_tools():
    files = tool_files()
    mentions = doc_mentions()
    fams = families(files)
    in_family = {p for v in fams.values() for _, p in v}
    retire = retire_candidates(files)
    groups = collections.OrderedDict((name, []) for name, _ in GROUPS)
    groups[OTHER_GROUP] = []
    singles = collections.defaultdict(list)
    for p in files:
        if p in in_family:
            continue
        rel = p.relative_to(TOOLS).as_posix()
        desc = header(p) or "(no header comment)"
        doc = best_doc([p.name], mentions)
        mark = " **retire?**" if rel in retire else ""
        g = group_of("tools/" + rel)
        if g == OTHER_GROUP:
            # Single original-versus-port checks go with the probe families of their subsystem.
            singles[family_topic(p.stem)].append("| %s | %s%s | %s | %s |" % (
                re.sub(r"^(validate|audit|check|compare|replay)_", "", p.stem), desc, mark, link(rel), doc_link(doc)))
            continue
        groups[g].append("| %s | %s%s | %s |" % (link(rel), desc, mark, doc_link(doc)))
    out = [
        "# DRIG tools",
        "",
        "Generated by `py -I tools/index_files.py` from each file's header comment; the grouping and the",
        "retire list are tables in that script. Check here before writing a new tool, and regenerate in the",
        "same commit when you add, rename or retire one (`tools/log_entry.py check` notes files missing here).",
        "\"Doc\" is the document that names the file most often. Documents: [docs/INDEX.md](../docs/INDEX.md).",
        "",
        "Contents: " + " · ".join("[%s](#%s)" % (n, anchor(n)) for n in [g for g in groups if g != OTHER_GROUP]
                                   + ["Original-versus-port checks (probe families and validators)", "Candidates to retire"]),
        "",
    ]
    for name, rows in groups.items():
        if not rows:
            continue
        out += ["## " + name, "", "| File | What it does | Doc |", "|---|---|---|"] + rows + [""]
    out += [
        "## Original-versus-port checks (probe families and validators)",
        "",
        "Native harnesses that run original grid.exe code in isolation and compare it with the Rust port:",
        "`probe_X.c` (the harness), `build_X_probe.cmd` (MSVC x86 build into local/; a build script is listed",
        "with the probe it compiles), `X_probe.h` (shared declarations), `validate_X.py` (runs it, compares",
        "with the port), `audit_X.py` and `replay_X.py` (offline checks of a capture). One row per topic; the",
        "description is the first header found. Rows with a single file are validators without a probe.",
        "",
    ]
    by_sub = collections.defaultdict(list)
    for t in sorted(fams):
        members = fams[t]
        desc = next((h for h in (header(p) for r, p in sorted(members, key=lambda rp: FAMILY_ORDER.get(rp[0], 9))) if h),
                    "(no header comment)")
        names = ", ".join("[%s](%s)" % (p.name, p.relative_to(TOOLS).as_posix()) for _, p in members)
        doc = best_doc([p.name for _, p in members], mentions, t)
        mark = " **retire?**" if any(p.relative_to(TOOLS).as_posix() in retire for _, p in members) else ""
        by_sub[family_topic(t)].append("| %s | %s%s | %s | %s |" % (t, desc, mark, names, doc_link(doc)))
    for name, rows in singles.items():
        by_sub[name].extend(rows)
    for name in [n for n, _ in FAMILY_TOPICS] + ["Other probe families"]:
        by_sub[name] = sorted(by_sub.get(name, []), key=str.lower)
        if by_sub.get(name):
            out += ["### " + name, "", "| Topic | What it checks | Files | Doc |", "|---|---|---|---|"] + by_sub[name] + [""]
    out += ["## Candidates to retire", "",
            "Not deleted; listed so nobody extends them. Check the reason before reusing one.", ""]
    if retire:
        out += ["| File | Reason |", "|---|---|"]
        out += ["| %s | %s |" % (link(r), why) for r, why in sorted(retire.items())]
    else:
        out.append("None.")
    return "\n".join(out) + "\n"


FAMILY_ORDER = {"build several": 7, "validator": 0, "audit": 1, "probe": 2, "replay": 3, "header": 4, "build": 5, "build regression": 6}


def anchor(title):
    return re.sub(r"[^a-z0-9 -]", "", title.lower()).replace(" ", "-")


# ---- docs index ---------------------------------------------------------------------------

def doc_files():
    return sorted(p for p in DOCS.rglob("*") if p.is_file() and p != DOCS_INDEX)


def doc_summary(path):
    if path.suffix.lower() != ".md":
        return path.name, header(path) or "data file"
    lines = path.read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").split("\n")
    title = next((l.lstrip("# ").strip() for l in lines if l.startswith("# ")), path.stem)
    para, started = [], False
    for l in lines:
        s = l.strip()
        if not started:
            if not s or s.startswith("#") or s.startswith("<!--"):
                continue
            started = True
        if not s or s.startswith("#") or s.startswith("|") or s.startswith("```"):
            break
        para.append(s)
    text = " ".join(para)
    # Drop a leading "19 October 2026, Claude Code (F437)." style line.
    text = re.sub(r"^\*?\*?\d{1,2} October 2026[^.]*?\.\s*", "", text)
    text = re.sub(r"^(Date|Status|Author)[^.]*\.\s*", "", text)
    return title.replace("|", "\\|"), first_sentence(text, 120) if text else ""


def doc_group(rel):
    for name, pats in DOC_GROUPS:
        if any(re.search(p, rel) for p in pats):
            return name
    return "Other"


def render_docs():
    groups = collections.OrderedDict((name, []) for name, _ in DOC_GROUPS)
    groups["Other"] = []
    for p in doc_files():
        rel = p.relative_to(DOCS).as_posix()
        title, summary = doc_summary(p)
        groups[doc_group(rel)].append("- [%s](%s) %s%s" % (rel, rel, title, (" — " + summary) if summary else ""))
    out = [
        "# DRIG documents",
        "",
        "Generated by `py -I tools/index_files.py` (title and first sentence of each document); the grouping",
        "is a table in that script. Check here before writing a new document, and regenerate in the same",
        "commit when you add or rename one. Current state: [PROJECT_LOG.md](../PROJECT_LOG.md); log entries:",
        "[log/INDEX.md](../log/INDEX.md); tools: [tools/README.md](../tools/README.md).",
        "",
        "Contents: " + " · ".join("[%s](#%s)" % (n, anchor(n)) for n, rows in groups.items() if rows),
        "",
    ]
    for name, rows in groups.items():
        if rows:
            out += ["## " + name, ""] + rows + [""]
    return "\n".join(out)


# ---- main ---------------------------------------------------------------------------------

def missing():
    """Files in tools/ and docs/ that the committed indexes do not name."""
    out = []
    for index, files, base in ((TOOLS_INDEX, tool_files(), TOOLS), (DOCS_INDEX, doc_files(), DOCS)):
        text = index.read_text(encoding="utf-8") if index.exists() else ""
        for p in files:
            rel = p.relative_to(base).as_posix()
            if "(%s)" % rel not in text:
                out.append(p.relative_to(ROOT).as_posix())
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="only list files the indexes miss")
    a = ap.parse_args()
    if a.check:
        m = missing()
        for f in m:
            print("not indexed:", f)
        return 1 if m else 0
    TOOLS_INDEX.write_bytes(render_tools().encode("utf-8"))
    DOCS_INDEX.write_bytes(render_docs().encode("utf-8"))
    print("wrote tools/README.md and docs/INDEX.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
