#!/usr/bin/env python3
"""Local PPTX fidelity evaluation. Runtime crates never invoke this tooling."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
RATINGS = ("PASS", "MINOR_DIFFERENCE", "MAJOR_DIFFERENCE", "FAILED_TO_RENDER", "NOT_COMPARED")
METRIC = {
    "name": "block-ssim-rgb-v1", "max_width": 640, "blur_radius": 0.6,
    "block_size": 8, "pass_mean": 0.015, "pass_worst_block": 0.15,
    "minor_mean": 0.08, "minor_worst_block": 0.6, "size_tolerance_pt": 0.1,
}


class EvaluationError(Exception):
    pass


def pillow():
    try:
        from PIL import Image, ImageChops, ImageFilter, ImageStat
        return Image, ImageChops, ImageFilter, ImageStat
    except ImportError as error:
        raise EvaluationError("Pillow is missing. Install compat/scripts/requirements.txt in a Python virtual environment.") from error


def execute(command, timeout=120, cwd=None, include_stderr=False):
    try:
        result = subprocess.run([str(x) for x in command], capture_output=True, text=True,
                                timeout=timeout, cwd=cwd, env={**os.environ, "LC_ALL": "C"})
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvaluationError(f"Cannot run {command[0]}: {error}") from error
    if result.returncode:
        raise EvaluationError(f"{command[0]} exited {result.returncode}: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout + (result.stderr if include_stderr else "")


def require_tool(name):
    path = shutil.which(name)
    if not path:
        raise EvaluationError(f"Missing development tool '{name}'. Install Poppler: brew install poppler (macOS), apt install poppler-utils (Debian/Ubuntu). It is not an ooxml2pdf dependency.")
    return path


def aggregate_warnings(warnings, locations=True):
    counts = Counter((w["kind"], w["feature"], w.get("location", "") if locations else "") for w in warnings)
    return [dict(kind=k, feature=f, location=l, count=n)
            for (k, f, l), n in sorted(counts.items(), key=lambda item: (-item[1], item[0]))]


def slide_rating(mean, worst_block):
    if mean <= METRIC["pass_mean"] and worst_block <= METRIC["pass_worst_block"]:
        return "PASS"
    if mean <= METRIC["minor_mean"] and worst_block <= METRIC["minor_worst_block"]:
        return "MINOR_DIFFERENCE"
    return "MAJOR_DIFFERENCE"


def compare_images(actual_path, reference_path, difference_path=None):
    """Box-window SSIM variant, not canonical Gaussian-window or multiscale SSIM."""
    Image, ImageChops, ImageFilter, ImageStat = pillow()
    with Image.open(actual_path) as source, Image.open(reference_path) as reference:
        actual_pixels, reference_pixels = list(source.size), list(reference.size)
        # Composite alpha onto white; compare rendered slides rather than hidden RGB.
        def white_rgb(image):
            rgba = image.convert("RGBA")
            return Image.alpha_composite(Image.new("RGBA", rgba.size, "white"), rgba).convert("RGB")
        actual, expected = white_rgb(source), white_rgb(reference)
    if difference_path:
        # Native-size diff is diagnostic only; unequal rasters get a white canvas.
        size = (max(actual.width, expected.width), max(actual.height, expected.height))
        a, b = Image.new("RGB", size, "white"), Image.new("RGB", size, "white")
        a.paste(actual); b.paste(expected)
        ImageChops.difference(a, b).save(difference_path)
    # Physical sizes are checked separately. Uniform DPI differences may be resampled;
    # aspect-ratio differences are reported and never silently stretched into a PASS.
    target_width = min(METRIC["max_width"], actual.width, expected.width)
    target_height = max(1, round(target_width * actual.height / actual.width))
    size = (target_width, target_height)
    actual = actual.resize(size, Image.Resampling.LANCZOS).filter(ImageFilter.GaussianBlur(METRIC["blur_radius"]))
    expected = expected.resize(size, Image.Resampling.LANCZOS).filter(ImageFilter.GaussianBlur(METRIC["blur_radius"]))
    rgb_error = sum(ImageStat.Stat(ImageChops.difference(actual, expected)).mean) / (3 * 255)
    a, b = list(actual.convert("L").get_flattened_data()), list(expected.convert("L").get_flattened_data())
    scores, ssim_scores, weights = [], [], []
    for y in range(0, target_height, METRIC["block_size"]):
        for x in range(0, target_width, METRIC["block_size"]):
            indices = [row * target_width + col
                       for row in range(y, min(y + 8, target_height))
                       for col in range(x, min(x + 8, target_width))]
            av, bv = [a[i] for i in indices], [b[i] for i in indices]
            n = len(indices)
            ma, mb = sum(av) / n, sum(bv) / n
            va = sum((v - ma) ** 2 for v in av) / n
            vb = sum((v - mb) ** 2 for v in bv) / n
            covariance = sum((u - ma) * (v - mb) for u, v in zip(av, bv)) / n
            ssim = ((2 * ma * mb + 6.5025) * (2 * covariance + 58.5225)
                    / ((ma * ma + mb * mb + 6.5025) * (va + vb + 58.5225)))
            # Include local chromatic error so isoluminant color changes are visible.
            box = (x, y, min(x + 8, target_width), min(y + 8, target_height))
            color_error = sum(ImageStat.Stat(ImageChops.difference(actual.crop(box), expected.crop(box))).mean) / (3 * 255)
            ssim_error = (1 - max(-1, min(1, ssim))) / 2
            ssim_scores.append(ssim_error)
            scores.append(max(ssim_error, color_error))
            weights.append(n)
    difference = sum(s * w for s, w in zip(scores, weights)) / sum(weights)
    return dict(actual_pixels=actual_pixels, reference_pixels=reference_pixels,
                pixel_dimensions_match=actual_pixels == reference_pixels,
                aspect_ratio_match=abs(actual_pixels[0] / actual_pixels[1] - reference_pixels[0] / reference_pixels[1]) <= 0.002,
                ssim_difference=sum(s * w for s, w in zip(ssim_scores, weights)) / sum(weights), blurred_rgb_error=rgb_error,
                difference=difference, worst_block_difference=max(scores),
                comparison_pixels=list(size))


def physical_size_match(actual, expected):
    return all(abs(actual[key] - expected[key]) <= METRIC["size_tolerance_pt"] for key in ("width_pt", "height_pt"))


def load_reference(directory):
    directory = Path(directory).resolve()
    manifest = directory / "reference.json"
    if not manifest.is_file():
        raise EvaluationError(f"Missing reference manifest: {manifest}. Use the 'reference' command after exporting slide PNGs.")
    try:
        data = json.loads(manifest.read_text())
        if not isinstance(data, dict):
            raise ValueError("manifest must be an object")
        if not isinstance(data.get("renderer"), str) or not data["renderer"].strip():
            raise ValueError("renderer must be named")
        if not data.get("pages"):
            raise ValueError("pages must be a nonempty ordered list")
        seen = set()
        for page in data["pages"]:
            image = (directory / page["image"]).resolve()
            if not image.is_relative_to(directory) or image in seen or not image.is_file():
                raise ValueError(f"missing, duplicate, or escaping image: {page['image']}")
            seen.add(image)
            for key in ("width_pt", "height_pt"):
                if isinstance(page[key], bool) or not isinstance(page[key], (float, int)) or not math.isfinite(page[key]) or page[key] <= 0:
                    raise ValueError(f"invalid {key}")
        return data
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise EvaluationError(f"Invalid reference manifest {manifest}: {error}") from error


def pdf_metadata(pdf, timeout):
    tool = require_tool("pdfinfo")
    info = execute([tool, pdf], timeout)
    match = re.search(r"^Pages:\s+(\d+)", info, re.M)
    if not match:
        raise EvaluationError("pdfinfo did not return a page count")
    count = int(match[1])
    detailed = execute([tool, "-f", 1, "-l", count, pdf], timeout)
    sizes = re.findall(r"^Page\s+(?:\d+\s+)?size:\s+([\d.]+) x ([\d.]+) pts", detailed, re.M)
    if len(sizes) != count:
        raise EvaluationError("pdfinfo did not return dimensions for every page")
    return [dict(width_pt=float(w), height_pt=float(h)) for w, h in sizes]


def git_metadata():
    try:
        return dict(commit=execute(["git", "rev-parse", "HEAD"], cwd=ROOT).strip(),
                    baseline=execute(["git", "describe", "--tags", "--always"], cwd=ROOT).strip(),
                    workspace_dirty=bool(execute(["git", "status", "--porcelain"], cwd=ROOT).strip()))
    except EvaluationError:
        return dict(commit=None, baseline=None, workspace_dirty=None)


def case_id(path):
    return re.sub(r"[^a-zA-Z0-9_.-]", "_", Path(path).stem) + "-" + hashlib.sha256(str(Path(path).resolve()).encode()).hexdigest()[:8]


def is_public(path):
    return Path(path).resolve().is_relative_to(ROOT / "compat/corpus/public")


def markdown_escape(value):
    return str(value).replace("|", "\\|").replace("\n", " ").replace("\r", " ")


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False, allow_nan=False) + "\n")


def case_markdown(result):
    ref = result.get("reference") or {}
    lines = ["# Compatibility Report", "", f"Case: {markdown_escape(result['input_file'])}",
             f"Reference: {markdown_escape(ref.get('renderer', 'unavailable'))} {markdown_escape(ref.get('version', ''))}",
             f"Engine: {result['engine'].get('engine_version', '?')} / {result['engine'].get('commit')}",
             f"Slides: {result['slide_count']} / {result.get('reference_slide_count', 'unavailable')}",
             f"Page-count match: {result.get('page_count_match')}; page-size match: {result.get('page_size_match')}",
             f"Overall paired-slide difference: {result.get('overall_difference')}; conversion seconds: {result.get('render_duration_seconds')}",
             "", "| Slide | Result | Difference | Pixels (actual / reference) | Images | Notes |",
             "| --- | --- | --- | --- | --- | --- |"]
    for slide in result["slides"]:
        score = slide.get("difference")
        links = " / ".join(f"[{label}](<{slide[key]}>)" for key, label in (("actual_image", "actual"), ("reference_image", "reference"), ("difference_image", "difference")) if key in slide)
        lines.append(f"| {slide['slide']} | {slide['rating']} | {format(score, '.6f') if score is not None else 'n/a'} | {slide.get('actual_pixels', 'n/a')} / {slide.get('reference_pixels', 'n/a')} | {links} | {markdown_escape('; '.join(slide.get('notes', [])))} |")
    lines += ["", "Warnings:"]
    lines += [f"- {markdown_escape(w['kind'])}: {markdown_escape(w['feature'])} ({markdown_escape(w['location'])}): {w['count']}" for w in result["warning_groups"]] or ["- None"]
    lines += ["", "Evaluation notes:"] + [f"- {markdown_escape(e)}" for e in result["evaluation_errors"]]
    lines += ["", f"Overall: **{result['overall_rating']}**", "", "Ratings are visual triage only. Warnings and semantic correctness require human review."]
    return "\n".join(lines) + "\n"


def check(input_file, output_dir, reference_dir=None, font_args=(), adapter=None, dpi=144, timeout=120):
    input_file, output_dir = Path(input_file).resolve(), Path(output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    # Fresh run directories prevent old PDFs/PNGs from masquerading as current output.
    artifacts = Path(tempfile.mkdtemp(prefix="run-", dir=output_dir))
    started = time.perf_counter()
    result = dict(schema_version=1, input_file=str(input_file), input_sha256=None,
                  engine=git_metadata(), metric=METRIC.copy(), raster_dpi=dpi,
                  reference=None, slide_count=0, reference_slide_count=None,
                  page_count_match=None, page_size_match=None, pages=[], slides=[],
                  warnings=[], warning_count=0, warning_groups=[], evaluation_errors=[],
                  render_duration_seconds=None, overall_difference=None,
                  overall_rating="FAILED_TO_RENDER", artifacts=str(artifacts))
    reference = None
    if reference_dir:
        try:
            reference = load_reference(reference_dir)
            result["reference"] = reference
            result["reference_slide_count"] = len(reference["pages"])
        except EvaluationError as error:
            result["evaluation_errors"].append(str(error))
    else:
        result["evaluation_errors"].append("No reference supplied; visual fidelity is not assessed.")
    telemetry = artifacts / "telemetry.json"
    try:
        if not input_file.is_file():
            raise EvaluationError(f"Missing PPTX input: {input_file}")
        result["input_sha256"] = hashlib.sha256(input_file.read_bytes()).hexdigest()
        rasterizer = require_tool("pdftoppm")
        require_tool("pdfinfo")
        pillow()
        result["rasterizer"] = execute([rasterizer, "-v"], timeout, include_stderr=True).strip()
        if adapter is None:
            execute(["cargo", "build", "--offline", "-p", "o2p-cli", "--example", "compat_render"], timeout, ROOT)
            adapter = ROOT / "target/debug/examples/compat_render"
        result["engine"].update(adapter_path=str(adapter), adapter_sha256=hashlib.sha256(Path(adapter).read_bytes()).hexdigest())
        pdf = artifacts / "rendered.pdf"
        try:
            execute([adapter, input_file, pdf, telemetry, *font_args], timeout)
        finally:
            if telemetry.exists():
                render = json.loads(telemetry.read_text())
                result["warnings"] = render["warnings"]
                result["pages"] = render["pages"]
                result["slide_count"] = len(render["pages"])
                result["engine"]["engine_version"] = render["engine_version"]
                result["render_duration_seconds"] = render["render_duration_seconds"]
        pages = pdf_metadata(pdf, timeout)
        result["pages"], result["slide_count"] = pages, len(pages)
        execute([rasterizer, "-r", dpi, "-png", pdf, artifacts / "slide"], timeout)
        images = sorted(artifacts.glob("slide-*.png"), key=lambda p: int(p.stem.split("-")[-1]))
        if len(images) != len(pages):
            raise EvaluationError("Rasterizer PNG count differs from PDF page count")
        result["overall_rating"] = "NOT_COMPARED"
        for index, image in enumerate(images):
            Image, *_ = pillow()
            with Image.open(image) as source:
                pixels = list(source.size)
            result["slides"].append(dict(slide=index + 1, actual_image=str(image),
                                         actual_pixels=pixels, actual_size=pages[index],
                                         rating="NOT_COMPARED", notes=[]))
        if reference:
            result["page_count_match"] = len(pages) == len(reference["pages"])
            sizes = []
            for index in range(max(len(pages), len(reference["pages"]))):
                if index >= len(pages):
                    result["slides"].append(dict(slide=index + 1, rating="MAJOR_DIFFERENCE", notes=["Missing engine page"]))
                    continue
                slide = result["slides"][index]
                if index >= len(reference["pages"]):
                    slide.update(rating="MAJOR_DIFFERENCE", notes=["No corresponding reference page"])
                    continue
                expected = reference["pages"][index]
                diff = artifacts / f"difference-{index + 1}.png"
                metrics = compare_images(images[index], Path(reference_dir) / expected["image"], diff)
                matched = physical_size_match(pages[index], expected)
                sizes.append(matched)
                slide.update(metrics, reference_size={k: expected[k] for k in ("width_pt", "height_pt")},
                             reference_image=str((Path(reference_dir) / expected["image"]).resolve()),
                             difference_image=str(diff), page_size_match=matched)
                slide["rating"] = slide_rating(metrics["difference"], metrics["worst_block_difference"])
                if not matched or not metrics["aspect_ratio_match"]:
                    slide.update(rating="MAJOR_DIFFERENCE", notes=["Physical size or image aspect ratio mismatch"])
                elif not metrics["pixel_dimensions_match"]:
                    slide["notes"].append("Different raster resolution; comparison resampled")
            result["page_size_match"] = result["page_count_match"] and all(sizes)
            differences = [s["difference"] for s in result["slides"] if "difference" in s]
            result["overall_difference"] = statistics.mean(differences) if differences else None
            result["worst_slide_difference"] = max(differences) if differences else None
            result["overall_rating"] = max((s["rating"] for s in result["slides"]), key=RATINGS.index)
            if not result["page_count_match"]:
                result["overall_rating"] = "MAJOR_DIFFERENCE"
                result["evaluation_errors"].append("Page-count mismatch; overall difference covers paired pages only.")
    except (EvaluationError, OSError, ValueError) as error:
        result["evaluation_errors"].append(str(error))
        result["overall_rating"] = "NOT_COMPARED" if result["slides"] else "FAILED_TO_RENDER"
    result["warning_count"] = len(result["warnings"])
    result["warning_groups"] = aggregate_warnings(result["warnings"])
    result["warnings_by_kind"] = dict(Counter(w["kind"] for w in result["warnings"]))
    result["total_duration_seconds"] = time.perf_counter() - started
    write_json(output_dir / "result.json", result)
    (output_dir / "report.md").write_text(case_markdown(result))
    return result


def batch_summary(results):
    counts = Counter(r["overall_rating"] for r in results)
    groups = aggregate_warnings([w for r in results for w in r["warnings"]], locations=False)
    for group in groups:
        group["case_count"] = sum(any(w["kind"] == group["kind"] and w["feature"] == group["feature"] for w in r["warnings"]) for r in results)
    return dict(schema_version=1, case_count=len(results), ratings={r: counts[r] for r in RATINGS},
                cases=[dict(input_file=r["input_file"], overall_rating=r["overall_rating"],
                            warning_count=r["warning_count"], overall_difference=r["overall_difference"],
                            artifacts=r["artifacts"]) for r in results], fidelity_gaps=groups)


def write_batch(results, directory):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    summary = batch_summary(results)
    write_json(directory / "summary.json", summary)
    lines = ["# Compatibility Corpus Report", "", f"{len(results)} cases", ""]
    lines += [f"- {rating}: {count}" for rating, count in summary["ratings"].items()]
    lines += ["", "| Case | Result | Warnings | Report |", "| --- | --- | --- | --- |"]
    lines += [f"| {markdown_escape(r['input_file'])} | {r['overall_rating']} | {r['warning_count']} | [details](<{Path(r['artifacts']).parent / 'report.md'}>) |" for r in results]
    (directory / "summary.md").write_text("\n".join(lines) + "\n")
    gaps = ["# Fidelity Gaps", "", "Warning occurrences are diagnostics, not counts of unique slide objects.",
            "Theme warnings can recur when a shared theme is processed for multiple slides.",
            "Rank by occurrences and affected cases; inspect per-case locations before choosing renderer work.",
            "", "| Kind | Feature | Occurrences | Cases |", "| --- | --- | --- | --- |"]
    gaps += [f"| {markdown_escape(g['kind'])} | {markdown_escape(g['feature'])} | {g['count']} | {g['case_count']} |" for g in summary["fidelity_gaps"]]
    if not summary["fidelity_gaps"]:
        gaps += ["", "No warnings observed. This does not establish semantic correctness or visual fidelity."]
    (directory / "fidelity-gaps.md").write_text("\n".join(gaps) + "\n")
    write_json(directory / "fidelity-gaps.json", summary["fidelity_gaps"])
    return summary


def create_reference(directory, renderer, version, width, height, notes):
    directory = Path(directory).resolve()
    images = sorted(directory.glob("*.png"), key=lambda p: [int(v) if v.isdigit() else v.lower() for v in re.split(r"(\d+)", p.name)])
    if not images:
        raise EvaluationError("No PNG reference images found")
    if (directory / "reference.json").exists():
        raise EvaluationError("reference.json already exists; edit it explicitly to preserve provenance")
    data = dict(renderer=renderer, version=version, notes=notes, created_utc=datetime.now(timezone.utc).isoformat(),
                pages=[dict(image=p.name, width_pt=width, height_pt=height) for p in images])
    write_json(directory / "reference.json", data)
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("check", "batch"):
        command = commands.add_parser(name)
        command.add_argument("input", type=Path)
        command.add_argument("--reference-dir" if name == "check" else "--reference-root", type=Path)
        command.add_argument("--output-dir", type=Path)
        if name == "batch":
            command.add_argument("--report-dir", type=Path)
        command.add_argument("--adapter", type=Path, help="prebuilt compat_render adapter")
        command.add_argument("--dpi", type=int, default=144)
        command.add_argument("--timeout", type=int, default=120)
        command.add_argument("--font-dir", type=Path, action="append", default=[])
        command.add_argument("--font", type=Path, action="append", default=[])
        command.add_argument("--fallback-family", action="append", default=[])
    ref = commands.add_parser("reference", help="write local metadata for exported reference PNGs")
    ref.add_argument("directory", type=Path)
    ref.add_argument("--renderer", required=True, help="e.g. Microsoft PowerPoint or LibreOffice")
    ref.add_argument("--version", required=True)
    ref.add_argument("--width-pt", required=True, type=float)
    ref.add_argument("--height-pt", required=True, type=float)
    ref.add_argument("--notes", required=True, help="export settings, OS, font setup, provenance")
    args = parser.parse_args()
    try:
        if args.command == "reference":
            if not all(math.isfinite(v) and v > 0 for v in (args.width_pt, args.height_pt)):
                raise EvaluationError("Reference dimensions must be positive finite point values")
            data = create_reference(args.directory, args.renderer, args.version, args.width_pt, args.height_pt, args.notes)
            print("Reference image order: " + ", ".join(p["image"] for p in data["pages"]))
            return 0
        if not 36 <= args.dpi <= 300 or args.timeout <= 0:
            raise EvaluationError("Use DPI between 36 and 300 and a positive timeout")
        if args.fallback_family and not (args.font or args.font_dir):
            raise EvaluationError("--fallback-family requires --font or --font-dir")
        visibility = "public" if is_public(args.input) else "private"
        output = args.output_dir or ROOT / "compat/outputs" / visibility
        font_args = []
        for flag, values in (("--font", args.font), ("--font-dir", args.font_dir), ("--fallback-family", args.fallback_family)):
            for value in values:
                font_args += [flag, str(value.resolve()) if isinstance(value, Path) else value]
        inputs = [args.input] if args.command == "check" else sorted(p for p in args.input.rglob("*") if p.is_file() and p.suffix.lower() == ".pptx")
        if not inputs:
            raise EvaluationError(f"No PPTX cases in {args.input}")
        results = []
        for path in inputs:
            reference = args.reference_dir if args.command == "check" else (args.reference_root / path.relative_to(args.input).with_suffix("") if args.reference_root else None)
            result = check(path, output / case_id(path), reference, font_args,
                           args.adapter.resolve() if args.adapter else None, args.dpi, args.timeout)
            results.append(result)
            print(f"{path.name}: {result['overall_rating']} ({result['warning_count']} warnings)")
        if args.command == "batch":
            summary = write_batch(results, args.report_dir or ROOT / "compat/reports" / visibility)
            print(f"{len(results)} cases: " + ", ".join(f"{v} {k}" for k, v in summary["ratings"].items()))
        # Nonzero for incomplete evaluation as well as major differences; reports always written.
        return int(any(r["overall_rating"] in ("FAILED_TO_RENDER", "NOT_COMPARED", "MAJOR_DIFFERENCE") for r in results))
    except (EvaluationError, OSError) as error:
        parser.exit(2, f"compat: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
