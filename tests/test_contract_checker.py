"""Regressions for the existing offline public-contract checker."""

import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "check_contracts", ROOT / "scripts" / "check-contracts.py"
)
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
REPORTS = (
    "toolchain-probe-report-v1.schema.json",
    "provider-execution-report-v2.schema.json",
)


class ContractCheckerTests(unittest.TestCase):
    def errors(self, document):
        errors = []
        CHECKER.check_schema_internals("fixture.schema.json", document, errors)
        return errors

    def report(self, filename):
        return json.loads((ROOT / "docs" / "contracts" / filename).read_text())

    def test_published_conditional_reports_keep_parent_property_declarations(self):
        for filename in REPORTS:
            with self.subTest(filename=filename):
                self.assertEqual([], self.errors(self.report(filename)))

    def test_same_instance_applicators_inherit_ancestor_declarations(self):
        refinement = {"properties": {}, "required": ["outer"]}
        for keyword in ("allOf", "anyOf", "oneOf", "if", "then", "else", "not",
                        "dependentSchemas"):
            with self.subTest(keyword=keyword):
                value = copy.deepcopy(refinement)
                if keyword in ("allOf", "anyOf", "oneOf"):
                    value = [value]
                elif keyword == "dependentSchemas":
                    value = {"trigger": value}
                document = {"properties": {"outer": {}}, keyword: value}
                self.assertEqual([], self.errors(document))

    def test_nested_conditional_refinement_keeps_ancestor_and_local_declarations(self):
        document = {
            "properties": {"outer": {}},
            "allOf": [{
                "if": {"properties": {"outer": {"const": "selected"}}},
                "then": {
                    "properties": {"local": {}},
                    "allOf": [{
                        "properties": {},
                        "required": ["outer", "local"],
                    }],
                },
            }],
        }
        self.assertEqual([], self.errors(document))

    def test_bare_required_can_refine_a_reference_without_redeclaring_properties(self):
        document = {
            "$defs": {"version": {"properties": {"normalized": {}}}},
            "properties": {
                "version": {
                    "$ref": "#/$defs/version",
                    "allOf": [{"required": ["normalized"]}],
                },
            },
        }
        self.assertEqual([], self.errors(document))

    def test_nested_instance_and_definition_scopes_cannot_borrow_parent_names(self):
        child = {"properties": {}, "required": ["outer"]}
        cases = [
            {"properties": {"outer": {}, "child": child}},
            {"properties": {"outer": {}}, "patternProperties": {"^child$": child}},
            {"properties": {"outer": {}}, "$defs": {"child": child}},
            {"properties": {"outer": {}}, "definitions": {"child": child}},
            *[
                {"properties": {"outer": {}}, keyword: child}
                for keyword in (
                    "items", "contains", "additionalProperties", "unevaluatedProperties",
                    "propertyNames", "unevaluatedItems", "contentSchema",
                )
            ],
            {"properties": {"outer": {}}, "prefixItems": [child]},
        ]
        for document in cases:
            with self.subTest(document=document):
                self.assertEqual(
                    ["fixture.schema.json: required property 'outer' is undeclared"],
                    self.errors(document),
                )

    def test_sibling_branches_do_not_donate_declarations(self):
        for keyword in ("allOf", "anyOf", "oneOf", "dependentSchemas"):
            branches = [
                {"properties": {"other_branch": {}}},
                {"properties": {}, "required": ["other_branch"]},
            ]
            document = {
                "properties": {"ancestor": {}},
                keyword: dict(zip(("first", "second"), branches))
                if keyword == "dependentSchemas" else branches,
            }
            with self.subTest(keyword=keyword):
                self.assertEqual(
                    ["fixture.schema.json: required property 'other_branch' is undeclared"],
                    self.errors(document),
                )

    def test_if_then_and_else_do_not_donate_declarations_to_each_other(self):
        for donor, recipient in (("if", "then"), ("then", "else"), ("else", "then")):
            document = {
                "properties": {"ancestor": {}},
                donor: {"properties": {"other_branch": {}}},
                recipient: {"properties": {}, "required": ["other_branch"]},
            }
            with self.subTest(donor=donor, recipient=recipient):
                self.assertEqual(
                    ["fixture.schema.json: required property 'other_branch' is undeclared"],
                    self.errors(document),
                )

    def test_schema_keyword_property_names_do_not_create_a_schema_scope(self):
        document = {
            "properties": {
                "properties": {"type": "object"},
                "required": {"type": "array", "items": {"type": "string"}},
                "allOf": {"properties": {}, "required": ["type"]},
            },
        }
        self.assertEqual(
            ["fixture.schema.json: required property 'type' is undeclared"],
            self.errors(document),
        )

    def test_annotation_and_literal_data_are_not_interpreted_as_schemas(self):
        data = {"required": ["unknown", "unknown"], "properties": {},
                "pattern": "[", "$ref": "#/$defs/missing"}
        document = {
            "properties": {
                "literal": {
                    "const": data, "default": data, "examples": [data], "enum": [data],
                },
            },
        }
        self.assertEqual([], self.errors(document))

    def test_boolean_schema_children_are_valid_leaves(self):
        document = {
            "properties": {"allowed": True, "forbidden": False},
            "allOf": [True],
            "additionalProperties": False,
        }
        self.assertEqual([], self.errors(document))

    def test_missing_real_parent_declarations_are_still_rejected(self):
        filename = REPORTS[0]
        for name in ("before_sha256", "after_sha256", "observation"):
            with self.subTest(name=name):
                document = self.report(filename)
                del document["$defs"]["tool_result"]["properties"][name]
                self.assertIn(
                    f"fixture.schema.json: required property {name!r} is undeclared",
                    self.errors(document),
                )
        document = self.report(REPORTS[1])
        del document["$defs"]["payload"]["properties"]["failure"]
        self.assertIn(
            "fixture.schema.json: required property 'failure' is undeclared",
            self.errors(document),
        )

    def test_real_contract_duplicate_requirements_are_still_rejected(self):
        document = self.report(REPORTS[0])
        document["required"].append(document["required"][0])
        self.assertIn(
            "fixture.schema.json: required property list contains duplicates",
            self.errors(document),
        )

    def test_real_contract_broken_reference_and_regex_are_still_rejected(self):
        document = self.report(REPORTS[1])
        document["properties"]["schema"]["$ref"] = "#/$defs/missing"
        document["properties"]["schema"]["pattern"] = "["
        errors = self.errors(document)
        self.assertIn(
            "fixture.schema.json: unresolved local definition #/$defs/missing", errors,
        )
        self.assertTrue(any("invalid regular expression" in error for error in errors))

    def test_malformed_required_values_are_reported_without_crashing(self):
        for required in ("outer", ["outer", {"invalid": "name"}], [1]):
            document = {"properties": {"outer": {}}, "required": required}
            with self.subTest(required=required):
                self.assertEqual(
                    ["fixture.schema.json: required must be an array of property names"],
                    self.errors(document),
                )

    def test_malformed_properties_are_reported(self):
        document = self.report(REPORTS[0])
        document["$defs"]["tool_result"]["properties"] = []
        self.assertIn(
            "fixture.schema.json: properties must be an object",
            self.errors(document),
        )


if __name__ == "__main__":
    unittest.main()
