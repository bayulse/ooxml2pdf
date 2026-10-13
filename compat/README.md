# Compatibility evaluation

Development tooling for the committed PPTX rendering foundation. Start with
[docs/COMPATIBILITY.md](../docs/COMPATIBILITY.md).

```sh
python3 -m venv compat/.venv
compat/.venv/bin/python -m pip install -r compat/scripts/requirements.txt
compat/.venv/bin/python compat/scripts/compat.py batch compat/corpus/public --font-dir fixtures/fonts
```

Without independent references the result is `NOT_COMPARED`, with PDFs, PNGs,
warnings, a corpus summary and a fidelity-gap report still generated.
