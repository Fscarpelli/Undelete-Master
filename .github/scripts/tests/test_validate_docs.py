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

    def test_docs_current_tree_001_repository_contract_passes(self) -> None:
        errors, stats = validator.validate_repository(ROOT)

        self.assertEqual(errors, [])
        self.assertEqual(stats["master_frs"], 58)
        self.assertEqual(stats["catalog_frs"], 58)
        self.assertEqual(stats["nfrs"], 8)
        self.assertEqual(stats["sdd016_requirements"], 11)
        self.assertEqual(stats["adr_topics"], 17)


if __name__ == "__main__":
    unittest.main()
