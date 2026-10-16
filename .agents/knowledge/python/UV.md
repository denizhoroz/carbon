# uv

uv (Python package and interpreter manager) knowledge from previous projects. Python-wide notes are in `PYTHON.md`.


# Architecture

- `pyproject.toml` + committed `uv.lock`; uv-managed CPython; a JSON file in the venv records what was installed (lock hash, build).
- uv found as bundled binary, else on `PATH`, else downloaded (SHA-256 checked); never shipped (~40–50 MB).
- Hardware-specific builds as optional extras (e.g. `cpu`, `cu130`); switching is `uv sync --extra <build>`.
- Run `uv cache clean` after installing.


# Experience

## uv used the global Python folder
**Solution:** Always set `UV_PYTHON_INSTALL_DIR=<root>/python`, `UV_CACHE_DIR=<root>/.uv-cache`, `UV_PYTHON_PREFERENCE=only-managed`.

## Two dependencies install the same module
**Problem:** Two libraries depended on different distributions of one package (a full and a headless build) that install the same module. **Solution:** `[tool.uv] override-dependencies = ["<unwanted-dist>; sys_platform == 'never'"]` keeps only the one you want (here the smaller headless one).
