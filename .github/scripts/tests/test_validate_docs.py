from __future__ import annotations

import importlib.util
import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "validate_docs.py"
SPEC = importlib.util.spec_from_file_location("validate_docs", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
validator = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = validator
SPEC.loader.exec_module(validator)

ROOT = Path(__file__).resolve().parents[3]

TAURI_AUTHORITY_SOURCE = """\
ALLOWED_COMMANDS = {
    "list_storage_sources",
    "select_scan_folder",
    "scan_storage_volume",
    "get_candidate_page",
    "query_candidate_page",
    "update_candidate_selection",
    "select_restore_destination",
    "create_restore_plan",
    "start_restore",
    "get_restore_job",
    "cancel_restore",
    "open_restore_destination",
}
"""

VALID_TAURI_REGISTRATION = """\
pub fn run() {
    tauri::Builder::default().invoke_handler(tauri::generate_handler![
        storage::list_storage_sources,
        storage::select_scan_folder,
        storage::scan_storage_volume,
        storage::get_candidate_page,
        storage::query_candidate_page,
        storage::update_candidate_selection,
        restore::select_restore_destination,
        restore::create_restore_plan,
        restore::start_restore,
        restore::get_restore_job,
        restore::cancel_restore,
        restore::open_restore_destination
    ]);
}
"""

VALID_TAURI_REGISTRATIONS = {
    "storage::list_storage_sources",
    "storage::select_scan_folder",
    "storage::scan_storage_volume",
    "storage::get_candidate_page",
    "storage::query_candidate_page",
    "storage::update_candidate_selection",
    "restore::select_restore_destination",
    "restore::create_restore_plan",
    "restore::start_restore",
    "restore::get_restore_job",
    "restore::cancel_restore",
    "restore::open_restore_destination",
}


class DocumentationValidatorTests(unittest.TestCase):
    def copy_repository_fixture(self, directory: str) -> Path:
        destination = Path(directory) / "repository"
        return Path(
            shutil.copytree(
                ROOT,
                destination,
                ignore=shutil.ignore_patterns(
                    ".git",
                    ".superpowers",
                    "coverage",
                    "dist",
                    "node_modules",
                    "target",
                ),
            )
        )

    def current_product_fixture(
        self,
        directory: str,
        *,
        documents: dict[str, str] | None = None,
        registration: str = VALID_TAURI_REGISTRATION,
    ) -> Path:
        root = Path(directory) / "repository"
        authority = root / ".github/scripts/validate_real_only_desktop.py"
        authority.parent.mkdir(parents=True)
        authority.write_text(TAURI_AUTHORITY_SOURCE, encoding="utf-8")
        lib_rs = root / "apps/desktop/src-tauri/src/lib.rs"
        lib_rs.parent.mkdir(parents=True)
        lib_rs.write_text(registration, encoding="utf-8")
        for relative, text in (documents or {}).items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        return root

    def test_docs_catalog_001_exact_fr_sets(self) -> None:
        text = (
            "| ID | Title | Behavior and acceptance criteria | Status |\n"
            "| --- | --- | --- | --- |\n"
            "| FR-001 | One | Behavior | Partial |\n"
            "| FR-002 | Two | Behavior | Not started |\n"
        )

        catalog, duplicates = validator.parse_functional_catalog(text)

        self.assertEqual(
            catalog,
            {"FR-001": "Partial", "FR-002": "Not started"},
        )
        self.assertEqual(duplicates, [])

    def test_docs_status_001_rejects_unapproved_requirement_status(self) -> None:
        self.assertFalse(validator.is_allowed_status("Plan" + "ned"))
        self.assertTrue(validator.is_allowed_status("Implemented-unverified"))

    def test_docs_matrix_001_expands_ranges_and_lists(self) -> None:
        self.assertEqual(
            validator.expand_fr_cell("FR-001–003, FR-010"),
            ("FR-001", "FR-002", "FR-003", "FR-010"),
        )

    def test_docs_requirement_sets_001_rejects_missing_nfr_or_sdd(self) -> None:
        errors: list[str] = []

        validator.validate_exact_requirement_set(
            {"NFR-001"},
            {"NFR-001", "NFR-002"},
            errors,
            "fixture NFR catalog",
        )

        self.assertTrue(any("missing" in error and "NFR-002" in error for error in errors), errors)

    def test_docs_status_sync_001_rejects_normative_matrix_drift(self) -> None:
        errors: list[str] = []

        validator.validate_status_alignment(
            {"SDD-UI-001": "Partial"},
            {"SDD-UI-001": "Not started"},
            errors,
            "fixture current matrix",
        )

        self.assertTrue(any("SDD-UI-001" in error and "disagrees" in error for error in errors), errors)

    def test_docs_repository_requirement_sets_002_rejects_removed_sections(self) -> None:
        fixtures = (
            (
                "docs/specs/002-non-functional-requirements.md",
                r"\n## NFR-008\b.*\Z",
                "NFR-008",
            ),
            (
                "docs/specs/016-foundation-hardening-and-safe-image-cli.md",
                r"\n### SDD-UI-003\b.*?(?=\n### SDD-|\n## |\Z)",
                "SDD-UI-003",
            ),
            (
                "docs/specs/017-real-only-image-desktop.md",
                r"\n### SDD-REAL-012\b.*?(?=\n### SDD-|\n## |\Z)",
                "SDD-REAL-012",
            ),
            (
                "docs/specs/018-windows-volume-and-folder-scan.md",
                r"\n### SDD-WIN-013\b.*?(?=\n### SDD-|\n## |\Z)",
                "SDD-WIN-013",
            ),
        )
        for relative, pattern, missing_id in fixtures:
            with self.subTest(missing_id=missing_id), tempfile.TemporaryDirectory() as directory:
                root = self.copy_repository_fixture(directory)
                path = root / relative
                text = path.read_text(encoding="utf-8")
                changed, count = re.subn(pattern, "", text, count=1, flags=re.DOTALL)
                self.assertEqual(count, 1)
                path.write_text(changed, encoding="utf-8")

                errors, _ = validator.validate_repository(root)

                self.assertTrue(
                    any("missing requirements" in error and missing_id in error for error in errors),
                    errors,
                )

    def test_docs_repository_status_sync_002_rejects_matrix_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy_repository_fixture(directory)
            path = root / "docs/traceability-matrix.md"
            text = path.read_text(encoding="utf-8")
            lines = text.splitlines()
            row_index = next(
                index
                for index, line in enumerate(lines)
                if line.startswith("| NFR-001 |")
            )
            self.assertTrue(lines[row_index].endswith("| Partial |"))
            lines[row_index] = (
                lines[row_index][: -len("| Partial |")]
                + "| Implemented-unverified |"
            )
            changed = "\n".join(lines) + "\n"
            self.assertNotEqual(changed, text)
            path.write_text(changed, encoding="utf-8")

            errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any("NFR-001" in error and "disagrees" in error for error in errors),
            errors,
        )

    def test_docs_repository_matrix_coverage_002_rejects_missing_nfr_row(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy_repository_fixture(directory)
            path = root / "docs/traceability-matrix.md"
            lines = path.read_text(encoding="utf-8").splitlines()
            changed = "\n".join(line for line in lines if not line.startswith("| NFR-008 |"))
            path.write_text(changed + "\n", encoding="utf-8")

            errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any("NFR matrix" in error and "NFR-008" in error for error in errors),
            errors,
        )

    def test_docs_real_matrix_001_rejects_missing_sdd017_row(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy_repository_fixture(directory)
            path = root / "docs/traceability-matrix.md"
            lines = path.read_text(encoding="utf-8").splitlines()
            changed = "\n".join(
                line for line in lines if not line.startswith("| SDD-REAL-012 |")
            )
            path.write_text(changed + "\n", encoding="utf-8")

            errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any(
                "real-only matrix" in error and "SDD-REAL-012" in error
                for error in errors
            ),
            errors,
        )

    def test_docs_windows_matrix_001_rejects_missing_sdd018_row(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy_repository_fixture(directory)
            path = root / "docs/traceability-matrix.md"
            lines = path.read_text(encoding="utf-8").splitlines()
            changed = "\n".join(
                line for line in lines if not line.startswith("| SDD-WIN-013 |")
            )
            path.write_text(changed + "\n", encoding="utf-8")

            errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any(
                "connected-volume matrix" in error and "SDD-WIN-013" in error
                for error in errors
            ),
            errors,
        )

    def test_docs_repository_code_path_002_rejects_broken_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy_repository_fixture(directory)
            path = root / "docs/traceability-matrix.md"
            text = path.read_text(encoding="utf-8")
            changed = text.replace(
                "`crates/core/src/source.rs`",
                "`crates/core/src/does-not-exist.rs`",
                1,
            )
            self.assertNotEqual(changed, text)
            path.write_text(changed, encoding="utf-8")

            errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any("implementation path does not exist" in error for error in errors),
            errors,
        )

    def test_docs_repository_justification_002_rejects_undefined_and_duplicate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy_repository_fixture(directory)
            matrix = root / "docs/traceability-matrix.md"
            matrix_text = matrix.read_text(encoding="utf-8").replace(
                "JUST-NFR-001-BROKER-PENDING",
                "JUST-NFR-001-UNDEFINED",
                1,
            )
            matrix.write_text(matrix_text, encoding="utf-8")
            justifications = root / "docs/test-justifications.md"
            justification_text = justifications.read_text(encoding="utf-8")
            first = re.search(
                r"(## JUST-[A-Z0-9-]+\n.*?)(?=\n## JUST-)",
                justification_text,
                re.DOTALL,
            )
            self.assertIsNotNone(first)
            justifications.write_text(
                justification_text + "\n" + first.group(1) + "\n",
                encoding="utf-8",
            )

            errors, _ = validator.validate_repository(root)

        self.assertTrue(any("undefined formal justification" in error for error in errors), errors)
        self.assertTrue(any("duplicate justification headings" in error for error in errors), errors)

    def test_docs_repository_actual_test_002_comment_is_not_a_test(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.copy_repository_fixture(directory)
            matrix = root / "docs/traceability-matrix.md"
            matrix_text = matrix.read_text(encoding="utf-8").replace(
                "`IO-SECTOR-ZERO-001`",
                "`COMMENT-ONLY-001`",
                1,
            )
            matrix.write_text(matrix_text, encoding="utf-8")
            source = root / "crates/core/src/lib.rs"
            source.write_text(
                source.read_text(encoding="utf-8") + "\n// COMMENT-ONLY-001\n",
                encoding="utf-8",
            )

            errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any("test ID is not resolvable" in error and "COMMENT-ONLY-001" in error for error in errors),
            errors,
        )

    def test_docs_actual_test_001_comments_do_not_resolve_test_ids(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rust = root / "crates" / "sample" / "src" / "lib.rs"
            rust.parent.mkdir(parents=True)
            rust.write_text(
                "// #[test]\n"
                "// fn fake_comment_001_looks_executable() {}\n"
                "const SENTINEL: () = (); // #[test]\n"
                "fn inline_comment_002_looks_executable() {}\n"
                "/* #[test]\n"
                "fn block_comment_003_looks_executable() {}\n"
                "*/\n"
                "/* outer comment\n"
                "/* inner comment */\n"
                "#[test] fn nested_comment_004_looks_executable() {}\n"
                "*/\n"
                'const FAKE: &str = "#[test] '
                'fn rust_string_005_looks_executable() {}";\n'
                'const URL: &str = "https://example.invalid/a//b"; '
                '#[test] fn real_declared_001_rejects_bad_input() {}\n',
                encoding="utf-8",
            )
            python_test = root / ".github" / "scripts" / "tests" / "test_sample.py"
            python_test.parent.mkdir(parents=True)
            python_test.write_text(
                "# PYTHON-COMMENT-001\n"
                "PYTHON_EXAMPLE = '''\n"
                "def test_python_string_002_looks_executable():\n"
                "    pass\n"
                "'''\n"
                "def test_python_declared_001_works():\n"
                "    pass\n",
                encoding="utf-8",
            )

            declarations = validator.executable_test_declarations(root)

        self.assertTrue(
            validator.test_id_is_resolvable("REAL-DECLARED-001", declarations)
        )
        self.assertTrue(
            validator.test_id_is_resolvable("PYTHON-DECLARED-001", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("FAKE-COMMENT-001", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("PYTHON-COMMENT-001", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("PYTHON-STRING-002", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("INLINE-COMMENT-002", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("BLOCK-COMMENT-003", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("NESTED-COMMENT-004", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("RUST-STRING-005", declarations)
        )

    def test_docs_actual_test_003_inline_web_comments_do_not_declare_tests(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            web = root / "apps" / "sample" / "src" / "comment.test.ts"
            web.parent.mkdir(parents=True)
            web.write_text(
                'const url = "https://example.invalid/a//b"; '
                'test("WEB-REAL-003 works", () => {});\n'
                'const sentinel = 1; // test("WEB-COMMENT-001 fake", () => {});\n'
                '/* test("WEB-BLOCK-002 fake", () => {}); */\n'
                'const marker = "/* retained string marker */";\n'
                'const fake = "test(\\"WEB-STRING-004 fake\\", () => {})";\n'
                "const fakeSingle = 'test(\"WEB-STRING-005 fake\", () => {})';\n"
                "const fakeTemplate = `test(\"WEB-STRING-006 fake\", () => {})`;\n"
                'const fakeRegex = /test("WEB-REGEX-008 fake", () => {})/;\n',
                encoding="utf-8",
            )

            declarations = validator.executable_test_declarations(root)

        self.assertTrue(
            validator.test_id_is_resolvable("WEB-REAL-003", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("WEB-COMMENT-001", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("WEB-BLOCK-002", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("WEB-STRING-004", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("WEB-STRING-005", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("WEB-STRING-006", declarations)
        )
        self.assertFalse(
            validator.test_id_is_resolvable("WEB-REGEX-008", declarations)
        )

    def test_docs_justification_001_duplicate_headings_are_rejected(self) -> None:
        text = (
            "## JUST-FIXTURE-DUPLICATE\n\n"
            "- **Requirements:** FR-001.\n\n"
            "## JUST-FIXTURE-DUPLICATE\n\n"
            "- **Requirements:** FR-002.\n"
        )

        duplicates = validator.duplicate_section_ids(
            text,
            "##",
            r"JUST-[A-Z0-9-]+",
        )

        self.assertEqual(duplicates, ["JUST-FIXTURE-DUPLICATE"])

    def test_docs_links_001_rejects_missing_local_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            docs = root / "docs"
            docs.mkdir()
            (docs / "readme.md").write_text(
                "[missing](does-not-exist.md)\n",
                encoding="utf-8",
            )
            errors: list[str] = []

            validator.validate_links_and_placeholders(root, errors)

        self.assertTrue(any("broken local link" in error for error in errors), errors)

    def test_docs_code_path_001_rejects_missing_implementation_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            row = validator.MatrixRow(
                requirements=("NFR-001",),
                cells=(
                    "NFR-001",
                    "design",
                    "`crates/missing/src/lib.rs`",
                    "`JUST-NFR-001-FIXTURE`",
                    "—",
                    "—",
                    "evidence",
                    "Partial",
                ),
            )
            errors: list[str] = []

            validator.validate_code_paths(root, row, errors, "fixture:NFR-001")

        self.assertTrue(
            any("implementation path does not exist" in error for error in errors),
            errors,
        )

    def test_docs_s0_001_requires_assumptions_inventory_and_risks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / "docs" / "evidence"
            evidence.mkdir(parents=True)
            (evidence / "environment-inventory.md").write_text(
                "# S0 Environment Inventory\n\n## Repository\n\nFixture.\n",
                encoding="utf-8",
            )
            errors: list[str] = []

            validator.validate_s0_inventory(root, errors)

        self.assertTrue(any("Assumptions" in error for error in errors), errors)
        self.assertTrue(
            any("Agents, skills, plugins, and MCP inventory" in error for error in errors),
            errors,
        )
        self.assertTrue(any("Preliminary risks" in error for error in errors), errors)

    def test_docs_s0_structure_002_rejects_empty_governance_sections(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / "docs" / "evidence"
            evidence.mkdir(parents=True)
            (evidence / "environment-inventory.md").write_text(
                "# S0 Environment Inventory\n\n"
                "## Assumptions\n\nA-S0-001.\n\n"
                "## Agents, skills, plugins, and MCP inventory\n\n"
                "| Category | Used capability | Purpose / boundary |\n"
                "| --- | --- | --- |\n"
                "| Agent | One | Fixture |\n\n"
                "## Preliminary risks\n\n"
                "[R-001](../risk-register.md).\n",
                encoding="utf-8",
            )
            errors: list[str] = []

            validator.validate_s0_inventory(root, errors)

        self.assertTrue(any("at least four unique A-S0" in error for error in errors), errors)
        self.assertTrue(any("skills/plugins" in error for error in errors), errors)
        self.assertTrue(any("at least four unique risk-register links" in error for error in errors), errors)

    def test_docs_adr_topic_001_requires_matching_topic_metadata(self) -> None:
        self.assertEqual(
            validator.adr_master_topic(
                "Master specification topic: 16 — image formats\n"
            ),
            16,
        )
        self.assertIsNone(validator.adr_master_topic("# ADR without topic\n"))

    def test_docs_current_product_001_rejects_natural_restore_status_claims(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                documents={
                    "README.md": "The desktop cannot restore data.\n",
                    "README.pt-BR.md": (
                        "Nem o desktop nem a CLI restauram dados.\n"
                    ),
                    "docs/specs/001-functional-requirements.md": (
                        "A restauração não está implementada.\n"
                    ),
                    "docs/specs/003-domain-model.md": (
                        "Não é possível abrir o destino da restauração.\n"
                    ),
                    "docs/specs/004-architecture.md": (
                        "Restore progress remains unsupported.\n"
                    ),
                    "docs/specs/011-security-and-privacy.md": (
                        "Restore cancellation remains unsupported.\n"
                    ),
                    "docs/specs/019-ntfs-coverage-and-jpeg-deep-scan.md": (
                        "Opening the restore destination remains unsupported.\n"
                    ),
                    "docs/risk-register.md": "| Restore | Not available |\n",
                },
            )
            errors: list[str] = []
            validator.validate_current_product_contract(root, errors)

        for capability in (
            "restore execution",
            "restore progress",
            "restore cancellation",
            "opening the restore destination",
        ):
            self.assertTrue(
                any(
                    f"obsolete current-status claim about {capability}" in error
                    for error in errors
                ),
                errors,
            )
        for relative in (
            "docs/specs/001-functional-requirements.md",
            "docs/specs/003-domain-model.md",
            "docs/specs/004-architecture.md",
            "docs/specs/011-security-and-privacy.md",
            "docs/specs/019-ntfs-coverage-and-jpeg-deep-scan.md",
            "docs/risk-register.md",
        ):
            self.assertTrue(
                any(error.startswith(f"{relative}:") for error in errors),
                errors,
            )

    def test_docs_current_product_002_binds_only_contextual_command_counts(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                documents={
                    "README.md": "\n".join(
                        (
                            "Run these 6 commands to verify the local gates.",
                            "The desktop inventory has 6 Tauri commands.",
                            "Tauri commands: 7.",
                            "O inventário possui 6 comandos Tauri.",
                            "Comandos Tauri: sete.",
                            "The baseline exposed four scan/inventory Tauri commands.",
                            "The current four-command Tauri surface is closed.",
                            "Historical baseline: six Tauri commands.",
                            "The scanner exposes four scan/inventory Tauri commands.",
                            "",
                        )
                    )
                },
            )
            errors: list[str] = []
            validator.validate_current_product_contract(root, errors)

        count_errors = [
            error
            for error in errors
            if "desktop/Tauri commands, but authoritative inventory has 12" in error
        ]
        self.assertEqual(
            {int(error.split(":", 2)[1]) for error in count_errors},
            {2, 3, 4, 5, 7, 9},
            count_errors,
        )

    def test_docs_current_product_003_allows_explicit_historical_sections(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                documents={
                    "docs/specs/018-windows-volume-and-folder-scan.md": (
                        "## Historical baseline\n"
                        "The inventory had six Tauri commands.\n"
                        "Restore cancellation is not available.\n"
                        "| Restore | Not available |\n"
                        "## Current state\n"
                        "The current inventory has 12 Tauri commands.\n"
                        "```text\n"
                        "SDD-018 baseline WebView\n"
                        "  -> four scan/inventory Tauri commands\n"
                        "```\n"
                        "## Original behavior\n"
                        "Restore progress is unavailable.\n"
                        "The inventory had five Tauri commands.\n"
                    )
                },
            )
            errors: list[str] = []
            validator.validate_current_product_contract(root, errors)

        self.assertEqual(errors, [])

    def test_docs_current_product_004_does_not_hide_current_comparison_claim(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                documents={
                    "README.md": (
                        "Compared with the historical baseline, restore "
                        "cancellation remains unsupported.\n"
                    )
                },
            )
            errors: list[str] = []

            validator.validate_current_product_contract(root, errors)

        self.assertTrue(
            any(
                error.startswith("README.md:1:")
                and "obsolete current-status claim about restore cancellation"
                in error
                for error in errors
            ),
            errors,
        )

    def test_docs_current_product_005_allows_prior_context_synonyms(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                documents={
                    "README.md": (
                        "Prior contract: seven Tauri commands.\n"
                        "Former implementation: eight Tauri commands.\n"
                        "Earlier inventory: nine Tauri commands.\n"
                    ),
                    "README.pt-BR.md": (
                        "Inventário anterior: seis comandos Tauri.\n"
                        "Contrato prévio: sete comandos Tauri.\n"
                        "Implementação anterior: oito comandos Tauri.\n"
                    ),
                    "docs/specs/018-windows-volume-and-folder-scan.md": (
                        "## Previous behavior\n"
                        "Restore progress is unavailable.\n"
                        "## Comportamento anterior\n"
                        "O cancelamento da restauração está indisponível.\n"
                    ),
                },
            )
            errors: list[str] = []

            validator.validate_current_product_contract(root, errors)

        self.assertEqual(errors, [])

    def test_docs_current_product_006_inline_history_only_exempts_old_counts(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                documents={
                    "README.md": (
                        "The former implementation had eight Tauri commands; "
                        "restore cancellation remains unsupported.\n"
                    )
                },
            )
            errors: list[str] = []

            validator.validate_current_product_contract(root, errors)

        self.assertFalse(
            any("documents 8 desktop/Tauri commands" in error for error in errors),
            errors,
        )
        self.assertTrue(
            any(
                "obsolete current-status claim about restore cancellation" in error
                for error in errors
            ),
            errors,
        )

    def test_docs_current_product_007_qualified_previous_only_exempts_old_count(
        self,
    ) -> None:
        cases = {
            "exact current inventory": (
                "The previous six-command scan surface became the 12-command "
                "Tauri surface.\n",
                None,
            ),
            "wrong current inventory": (
                "The previous six-command scan surface became the 11-command "
                "Tauri surface.\n",
                11,
            ),
        }
        for label, (claim, wrong_count) in cases.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as directory:
                root = self.current_product_fixture(
                    directory,
                    documents={"README.md": claim},
                )
                errors: list[str] = []

                validator.validate_current_product_contract(root, errors)

            count_errors = [
                error
                for error in errors
                if "desktop/Tauri commands, but authoritative inventory has 12"
                in error
            ]
            self.assertFalse(
                any("documents 6 desktop/Tauri commands" in error for error in errors),
                errors,
            )
            if wrong_count is None:
                self.assertEqual(count_errors, [])
            else:
                self.assertTrue(
                    any(
                        f"documents {wrong_count} desktop/Tauri commands" in error
                        for error in count_errors
                    ),
                    count_errors,
                )

    def test_docs_current_product_008_validates_aggregate_command_inventory(
        self,
    ) -> None:
        cases = {
            "current ADR wording": (
                "The existing six result/selection commands and these six restore "
                "commands are\n"
                "the complete desktop command inventory.\n",
                None,
            ),
            "wrong current ADR wording": (
                "The existing five result/selection commands and these six restore "
                "commands are\n"
                "the complete desktop command inventory.\n",
                11,
            ),
            "wrong number-first wording": (
                "Five existing commands plus six restore commands form the complete "
                "inventory.\n",
                11,
            ),
        }
        for label, (claim, wrong_count) in cases.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as directory:
                root = self.current_product_fixture(
                    directory,
                    documents={
                        "docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md": claim
                    },
                )
                errors: list[str] = []

                validator.validate_current_product_contract(root, errors)

            count_errors = [
                error
                for error in errors
                if "desktop/Tauri commands, but authoritative inventory has 12"
                in error
            ]
            if wrong_count is None:
                self.assertEqual(count_errors, [])
            else:
                self.assertTrue(
                    any(
                        f"documents {wrong_count} desktop/Tauri commands" in error
                        for error in count_errors
                    ),
                    count_errors,
                )

    def test_docs_current_product_009_ignores_rust_comment_and_string_decoys(
        self,
    ) -> None:
        registration = (
            'const DECOY: &str = r#"generate_handler![evil::string_decoy]"#;\n'
            "// generate_handler![evil::line_comment_decoy]\n"
            "/* generate_handler![evil::block_comment_decoy] */\n"
            + VALID_TAURI_REGISTRATION.replace(
                "        storage::select_scan_folder,",
                "        storage::select_scan_folder, /* ], fake::entry, */",
            )
        )
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                registration=registration,
            )
            errors: list[str] = []

            commands = validator.registered_tauri_commands(root, errors)

        self.assertEqual(commands, VALID_TAURI_REGISTRATIONS)
        self.assertEqual(errors, [])

    def test_docs_current_product_010_rejects_tauri_module_path_substitution(
        self,
    ) -> None:
        registration = VALID_TAURI_REGISTRATION.replace(
            "storage::list_storage_sources",
            "evil::list_storage_sources",
            1,
        )
        with tempfile.TemporaryDirectory() as directory:
            root = self.current_product_fixture(
                directory,
                registration=registration,
            )
            errors: list[str] = []

            validator.validate_current_product_contract(root, errors)

        self.assertTrue(
            any(
                "registered desktop command inventory differs from authoritative inventory"
                in error
                and "evil::list_storage_sources" in error
                for error in errors
            ),
            errors,
        )

    def test_docs_current_product_011_rejects_malformed_tauri_registrations(
        self,
    ) -> None:
        registrations = {
            "duplicate": """\
tauri::generate_handler![
    storage::list_storage_sources,
    storage::list_storage_sources
]
""",
            "second macro": """\
tauri::generate_handler![storage::list_storage_sources]
tauri::generate_handler![restore::start_restore]
""",
            "malformed": """\
tauri::generate_handler![
    storage::list_storage_sources,,
    restore::start_restore
]
""",
        }
        for label, registration in registrations.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as directory:
                root = self.current_product_fixture(
                    directory,
                    registration=registration,
                )
                errors: list[str] = []

                commands = validator.registered_tauri_commands(root, errors)

            self.assertIsNone(commands)
            self.assertNotEqual(errors, [])

    def test_docs_current_tree_001_repository_contract_passes(self) -> None:
        errors, stats = validator.validate_repository(ROOT)

        self.assertEqual(errors, [])
        self.assertEqual(stats["master_frs"], 58)
        self.assertEqual(stats["catalog_frs"], 58)
        self.assertEqual(stats["nfrs"], 8)
        self.assertEqual(stats["sdd016_requirements"], 11)
        self.assertEqual(stats["sdd017_requirements"], 12)
        self.assertEqual(stats["sdd018_requirements"], 13)
        self.assertEqual(stats["adr_topics"], 17)


if __name__ == "__main__":
    unittest.main()
