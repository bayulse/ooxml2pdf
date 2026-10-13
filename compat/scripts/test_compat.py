"""Evaluation tests; run with: python -m unittest discover -s compat/scripts -v."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import xml.etree.ElementTree as ET

import compat
import generate_corpus


class CompatibilityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.Image, *_ = compat.pillow()

    def image(self, name, color="white", size=(96, 64)):
        path = self.root / name
        self.Image.new("RGB", size, color).save(path)
        return path

    def reference(self, count=1, size=(960, 540)):
        ref = self.root / "reference"
        ref.mkdir(exist_ok=True)
        pages = []
        for i in range(count):
            self.Image.new("RGB", (96, 54), "white").save(ref / f"Slide{i + 1}.png")
            pages.append(dict(image=f"Slide{i + 1}.png", width_pt=size[0], height_pt=size[1]))
        compat.write_json(ref / "reference.json", dict(renderer="Test renderer (not Office)", version="1", pages=pages))
        return ref

    def run_case(self, reference=None, render_error=False, page_count=1, legacy_adapter=False):
        input_file = self.root / "input.pptx"
        input_file.write_bytes(b"test input")
        adapter = self.root / "adapter"
        adapter.write_bytes(b"test executable identity")

        def fake_execute(command, *args, **kwargs):
            if command[0] == adapter:
                compat.write_json(Path(command[3]), dict(engine_version="test", render_duration_seconds=0.125,
                    pages=[dict(width_pt=960, height_pt=540)] * page_count,
                    font_resolutions=[dict(requested_family="Example Display", resolved_family="Example Display",
                        resolved_weight=400, resolved_italic=False, postscript_name="ExampleDisplay")],
                    warnings=[dict(kind="UnsupportedSlideContent", feature="gradFill", location="ppt/slides/slide1.xml", message="unsupported gradient")],
                    error="malformed input" if render_error else None))
                if legacy_adapter:
                    telemetry = json.loads(Path(command[3]).read_text())
                    del telemetry["font_resolutions"]
                    compat.write_json(Path(command[3]), telemetry)
                if render_error:
                    raise compat.EvaluationError("malformed input")
                Path(command[2]).write_bytes(b"pdf")
                return ""
            if command[0] == "pdftoppm":
                if "-v" in command:
                    return "pdftoppm version test"
                for i in range(page_count):
                    self.Image.new("RGB", (96, 54), "white").save(str(command[-1]) + f"-{i + 1}.png")
                return ""
            raise AssertionError(command)

        with patch.object(compat, "git_metadata", return_value={"commit": "baseline", "workspace_dirty": False}), \
             patch.object(compat, "require_tool", side_effect=lambda name: name), \
             patch.object(compat, "pdf_metadata", return_value=[dict(width_pt=960, height_pt=540)] * page_count), \
             patch.object(compat, "execute", side_effect=fake_execute):
            return compat.check(input_file, self.root / "output", reference, adapter=adapter)

    def test_legacy_adapter_without_font_resolutions_remains_supported(self):
        result = self.run_case(self.reference(), legacy_adapter=True)
        self.assertEqual(result["overall_rating"], "PASS")
        self.assertEqual(result["font_resolutions"], [])

    def test_report_serialization_and_render_telemetry(self):
        result = self.run_case(self.reference())
        serialized = json.loads((self.root / "output/result.json").read_text())
        self.assertEqual(serialized, result)
        self.assertEqual(result["overall_rating"], "PASS")
        self.assertEqual(result["warning_count"], 1)
        self.assertEqual(result["render_duration_seconds"], 0.125)
        self.assertEqual(result["engine"]["commit"], "baseline")
        self.assertIn("gradFill", (self.root / "output/report.md").read_text())
        self.assertEqual(result["font_resolutions"][0]["requested_family"], "Example Display")
        self.assertEqual(result["font_resolutions"][0]["postscript_name"], "ExampleDisplay")

    def test_warning_aggregation_by_kind_feature_and_location(self):
        a = dict(kind="UnsupportedSlideContent", feature="gradFill", location="slide1")
        b = dict(a, location="slide2")
        c = dict(a, kind="UnsupportedTheme")
        groups = compat.aggregate_warnings([a, a, b, c])
        self.assertEqual(len(groups), 3)
        self.assertEqual(groups[0]["count"], 2)
        self.assertEqual(compat.aggregate_warnings([a, a, b], locations=False)[0]["count"], 3)

    def test_page_count_mismatch_missing_engine_page(self):
        result = self.run_case(self.reference(count=2))
        self.assertFalse(result["page_count_match"])
        self.assertEqual(result["overall_rating"], "MAJOR_DIFFERENCE")
        self.assertEqual(result["slides"][1]["notes"], ["Missing engine page"])

    def test_page_count_mismatch_extra_engine_page(self):
        result = self.run_case(self.reference(), page_count=2)
        self.assertFalse(result["page_count_match"])
        self.assertEqual(result["slides"][1]["rating"], "MAJOR_DIFFERENCE")

    def test_page_size_mismatch_is_major_even_with_identical_pixels(self):
        result = self.run_case(self.reference(size=(720, 405)))
        self.assertFalse(result["page_size_match"])
        self.assertEqual(result["overall_rating"], "MAJOR_DIFFERENCE")
        self.assertEqual(result["overall_difference"], 0)

    def test_missing_reference_is_not_a_pass_or_render_failure(self):
        result = self.run_case(self.root / "absent")
        self.assertEqual(result["overall_rating"], "NOT_COMPARED")
        self.assertIn("Missing reference manifest", result["evaluation_errors"][0])
        self.assertIsNone(result["overall_difference"])

    def test_no_reference_still_collects_actual_pixels_and_pdf_metadata(self):
        result = self.run_case()
        self.assertEqual(result["overall_rating"], "NOT_COMPARED")
        self.assertEqual(result["slides"][0]["actual_pixels"], [96, 54])

    def test_missing_rasterizer_writes_useful_failure_report(self):
        with patch.object(compat.shutil, "which", return_value=None):
            with self.assertRaisesRegex(compat.EvaluationError, "Install Poppler"):
                compat.require_tool("pdftoppm")
            input_file = self.root / "input.pptx"
            input_file.write_bytes(b"input")
            result = compat.check(input_file, self.root / "failure")
        self.assertEqual(result["overall_rating"], "FAILED_TO_RENDER")
        self.assertIn("pdftoppm", result["evaluation_errors"][1])
        self.assertTrue((self.root / "failure/result.json").exists())

    def test_failed_conversion_retains_structured_warnings(self):
        result = self.run_case(render_error=True)
        self.assertEqual(result["overall_rating"], "FAILED_TO_RENDER")
        self.assertEqual(result["warning_count"], 1)

    def test_old_raster_files_cannot_inflate_page_count(self):
        first = self.run_case(self.reference())
        self.Image.new("RGB", (96, 54)).save(Path(first["artifacts"]) / "slide-99.png")
        second = self.run_case(self.root / "reference")
        self.assertNotEqual(first["artifacts"], second["artifacts"])
        self.assertEqual(second["overall_rating"], "PASS")

    def test_metric_identity_and_large_difference(self):
        white, black = self.image("white.png"), self.image("black.png", "black")
        identical = compat.compare_images(white, white)
        large = compat.compare_images(white, black)
        self.assertEqual(identical["difference"], 0)
        self.assertGreater(large["difference"], 0.9)
        self.assertEqual(compat.slide_rating(large["difference"], large["worst_block_difference"]), "MAJOR_DIFFERENCE")

    def test_small_antialiasing_change_is_not_catastrophic(self):
        white, near = self.image("white.png"), self.image("near.png", (254, 254, 254))
        metrics = compat.compare_images(white, near)
        self.assertEqual(compat.slide_rating(metrics["difference"], metrics["worst_block_difference"]), "PASS")

    def test_small_edge_antialiasing_changes_are_tolerated(self):
        first, second = self.Image.new("RGB", (96, 64), "white"), self.Image.new("RGB", (96, 64), "white")
        first.paste((192, 192, 192), (24, 16, 72, 48))
        second.paste((188, 188, 188), (24, 16, 72, 48))
        first.paste("black", (25, 17, 71, 47)); second.paste("black", (25, 17, 71, 47))
        first.save(self.root / "first.png"); second.save(self.root / "second.png")
        metrics = compat.compare_images(self.root / "first.png", self.root / "second.png")
        self.assertEqual(compat.slide_rating(metrics["difference"], metrics["worst_block_difference"]), "PASS")

    def test_pdf_metadata_reads_each_physical_page(self):
        with patch.object(compat, "require_tool", return_value="pdfinfo"), \
             patch.object(compat, "execute", side_effect=["Pages: 2\n", "Page    1 size: 960 x 540 pts\nPage    2 size: 720 x 405 pts\n"]):
            self.assertEqual(compat.pdf_metadata(self.root / "deck.pdf", 1),
                             [dict(width_pt=960, height_pt=540), dict(width_pt=720, height_pt=405)])

    def test_pdf_metadata_rejects_incomplete_output(self):
        with patch.object(compat, "require_tool", return_value="pdfinfo"), \
             patch.object(compat, "execute", side_effect=["Pages: 2\n", "Page size: 960 x 540 pts\n"]):
            with self.assertRaisesRegex(compat.EvaluationError, "every page"):
                compat.pdf_metadata(self.root / "deck.pdf", 1)

    def test_local_omission_survives_low_slide_mean(self):
        white = self.image("white.png", size=(256, 256))
        mark = self.Image.new("RGB", (256, 256), "white")
        mark.paste("black", (32, 32, 48, 48))
        mark.save(self.root / "mark.png")
        metrics = compat.compare_images(white, self.root / "mark.png")
        self.assertLess(metrics["difference"], compat.METRIC["pass_mean"])
        self.assertEqual(compat.slide_rating(metrics["difference"], metrics["worst_block_difference"]), "MAJOR_DIFFERENCE")

    def test_chromatic_difference_is_measured(self):
        first, second = self.image("a.png", (255, 0, 0)), self.image("b.png", (0, 130, 0))
        self.assertGreater(compat.compare_images(first, second)["blurred_rgb_error"], 0.4)

    def test_different_dpi_reports_pixels_separately(self):
        first, second = self.image("a.png"), self.image("b.png", size=(192, 128))
        metrics = compat.compare_images(first, second)
        self.assertFalse(metrics["pixel_dimensions_match"])
        self.assertTrue(metrics["aspect_ratio_match"])
        self.assertEqual(metrics["difference"], 0)

    def test_rating_threshold_boundaries(self):
        self.assertEqual(compat.slide_rating(0.015, 0.15), "PASS")
        self.assertEqual(compat.slide_rating(0.015001, 0.15), "MINOR_DIFFERENCE")
        self.assertEqual(compat.slide_rating(0.08, 0.6), "MINOR_DIFFERENCE")
        self.assertEqual(compat.slide_rating(0.080001, 0.6), "MAJOR_DIFFERENCE")
        self.assertEqual(compat.slide_rating(0, 0.600001), "MAJOR_DIFFERENCE")

    def test_batch_summary_and_fidelity_gap_reports(self):
        results = [self.run_case(self.reference()), self.run_case()]
        summary = compat.write_batch(results, self.root / "batch")
        self.assertEqual(summary["case_count"], 2)
        self.assertEqual(summary["ratings"]["PASS"], 1)
        self.assertEqual(summary["ratings"]["NOT_COMPARED"], 1)
        self.assertEqual(summary["fidelity_gaps"][0]["case_count"], 2)
        self.assertEqual(summary["fidelity_gaps"][0]["count"], 2)
        self.assertEqual(json.loads((self.root / "batch/summary.json").read_text()), summary)

    def test_invalid_reference_provenance_dimensions_and_traversal(self):
        ref = self.reference()
        manifest = ref / "reference.json"
        original = json.loads(manifest.read_text())
        for key, value in (("renderer", ""), ("pages", [{"image": "../../secret.png", "width_pt": 960, "height_pt": 540}]),
                           ("pages", [{"image": "Slide1.png", "width_pt": float("nan"), "height_pt": 540}])):
            data = dict(original, **{key: value})
            manifest.write_text(json.dumps(data))
            with self.assertRaises(compat.EvaluationError):
                compat.load_reference(ref)

    def test_reference_creation_uses_natural_order_and_preserves_existing_manifest(self):
        self.image("Slide10.png"); self.image("Slide2.png"); self.image("Slide1.png")
        data = compat.create_reference(self.root, "LibreOffice", "test", 960, 540, "manual export")
        self.assertEqual([p["image"] for p in data["pages"]], ["Slide1.png", "Slide2.png", "Slide10.png"])
        self.assertEqual(compat.load_reference(self.root)["renderer"], "LibreOffice")
        with self.assertRaisesRegex(compat.EvaluationError, "already exists"):
            compat.create_reference(self.root, "PowerPoint", "test", 960, 540, "test")

    def test_private_defaults_and_case_name_collisions(self):
        self.assertFalse(compat.is_public(self.root / "private.pptx"))
        self.assertTrue(compat.is_public(compat.ROOT / "compat/corpus/public/deck.pptx"))
        self.assertNotEqual(compat.case_id(self.root / "a/deck.pptx"), compat.case_id(self.root / "b/deck.pptx"))

    def test_synthetic_generator_is_deterministic_and_public_corpus_has_valid_two_slide_xml(self):
        for case in generate_corpus.cases():
            path = compat.ROOT / "compat/corpus/public" / (case[0] + ".pptx")
            before = path.read_bytes()
            generated = generate_corpus.build_deck(*case, directory=self.root / "generated")
            repeated = generate_corpus.build_deck(*case, directory=self.root / "repeated")
            self.assertEqual(generated.read_bytes(), repeated.read_bytes())
            # The public benchmark now contains paired PowerPoint-authored decks.
            # Synthetic generation must not overwrite or redefine those inputs.
            self.assertEqual(path.read_bytes(), before)
            with zipfile.ZipFile(path) as archive:
                slides = [name for name in archive.namelist() if re_slide(name)]
                self.assertEqual(len(slides), 2)
                for name in archive.namelist():
                    if name.endswith((".xml", ".rels")):
                        ET.fromstring(archive.read(name))

    def test_real_adapter_json_and_pdf_match_normal_o2p_conversion(self):
        adapter = compat.ROOT / "target/debug/examples/compat_render"
        cli = compat.ROOT / "target/debug/o2p"
        if not adapter.exists() or not cli.exists():
            self.skipTest("Build o2p and compat_render to exercise conversion parity")
        input_file = compat.ROOT / "compat/corpus/public/001-basic-shapes.pptx"
        font_args = ["--font-dir", str(compat.ROOT / "fixtures/fonts")]
        subprocess.run([str(adapter), str(input_file), str(self.root / "adapter.pdf"), str(self.root / "telemetry.json"), *font_args], check=True, capture_output=True)
        subprocess.run([str(cli), "convert", str(input_file), "-o", str(self.root / "cli.pdf"), *font_args], check=True, capture_output=True)
        self.assertEqual((self.root / "adapter.pdf").read_bytes(), (self.root / "cli.pdf").read_bytes())
        self.assertEqual(len(json.loads((self.root / "telemetry.json").read_text())["pages"]), 2)
        resolutions = json.loads((self.root / "telemetry.json").read_text())["font_resolutions"]
        self.assertTrue(resolutions)
        self.assertTrue(all({"requested_family", "resolved_family", "resolved_subfamily",
            "resolved_weight", "resolved_italic", "postscript_name", "full_name"} <= entry.keys()
            for entry in resolutions))
        self.assertEqual(len(resolutions), len({json.dumps(entry, sort_keys=True) for entry in resolutions}))

    def test_real_poppler_pipeline_self_check_not_office(self):
        adapter = compat.ROOT / "target/debug/examples/compat_render"
        if not adapter.exists() or not all(compat.shutil.which(tool) for tool in ("pdftoppm", "pdfinfo")):
            self.skipTest("Build adapter and install optional Poppler for pipeline smoke test")
        input_file = compat.ROOT / "compat/corpus/public/002-images.pptx"
        font_args = ["--font-dir", str(compat.ROOT / "fixtures/fonts")]
        first = compat.check(input_file, self.root / "first", font_args=font_args, adapter=adapter)
        self.assertEqual(first["overall_rating"], "NOT_COMPARED")
        reference = self.root / "self-check-reference"
        reference.mkdir()
        for index, slide in enumerate(first["slides"]):
            compat.shutil.copyfile(slide["actual_image"], reference / f"Slide{index + 1}.png")
        compat.create_reference(reference, "Harness self-check (o2p; not independent Office reference)", "test", 960, 540, "Temporary pipeline test only")
        second = compat.check(input_file, self.root / "second", reference, font_args=font_args, adapter=adapter)
        self.assertEqual(second["overall_rating"], "PASS")
        self.assertEqual(second["overall_difference"], 0)
        self.assertTrue(second["page_count_match"])
        self.assertTrue(second["page_size_match"])
        self.assertEqual(second["slide_count"], 2)
        self.assertTrue(all(Path(s["difference_image"]).exists() for s in second["slides"]))


def re_slide(name):
    return name.startswith("ppt/slides/slide") and name.endswith(".xml") and "/_rels/" not in name


if __name__ == "__main__":
    unittest.main()
