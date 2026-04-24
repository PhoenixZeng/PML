use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

const META_TYPE: &str = "type";
const META_CONTENT: &str = "content";
const META_TAG: &str = "tag";
const META_ORDER: &str = "order";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmlBlock {
    pub name: String,
    pub ty: String,
    pub content: String,
    pub paired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmlTreeOptions {
    pub meta_prefix: String,
}

impl Default for PmlTreeOptions {
    fn default() -> Self {
        Self {
            meta_prefix: "$".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PmlTreeValue {
    Object(BTreeMap<String, PmlTreeValue>),
    Array(Vec<PmlTreeValue>),
    String(String),
    Integer(i64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmlError {
    pub line: usize,
    pub kind: PmlErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PmlErrorKind {
    ExpectedBlockHeader,
    InvalidControlLine(String),
    InvalidName(String),
    InvalidType(String),
    StrayClosingBlock(String),
    TypeMismatch { expected: String, found: String },
    MetaFieldConflict(String),
    InvalidTree(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Control {
    Open { name: String, ty: String },
    Close { name: String, ty: Option<String> },
}

#[derive(Debug, Clone, Copy)]
struct Line {
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct CollectedBlock {
    block: PmlBlock,
    order: Option<i64>,
    sequence: usize,
}

#[derive(Debug, Default)]
struct ExtractedMeta {
    ty: Option<String>,
    content: Option<String>,
    tag: Option<String>,
    order: Option<i64>,
    has_any: bool,
}

impl fmt::Display for PmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PmlErrorKind::ExpectedBlockHeader => {
                write!(f, "line {}: expected a PML block header", self.line)
            }
            PmlErrorKind::InvalidControlLine(line) => {
                write!(f, "line {}: invalid control line `{}`", self.line, line)
            }
            PmlErrorKind::InvalidName(name) => {
                write!(f, "line {}: invalid block name `{}`", self.line, name)
            }
            PmlErrorKind::InvalidType(ty) => {
                write!(f, "line {}: invalid block type `{}`", self.line, ty)
            }
            PmlErrorKind::StrayClosingBlock(name) => {
                write!(f, "line {}: stray closing block `/{}`", self.line, name)
            }
            PmlErrorKind::TypeMismatch { expected, found } => write!(
                f,
                "line {}: closing type `{}` does not match opening type `{}`",
                self.line, found, expected
            ),
            PmlErrorKind::MetaFieldConflict(name) => {
                write!(
                    f,
                    "line {}: tree meta field conflicts with child key `{}`",
                    self.line, name
                )
            }
            PmlErrorKind::InvalidTree(detail) => {
                write!(f, "line {}: invalid PML tree: {}", self.line, detail)
            }
        }
    }
}

impl Error for PmlError {}

pub fn parse_pml(input: &str) -> Result<Vec<PmlBlock>, PmlError> {
    let input = normalize_newlines(input);
    let lines = collect_lines(&input);
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = line_text(&input, lines[i]);
        let control = parse_control_line(line, i + 1)?;

        match control {
            None if line.trim().is_empty() => {
                i += 1;
            }
            None => {
                return Err(PmlError {
                    line: i + 1,
                    kind: PmlErrorKind::ExpectedBlockHeader,
                });
            }
            Some(Control::Close { name, .. }) => {
                return Err(PmlError {
                    line: i + 1,
                    kind: PmlErrorKind::StrayClosingBlock(name),
                });
            }
            Some(Control::Open { name, ty }) => {
                let close = find_matching_close(&input, &lines, i + 1, &name, &ty)?;

                if let Some(close_idx) = close {
                    blocks.push(PmlBlock {
                        name,
                        ty,
                        content: content_between(&input, &lines, i + 1, close_idx),
                        paired: true,
                    });
                    i = close_idx + 1;
                } else {
                    let end = find_next_opening(&input, &lines, i + 1);
                    blocks.push(PmlBlock {
                        name,
                        ty,
                        content: content_between(&input, &lines, i + 1, end),
                        paired: false,
                    });
                    i = end;
                }
            }
        }
    }

    Ok(blocks)
}

pub fn parse_pml_tree(input: &str, options: &PmlTreeOptions) -> Result<PmlTreeValue, PmlError> {
    let blocks = parse_pml(input)?;
    blocks_to_tree(&blocks, options)
}

pub fn blocks_to_tree(
    blocks: &[PmlBlock],
    options: &PmlTreeOptions,
) -> Result<PmlTreeValue, PmlError> {
    let mut root = PmlTreeValue::Object(BTreeMap::new());

    for (order, block) in blocks.iter().enumerate() {
        insert_block_into_tree(&mut root, block, order as i64, options)?;
    }

    Ok(root)
}

pub fn tree_to_blocks(
    tree: &PmlTreeValue,
    options: &PmlTreeOptions,
) -> Result<Vec<PmlBlock>, PmlError> {
    let root = match tree {
        PmlTreeValue::Object(root) => root,
        _ => {
            return Err(PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidTree("root must be an object".to_string()),
            });
        }
    };

    let mut collected = Vec::new();
    let mut path = Vec::new();
    let mut sequence = 0;
    collect_blocks_from_object(
        root,
        true,
        &mut path,
        &mut collected,
        &mut sequence,
        options,
    )?;

    collected.sort_by(|left, right| match (left.order, right.order) {
        (Some(a), Some(b)) => a.cmp(&b).then(left.sequence.cmp(&right.sequence)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.sequence.cmp(&right.sequence),
    });

    Ok(collected.into_iter().map(|item| item.block).collect())
}

pub fn render_pml_tree(tree: &PmlTreeValue, options: &PmlTreeOptions) -> Result<String, PmlError> {
    let blocks = tree_to_blocks(tree, options)?;
    Ok(render_blocks(&blocks))
}

pub struct PmlBuilder {
    blocks: Vec<PmlBlock>,
}

impl PmlBuilder {
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    pub fn push_short(
        &mut self,
        name: impl Into<String>,
        ty: Option<&str>,
        content: impl Into<String>,
    ) -> Result<&mut Self, PmlError> {
        self.push_block(name, ty, content, false)
    }

    pub fn push_paired(
        &mut self,
        name: impl Into<String>,
        ty: Option<&str>,
        content: impl Into<String>,
    ) -> Result<&mut Self, PmlError> {
        self.push_block(name, ty, content, true)
    }

    pub fn build(&self) -> String {
        render_blocks(&self.blocks)
    }

    fn push_block(
        &mut self,
        name: impl Into<String>,
        ty: Option<&str>,
        content: impl Into<String>,
        paired: bool,
    ) -> Result<&mut Self, PmlError> {
        let name = name.into();
        if !is_valid_name(&name) {
            return Err(PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidName(name),
            });
        }

        let ty = normalize_type(ty.unwrap_or("text")).map_err(|kind| PmlError { line: 0, kind })?;
        self.blocks.push(PmlBlock {
            name,
            ty,
            content: normalize_newlines(&content.into()),
            paired,
        });
        Ok(self)
    }
}

impl Default for PmlBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_blocks(blocks: &[PmlBlock]) -> String {
    let mut out = String::new();

    for block in blocks {
        out.push('[');
        out.push_str(&block.name);
        if block.ty != "text" {
            out.push(':');
            out.push_str(&block.ty);
        }
        out.push_str("]\n");
        out.push_str(&block.content);

        if !block.content.is_empty() {
            out.push('\n');
        }

        if should_render_as_paired(block) {
            out.push_str("[/");
            out.push_str(&block.name);
            out.push_str("]\n");
        }
    }

    out
}

fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn collect_lines(input: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0;

    for (idx, ch) in input.char_indices() {
        if ch == '\n' {
            lines.push(Line { start, end: idx });
            start = idx + 1;
        }
    }

    if start < input.len() {
        lines.push(Line {
            start,
            end: input.len(),
        });
    }

    lines
}

fn line_text(input: &str, line: Line) -> &str {
    &input[line.start..line.end]
}

fn content_between(input: &str, lines: &[Line], start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }

    input[lines[start].start..lines[end - 1].end].to_string()
}

fn find_matching_close(
    input: &str,
    lines: &[Line],
    start: usize,
    name: &str,
    ty: &str,
) -> Result<Option<usize>, PmlError> {
    for idx in start..lines.len() {
        let line = line_text(input, lines[idx]);
        if let Ok(Some(Control::Close {
            name: close_name,
            ty: close_ty,
        })) = parse_control_line(line, idx + 1)
        {
            if close_name == name {
                if let Some(close_ty) = close_ty {
                    if close_ty != ty {
                        return Err(PmlError {
                            line: idx + 1,
                            kind: PmlErrorKind::TypeMismatch {
                                expected: ty.to_string(),
                                found: close_ty,
                            },
                        });
                    }
                }
                return Ok(Some(idx));
            }
        }
    }

    Ok(None)
}

fn find_next_opening(input: &str, lines: &[Line], start: usize) -> usize {
    for idx in start..lines.len() {
        let line = line_text(input, lines[idx]);
        if matches!(
            parse_control_line(line, idx + 1),
            Ok(Some(Control::Open { .. }))
        ) {
            return idx;
        }
    }

    lines.len()
}

fn parse_control_line(line: &str, line_no: usize) -> Result<Option<Control>, PmlError> {
    if !line.starts_with('[') && !line.ends_with(']') {
        return Ok(None);
    }

    if !(line.starts_with('[') && line.ends_with(']')) {
        return Err(PmlError {
            line: line_no,
            kind: PmlErrorKind::InvalidControlLine(line.to_string()),
        });
    }

    let inner = &line[1..line.len() - 1];
    if inner.is_empty() {
        return Err(PmlError {
            line: line_no,
            kind: PmlErrorKind::InvalidControlLine(line.to_string()),
        });
    }

    if let Some(rest) = inner.strip_prefix('/') {
        let (name, ty) = parse_name_and_optional_type(rest, line_no)?;
        Ok(Some(Control::Close { name, ty }))
    } else {
        let (name, ty) = parse_name_and_required_type(inner, line_no)?;
        Ok(Some(Control::Open { name, ty }))
    }
}

fn should_render_as_paired(block: &PmlBlock) -> bool {
    block.paired || content_has_control_line(&block.content)
}

fn content_has_control_line(content: &str) -> bool {
    if content.is_empty() {
        return false;
    }

    for line in normalize_newlines(content).split('\n') {
        match parse_control_line(line, 0) {
            Ok(Some(_)) => return true,
            Ok(None) | Err(_) => {}
        }
    }

    false
}

fn parse_name_and_required_type(input: &str, line_no: usize) -> Result<(String, String), PmlError> {
    let (name, ty) = match input.split_once(':') {
        Some((name, ty)) => (name, ty),
        None => (input, "text"),
    };

    validate_name_or_err(name, line_no)?;
    let ty = normalize_type(ty).map_err(|kind| PmlError {
        line: line_no,
        kind,
    })?;

    Ok((name.to_string(), ty))
}

fn parse_name_and_optional_type(
    input: &str,
    line_no: usize,
) -> Result<(String, Option<String>), PmlError> {
    let (name, ty) = match input.split_once(':') {
        Some((name, ty)) => (name, Some(ty)),
        None => (input, None),
    };

    validate_name_or_err(name, line_no)?;
    let ty = match ty {
        Some(ty) => Some(normalize_type(ty).map_err(|kind| PmlError {
            line: line_no,
            kind,
        })?),
        None => None,
    };

    Ok((name.to_string(), ty))
}

fn validate_name_or_err(name: &str, line_no: usize) -> Result<(), PmlError> {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(PmlError {
            line: line_no,
            kind: PmlErrorKind::InvalidName(name.to_string()),
        })
    }
}

fn normalize_type(ty: &str) -> Result<String, PmlErrorKind> {
    if ty.is_empty() || !ty.chars().all(is_type_char) {
        return Err(PmlErrorKind::InvalidType(ty.to_string()));
    }

    let ty = ty.to_ascii_lowercase();
    if ty == "md" {
        Ok("markdown".to_string())
    } else {
        Ok(ty)
    }
}

fn is_valid_name(name: &str) -> bool {
    let mut parts = name.split('#');
    let main = match parts.next() {
        Some(main) if !main.is_empty() => main,
        _ => return false,
    };
    let tag = parts.next();

    if parts.next().is_some() || !is_valid_main_name(main) {
        return false;
    }

    match tag {
        Some(tag) => is_valid_tag(tag),
        None => true,
    }
}

fn is_valid_main_name(main: &str) -> bool {
    main.split('.')
        .all(|segment| !segment.is_empty() && segment.chars().all(is_name_char))
}

fn is_valid_tag(tag: &str) -> bool {
    !tag.is_empty() && tag.chars().all(is_name_char)
}

fn is_name_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn is_type_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

fn insert_block_into_tree(
    root: &mut PmlTreeValue,
    block: &PmlBlock,
    order: i64,
    options: &PmlTreeOptions,
) -> Result<(), PmlError> {
    let (main_name, tag) = split_block_name(&block.name);
    let segments: Vec<&str> = main_name.split('.').collect();
    let deepest = deepest_unique_prefix(root, &segments);

    if deepest == segments.len() {
        let existing = get_unique_object_mut(root, &segments)?;
        if !object_has_meta(existing, options) {
            apply_block_meta(existing, block, order, tag, options)?;
            return Ok(());
        }
    }

    let prefix_len = if deepest == segments.len() {
        segments.len().saturating_sub(1)
    } else {
        deepest
    };
    let remaining = &segments[prefix_len..];
    let parent = get_unique_object_mut(root, &segments[..prefix_len])?;
    attach_remaining_path(
        parent,
        prefix_len == 0,
        remaining,
        block,
        order,
        tag,
        options,
    )
}

fn split_block_name(name: &str) -> (&str, Option<&str>) {
    match name.split_once('#') {
        Some((main, tag)) => (main, Some(tag)),
        None => (name, None),
    }
}

fn deepest_unique_prefix(root: &PmlTreeValue, segments: &[&str]) -> usize {
    let mut deepest = 0;
    let mut current = match root {
        PmlTreeValue::Object(object) => vec![object],
        _ => return 0,
    };

    for (index, segment) in segments.iter().enumerate() {
        let mut next = Vec::new();

        for object in &current {
            if let Some(value) = object.get(*segment) {
                collect_child_objects(value, &mut next);
            }
        }

        if next.len() == 1 {
            deepest = index + 1;
            current = next;
        } else {
            break;
        }
    }

    deepest
}

fn collect_child_objects<'a>(
    value: &'a PmlTreeValue,
    out: &mut Vec<&'a BTreeMap<String, PmlTreeValue>>,
) {
    match value {
        PmlTreeValue::Object(object) => out.push(object),
        PmlTreeValue::Array(items) => {
            for item in items {
                if let PmlTreeValue::Object(object) = item {
                    out.push(object);
                }
            }
        }
        _ => {}
    }
}

fn get_unique_object_mut<'a>(
    value: &'a mut PmlTreeValue,
    segments: &[&str],
) -> Result<&'a mut BTreeMap<String, PmlTreeValue>, PmlError> {
    match value {
        PmlTreeValue::Object(object) => get_unique_object_from_object_mut(object, segments),
        _ => Err(PmlError {
            line: 0,
            kind: PmlErrorKind::InvalidTree("root must be an object".to_string()),
        }),
    }
}

fn get_unique_object_from_object_mut<'a>(
    object: &'a mut BTreeMap<String, PmlTreeValue>,
    segments: &[&str],
) -> Result<&'a mut BTreeMap<String, PmlTreeValue>, PmlError> {
    if segments.is_empty() {
        return Ok(object);
    }

    let key = segments[0];
    let child = object.get_mut(key).ok_or_else(|| PmlError {
        line: 0,
        kind: PmlErrorKind::InvalidTree(format!("missing object path `{}`", key)),
    })?;

    get_unique_object_from_value_mut(child, segments, 1)
}

fn get_unique_object_from_value_mut<'a>(
    value: &'a mut PmlTreeValue,
    full_segments: &[&str],
    index: usize,
) -> Result<&'a mut BTreeMap<String, PmlTreeValue>, PmlError> {
    match value {
        PmlTreeValue::Object(object) => {
            if index >= full_segments.len() {
                Ok(object)
            } else {
                let child = object
                    .get_mut(full_segments[index])
                    .ok_or_else(|| PmlError {
                        line: 0,
                        kind: PmlErrorKind::InvalidTree(format!(
                            "missing object path `{}`",
                            full_segments[..=index].join(".")
                        )),
                    })?;
                get_unique_object_from_value_mut(child, full_segments, index + 1)
            }
        }
        PmlTreeValue::Array(items) => {
            let mut object_index = None;

            for (idx, item) in items.iter().enumerate() {
                if matches!(item, PmlTreeValue::Object(_)) {
                    if object_index.is_some() {
                        return Err(PmlError {
                            line: 0,
                            kind: PmlErrorKind::InvalidTree(format!(
                                "path `{}` is not unique",
                                full_segments[..index].join(".")
                            )),
                        });
                    }
                    object_index = Some(idx);
                }
            }

            let idx = object_index.ok_or_else(|| PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidTree(format!(
                    "path `{}` does not resolve to an object",
                    full_segments[..index].join(".")
                )),
            })?;

            get_unique_object_from_value_mut(&mut items[idx], full_segments, index)
        }
        _ => Err(PmlError {
            line: 0,
            kind: PmlErrorKind::InvalidTree(format!(
                "path `{}` does not resolve to an object",
                full_segments[..index].join(".")
            )),
        }),
    }
}

fn attach_remaining_path(
    parent: &mut BTreeMap<String, PmlTreeValue>,
    parent_is_root: bool,
    remaining: &[&str],
    block: &PmlBlock,
    order: i64,
    tag: Option<&str>,
    options: &PmlTreeOptions,
) -> Result<(), PmlError> {
    if remaining.is_empty() {
        return Err(PmlError {
            line: 0,
            kind: PmlErrorKind::InvalidTree("empty path cannot be attached".to_string()),
        });
    }

    let key = remaining[0];
    ensure_child_key_allowed(parent_is_root, key, options)?;
    let leaf = build_leaf_object(block, order, tag, options)?;

    if remaining.len() == 1 {
        let should_promote = match parent.get(key) {
            Some(PmlTreeValue::Object(object)) => object_has_meta(object, options),
            Some(PmlTreeValue::Array(_)) => false,
            Some(_) => {
                return Err(PmlError {
                    line: 0,
                    kind: PmlErrorKind::InvalidTree(format!(
                        "path `{}` does not resolve to an object",
                        key
                    )),
                })
            }
            None => false,
        };

        if should_promote {
            let existing = parent.remove(key).ok_or_else(|| PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidTree(format!("missing key `{}`", key)),
            })?;
            parent.insert(
                key.to_string(),
                PmlTreeValue::Array(vec![existing, PmlTreeValue::Object(leaf)]),
            );
            return Ok(());
        }

        match parent.get_mut(key) {
            Some(PmlTreeValue::Object(object)) => {
                apply_block_meta(object, block, order, tag, options)
            }
            Some(PmlTreeValue::Array(items)) => {
                items.push(PmlTreeValue::Object(leaf));
                Ok(())
            }
            Some(_) => Err(PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidTree(format!(
                    "path `{}` does not resolve to an object",
                    key
                )),
            }),
            None => {
                parent.insert(key.to_string(), PmlTreeValue::Object(leaf));
                Ok(())
            }
        }
    } else {
        let branch = build_chain_value(&remaining[1..], leaf, options)?;

        match parent.get_mut(key) {
            Some(PmlTreeValue::Object(object)) => {
                attach_remaining_path(object, false, &remaining[1..], block, order, tag, options)
            }
            Some(PmlTreeValue::Array(items)) => {
                items.push(branch);
                Ok(())
            }
            Some(_) => Err(PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidTree(format!(
                    "path `{}` does not resolve to an object",
                    key
                )),
            }),
            None => {
                parent.insert(key.to_string(), branch);
                Ok(())
            }
        }
    }
}

fn build_leaf_object(
    block: &PmlBlock,
    order: i64,
    tag: Option<&str>,
    options: &PmlTreeOptions,
) -> Result<BTreeMap<String, PmlTreeValue>, PmlError> {
    let mut object = BTreeMap::new();
    apply_block_meta(&mut object, block, order, tag, options)?;
    Ok(object)
}

fn build_chain_value(
    tail: &[&str],
    leaf: BTreeMap<String, PmlTreeValue>,
    options: &PmlTreeOptions,
) -> Result<PmlTreeValue, PmlError> {
    let mut current = PmlTreeValue::Object(leaf);

    for segment in tail.iter().rev() {
        ensure_child_key_allowed(false, segment, options)?;
        let mut object = BTreeMap::new();
        object.insert((*segment).to_string(), current);
        current = PmlTreeValue::Object(object);
    }

    Ok(current)
}

fn apply_block_meta(
    object: &mut BTreeMap<String, PmlTreeValue>,
    block: &PmlBlock,
    order: i64,
    tag: Option<&str>,
    options: &PmlTreeOptions,
) -> Result<(), PmlError> {
    insert_meta_field(
        object,
        &meta_key(options, META_TYPE),
        PmlTreeValue::String(block.ty.clone()),
        options,
    )?;
    insert_meta_field(
        object,
        &meta_key(options, META_CONTENT),
        PmlTreeValue::String(block.content.clone()),
        options,
    )?;
    insert_meta_field(
        object,
        &meta_key(options, META_ORDER),
        PmlTreeValue::Integer(order),
        options,
    )?;

    if let Some(tag) = tag {
        insert_meta_field(
            object,
            &meta_key(options, META_TAG),
            PmlTreeValue::String(tag.to_string()),
            options,
        )?;
    }

    Ok(())
}

fn insert_meta_field(
    object: &mut BTreeMap<String, PmlTreeValue>,
    key: &str,
    value: PmlTreeValue,
    options: &PmlTreeOptions,
) -> Result<(), PmlError> {
    if let Some(existing) = object.get(key) {
        if existing != &value {
            return Err(PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidTree(format!("duplicate meta field `{}`", key)),
            });
        }
        return Ok(());
    }

    if is_meta_child_conflict(object, key, options) {
        return Err(PmlError {
            line: 0,
            kind: PmlErrorKind::MetaFieldConflict(key.to_string()),
        });
    }

    object.insert(key.to_string(), value);
    Ok(())
}

fn object_has_meta(object: &BTreeMap<String, PmlTreeValue>, options: &PmlTreeOptions) -> bool {
    object.contains_key(&meta_key(options, META_TYPE))
        || object.contains_key(&meta_key(options, META_CONTENT))
        || object.contains_key(&meta_key(options, META_TAG))
        || object.contains_key(&meta_key(options, META_ORDER))
}

fn meta_key(options: &PmlTreeOptions, name: &str) -> String {
    format!("{}{}", options.meta_prefix, name)
}

fn ensure_child_key_allowed(
    parent_is_root: bool,
    key: &str,
    options: &PmlTreeOptions,
) -> Result<(), PmlError> {
    if !parent_is_root && is_reserved_child_key(key, options) {
        return Err(PmlError {
            line: 0,
            kind: PmlErrorKind::MetaFieldConflict(key.to_string()),
        });
    }

    Ok(())
}

fn is_reserved_child_key(key: &str, options: &PmlTreeOptions) -> bool {
    key == meta_key(options, META_TYPE)
        || key == meta_key(options, META_CONTENT)
        || key == meta_key(options, META_TAG)
        || key == meta_key(options, META_ORDER)
}

fn is_meta_child_conflict(
    object: &BTreeMap<String, PmlTreeValue>,
    key: &str,
    options: &PmlTreeOptions,
) -> bool {
    options.meta_prefix.is_empty()
        && object.contains_key(key)
        && is_reserved_child_key(key, options)
}

fn collect_blocks_from_object(
    object: &BTreeMap<String, PmlTreeValue>,
    is_root: bool,
    path: &mut Vec<String>,
    output: &mut Vec<CollectedBlock>,
    sequence: &mut usize,
    options: &PmlTreeOptions,
) -> Result<(), PmlError> {
    if !is_root {
        let meta = extract_meta(object, options)?;
        if meta.has_any {
            let base_name = path.join(".");
            let name = match meta.tag {
                Some(tag) => format!("{}#{}", base_name, tag),
                None => base_name,
            };
            validate_name_or_err(&name, 0)?;

            let ty = normalize_type(meta.ty.as_deref().unwrap_or("text"))
                .map_err(|kind| PmlError { line: 0, kind })?;
            let content = normalize_newlines(meta.content.as_deref().unwrap_or(""));
            output.push(CollectedBlock {
                block: PmlBlock {
                    name,
                    ty,
                    content,
                    paired: false,
                },
                order: meta.order,
                sequence: *sequence,
            });
            *sequence += 1;
        }
    }

    for (key, value) in object {
        if is_root {
            if is_reserved_child_key(key, options) {
                return Err(PmlError {
                    line: 0,
                    kind: PmlErrorKind::InvalidTree(format!(
                        "root object cannot contain meta field `{}`",
                        key
                    )),
                });
            }
        } else if is_reserved_child_key(key, options) {
            continue;
        }

        path.push(key.clone());
        collect_blocks_from_value(value, path, output, sequence, options)?;
        path.pop();
    }

    Ok(())
}

fn collect_blocks_from_value(
    value: &PmlTreeValue,
    path: &mut Vec<String>,
    output: &mut Vec<CollectedBlock>,
    sequence: &mut usize,
    options: &PmlTreeOptions,
) -> Result<(), PmlError> {
    match value {
        PmlTreeValue::Object(object) => {
            collect_blocks_from_object(object, false, path, output, sequence, options)
        }
        PmlTreeValue::Array(items) => {
            for item in items {
                match item {
                    PmlTreeValue::Object(object) => {
                        collect_blocks_from_object(object, false, path, output, sequence, options)?
                    }
                    PmlTreeValue::Array(_) => {
                        collect_blocks_from_value(item, path, output, sequence, options)?
                    }
                    _ => {
                        return Err(PmlError {
                            line: 0,
                            kind: PmlErrorKind::InvalidTree(format!(
                                "array at `{}` must contain objects",
                                path.join(".")
                            )),
                        })
                    }
                }
            }
            Ok(())
        }
        _ => Err(PmlError {
            line: 0,
            kind: PmlErrorKind::InvalidTree(format!(
                "path `{}` must resolve to an object or array",
                path.join(".")
            )),
        }),
    }
}

fn extract_meta(
    object: &BTreeMap<String, PmlTreeValue>,
    options: &PmlTreeOptions,
) -> Result<ExtractedMeta, PmlError> {
    let mut meta = ExtractedMeta::default();

    if let Some(value) = object.get(&meta_key(options, META_TYPE)) {
        meta.ty = Some(expect_tree_string(value, &meta_key(options, META_TYPE))?);
        meta.has_any = true;
    }
    if let Some(value) = object.get(&meta_key(options, META_CONTENT)) {
        meta.content = Some(expect_tree_string(value, &meta_key(options, META_CONTENT))?);
        meta.has_any = true;
    }
    if let Some(value) = object.get(&meta_key(options, META_TAG)) {
        meta.tag = Some(expect_tree_string(value, &meta_key(options, META_TAG))?);
        meta.has_any = true;
    }
    if let Some(value) = object.get(&meta_key(options, META_ORDER)) {
        let order = expect_tree_integer(value, &meta_key(options, META_ORDER))?;
        if order < 0 {
            return Err(PmlError {
                line: 0,
                kind: PmlErrorKind::InvalidTree(format!(
                    "meta field `{}` must be a non-negative integer",
                    meta_key(options, META_ORDER)
                )),
            });
        }
        meta.order = Some(order);
        meta.has_any = true;
    }

    Ok(meta)
}

fn expect_tree_string(value: &PmlTreeValue, key: &str) -> Result<String, PmlError> {
    match value {
        PmlTreeValue::String(text) => Ok(text.clone()),
        _ => Err(PmlError {
            line: 0,
            kind: PmlErrorKind::InvalidTree(format!("meta field `{}` must be a string", key)),
        }),
    }
}

fn expect_tree_integer(value: &PmlTreeValue, key: &str) -> Result<i64, PmlError> {
    match value {
        PmlTreeValue::Integer(number) => Ok(*number),
        _ => Err(PmlError {
            line: 0,
            kind: PmlErrorKind::InvalidTree(format!("meta field `{}` must be an integer", key)),
        }),
    }
}
