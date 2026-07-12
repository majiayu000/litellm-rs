#!/usr/bin/env python3
"""Find session identifiers passed to production Rust log macros."""

from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path


LOG_MACROS = {"trace", "debug", "info", "warn", "error"}
SESSION_IDENTIFIERS = {"session_id", "session_token", "sid"}
OPEN_TO_CLOSE = {"(": ")", "[": "]", "{": "}"}
CLOSE_TO_OPEN = {value: key for key, value in OPEN_TO_CLOSE.items()}


class ScanError(ValueError):
    """Raised when a Rust source file cannot be scanned safely."""


@dataclass(frozen=True)
class Token:
    kind: str
    value: str
    line: int


@dataclass(frozen=True)
class Finding:
    path: Path
    line: int
    macro: str
    identifiers: tuple[str, ...]

    def render(self) -> str:
        names = ",".join(self.identifiers)
        return f"{self.path}:{self.line}: {self.macro}! references {names}"


def _skip_quoted(source: str, quote_index: int, path: Path) -> int:
    index = quote_index + 1
    while index < len(source):
        if source[index] == "\\":
            index += 2
            continue
        if source[index] == '"':
            return index + 1
        index += 1
    raise ScanError(f"{path}: unterminated string literal")


def _raw_string_end(source: str, index: int, path: Path) -> int | None:
    if source.startswith(("br", "cr"), index):
        marker_index = index + 2
    elif source.startswith("r", index):
        marker_index = index + 1
    else:
        return None

    hash_index = marker_index
    while hash_index < len(source) and source[hash_index] == "#":
        hash_index += 1
    if hash_index >= len(source) or source[hash_index] != '"':
        return None

    closing = '"' + ("#" * (hash_index - marker_index))
    end = source.find(closing, hash_index + 1)
    if end < 0:
        raise ScanError(f"{path}: unterminated raw string literal")
    return end + len(closing)


def _char_literal_end(source: str, index: int) -> int | None:
    value_index = index + 1
    if value_index >= len(source) or source[value_index] in "\r\n'":
        return None

    if source[value_index] != "\\":
        end = value_index + 1
    elif value_index + 1 >= len(source):
        return None
    elif source[value_index + 1] == "x":
        end = value_index + 4
    elif source[value_index + 1] == "u" and source.startswith("\\u{", value_index):
        closing = source.find("}", value_index + 3)
        if closing < 0:
            return None
        end = closing + 1
    else:
        end = value_index + 2

    if end < len(source) and source[end] == "'":
        return end + 1
    return None


def tokenize(source: str, path: Path) -> list[Token]:
    tokens: list[Token] = []
    index = 0
    line = 1

    while index < len(source):
        char = source[index]
        if char.isspace():
            line += char == "\n"
            index += 1
            continue

        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            if newline < 0:
                break
            index = newline
            continue

        if source.startswith("/*", index):
            start_line = line
            depth = 1
            cursor = index + 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    line += source[cursor] == "\n"
                    cursor += 1
            if depth:
                raise ScanError(f"{path}:{start_line}: unterminated block comment")
            index = cursor
            continue

        raw_end = _raw_string_end(source, index, path)
        if raw_end is not None:
            line += source.count("\n", index, raw_end)
            index = raw_end
            continue

        if char == '"' or (char in "bc" and source.startswith('"', index + 1)):
            quote_index = index if char == '"' else index + 1
            end = _skip_quoted(source, quote_index, path)
            line += source.count("\n", index, end)
            index = end
            continue

        if char == "'" or (char == "b" and source.startswith("'", index + 1)):
            quote_index = index if char == "'" else index + 1
            end = _char_literal_end(source, quote_index)
            if end is not None:
                index = end
                continue

        if char.isalpha() or char == "_":
            end = index + 1
            while end < len(source) and (source[end].isalnum() or source[end] == "_"):
                end += 1
            tokens.append(Token("identifier", source[index:end], line))
            index = end
            continue

        if source.startswith("::", index):
            tokens.append(Token("punctuation", "::", line))
            index += 2
            continue

        tokens.append(Token("punctuation", char, line))
        index += 1

    return tokens


def _macro_name(tokens: list[Token], index: int) -> str | None:
    token = tokens[index]
    if token.kind != "identifier" or token.value not in LOG_MACROS:
        return None
    if index + 2 >= len(tokens) or tokens[index + 1].value != "!":
        return None
    if tokens[index + 2].value not in OPEN_TO_CLOSE:
        return None

    if index > 0 and tokens[index - 1].value == "::":
        if index < 2 or tokens[index - 2].value != "tracing":
            return None
        return f"tracing::{token.value}"
    return token.value


def _matching_delimiter(tokens: list[Token], open_index: int, path: Path) -> int:
    stack: list[Token] = []
    for index in range(open_index, len(tokens)):
        token = tokens[index]
        if token.value in OPEN_TO_CLOSE:
            stack.append(token)
            continue
        if token.value not in CLOSE_TO_OPEN:
            continue
        if not stack or stack[-1].value != CLOSE_TO_OPEN[token.value]:
            raise ScanError(f"{path}:{token.line}: mismatched macro delimiter")
        stack.pop()
        if not stack:
            return index
    token = tokens[open_index]
    raise ScanError(f"{path}:{token.line}: unterminated log macro invocation")


def scan_source(source: str, path: Path) -> list[Finding]:
    tokens = tokenize(source, path)
    findings: list[Finding] = []

    for index in range(len(tokens)):
        macro = _macro_name(tokens, index)
        if macro is None:
            continue
        open_index = index + 2
        close_index = _matching_delimiter(tokens, open_index, path)
        identifiers = tuple(
            sorted(
                {
                    token.value
                    for token in tokens[open_index + 1 : close_index]
                    if token.kind == "identifier" and token.value in SESSION_IDENTIFIERS
                }
            )
        )
        if identifiers:
            findings.append(Finding(path, tokens[index].line, macro, identifiers))

    return findings


def scan_tree(root: Path) -> list[Finding]:
    if not root.is_dir():
        raise ScanError(f"source directory does not exist: {root}")

    findings: list[Finding] = []
    for path in sorted(root.rglob("*.rs")):
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise ScanError(f"cannot read {path}: {error}") from error
        findings.extend(scan_source(source, path))
    return findings


def run_self_test() -> None:
    source = r"""
fn examples(sid: &str, session_token: &str, session_id: &str) {
    warn!("invalid session; {}", sid);
    match true {
        true => tracing::trace!("invalid {}", session_token),
        false => info!("valid"),
    }
    debug!(
        "nested {}",
        redact(session_id.clone()),
    );
    info!("session_id and sid are text only");
    let raw = r#"warn!(session_token)"#;
    // error!("{}", session_id);
    /* warn!("{}", sid); */
    audit!("{}", sid);
    other::warn!("{}", sid);
}
"""
    findings = scan_source(source, Path("self-test.rs"))
    actual = [(finding.macro, finding.identifiers) for finding in findings]
    expected = [
        ("warn", ("sid",)),
        ("tracing::trace", ("session_token",)),
        ("debug", ("session_id",)),
    ]
    if actual != expected:
        raise ScanError(f"self-test mismatch: expected {expected}, got {actual}")


def main(argv: list[str]) -> int:
    try:
        if argv == ["--self-test"]:
            run_self_test()
            print("Log session scanner self-test passed.")
            return 0
        if len(argv) != 1:
            print(f"usage: {Path(sys.argv[0]).name} <source-dir>", file=sys.stderr)
            return 2
        for finding in scan_tree(Path(argv[0])):
            print(finding.render())
        return 0
    except ScanError as error:
        print(f"Log session scanner failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
