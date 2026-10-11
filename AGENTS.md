# AGENTS.md

## Project

This repository contains `ooxml2pdf`, a lightweight native Rust
OOXML-to-PDF rendering engine.

Supported formats eventually:

- DOCX -> PDF
- XLSX -> PDF
- PPTX -> PDF

The first implementation target is PPTX -> PDF.

## Core principles

- Rust-native processing.
- Fully offline at runtime.
- No LibreOffice.
- No Chromium.
- No WebView.
- No JavaScript runtime.
- No external processes.
- No cloud APIs.
- Core library must not depend on Flutter.
- Input documents must be treated as untrusted data.
- Prefer correctness and maintainability over broad feature coverage.

## Architecture

The intended rendering pipeline is:

OOXML package
-> format parser
-> normalized document model
-> layout
-> common DisplayList
-> PDF renderer

PDF renderer must not understand PPTX/DOCX/XLSX semantics.

Format parsers must not directly generate PDF commands.

## Current priority

Do not implement DOCX or XLSX unless explicitly requested.

Current milestone:

PPTX -> PDF proof of concept.

Initial PPTX feature scope:

- slide dimensions
- text boxes
- text runs
- images
- rectangles
- ellipses
- lines
- fill colors
- borders

Do not implement yet:

- charts
- SmartArt
- animations
- WordArt
- complex effects
- DOCX
- XLSX
- Flutter integration

## Engineering rules

- Do not silently ignore unsupported OOXML features.
- Return structured warnings instead.
- Avoid panics for malformed user input.
- Do not expose third-party parser types through the public API.
- Keep third-party dependencies behind adapters where reasonable.
- Add tests for every parser or rendering behavior.
- Add regression fixtures for rendering bugs.
- Run cargo fmt, cargo clippy, and cargo test before completing work.
- Do not perform large architectural rewrites without documenting why.

## Dependency policy

Before introducing a dependency:

1. explain why it is necessary,
2. check its license,
3. prefer maintained Rust libraries,
4. avoid dependencies that introduce network/runtime services,
5. avoid GPL dependencies unless explicitly approved.

## Definition of done

A change is complete only when:

- code compiles,
- tests pass,
- clippy passes,
- architecture remains consistent with the design documents,
- new behavior is covered by tests,
- documentation is updated when public behavior changes.
