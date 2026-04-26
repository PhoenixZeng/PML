from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Dict, List, Optional, Tuple


META_TYPE = "type"
META_CONTENT = "content"
META_TAG = "tag"
META_ORDER = "order"


@dataclass(frozen=True)
class PmlBlock:
    name: str
    ty: str
    content: str
    paired: bool


@dataclass(frozen=True)
class PmlTreeOptions:
    meta_prefix: str = "$"


class PmlErrorKind(str, Enum):
    EXPECTED_BLOCK_HEADER = "expected_block_header"
    INVALID_CONTROL_LINE = "invalid_control_line"
    INVALID_NAME = "invalid_name"
    INVALID_TYPE = "invalid_type"
    STRAY_CLOSING_BLOCK = "stray_closing_block"
    META_FIELD_CONFLICT = "meta_field_conflict"
    INVALID_TREE = "invalid_tree"


class PmlError(Exception):
    def __init__(
        self,
        line: int,
        kind: PmlErrorKind,
        detail: str = "",
        expected: str = "",
        found: str = "",
    ):
        self.line = line
        self.kind = kind
        self.detail = detail
        self.expected = expected
        self.found = found
        super().__init__(str(self))

    def __str__(self) -> str:
        if self.kind == PmlErrorKind.EXPECTED_BLOCK_HEADER:
            return f"line {self.line}: expected a PML block header"
        if self.kind == PmlErrorKind.INVALID_CONTROL_LINE:
            return f"line {self.line}: invalid control line `{self.detail}`"
        if self.kind == PmlErrorKind.INVALID_NAME:
            return f"line {self.line}: invalid block name `{self.detail}`"
        if self.kind == PmlErrorKind.INVALID_TYPE:
            return f"line {self.line}: invalid block type `{self.detail}`"
        if self.kind == PmlErrorKind.STRAY_CLOSING_BLOCK:
            return f"line {self.line}: stray closing block `/{self.detail}`"
        if self.kind == PmlErrorKind.META_FIELD_CONFLICT:
            return f"line {self.line}: tree meta field conflicts with child key `{self.detail}`"
        if self.kind == PmlErrorKind.INVALID_TREE:
            return f"line {self.line}: invalid PML tree: {self.detail}"
        return f"line {self.line}: {self.kind.value}"


@dataclass(frozen=True)
class _Line:
    start: int
    end: int


@dataclass
class _CollectedBlock:
    block: PmlBlock
    order: Optional[int]
    sequence: int


def parse_pml(text: str) -> List[PmlBlock]:
    text = _normalize_newlines(text)
    lines = _collect_lines(text)
    blocks: List[PmlBlock] = []
    i = 0

    while i < len(lines):
        line = _line_text(text, lines[i])
        control = _parse_control_line(line, i + 1)

        if control is None and line.strip() == "":
            i += 1
            continue

        if control is None:
            raise PmlError(i + 1, PmlErrorKind.EXPECTED_BLOCK_HEADER)

        kind, name, ty = control
        if kind == "close":
            raise PmlError(i + 1, PmlErrorKind.STRAY_CLOSING_BLOCK, detail=name)

        close_index = _find_matching_close(text, lines, i + 1, name)
        if close_index is not None:
            blocks.append(
                PmlBlock(
                    name=name,
                    ty=ty,
                    content=_content_between(text, lines, i + 1, close_index),
                    paired=True,
                )
            )
            i = close_index + 1
        else:
            end = _find_next_opening(text, lines, i + 1)
            blocks.append(
                PmlBlock(
                    name=name,
                    ty=ty,
                    content=_content_between(text, lines, i + 1, end),
                    paired=False,
                )
            )
            i = end

    return blocks


def parse_pml_tree(text: str, options: Optional[PmlTreeOptions] = None) -> Dict[str, Any]:
    return blocks_to_tree(parse_pml(text), options)


def blocks_to_tree(blocks: List[PmlBlock], options: Optional[PmlTreeOptions] = None) -> Dict[str, Any]:
    opts = options or PmlTreeOptions()
    root: Dict[str, Any] = {}

    for index, block in enumerate(blocks):
        _insert_block_into_tree(root, block, index, opts)

    return root


def tree_to_blocks(tree: Dict[str, Any], options: Optional[PmlTreeOptions] = None) -> List[PmlBlock]:
    if not isinstance(tree, dict):
        raise PmlError(0, PmlErrorKind.INVALID_TREE, detail="root must be an object")

    opts = options or PmlTreeOptions()
    collected: List[_CollectedBlock] = []
    path: List[str] = []
    sequence = 0
    sequence = _collect_blocks_from_object(tree, True, path, collected, sequence, opts)

    collected.sort(
        key=lambda item: (
            0 if item.order is not None else 1,
            item.order if item.order is not None else 0,
            item.sequence,
        )
    )
    return [item.block for item in collected]


def render_pml_tree(tree: Dict[str, Any], options: Optional[PmlTreeOptions] = None) -> str:
    return render_blocks(tree_to_blocks(tree, options))


class PmlBuilder:
    def __init__(self) -> None:
        self._blocks: List[PmlBlock] = []

    def push_short(self, name: str, ty: Optional[str], content: str) -> "PmlBuilder":
        return self._push_block(name, ty, content, False)

    def push_paired(self, name: str, ty: Optional[str], content: str) -> "PmlBuilder":
        return self._push_block(name, ty, content, True)

    def build(self) -> str:
        return render_blocks(self._blocks)

    def _push_block(self, name: str, ty: Optional[str], content: str, paired: bool) -> "PmlBuilder":
        if not _is_valid_name(name):
            raise PmlError(0, PmlErrorKind.INVALID_NAME, detail=name)
        normalized_type = _normalize_type("" if ty is None else ty)
        self._blocks.append(
            PmlBlock(
                name=name,
                ty=normalized_type,
                content=_normalize_newlines(content),
                paired=paired,
            )
        )
        return self


def render_blocks(blocks: List[PmlBlock]) -> str:
    parts: List[str] = []
    for block in blocks:
        header = f"[{block.name}"
        if block.ty != "":
            header += f":{block.ty}"
        header += "]\n"
        parts.append(header)
        parts.append(block.content)
        if block.content:
            parts.append("\n")
        if _should_render_as_paired(block):
            parts.append(f"[/{block.name}]\n")
    return "".join(parts)


def _normalize_newlines(text: str) -> str:
    return text.replace("\r\n", "\n").replace("\r", "\n")


def _collect_lines(text: str) -> List[_Line]:
    lines: List[_Line] = []
    start = 0
    for index, ch in enumerate(text):
        if ch == "\n":
            lines.append(_Line(start, index))
            start = index + 1
    if start < len(text):
        lines.append(_Line(start, len(text)))
    return lines


def _line_text(text: str, line: _Line) -> str:
    return text[line.start : line.end]


def _content_between(text: str, lines: List[_Line], start: int, end: int) -> str:
    if start >= end:
        return ""
    return text[lines[start].start : lines[end - 1].end]


def _find_matching_close(text: str, lines: List[_Line], start: int, name: str) -> Optional[int]:
    for idx in range(start, len(lines)):
        line = _line_text(text, lines[idx])
        control = _parse_control_line(line, idx + 1)
        if control is None:
            continue
        kind, close_name, _close_ty = control
        if kind == "close" and close_name == name:
            return idx
    return None


def _find_next_opening(text: str, lines: List[_Line], start: int) -> int:
    for idx in range(start, len(lines)):
        line = _line_text(text, lines[idx])
        control = _parse_control_line(line, idx + 1)
        if control is not None and control[0] == "open":
            return idx
    return len(lines)


def _parse_control_line(line: str, line_no: int) -> Optional[Tuple[str, str, Optional[str] | str]]:
    if not line.startswith("[") and not line.endswith("]"):
        return None
    if not (line.startswith("[") and line.endswith("]")):
        raise PmlError(line_no, PmlErrorKind.INVALID_CONTROL_LINE, detail=line)

    inner = line[1:-1]
    if inner == "":
        raise PmlError(line_no, PmlErrorKind.INVALID_CONTROL_LINE, detail=line)

    if inner.startswith("/"):
        name = _parse_closing_name(inner[1:], line_no, line)
        return ("close", name, None)

    name, ty = _parse_name_and_required_type(inner, line_no)
    return ("open", name, ty)


def _should_render_as_paired(block: PmlBlock) -> bool:
    if block.paired:
        return True
    return _content_has_control_line(block.content)


def _content_has_control_line(content: str) -> bool:
    if content == "":
        return False
    for line in _normalize_newlines(content).split("\n"):
        try:
            if _parse_control_line(line, 0) is not None:
                return True
        except PmlError:
            continue
    return False


def _parse_name_and_required_type(value: str, line_no: int) -> Tuple[str, str]:
    if ":" in value:
        name, ty = value.split(":", 1)
    else:
        name, ty = value, ""
    _validate_name_or_raise(name, line_no)
    return name, _normalize_type(ty, line_no)


def _parse_closing_name(value: str, line_no: int, line: str) -> str:
    if ":" in value:
        raise PmlError(line_no, PmlErrorKind.INVALID_CONTROL_LINE, detail=line)
    _validate_name_or_raise(value, line_no)
    return value


def _validate_name_or_raise(name: str, line_no: int) -> None:
    if not _is_valid_name(name):
        raise PmlError(line_no, PmlErrorKind.INVALID_NAME, detail=name)


def _normalize_type(value: str, line_no: int = 0) -> str:
    if value == "":
        return ""
    if not all(_is_type_char(ch) for ch in value):
        raise PmlError(line_no, PmlErrorKind.INVALID_TYPE, detail=value)
    value = value.lower()
    return "markdown" if value == "md" else value


def _is_valid_name(name: str) -> bool:
    parts = name.split("#")
    if len(parts) > 2 or not parts[0]:
        return False
    main = parts[0]
    tag = parts[1] if len(parts) == 2 else None
    return _is_valid_main_name(main) and (tag is None or _is_valid_tag(tag))


def _is_valid_main_name(main: str) -> bool:
    segments = main.split(".")
    return all(segment and all(_is_name_char(ch) for ch in segment) for segment in segments)


def _is_valid_tag(tag: str) -> bool:
    return bool(tag) and all(_is_name_char(ch) for ch in tag)


def _is_name_char(ch: str) -> bool:
    return ch == "_" or ch.isalnum()


def _is_type_char(ch: str) -> bool:
    return ch.isascii() and (ch.isalnum() or ch == "_" or ch == "-")


def _insert_block_into_tree(root: Dict[str, Any], block: PmlBlock, order: int, options: PmlTreeOptions) -> None:
    main_name, tag = _split_block_name(block.name)
    segments = main_name.split(".")
    deepest = _deepest_unique_prefix(root, segments)

    if deepest == len(segments):
        existing = _get_unique_object(root, segments)
        if not _object_has_meta(existing, options):
            _apply_block_meta(existing, block, order, tag, options)
            return

    prefix_len = len(segments) - 1 if deepest == len(segments) else deepest
    parent = _get_unique_object(root, segments[:prefix_len])
    _attach_remaining_path(parent, prefix_len == 0, segments[prefix_len:], block, order, tag, options)


def _split_block_name(name: str) -> Tuple[str, Optional[str]]:
    if "#" in name:
        main, tag = name.split("#", 1)
        return main, tag
    return name, None


def _deepest_unique_prefix(root: Dict[str, Any], segments: List[str]) -> int:
    current: List[Dict[str, Any]] = [root]
    deepest = 0

    for index, segment in enumerate(segments):
        next_objects: List[Dict[str, Any]] = []
        for obj in current:
            if segment in obj:
                _collect_child_objects(obj[segment], next_objects)
        if len(next_objects) == 1:
            deepest = index + 1
            current = next_objects
        else:
            break

    return deepest


def _collect_child_objects(value: Any, out: List[Dict[str, Any]]) -> None:
    if isinstance(value, dict):
        out.append(value)
    elif isinstance(value, list):
        for item in value:
            if isinstance(item, dict):
                out.append(item)


def _get_unique_object(root: Dict[str, Any], segments: List[str]) -> Dict[str, Any]:
    current: Any = root
    index = 0

    while True:
        if isinstance(current, dict):
            if index == len(segments):
                return current
            key = segments[index]
            if key not in current:
                raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"missing object path `{'.'.join(segments[: index + 1])}`")
            current = current[key]
            index += 1
        elif isinstance(current, list):
            objects = [item for item in current if isinstance(item, dict)]
            if len(objects) != 1:
                raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"path `{'.'.join(segments[:index])}` is not unique")
            current = objects[0]
        else:
            raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"path `{'.'.join(segments[:index])}` does not resolve to an object")


def _attach_remaining_path(
    parent: Dict[str, Any],
    parent_is_root: bool,
    remaining: List[str],
    block: PmlBlock,
    order: int,
    tag: Optional[str],
    options: PmlTreeOptions,
) -> None:
    if not remaining:
        raise PmlError(0, PmlErrorKind.INVALID_TREE, detail="empty path cannot be attached")

    key = remaining[0]
    _ensure_child_key_allowed(parent_is_root, key, options)
    leaf = _build_leaf_object(block, order, tag, options)

    if len(remaining) == 1:
        existing = parent.get(key)
        if isinstance(existing, dict) and _object_has_meta(existing, options):
            parent[key] = [existing, leaf]
            return
        if isinstance(existing, dict):
            _apply_block_meta(existing, block, order, tag, options)
            return
        if isinstance(existing, list):
            existing.append(leaf)
            return
        if existing is not None:
            raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"path `{key}` does not resolve to an object")
        parent[key] = leaf
        return

    branch = _build_chain_value(remaining[1:], leaf, options)
    existing = parent.get(key)
    if isinstance(existing, dict):
        _attach_remaining_path(existing, False, remaining[1:], block, order, tag, options)
        return
    if isinstance(existing, list):
        existing.append(branch)
        return
    if existing is not None:
        raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"path `{key}` does not resolve to an object")
    parent[key] = branch


def _build_leaf_object(block: PmlBlock, order: int, tag: Optional[str], options: PmlTreeOptions) -> Dict[str, Any]:
    obj: Dict[str, Any] = {}
    _apply_block_meta(obj, block, order, tag, options)
    return obj


def _build_chain_value(tail: List[str], leaf: Dict[str, Any], options: PmlTreeOptions) -> Dict[str, Any]:
    current: Dict[str, Any] = leaf
    for segment in reversed(tail):
        _ensure_child_key_allowed(False, segment, options)
        current = {segment: current}
    return current


def _apply_block_meta(obj: Dict[str, Any], block: PmlBlock, order: int, tag: Optional[str], options: PmlTreeOptions) -> None:
    _insert_meta_field(obj, _meta_key(options, META_TYPE), block.ty, options)
    _insert_meta_field(obj, _meta_key(options, META_CONTENT), block.content, options)
    _insert_meta_field(obj, _meta_key(options, META_ORDER), order, options)
    if tag is not None:
        _insert_meta_field(obj, _meta_key(options, META_TAG), tag, options)


def _insert_meta_field(obj: Dict[str, Any], key: str, value: Any, options: PmlTreeOptions) -> None:
    if key in obj:
        if obj[key] != value:
            raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"duplicate meta field `{key}`")
        return
    if options.meta_prefix == "" and key in obj and _is_reserved_child_key(key, options):
        raise PmlError(0, PmlErrorKind.META_FIELD_CONFLICT, detail=key)
    obj[key] = value


def _object_has_meta(obj: Dict[str, Any], options: PmlTreeOptions) -> bool:
    return any(_meta_key(options, name) in obj for name in (META_TYPE, META_CONTENT, META_TAG, META_ORDER))


def _meta_key(options: PmlTreeOptions, name: str) -> str:
    return f"{options.meta_prefix}{name}"


def _ensure_child_key_allowed(parent_is_root: bool, key: str, options: PmlTreeOptions) -> None:
    if not parent_is_root and _is_reserved_child_key(key, options):
        raise PmlError(0, PmlErrorKind.META_FIELD_CONFLICT, detail=key)


def _is_reserved_child_key(key: str, options: PmlTreeOptions) -> bool:
    return key in {_meta_key(options, META_TYPE), _meta_key(options, META_CONTENT), _meta_key(options, META_TAG), _meta_key(options, META_ORDER)}


def _collect_blocks_from_object(
    obj: Dict[str, Any],
    is_root: bool,
    path: List[str],
    collected: List[_CollectedBlock],
    sequence: int,
    options: PmlTreeOptions,
) -> int:
    if not is_root:
        meta = _extract_meta(obj, options)
        if meta["has_any"]:
            base_name = ".".join(path)
            tag = meta["tag"]
            name = f"{base_name}#{tag}" if tag is not None else base_name
            _validate_name_or_raise(name, 0)
            ty = _normalize_type(meta["ty"] or "")
            content = _normalize_newlines(meta["content"] or "")
            collected.append(
                _CollectedBlock(
                    block=PmlBlock(name=name, ty=ty, content=content, paired=False),
                    order=meta["order"],
                    sequence=sequence,
                )
            )
            sequence += 1

    for key, value in obj.items():
        if is_root:
            if _is_reserved_child_key(key, options):
                raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"root object cannot contain meta field `{key}`")
        elif _is_reserved_child_key(key, options):
            continue
        path.append(key)
        sequence = _collect_blocks_from_value(value, path, collected, sequence, options)
        path.pop()

    return sequence


def _collect_blocks_from_value(
    value: Any,
    path: List[str],
    collected: List[_CollectedBlock],
    sequence: int,
    options: PmlTreeOptions,
) -> int:
    if isinstance(value, dict):
        return _collect_blocks_from_object(value, False, path, collected, sequence, options)
    if isinstance(value, list):
        for item in value:
            if isinstance(item, dict):
                sequence = _collect_blocks_from_object(item, False, path, collected, sequence, options)
            elif isinstance(item, list):
                sequence = _collect_blocks_from_value(item, path, collected, sequence, options)
            else:
                raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"array at `{'.'.join(path)}` must contain objects")
        return sequence
    raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"path `{'.'.join(path)}` must resolve to an object or array")


def _extract_meta(obj: Dict[str, Any], options: PmlTreeOptions) -> Dict[str, Any]:
    meta = {"ty": None, "content": None, "tag": None, "order": None, "has_any": False}

    type_key = _meta_key(options, META_TYPE)
    if type_key in obj:
        meta["ty"] = _expect_tree_string(obj[type_key], type_key)
        meta["has_any"] = True

    content_key = _meta_key(options, META_CONTENT)
    if content_key in obj:
        meta["content"] = _expect_tree_string(obj[content_key], content_key)
        meta["has_any"] = True

    tag_key = _meta_key(options, META_TAG)
    if tag_key in obj:
        meta["tag"] = _expect_tree_string(obj[tag_key], tag_key)
        meta["has_any"] = True

    order_key = _meta_key(options, META_ORDER)
    if order_key in obj:
        order = _expect_tree_integer(obj[order_key], order_key)
        if order < 0:
            raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"meta field `{order_key}` must be a non-negative integer")
        meta["order"] = order
        meta["has_any"] = True

    return meta


def _expect_tree_string(value: Any, key: str) -> str:
    if isinstance(value, str):
        return value
    raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"meta field `{key}` must be a string")


def _expect_tree_integer(value: Any, key: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise PmlError(0, PmlErrorKind.INVALID_TREE, detail=f"meta field `{key}` must be an integer")
    return value
