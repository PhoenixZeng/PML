"use strict";

const META_TYPE = "type";
const META_CONTENT = "content";
const META_TAG = "tag";
const META_ORDER = "order";

class PmlBlock {
  constructor(name, type, content, paired) {
    this.name = name;
    this.ty = type;
    this.content = content;
    this.paired = paired;
  }
}

class PmlTreeOptions {
  constructor(metaPrefix = "$") {
    this.metaPrefix = metaPrefix;
  }
}

const PmlErrorKind = Object.freeze({
  EXPECTED_BLOCK_HEADER: "expected_block_header",
  INVALID_CONTROL_LINE: "invalid_control_line",
  INVALID_NAME: "invalid_name",
  INVALID_TYPE: "invalid_type",
  STRAY_CLOSING_BLOCK: "stray_closing_block",
  TYPE_MISMATCH: "type_mismatch",
  META_FIELD_CONFLICT: "meta_field_conflict",
  INVALID_TREE: "invalid_tree",
});

class PmlError extends Error {
  constructor(line, kind, detail = "", expected = "", found = "") {
    super();
    this.line = line;
    this.kind = kind;
    this.detail = detail;
    this.expected = expected;
    this.found = found;
    this.message = this.toString();
  }

  toString() {
    switch (this.kind) {
      case PmlErrorKind.EXPECTED_BLOCK_HEADER:
        return `line ${this.line}: expected a PML block header`;
      case PmlErrorKind.INVALID_CONTROL_LINE:
        return `line ${this.line}: invalid control line \`${this.detail}\``;
      case PmlErrorKind.INVALID_NAME:
        return `line ${this.line}: invalid block name \`${this.detail}\``;
      case PmlErrorKind.INVALID_TYPE:
        return `line ${this.line}: invalid block type \`${this.detail}\``;
      case PmlErrorKind.STRAY_CLOSING_BLOCK:
        return `line ${this.line}: stray closing block \`/${this.detail}\``;
      case PmlErrorKind.TYPE_MISMATCH:
        return `line ${this.line}: closing type \`${this.found}\` does not match opening type \`${this.expected}\``;
      case PmlErrorKind.META_FIELD_CONFLICT:
        return `line ${this.line}: tree meta field conflicts with child key \`${this.detail}\``;
      case PmlErrorKind.INVALID_TREE:
        return `line ${this.line}: invalid PML tree: ${this.detail}`;
      default:
        return `line ${this.line}: ${this.kind}`;
    }
  }
}

class PmlBuilder {
  constructor() {
    this.blocks = [];
  }

  pushShort(name, type, content) {
    return this.#pushBlock(name, type, content, false);
  }

  pushPaired(name, type, content) {
    return this.#pushBlock(name, type, content, true);
  }

  build() {
    return renderBlocks(this.blocks);
  }

  #pushBlock(name, type, content, paired) {
    if (!isValidName(name)) {
      throw new PmlError(0, PmlErrorKind.INVALID_NAME, name);
    }
    this.blocks.push(
      new PmlBlock(name, normalizeType(type ?? "text"), normalizeNewlines(content), paired),
    );
    return this;
  }
}

function parsePml(input) {
  const text = normalizeNewlines(input);
  const lines = collectLines(text);
  const blocks = [];
  let i = 0;

  while (i < lines.length) {
    const line = lineText(text, lines[i]);
    const control = parseControlLine(line, i + 1);

    if (control === null && line.trim() === "") {
      i += 1;
      continue;
    }
    if (control === null) {
      throw new PmlError(i + 1, PmlErrorKind.EXPECTED_BLOCK_HEADER);
    }
    if (control.kind === "close") {
      throw new PmlError(i + 1, PmlErrorKind.STRAY_CLOSING_BLOCK, control.name);
    }

    const closeIndex = findMatchingClose(text, lines, i + 1, control.name, control.ty);
    if (closeIndex !== null) {
      blocks.push(
        new PmlBlock(
          control.name,
          control.ty,
          contentBetween(text, lines, i + 1, closeIndex),
          true,
        ),
      );
      i = closeIndex + 1;
    } else {
      const end = findNextOpening(text, lines, i + 1);
      blocks.push(
        new PmlBlock(
          control.name,
          control.ty,
          contentBetween(text, lines, i + 1, end),
          false,
        ),
      );
      i = end;
    }
  }

  return blocks;
}

function parsePmlTree(input, options = new PmlTreeOptions()) {
  return blocksToTree(parsePml(input), options);
}

function blocksToTree(blocks, options = new PmlTreeOptions()) {
  const root = {};
  blocks.forEach((block, index) => {
    insertBlockIntoTree(root, block, index, options);
  });
  return root;
}

function treeToBlocks(tree, options = new PmlTreeOptions()) {
  if (!isPlainObject(tree)) {
    throw new PmlError(0, PmlErrorKind.INVALID_TREE, "root must be an object");
  }

  const collected = [];
  const path = [];
  const state = { sequence: 0 };
  collectBlocksFromObject(tree, true, path, collected, state, options);

  collected.sort((left, right) => {
    if (left.order !== null && right.order !== null) {
      return left.order - right.order || left.sequence - right.sequence;
    }
    if (left.order !== null) {
      return -1;
    }
    if (right.order !== null) {
      return 1;
    }
    return left.sequence - right.sequence;
  });

  return collected.map((item) => item.block);
}

function renderPmlTree(tree, options = new PmlTreeOptions()) {
  return renderBlocks(treeToBlocks(tree, options));
}

function renderBlocks(blocks) {
  let out = "";
  for (const block of blocks) {
    out += `[${block.name}`;
    if (block.ty !== "text") {
      out += `:${block.ty}`;
    }
    out += "]\n";
    out += block.content;
    if (block.content.length > 0) {
      out += "\n";
    }
    if (shouldRenderAsPaired(block)) {
      out += `[/${block.name}]\n`;
    }
  }
  return out;
}

function normalizeNewlines(input) {
  return input.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
}

function collectLines(text) {
  const lines = [];
  let start = 0;
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === "\n") {
      lines.push({ start, end: i });
      start = i + 1;
    }
  }
  if (start < text.length) {
    lines.push({ start, end: text.length });
  }
  return lines;
}

function lineText(text, line) {
  return text.slice(line.start, line.end);
}

function contentBetween(text, lines, start, end) {
  if (start >= end) {
    return "";
  }
  return text.slice(lines[start].start, lines[end - 1].end);
}

function findMatchingClose(text, lines, start, name, ty) {
  for (let idx = start; idx < lines.length; idx += 1) {
    const control = parseControlLine(lineText(text, lines[idx]), idx + 1);
    if (control && control.kind === "close" && control.name === name) {
      if (control.ty !== null && control.ty !== ty) {
        throw new PmlError(idx + 1, PmlErrorKind.TYPE_MISMATCH, "", ty, control.ty);
      }
      return idx;
    }
  }
  return null;
}

function findNextOpening(text, lines, start) {
  for (let idx = start; idx < lines.length; idx += 1) {
    const control = parseControlLine(lineText(text, lines[idx]), idx + 1);
    if (control && control.kind === "open") {
      return idx;
    }
  }
  return lines.length;
}

function parseControlLine(line, lineNo) {
  if (!line.startsWith("[") && !line.endsWith("]")) {
    return null;
  }
  if (!(line.startsWith("[") && line.endsWith("]"))) {
    throw new PmlError(lineNo, PmlErrorKind.INVALID_CONTROL_LINE, line);
  }

  const inner = line.slice(1, -1);
  if (inner.length === 0) {
    throw new PmlError(lineNo, PmlErrorKind.INVALID_CONTROL_LINE, line);
  }

  if (inner.startsWith("/")) {
    const [name, ty] = parseNameAndOptionalType(inner.slice(1), lineNo);
    return { kind: "close", name, ty };
  }

  const [name, ty] = parseNameAndRequiredType(inner, lineNo);
  return { kind: "open", name, ty };
}

function shouldRenderAsPaired(block) {
  return block.paired || contentHasControlLine(block.content);
}

function contentHasControlLine(content) {
  if (content.length === 0) {
    return false;
  }
  for (const line of normalizeNewlines(content).split("\n")) {
    try {
      if (parseControlLine(line, 0) !== null) {
        return true;
      }
    } catch (_error) {
      continue;
    }
  }
  return false;
}

function parseNameAndRequiredType(value, lineNo) {
  const split = value.indexOf(":");
  const name = split >= 0 ? value.slice(0, split) : value;
  const type = split >= 0 ? value.slice(split + 1) : "text";
  validateNameOrThrow(name, lineNo);
  return [name, normalizeType(type, lineNo)];
}

function parseNameAndOptionalType(value, lineNo) {
  const split = value.indexOf(":");
  if (split >= 0) {
    const name = value.slice(0, split);
    const type = value.slice(split + 1);
    validateNameOrThrow(name, lineNo);
    return [name, normalizeType(type, lineNo)];
  }
  validateNameOrThrow(value, lineNo);
  return [value, null];
}

function validateNameOrThrow(name, lineNo) {
  if (!isValidName(name)) {
    throw new PmlError(lineNo, PmlErrorKind.INVALID_NAME, name);
  }
}

function normalizeType(type, lineNo = 0) {
  if (type.length === 0 || ![...type].every(isTypeChar)) {
    throw new PmlError(lineNo, PmlErrorKind.INVALID_TYPE, type);
  }
  const normalized = type.toLowerCase();
  return normalized === "md" ? "markdown" : normalized;
}

function isValidName(name) {
  const parts = name.split("#");
  if (parts.length > 2 || parts[0].length === 0) {
    return false;
  }
  return isValidMainName(parts[0]) && (parts.length === 1 || isValidTag(parts[1]));
}

function isValidMainName(main) {
  return main.split(".").every((segment) => segment.length > 0 && [...segment].every(isNameChar));
}

function isValidTag(tag) {
  return tag.length > 0 && [...tag].every(isNameChar);
}

function isNameChar(ch) {
  return ch === "_" || /[\p{L}\p{N}]/u.test(ch);
}

function isTypeChar(ch) {
  return /[A-Za-z0-9_-]/.test(ch);
}

function insertBlockIntoTree(root, block, order, options) {
  const [mainName, tag] = splitBlockName(block.name);
  const segments = mainName.split(".");
  const deepest = deepestUniquePrefix(root, segments);

  if (deepest === segments.length) {
    const existing = getUniqueObject(root, segments);
    if (!objectHasMeta(existing, options)) {
      applyBlockMeta(existing, block, order, tag, options);
      return;
    }
  }

  const prefixLength = deepest === segments.length ? Math.max(segments.length - 1, 0) : deepest;
  const parent = getUniqueObject(root, segments.slice(0, prefixLength));
  attachRemainingPath(parent, prefixLength === 0, segments.slice(prefixLength), block, order, tag, options);
}

function splitBlockName(name) {
  const split = name.indexOf("#");
  if (split >= 0) {
    return [name.slice(0, split), name.slice(split + 1)];
  }
  return [name, null];
}

function deepestUniquePrefix(root, segments) {
  let current = [root];
  let deepest = 0;

  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    const next = [];
    for (const object of current) {
      if (hasOwn(object, segment)) {
        collectChildObjects(object[segment], next);
      }
    }
    if (next.length === 1) {
      deepest = index + 1;
      current = next;
    } else {
      break;
    }
  }

  return deepest;
}

function collectChildObjects(value, out) {
  if (isPlainObject(value)) {
    out.push(value);
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      if (isPlainObject(item)) {
        out.push(item);
      }
    }
  }
}

function getUniqueObject(root, segments) {
  let current = root;
  let index = 0;

  while (true) {
    if (isPlainObject(current)) {
      if (index === segments.length) {
        return current;
      }
      const key = segments[index];
      if (!hasOwn(current, key)) {
        throw new PmlError(0, PmlErrorKind.INVALID_TREE, `missing object path \`${segments.slice(0, index + 1).join(".")}\``);
      }
      current = current[key];
      index += 1;
      continue;
    }

    if (Array.isArray(current)) {
      const objects = current.filter((item) => isPlainObject(item));
      if (objects.length !== 1) {
        throw new PmlError(0, PmlErrorKind.INVALID_TREE, `path \`${segments.slice(0, index).join(".")}\` is not unique`);
      }
      current = objects[0];
      continue;
    }

    throw new PmlError(0, PmlErrorKind.INVALID_TREE, `path \`${segments.slice(0, index).join(".")}\` does not resolve to an object`);
  }
}

function attachRemainingPath(parent, parentIsRoot, remaining, block, order, tag, options) {
  if (remaining.length === 0) {
    throw new PmlError(0, PmlErrorKind.INVALID_TREE, "empty path cannot be attached");
  }

  const key = remaining[0];
  ensureChildKeyAllowed(parentIsRoot, key, options);
  const leaf = buildLeafObject(block, order, tag, options);

  if (remaining.length === 1) {
    const existing = parent[key];
    if (isPlainObject(existing) && objectHasMeta(existing, options)) {
      parent[key] = [existing, leaf];
      return;
    }
    if (isPlainObject(existing)) {
      applyBlockMeta(existing, block, order, tag, options);
      return;
    }
    if (Array.isArray(existing)) {
      existing.push(leaf);
      return;
    }
    if (existing !== undefined) {
      throw new PmlError(0, PmlErrorKind.INVALID_TREE, `path \`${key}\` does not resolve to an object`);
    }
    parent[key] = leaf;
    return;
  }

  const branch = buildChainValue(remaining.slice(1), leaf, options);
  const existing = parent[key];
  if (isPlainObject(existing)) {
    attachRemainingPath(existing, false, remaining.slice(1), block, order, tag, options);
    return;
  }
  if (Array.isArray(existing)) {
    existing.push(branch);
    return;
  }
  if (existing !== undefined) {
    throw new PmlError(0, PmlErrorKind.INVALID_TREE, `path \`${key}\` does not resolve to an object`);
  }
  parent[key] = branch;
}

function buildLeafObject(block, order, tag, options) {
  const object = {};
  applyBlockMeta(object, block, order, tag, options);
  return object;
}

function buildChainValue(tail, leaf, options) {
  let current = leaf;
  for (let index = tail.length - 1; index >= 0; index -= 1) {
    const segment = tail[index];
    ensureChildKeyAllowed(false, segment, options);
    current = { [segment]: current };
  }
  return current;
}

function applyBlockMeta(object, block, order, tag, options) {
  insertMetaField(object, metaKey(options, META_TYPE), block.ty, options);
  insertMetaField(object, metaKey(options, META_CONTENT), block.content, options);
  insertMetaField(object, metaKey(options, META_ORDER), order, options);
  if (tag !== null) {
    insertMetaField(object, metaKey(options, META_TAG), tag, options);
  }
}

function insertMetaField(object, key, value) {
  if (hasOwn(object, key)) {
    if (object[key] !== value) {
      throw new PmlError(0, PmlErrorKind.INVALID_TREE, `duplicate meta field \`${key}\``);
    }
    return;
  }
  object[key] = value;
}

function objectHasMeta(object, options) {
  return [META_TYPE, META_CONTENT, META_TAG, META_ORDER].some((name) => hasOwn(object, metaKey(options, name)));
}

function metaKey(options, name) {
  return `${options.metaPrefix}${name}`;
}

function ensureChildKeyAllowed(parentIsRoot, key, options) {
  if (!parentIsRoot && isReservedChildKey(key, options)) {
    throw new PmlError(0, PmlErrorKind.META_FIELD_CONFLICT, key);
  }
}

function isReservedChildKey(key, options) {
  return [META_TYPE, META_CONTENT, META_TAG, META_ORDER]
    .map((name) => metaKey(options, name))
    .includes(key);
}

function collectBlocksFromObject(object, isRoot, path, collected, state, options) {
  if (!isRoot) {
    const meta = extractMeta(object, options);
    if (meta.hasAny) {
      const baseName = path.join(".");
      const name = meta.tag === null ? baseName : `${baseName}#${meta.tag}`;
      validateNameOrThrow(name, 0);
      const ty = normalizeType(meta.ty ?? "text");
      const content = normalizeNewlines(meta.content ?? "");
      collected.push({
        block: new PmlBlock(name, ty, content, false),
        order: meta.order,
        sequence: state.sequence,
      });
      state.sequence += 1;
    }
  }

  for (const [key, value] of Object.entries(object)) {
    if (isRoot) {
      if (isReservedChildKey(key, options)) {
        throw new PmlError(0, PmlErrorKind.INVALID_TREE, `root object cannot contain meta field \`${key}\``);
      }
    } else if (isReservedChildKey(key, options)) {
      continue;
    }

    path.push(key);
    collectBlocksFromValue(value, path, collected, state, options);
    path.pop();
  }
}

function collectBlocksFromValue(value, path, collected, state, options) {
  if (isPlainObject(value)) {
    collectBlocksFromObject(value, false, path, collected, state, options);
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      if (isPlainObject(item)) {
        collectBlocksFromObject(item, false, path, collected, state, options);
      } else if (Array.isArray(item)) {
        collectBlocksFromValue(item, path, collected, state, options);
      } else {
        throw new PmlError(0, PmlErrorKind.INVALID_TREE, `array at \`${path.join(".")}\` must contain objects`);
      }
    }
    return;
  }
  throw new PmlError(0, PmlErrorKind.INVALID_TREE, `path \`${path.join(".")}\` must resolve to an object or array`);
}

function extractMeta(object, options) {
  const meta = {
    ty: null,
    content: null,
    tag: null,
    order: null,
    hasAny: false,
  };

  const typeKey = metaKey(options, META_TYPE);
  if (hasOwn(object, typeKey)) {
    meta.ty = expectTreeString(object[typeKey], typeKey);
    meta.hasAny = true;
  }

  const contentKey = metaKey(options, META_CONTENT);
  if (hasOwn(object, contentKey)) {
    meta.content = expectTreeString(object[contentKey], contentKey);
    meta.hasAny = true;
  }

  const tagKey = metaKey(options, META_TAG);
  if (hasOwn(object, tagKey)) {
    meta.tag = expectTreeString(object[tagKey], tagKey);
    meta.hasAny = true;
  }

  const orderKey = metaKey(options, META_ORDER);
  if (hasOwn(object, orderKey)) {
    const order = expectTreeInteger(object[orderKey], orderKey);
    if (order < 0) {
      throw new PmlError(0, PmlErrorKind.INVALID_TREE, `meta field \`${orderKey}\` must be a non-negative integer`);
    }
    meta.order = order;
    meta.hasAny = true;
  }

  return meta;
}

function expectTreeString(value, key) {
  if (typeof value === "string") {
    return value;
  }
  throw new PmlError(0, PmlErrorKind.INVALID_TREE, `meta field \`${key}\` must be a string`);
}

function expectTreeInteger(value, key) {
  if (typeof value === "number" && Number.isInteger(value)) {
    return value;
  }
  throw new PmlError(0, PmlErrorKind.INVALID_TREE, `meta field \`${key}\` must be an integer`);
}

function hasOwn(object, key) {
  return Object.prototype.hasOwnProperty.call(object, key);
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

module.exports = {
  PmlBlock,
  PmlError,
  PmlErrorKind,
  PmlBuilder,
  PmlTreeOptions,
  parsePml,
  parsePmlTree,
  blocksToTree,
  treeToBlocks,
  renderBlocks,
  renderPmlTree,
};
