### Tickets

| Done | Priority | Severity | Title | Description | Suggested Fix |
|:---|:---|:---|:---|:---|:---|
| [x] | P1 | Medium | Documentation Mismatch for Build Process | The `README.md` file's "Building Executable" section still contains instructions for `cx-freeze`, but the project has been migrated to `pyinstaller` in `setup.py`. This will confuse anyone trying to build the application from source. | Update the "Building Executable" section in `README.md` with the correct commands and instructions for using `pyinstaller`. |
| [x] | P2 | Low | Inconsistent Comment in `run.py` | The comment in `run.py` for handling frozen executables says `Running as frozen executable (cx_Freeze)`, but the logic and build dependency (`setup.py`) have been changed to PyInstaller. | Change the comment in `run.py` from `(cx_Freeze)` to `(PyInstaller)` to accurately reflect the build system. |
| [x] | P3 | Low | Ambiguous Installation Method | The `README.md` removed the `pip install` instructions, but `setup.py` still defines `console_scripts` entry points, suggesting it can be installed as a package. This creates ambiguity about the intended distribution method. | Decide on the primary distribution method. If it's the executable, consider removing the `entry_points` from `setup.py` to avoid confusion. If `pip install` is still supported, restore that section in the `README.md`. |

### Unnecessary Code

- [ ] **`.gitignore`**: The removal of audio file patterns (`*.wav`, `*.mp3`, etc.) is acceptable as the project includes `.wav` files in its source tree (`src/sounds`), and this change prevents Git from ignoring them. No action is needed, but the change is noted.
- [ ] **`src/quillscribe/main.py`**: The refactoring of the icon loading logic is a minor but positive code quality improvement, making the code slightly cleaner.

### Edge Cases

- [ ] **PyInstaller Build Integrity**: The application's stability is now dependent on the PyInstaller build configuration. If the `.spec` file (or build command) fails to include the `icons` and `sounds` directories as data files, the application will run but fail to load these resources, resulting in a degraded user experience (missing icons/sounds).
- [ ] **Permissions on Frozen App**: When running the compiled `.exe` on a user's machine, it must have permission to write its configuration file. The `ConfigManager` should be checked to ensure it saves data in an appropriate user-specific location (e.g., `APPDATA`) rather than next to the executable, which may be in a restricted directory like `Program Files`.
- [ ] **`sys.path` Modification**: The use of `sys.path.insert(0, ...)` in `run.py` is a potential risk. If a file in the `src` directory has the same name as a standard Python library, it could be imported incorrectly, leading to unexpected behavior. The risk is low but present.

### Out‑of‑Scope Observations

- [ ] **`build_setup.py` & `installer.nsi`**: The repository contains files like `build_setup.py`, `build_installer.ps1`, and `installer.nsi` which appear related to building the application. These were not modified in the current diff and may now be obsolete or require updates to work with PyInstaller instead of cx_Freeze. They should be reviewed and either updated or removed.
- [ ] **Missing Build Instructions**: There are no explicit instructions or scripts provided for running PyInstaller with the correct options (e.g., adding data files like `icons` and `sounds`). This should be documented in the `README.md` or via a build script to ensure consistent builds.

---

### QA Summary
Overall, the changes represent a logical and well-executed migration from `cx-freeze` to `pyinstaller`. The code correctly handles resource pathing for frozen executables. The primary risks are not in the code itself, but in the surrounding process: outdated documentation and the lack of a defined build process for PyInstaller could cause issues for developers and prevent the application from being built correctly. The identified issues are mostly low-risk, with the documentation mismatch being the most pressing concern.
