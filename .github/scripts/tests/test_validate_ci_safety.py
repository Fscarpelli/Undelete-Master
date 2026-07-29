from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "validate_ci_safety.py"
SPEC = importlib.util.spec_from_file_location("validate_ci_safety", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
validator = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = validator
SPEC.loader.exec_module(validator)


class CiSafetyValidatorTests(unittest.TestCase):
    def make_repo(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.parent.mkdir(parents=True)
        workflow.write_text(
            "jobs:\n  safe:\n    runs-on: ubuntu-latest\n    steps:\n"
            "      - run: python .github/scripts/check.py\n",
            encoding="utf-8",
        )
        script = root / ".github" / "scripts" / "check.py"
        script.parent.mkdir(parents=True)
        script.write_text("print('safe fixture check')\n", encoding="utf-8")
        rust = root / "crates" / "example" / "src" / "lib.rs"
        rust.parent.mkdir(parents=True)
        rust.write_text(
            "#![forbid(unsafe_code)]\n"
            "#[cfg(test)] mod tests { #[test] fn safe() { assert!(true); } }\n",
            encoding="utf-8",
        )
        return temporary, root

    def test_safe_repository_passes(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)

        errors, workflow_count, surface_count = validator.validate_repository(root)

        self.assertEqual(errors, [])
        self.assertEqual(workflow_count, 1)
        self.assertEqual(surface_count, 2)

    def test_ci_safety_inline_001_inline_cfg_test_is_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Clear" + "-Disk"
        rust = root / "crates" / "example" / "src" / "lib.rs"
        rust.write_text(
            "#[cfg(test)] mod tests {\n"
            "  #[test] fn destructive() {\n"
            f'    std::process::Command::new("powershell").arg("{command}");\n'
            "  }\n"
            "}\n",
            encoding="utf-8",
        )

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(any("crates/example/src/lib.rs" in error for error in errors))

    def test_ci_safety_workflows_001_every_workflow_is_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        extra = root / ".github" / "workflows" / "unsafe.yaml"
        extra.write_text(
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            f"      - run: {command} -DriveLetter X\n",
            encoding="utf-8",
        )

        errors, workflow_count, _ = validator.validate_repository(root)

        self.assertEqual(workflow_count, 2)
        self.assertTrue(any("unsafe.yaml" in error for error in errors), errors)

    def test_first_party_helper_script_is_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        device = "\\\\.\\" + "Physical" + "Drive7"
        helper = root / "scripts" / "device.ps1"
        helper.parent.mkdir(parents=True)
        helper.write_text(f'Get-Content -LiteralPath "{device}"\n', encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("windows-raw-device" in error for error in errors), errors)
        self.assertTrue(any("scripts/device.ps1" in error for error in errors))

    def test_ci_safety_package_lifecycle_001_manifest_scripts_are_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Clear" + "-Disk"
        manifest = root / "apps" / "desktop" / "package.json"
        manifest.parent.mkdir(parents=True)
        manifest.write_text(
            '{"scripts":{"pretest":"powershell '
            + command
            + ' -Number 7","test":"vitest run"}}\n',
            encoding="utf-8",
        )

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(any("apps/desktop/package.json" in error for error in errors))

    def test_ci_safety_web_raw_device_001_production_web_source_is_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        source = root / "apps" / "desktop" / "src" / "device.ts"
        source.parent.mkdir(parents=True)
        device = "\\\\.\\" + "Physical" + "Drive7"
        source.write_text(f'export const source = "{device}";\n', encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("windows-raw-device" in error for error in errors), errors)
        self.assertTrue(any("apps/desktop/src/device.ts" in error for error in errors))

    def test_ci_safety_shebang_001_extensionless_helper_is_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(
            "#!/usr/bin/env pwsh\n" + command + " -DriveLetter X\n",
            encoding="utf-8",
        )

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(any("scripts/storage-helper" in error for error in errors))

    def test_ci_safety_invoked_helper_001_workflow_target_without_shebang_is_scanned(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            "      - run: pwsh -NoProfile -File scripts/storage-helper\n",
            encoding="utf-8",
        )
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(any("scripts/storage-helper" in error for error in errors), errors)

    def test_ci_safety_invoked_helper_002_manifest_target_without_shebang_is_scanned(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        manifest = root / "apps" / "desktop" / "package.json"
        manifest.parent.mkdir(parents=True)
        manifest.write_text(
            '{"scripts":{"test":"pwsh -NoProfile -File scripts/storage-helper"}}\n',
            encoding="utf-8",
        )
        helper = manifest.parent / "scripts" / "storage-helper"
        helper.parent.mkdir()
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(
            any("apps/desktop/scripts/storage-helper" in error for error in errors),
            errors,
        )

    def test_ci_safety_invoked_helper_003_repository_escape_is_rejected(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            "      - run: pwsh -File ../outside-checkout-helper\n",
            encoding="utf-8",
        )

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(
            any("escapes repository inspection boundary" in error for error in errors),
            errors,
        )

    def test_ci_safety_invoked_helper_004_workflow_comment_is_not_execution(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "name: safe\n"
            "# Historical example only: pwsh -File scripts/storage-helper\n"
            f"# Historical prohibited-command example only: {command} X\n"
            "jobs:\n  safe:\n    runs-on: ubuntu-latest\n    steps:\n"
            "      - run: python .github/scripts/check.py\n"
            '      - run: echo "pwsh -File scripts/storage-helper"\n'
            "      - run: echo pwsh -File scripts/storage-helper\n",
            encoding="utf-8",
        )
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertEqual(errors, [])

    def test_ci_safety_invoked_helper_005_quoted_yaml_run_is_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            '      - run: "pwsh -File scripts/storage-helper"\n',
            encoding="utf-8",
        )
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(any("scripts/storage-helper" in error for error in errors), errors)

    def test_ci_safety_invoked_helper_006_working_directory_is_execution_base(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            "      - working-directory: apps/desktop\n"
            "        run: pwsh -File scripts/storage-helper\n",
            encoding="utf-8",
        )
        safe_collision = root / "scripts" / "storage-helper"
        safe_collision.parent.mkdir(parents=True)
        safe_collision.write_text("Write-Output safe\n", encoding="utf-8")
        helper = root / "apps" / "desktop" / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(
            any("apps/desktop/scripts/storage-helper" in error for error in errors),
            errors,
        )

    def test_ci_safety_invoked_helper_007_powershell_here_string_is_not_execution(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  safe:\n    runs-on: windows-latest\n    steps:\n"
            "      - shell: pwsh\n"
            "        run: |\n"
            "          $example = @'\n"
            "          pwsh -File scripts/storage-helper\n"
            f"          {command} X\n"
            "          '@\n"
            "          Write-Output $example\n",
            encoding="utf-8",
        )
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertEqual(errors, [])

    def test_ci_safety_invoked_helper_008_quoted_run_with_comment_is_scanned(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            '      - run: "pwsh -File scripts/storage-helper" # real invocation\n',
            encoding="utf-8",
        )
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(any("scripts/storage-helper" in error for error in errors), errors)

    def test_ci_safety_invoked_helper_009_defaults_working_directory_is_used(
        self,
    ) -> None:
        workflow_fixtures = (
            "defaults:\n"
            "  run:\n"
            "    working-directory: apps/desktop\n"
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            "      - run: pwsh -File scripts/storage-helper\n",
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n"
            "    defaults:\n      run:\n"
            "        working-directory: apps/desktop\n"
            "    steps:\n"
            "      - run: pwsh -File scripts/storage-helper\n",
        )
        for workflow_text in workflow_fixtures:
            with self.subTest(workflow=workflow_text), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                workflow = root / ".github" / "workflows" / "quality.yml"
                workflow.parent.mkdir(parents=True)
                workflow.write_text(workflow_text, encoding="utf-8")
                safe_collision = root / "scripts" / "storage-helper"
                safe_collision.parent.mkdir(parents=True)
                safe_collision.write_text("Write-Output safe\n", encoding="utf-8")
                command = "Format" + "-Volume"
                helper = root / "apps" / "desktop" / "scripts" / "storage-helper"
                helper.parent.mkdir(parents=True)
                helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

                errors, _, _ = validator.validate_repository(root)

                self.assertTrue(any("storage-shell" in error for error in errors), errors)
                self.assertTrue(
                    any("apps/desktop/scripts/storage-helper" in error for error in errors),
                    errors,
                )

    def test_ci_safety_invoked_helper_010_folded_yaml_run_is_scanned(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  unsafe:\n    runs-on: windows-latest\n    steps:\n"
            "      - run: >\n"
            "          pwsh\n"
            "          -File scripts/storage-helper\n",
            encoding="utf-8",
        )
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(any("scripts/storage-helper" in error for error in errors), errors)

    def test_ci_safety_invoked_helper_011_composite_action_helper_is_scanned(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  safe:\n    runs-on: windows-latest\n    steps:\n"
            "      - uses: ./.github/actions/check\n",
            encoding="utf-8",
        )
        action = root / ".github" / "actions" / "check" / "action.yml"
        action.parent.mkdir(parents=True)
        action.write_text(
            "name: check\ndescription: check\nruns:\n  using: composite\n  steps:\n"
            "    - shell: pwsh\n"
            "      run: pwsh -File .github/actions/check/scripts/storage-helper\n",
            encoding="utf-8",
        )
        command = "Format" + "-Volume"
        helper = action.parent / "scripts" / "storage-helper"
        helper.parent.mkdir()
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertTrue(any("storage-shell" in error for error in errors), errors)
        self.assertTrue(
            any(
                ".github/actions/check/scripts/storage-helper" in error
                for error in errors
            ),
            errors,
        )

    def test_ci_safety_invoked_helper_012_multiline_literals_are_not_execution(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        command = "Format" + "-Volume"
        workflow = root / ".github" / "workflows" / "quality.yml"
        workflow.write_text(
            "jobs:\n  safe:\n    runs-on: windows-latest\n    steps:\n"
            "      - shell: pwsh\n"
            "        run: |\n"
            '          $quoted = "\n'
            "          pwsh -File scripts/storage-helper\n"
            f"          {command} X\n"
            '          "\n'
            "          <#\n"
            "          pwsh -File scripts/storage-helper\n"
            f"          {command} X\n"
            "          #>\n"
            "          Write-Output safe\n",
            encoding="utf-8",
        )
        helper = root / "scripts" / "storage-helper"
        helper.parent.mkdir(parents=True)
        helper.write_text(command + " -DriveLetter X\n", encoding="utf-8")

        errors, _, _ = validator.validate_repository(root)

        self.assertEqual(errors, [])

    def test_missing_workflow_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rust = root / "crates" / "example" / "src" / "lib.rs"
            rust.parent.mkdir(parents=True)
            rust.write_text("#![forbid(unsafe_code)]\n", encoding="utf-8")

            errors, workflow_count, _ = validator.validate_repository(root)

        self.assertEqual(workflow_count, 0)
        self.assertTrue(any("no YAML workflow" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
