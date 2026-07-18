"""Closed JSON Schema 2020-12 contract for remem's published schemas."""

from __future__ import annotations

import math
import re
import subprocess
from pathlib import Path
from typing import Any

from specrail_lib import SUPPORTED_SCHEMA_KEYS


JSON_TYPES = frozenset(
    {"array", "boolean", "integer", "null", "number", "object", "string"}
)
ADOPTED_SCHEMA_KEYWORDS = frozenset(
    {
        "$anchor", "$comment", "$defs", "$dynamicAnchor", "$dynamicRef", "$id",
        "$ref", "$schema", "$vocabulary", "additionalProperties", "allOf", "anyOf",
        "const", "contains", "contentEncoding", "contentMediaType", "contentSchema",
        "default", "dependentRequired", "dependentSchemas", "deprecated", "description",
        "else", "enum", "examples", "exclusiveMaximum", "exclusiveMinimum", "format",
        "if", "items", "maxContains", "maxItems", "maxLength", "maxProperties",
        "maximum", "minContains", "minItems", "minLength", "minProperties", "minimum",
        "multipleOf", "not", "oneOf", "pattern", "patternProperties", "prefixItems",
        "properties", "propertyNames", "readOnly", "required", "then", "title", "type",
        "unevaluatedItems", "unevaluatedProperties", "uniqueItems", "writeOnly",
    }
)
RUNTIME_PROFILE_SCHEMAS = frozenset({"duplicate_work_evidence.schema.json"})
UNRESOLVED_REFERENCE_KEYWORDS = ("$dynamicRef", "$ref")

SCHEMA_MAP_KEYWORDS = ("$defs", "dependentSchemas", "patternProperties", "properties")
SCHEMA_LIST_KEYWORDS = ("allOf", "anyOf", "oneOf", "prefixItems")
SCHEMA_NODE_KEYWORDS = (
    "contains", "contentSchema", "else", "if", "items", "not", "propertyNames", "then",
    "additionalProperties", "unevaluatedItems", "unevaluatedProperties",
)
STRING_KEYWORDS = (
    "$anchor", "$comment", "$dynamicAnchor", "$dynamicRef", "$id", "$ref", "$schema",
    "contentEncoding", "contentMediaType", "description", "format", "title",
)
BOOLEAN_KEYWORDS = ("deprecated", "readOnly", "uniqueItems", "writeOnly")
NON_NEGATIVE_INTEGER_KEYWORDS = (
    "maxContains", "maxItems", "maxLength", "maxProperties",
    "minContains", "minItems", "minLength", "minProperties",
)
NUMBER_KEYWORDS = ("exclusiveMaximum", "exclusiveMinimum", "maximum", "minimum")

KEYWORD_APPLICABLE_TYPES = {
    "additionalProperties": {"object"},
    "dependentRequired": {"object"},
    "dependentSchemas": {"object"},
    "maxProperties": {"object"},
    "minProperties": {"object"},
    "patternProperties": {"object"},
    "properties": {"object"},
    "propertyNames": {"object"},
    "required": {"object"},
    "unevaluatedProperties": {"object"},
    "contains": {"array"},
    "items": {"array"},
    "maxContains": {"array"},
    "maxItems": {"array"},
    "minContains": {"array"},
    "minItems": {"array"},
    "prefixItems": {"array"},
    "unevaluatedItems": {"array"},
    "uniqueItems": {"array"},
    "contentEncoding": {"string"},
    "contentMediaType": {"string"},
    "contentSchema": {"string"},
    "format": {"string"},
    "maxLength": {"string"},
    "minLength": {"string"},
    "pattern": {"string"},
    "exclusiveMaximum": {"integer", "number"},
    "exclusiveMinimum": {"integer", "number"},
    "maximum": {"integer", "number"},
    "minimum": {"integer", "number"},
    "multipleOf": {"integer", "number"},
}
TYPE_SPECIFIC_SCHEMA_KEYWORDS = frozenset(
    {
        "additionalProperties", "contains", "contentEncoding", "contentMediaType",
        "contentSchema", "dependentRequired", "dependentSchemas", "exclusiveMaximum",
        "exclusiveMinimum", "format", "items", "maxContains", "maxItems", "maxLength",
        "maxProperties", "maximum", "minContains", "minItems", "minLength",
        "minProperties", "minimum", "multipleOf", "pattern", "patternProperties",
        "prefixItems", "properties", "propertyNames", "required", "unevaluatedItems",
        "unevaluatedProperties", "uniqueItems",
    }
)
assert frozenset(KEYWORD_APPLICABLE_TYPES) == TYPE_SPECIFIC_SCHEMA_KEYWORDS
assert TYPE_SPECIFIC_SCHEMA_KEYWORDS <= ADOPTED_SCHEMA_KEYWORDS

RUNTIME_TYPED_KEYWORDS = {
    "additionalProperties": {"object"},
    "exclusiveMaximum": {"integer", "number"},
    "exclusiveMinimum": {"integer", "number"},
    "items": {"array"},
    "minItems": {"array"},
    "minLength": {"string"},
    "minimum": {"integer", "number"},
    "properties": {"object"},
    "required": {"object"},
}
ECMASCRIPT_REGEX_CHECK = (
    "try { new RegExp(process.argv[1], 'u'); } "
    "catch (error) { console.error(error.message); process.exit(1); }"
)


def uses_runtime_profile(path: Path) -> bool:
    return path.name in RUNTIME_PROFILE_SCHEMAS


def _is_json_number(value: object) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and (not isinstance(value, float) or math.isfinite(value))
    )


def _json_values_equal(left: object, right: object) -> bool:
    if _is_json_number(left) and _is_json_number(right):
        return left == right
    if type(left) is not type(right):
        return False
    if isinstance(left, list):
        return len(left) == len(right) and all(  # type: ignore[arg-type]
            _json_values_equal(a, b) for a, b in zip(left, right)  # type: ignore[arg-type]
        )
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(  # type: ignore[union-attr]
            _json_values_equal(left[key], right[key]) for key in left  # type: ignore[index]
        )
    return left == right


def _validate_pattern(
    value: object,
    relative_path: Path,
    keyword_path: str,
    errors: list[str],
    *,
    runtime_profile: bool,
) -> None:
    if not isinstance(value, str):
        errors.append(f"{relative_path}: {keyword_path} must be a string")
        return
    try:
        completed = subprocess.run(
            ["node", "-e", ECMASCRIPT_REGEX_CHECK, "--", value],
            capture_output=True,
            text=True,
            check=False,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        errors.append(f"{relative_path}: {keyword_path} ECMAScript regex engine failed: {exc}")
    else:
        if completed.returncode != 0:
            detail = completed.stderr.strip() or f"node exited {completed.returncode}"
            errors.append(
                f"{relative_path}: {keyword_path} invalid ECMAScript regex: {detail}"
            )
    if runtime_profile:
        try:
            re.compile(value)
        except re.error as exc:
            errors.append(
                f"{relative_path}: {keyword_path} invalid Python runtime regex: {exc.msg}"
            )


def _declared_types(
    schema: dict[str, Any], relative_path: Path, schema_path: str, errors: list[str]
) -> set[str] | None:
    if "type" not in schema:
        return None
    value = schema["type"]
    values = value if isinstance(value, list) else [value]
    if (
        not values
        or not all(isinstance(item, str) for item in values)
        or not set(values) <= JSON_TYPES
        or len(set(values)) != len(values)
    ):
        errors.append(
            f"{relative_path}: {schema_path}.type must be a supported JSON type "
            "or non-empty unique array of supported JSON types"
        )
        return None
    return set(values)


def _validate_shapes(
    schema: dict[str, Any],
    relative_path: Path,
    schema_path: str,
    errors: list[str],
    *,
    runtime_profile: bool,
) -> set[str] | None:
    declared_types = _declared_types(schema, relative_path, schema_path, errors)
    for keyword in STRING_KEYWORDS:
        if keyword in schema and not isinstance(schema[keyword], str):
            errors.append(f"{relative_path}: {schema_path}.{keyword} must be a string")
    for keyword in UNRESOLVED_REFERENCE_KEYWORDS:
        if keyword in schema:
            errors.append(
                f"{relative_path}: {schema_path}.{keyword} is unsupported until "
                "reference resolution is implemented"
            )
    for keyword in BOOLEAN_KEYWORDS:
        if keyword in schema and not isinstance(schema[keyword], bool):
            errors.append(f"{relative_path}: {schema_path}.{keyword} must be a boolean")
    for keyword in NON_NEGATIVE_INTEGER_KEYWORDS:
        value = schema.get(keyword)
        if keyword in schema and (
            not isinstance(value, int) or isinstance(value, bool) or value < 0
        ):
            errors.append(
                f"{relative_path}: {schema_path}.{keyword} must be a non-negative integer"
            )
    for keyword in NUMBER_KEYWORDS:
        if keyword in schema and not _is_json_number(schema[keyword]):
            errors.append(f"{relative_path}: {schema_path}.{keyword} must be a JSON number")
    if "multipleOf" in schema and (
        not _is_json_number(schema["multipleOf"]) or schema["multipleOf"] <= 0
    ):
        errors.append(f"{relative_path}: {schema_path}.multipleOf must be positive")
    if "enum" in schema:
        enum = schema["enum"]
        if not isinstance(enum, list) or not enum:
            errors.append(f"{relative_path}: {schema_path}.enum must be a non-empty array")
        elif any(
            _json_values_equal(enum[index], enum[prior])
            for index in range(len(enum))
            for prior in range(index)
        ):
            errors.append(f"{relative_path}: {schema_path}.enum must contain unique JSON values")
    if "examples" in schema and not isinstance(schema["examples"], list):
        errors.append(f"{relative_path}: {schema_path}.examples must be an array")
    if "$vocabulary" in schema:
        vocabulary = schema["$vocabulary"]
        if not isinstance(vocabulary, dict) or not all(
            isinstance(uri, str) and isinstance(required, bool)
            for uri, required in vocabulary.items()
        ):
            errors.append(
                f"{relative_path}: {schema_path}.$vocabulary must be an object of booleans"
            )
    for keyword in ("required",):
        if keyword in schema:
            names = schema[keyword]
            if not isinstance(names, list) or not all(isinstance(name, str) for name in names) or len(set(names)) != len(names):
                errors.append(
                    f"{relative_path}: {schema_path}.{keyword} must be a unique array of strings"
                )
    if "dependentRequired" in schema:
        dependencies = schema["dependentRequired"]
        if not isinstance(dependencies, dict):
            errors.append(f"{relative_path}: {schema_path}.dependentRequired must be an object")
        else:
            for name, names in dependencies.items():
                if not isinstance(names, list) or not all(isinstance(item, str) for item in names) or len(set(names)) != len(names):
                    errors.append(
                        f"{relative_path}: {schema_path}.dependentRequired.{name} "
                        "must be a unique array of strings"
                    )
    if "pattern" in schema:
        _validate_pattern(
            schema["pattern"], relative_path, f"{schema_path}.pattern", errors,
            runtime_profile=runtime_profile,
        )
    return declared_types


def _validate_applicability(
    schema: dict[str, Any],
    declared_types: set[str] | None,
    relative_path: Path,
    schema_path: str,
    errors: list[str],
) -> None:
    if declared_types is None:
        return
    for keyword, applicable_types in KEYWORD_APPLICABLE_TYPES.items():
        if keyword in schema and declared_types.isdisjoint(applicable_types):
            errors.append(
                f"{relative_path}: {schema_path}.{keyword} is incompatible with declared "
                f"type {' or '.join(sorted(declared_types))}; applicable type is "
                f"{' or '.join(sorted(applicable_types))}"
            )


def _validate_runtime_profile(
    schema: dict[str, Any],
    declared_types: set[str] | None,
    relative_path: Path,
    schema_path: str,
    errors: list[str],
) -> None:
    for keyword in sorted(set(schema) - SUPPORTED_SCHEMA_KEYS):
        if keyword in ADOPTED_SCHEMA_KEYWORDS:
            errors.append(
                f"{relative_path}: {schema_path}: runtime profile does not support "
                f"JSON Schema keyword {keyword!r}"
            )
    for keyword, compatible_types in RUNTIME_TYPED_KEYWORDS.items():
        if keyword not in schema:
            continue
        expected = " or ".join(sorted(compatible_types))
        if declared_types is None:
            errors.append(
                f"{relative_path}: {schema_path}.{keyword} requires explicit type {expected}"
            )
        elif not declared_types <= compatible_types:
            errors.append(
                f"{relative_path}: {schema_path}.{keyword} requires only type {expected}"
            )


def _validate_schema_child(
    child: object,
    relative_path: Path,
    child_path: str,
    errors: list[str],
    *,
    runtime_profile: bool,
    runtime_boolean_supported: bool = False,
) -> None:
    if isinstance(child, bool):
        if runtime_profile and not runtime_boolean_supported:
            errors.append(
                f"{relative_path}: {child_path} boolean schema is not executable by "
                "the Python runtime validator"
            )
    elif isinstance(child, dict):
        errors.extend(
            validate_schema_node(
                child, relative_path, child_path, runtime_profile=runtime_profile
            )
        )
    else:
        errors.append(f"{relative_path}: {child_path} must be a boolean or object")


def validate_schema_node(
    schema: dict[str, Any],
    relative_path: Path,
    schema_path: str = "$",
    *,
    runtime_profile: bool = False,
) -> list[str]:
    """Validate one object schema and every official schema-position child."""

    errors: list[str] = []
    for keyword in sorted(set(schema) - ADOPTED_SCHEMA_KEYWORDS):
        errors.append(
            f"{relative_path}: {schema_path}: unsupported JSON Schema keyword {keyword!r}"
        )
    declared_types = _validate_shapes(
        schema, relative_path, schema_path, errors, runtime_profile=runtime_profile
    )
    _validate_applicability(schema, declared_types, relative_path, schema_path, errors)
    if runtime_profile:
        _validate_runtime_profile(schema, declared_types, relative_path, schema_path, errors)

    for keyword in SCHEMA_MAP_KEYWORDS:
        if keyword not in schema:
            continue
        children = schema[keyword]
        keyword_path = f"{schema_path}.{keyword}"
        if not isinstance(children, dict):
            errors.append(f"{relative_path}: {keyword_path} must be an object")
            continue
        for name, child in children.items():
            child_path = f"{keyword_path}.{name}"
            if keyword == "patternProperties":
                _validate_pattern(
                    name, relative_path, f"{keyword_path}[{name!r}]", errors,
                    runtime_profile=runtime_profile,
                )
            _validate_schema_child(
                child, relative_path, child_path, errors, runtime_profile=runtime_profile
            )

    for keyword in SCHEMA_LIST_KEYWORDS:
        if keyword not in schema:
            continue
        children = schema[keyword]
        keyword_path = f"{schema_path}.{keyword}"
        if not isinstance(children, list) or not children:
            errors.append(f"{relative_path}: {keyword_path} must be a non-empty array")
            continue
        for index, child in enumerate(children):
            _validate_schema_child(
                child, relative_path, f"{keyword_path}[{index}]", errors,
                runtime_profile=runtime_profile,
            )

    for keyword in SCHEMA_NODE_KEYWORDS:
        if keyword in schema:
            _validate_schema_child(
                schema[keyword], relative_path, f"{schema_path}.{keyword}", errors,
                runtime_profile=runtime_profile,
                runtime_boolean_supported=keyword == "additionalProperties",
            )
    return errors
