import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

public final class Pml {
    private static final String META_TYPE = "type";
    private static final String META_CONTENT = "content";
    private static final String META_TAG = "tag";
    private static final String META_ORDER = "order";

    public static final class PmlBlock {
        public final String name;
        public final String ty;
        public final String content;
        public final boolean paired;

        public PmlBlock(String name, String ty, String content, boolean paired) {
            this.name = name;
            this.ty = ty;
            this.content = content;
            this.paired = paired;
        }
    }

    public static final class PmlTreeOptions {
        public final String metaPrefix;

        public PmlTreeOptions() {
            this("$");
        }

        public PmlTreeOptions(String metaPrefix) {
            this.metaPrefix = metaPrefix;
        }
    }

    public enum PmlErrorKind {
        EXPECTED_BLOCK_HEADER,
        INVALID_CONTROL_LINE,
        INVALID_NAME,
        INVALID_TYPE,
        STRAY_CLOSING_BLOCK,
        META_FIELD_CONFLICT,
        INVALID_TREE
    }

    public static final class PmlError extends RuntimeException {
        public final int line;
        public final PmlErrorKind kind;
        public final String detail;
        public final String expected;
        public final String found;

        public PmlError(int line, PmlErrorKind kind) {
            this(line, kind, "", "", "");
        }

        public PmlError(int line, PmlErrorKind kind, String detail) {
            this(line, kind, detail, "", "");
        }

        public PmlError(int line, PmlErrorKind kind, String detail, String expected, String found) {
            super(renderMessage(line, kind, detail, expected, found));
            this.line = line;
            this.kind = kind;
            this.detail = detail;
            this.expected = expected;
            this.found = found;
        }

        private static String renderMessage(int line, PmlErrorKind kind, String detail, String expected, String found) {
            return switch (kind) {
                case EXPECTED_BLOCK_HEADER -> "line " + line + ": expected a PML block header";
                case INVALID_CONTROL_LINE -> "line " + line + ": invalid control line `" + detail + "`";
                case INVALID_NAME -> "line " + line + ": invalid block name `" + detail + "`";
                case INVALID_TYPE -> "line " + line + ": invalid block type `" + detail + "`";
                case STRAY_CLOSING_BLOCK -> "line " + line + ": stray closing block `/" + detail + "`";
                case META_FIELD_CONFLICT -> "line " + line + ": tree meta field conflicts with child key `" + detail + "`";
                case INVALID_TREE -> "line " + line + ": invalid PML tree: " + detail;
            };
        }
    }

    public static final class PmlBuilder {
        private final List<PmlBlock> blocks = new ArrayList<>();

        public PmlBuilder pushShort(String name, String ty, String content) {
            return pushBlock(name, ty, content, false);
        }

        public PmlBuilder pushPaired(String name, String ty, String content) {
            return pushBlock(name, ty, content, true);
        }

        public String build() {
            return renderBlocks(blocks);
        }

        private PmlBuilder pushBlock(String name, String ty, String content, boolean paired) {
            if (!isValidName(name)) {
                throw new PmlError(0, PmlErrorKind.INVALID_NAME, name);
            }
            String normalizedType = normalizeType(ty == null ? "" : ty, 0);
            blocks.add(new PmlBlock(name, normalizedType, normalizeNewlines(content), paired));
            return this;
        }
    }

    private record Line(int start, int end) {}

    private record Control(String kind, String name, String ty) {}

    private record NameAndType(String name, String ty) {}

    private record CollectedBlock(PmlBlock block, Integer order, int sequence) {}

    private record ExtractedMeta(String ty, String content, String tag, Integer order, boolean hasAny) {}

    private Pml() {}

    public static List<PmlBlock> parsePml(String input) {
        String text = normalizeNewlines(input);
        List<Line> lines = collectLines(text);
        List<PmlBlock> blocks = new ArrayList<>();
        int i = 0;

        while (i < lines.size()) {
            String line = lineText(text, lines.get(i));
            Control control = parseControlLine(line, i + 1);

            if (control == null && line.trim().isEmpty()) {
                i += 1;
                continue;
            }
            if (control == null) {
                throw new PmlError(i + 1, PmlErrorKind.EXPECTED_BLOCK_HEADER);
            }
            if (Objects.equals(control.kind(), "close")) {
                throw new PmlError(i + 1, PmlErrorKind.STRAY_CLOSING_BLOCK, control.name());
            }

            Integer closeIndex = findMatchingClose(text, lines, i + 1, control.name());
            if (closeIndex != null) {
                blocks.add(new PmlBlock(
                    control.name(),
                    control.ty(),
                    contentBetween(text, lines, i + 1, closeIndex),
                    true
                ));
                i = closeIndex + 1;
            } else {
                int end = findNextOpening(text, lines, i + 1);
                blocks.add(new PmlBlock(
                    control.name(),
                    control.ty(),
                    contentBetween(text, lines, i + 1, end),
                    false
                ));
                i = end;
            }
        }

        return blocks;
    }

    public static Map<String, Object> parsePmlTree(String input, PmlTreeOptions options) {
        return blocksToTree(parsePml(input), options);
    }

    public static Map<String, Object> blocksToTree(List<PmlBlock> blocks, PmlTreeOptions options) {
        Map<String, Object> root = new LinkedHashMap<>();
        for (int i = 0; i < blocks.size(); i += 1) {
            insertBlockIntoTree(root, blocks.get(i), i, options);
        }
        return root;
    }

    public static List<PmlBlock> treeToBlocks(Map<String, Object> tree, PmlTreeOptions options) {
        List<CollectedBlock> collected = new ArrayList<>();
        List<String> path = new ArrayList<>();
        int[] sequence = new int[] {0};
        collectBlocksFromObject(tree, true, path, collected, sequence, options);

        collected.sort(new Comparator<>() {
            @Override
            public int compare(CollectedBlock left, CollectedBlock right) {
                if (left.order() != null && right.order() != null) {
                    int byOrder = Integer.compare(left.order(), right.order());
                    return byOrder != 0 ? byOrder : Integer.compare(left.sequence(), right.sequence());
                }
                if (left.order() != null) {
                    return -1;
                }
                if (right.order() != null) {
                    return 1;
                }
                return Integer.compare(left.sequence(), right.sequence());
            }
        });

        List<PmlBlock> blocks = new ArrayList<>();
        for (CollectedBlock item : collected) {
            blocks.add(item.block());
        }
        return blocks;
    }

    public static String renderPmlTree(Map<String, Object> tree, PmlTreeOptions options) {
        return renderBlocks(treeToBlocks(tree, options));
    }

    public static String renderBlocks(List<PmlBlock> blocks) {
        StringBuilder out = new StringBuilder();
        for (PmlBlock block : blocks) {
            out.append('[').append(block.name);
            if (!block.ty.isEmpty()) {
                out.append(':').append(block.ty);
            }
            out.append("]\n");
            out.append(block.content);
            if (!block.content.isEmpty()) {
                out.append('\n');
            }
            if (shouldRenderAsPaired(block)) {
                out.append("[/").append(block.name).append("]\n");
            }
        }
        return out.toString();
    }

    private static String normalizeNewlines(String input) {
        return input.replace("\r\n", "\n").replace('\r', '\n');
    }

    private static List<Line> collectLines(String text) {
        List<Line> lines = new ArrayList<>();
        int start = 0;
        for (int i = 0; i < text.length(); i += 1) {
            if (text.charAt(i) == '\n') {
                lines.add(new Line(start, i));
                start = i + 1;
            }
        }
        if (start < text.length()) {
            lines.add(new Line(start, text.length()));
        }
        return lines;
    }

    private static String lineText(String text, Line line) {
        return text.substring(line.start(), line.end());
    }

    private static String contentBetween(String text, List<Line> lines, int start, int end) {
        if (start >= end) {
            return "";
        }
        return text.substring(lines.get(start).start(), lines.get(end - 1).end());
    }

    private static Integer findMatchingClose(String text, List<Line> lines, int start, String name) {
        for (int idx = start; idx < lines.size(); idx += 1) {
            Control control = parseControlLine(lineText(text, lines.get(idx)), idx + 1);
            if (control != null && Objects.equals(control.kind(), "close") && Objects.equals(control.name(), name)) {
                return idx;
            }
        }
        return null;
    }

    private static int findNextOpening(String text, List<Line> lines, int start) {
        for (int idx = start; idx < lines.size(); idx += 1) {
            Control control = parseControlLine(lineText(text, lines.get(idx)), idx + 1);
            if (control != null && Objects.equals(control.kind(), "open")) {
                return idx;
            }
        }
        return lines.size();
    }

    private static Control parseControlLine(String line, int lineNo) {
        if (!line.startsWith("[") && !line.endsWith("]")) {
            return null;
        }
        if (!(line.startsWith("[") && line.endsWith("]"))) {
            throw new PmlError(lineNo, PmlErrorKind.INVALID_CONTROL_LINE, line);
        }

        String inner = line.substring(1, line.length() - 1);
        if (inner.isEmpty()) {
            throw new PmlError(lineNo, PmlErrorKind.INVALID_CONTROL_LINE, line);
        }

        if (inner.startsWith("/")) {
            String name = parseClosingName(inner.substring(1), lineNo, line);
            return new Control("close", name, null);
        }

        NameAndType parsed = parseNameAndRequiredType(inner, lineNo);
        return new Control("open", parsed.name(), parsed.ty());
    }

    private static boolean shouldRenderAsPaired(PmlBlock block) {
        return block.paired || contentHasControlLine(block.content);
    }

    private static boolean contentHasControlLine(String content) {
        if (content.isEmpty()) {
            return false;
        }
        String normalized = normalizeNewlines(content);
        for (String line : normalized.split("\n", -1)) {
            try {
                if (parseControlLine(line, 0) != null) {
                    return true;
                }
            } catch (PmlError ignored) {
                // ignore
            }
        }
        return false;
    }

    private static NameAndType parseNameAndRequiredType(String input, int lineNo) {
        int split = input.indexOf(':');
        String name = split >= 0 ? input.substring(0, split) : input;
        String ty = split >= 0 ? input.substring(split + 1) : "";
        validateNameOrThrow(name, lineNo);
        return new NameAndType(name, normalizeType(ty, lineNo));
    }

    private static String parseClosingName(String input, int lineNo, String line) {
        if (input.indexOf(':') >= 0) {
            throw new PmlError(lineNo, PmlErrorKind.INVALID_CONTROL_LINE, line);
        }
        validateNameOrThrow(input, lineNo);
        return input;
    }

    private static void validateNameOrThrow(String name, int lineNo) {
        if (!isValidName(name)) {
            throw new PmlError(lineNo, PmlErrorKind.INVALID_NAME, name);
        }
    }

    private static String normalizeType(String ty, int lineNo) {
        if (ty.isEmpty()) {
            return "";
        }
        for (int i = 0; i < ty.length(); i += 1) {
            if (!isTypeChar(ty.charAt(i))) {
                throw new PmlError(lineNo, PmlErrorKind.INVALID_TYPE, ty);
            }
        }
        String normalized = ty.toLowerCase(Locale.ROOT);
        return normalized.equals("md") ? "markdown" : normalized;
    }

    private static boolean isValidName(String name) {
        String[] parts = name.split("#", -1);
        if (parts.length > 2 || parts[0].isEmpty()) {
            return false;
        }
        return isValidMainName(parts[0]) && (parts.length == 1 || isValidTag(parts[1]));
    }

    private static boolean isValidMainName(String main) {
        String[] segments = main.split("\\.", -1);
        for (String segment : segments) {
            if (segment.isEmpty()) {
                return false;
            }
            for (int offset = 0; offset < segment.length();) {
                int codePoint = segment.codePointAt(offset);
                if (!isNameChar(codePoint)) {
                    return false;
                }
                offset += Character.charCount(codePoint);
            }
        }
        return true;
    }

    private static boolean isValidTag(String tag) {
        if (tag.isEmpty()) {
            return false;
        }
        for (int offset = 0; offset < tag.length();) {
            int codePoint = tag.codePointAt(offset);
            if (!isNameChar(codePoint)) {
                return false;
            }
            offset += Character.charCount(codePoint);
        }
        return true;
    }

    private static boolean isNameChar(int codePoint) {
        return codePoint == '_' || Character.isLetterOrDigit(codePoint);
    }

    private static boolean isTypeChar(char ch) {
        return (ch >= 'A' && ch <= 'Z')
            || (ch >= 'a' && ch <= 'z')
            || (ch >= '0' && ch <= '9')
            || ch == '_'
            || ch == '-';
    }

    private static void insertBlockIntoTree(Map<String, Object> root, PmlBlock block, int order, PmlTreeOptions options) {
        String[] split = splitBlockName(block.name);
        String[] segments = split[0].split("\\.");
        String tag = split[1];
        int deepest = deepestUniquePrefix(root, segments);

        if (deepest == segments.length) {
            Map<String, Object> existing = getUniqueObject(root, segments);
            if (!objectHasMeta(existing, options)) {
                applyBlockMeta(existing, block, order, tag, options);
                return;
            }
        }

        int prefixLength = deepest == segments.length ? Math.max(segments.length - 1, 0) : deepest;
        Map<String, Object> parent = getUniqueObject(root, slice(segments, 0, prefixLength));
        attachRemainingPath(parent, prefixLength == 0, slice(segments, prefixLength, segments.length), block, order, tag, options);
    }

    private static String[] splitBlockName(String name) {
        int split = name.indexOf('#');
        if (split >= 0) {
            return new String[] {name.substring(0, split), name.substring(split + 1)};
        }
        return new String[] {name, null};
    }

    private static int deepestUniquePrefix(Map<String, Object> root, String[] segments) {
        List<Map<String, Object>> current = new ArrayList<>();
        current.add(root);
        int deepest = 0;

        for (int index = 0; index < segments.length; index += 1) {
            String segment = segments[index];
            List<Map<String, Object>> next = new ArrayList<>();
            for (Map<String, Object> object : current) {
                if (object.containsKey(segment)) {
                    collectChildObjects(object.get(segment), next);
                }
            }
            if (next.size() == 1) {
                deepest = index + 1;
                current = next;
            } else {
                break;
            }
        }

        return deepest;
    }

    private static void collectChildObjects(Object value, List<Map<String, Object>> out) {
        if (value instanceof Map<?, ?> map) {
            out.add(castObject(map));
            return;
        }
        if (value instanceof List<?> list) {
            for (Object item : list) {
                if (item instanceof Map<?, ?> map) {
                    out.add(castObject(map));
                }
            }
        }
    }

    private static Map<String, Object> getUniqueObject(Map<String, Object> root, String[] segments) {
        Object current = root;
        int index = 0;

        while (true) {
            if (current instanceof Map<?, ?> map) {
                Map<String, Object> object = castObject(map);
                if (index == segments.length) {
                    return object;
                }
                String key = segments[index];
                if (!object.containsKey(key)) {
                    throw new PmlError(0, PmlErrorKind.INVALID_TREE, "missing object path `" + joinSegments(segments, index + 1) + "`");
                }
                current = object.get(key);
                index += 1;
                continue;
            }

            if (current instanceof List<?> list) {
                List<Map<String, Object>> objects = new ArrayList<>();
                for (Object item : list) {
                    if (item instanceof Map<?, ?> map) {
                        objects.add(castObject(map));
                    }
                }
                if (objects.size() != 1) {
                    throw new PmlError(0, PmlErrorKind.INVALID_TREE, "path `" + joinSegments(segments, index) + "` is not unique");
                }
                current = objects.get(0);
                continue;
            }

            throw new PmlError(0, PmlErrorKind.INVALID_TREE, "path `" + joinSegments(segments, index) + "` does not resolve to an object");
        }
    }

    private static void attachRemainingPath(
        Map<String, Object> parent,
        boolean parentIsRoot,
        String[] remaining,
        PmlBlock block,
        int order,
        String tag,
        PmlTreeOptions options
    ) {
        if (remaining.length == 0) {
            throw new PmlError(0, PmlErrorKind.INVALID_TREE, "empty path cannot be attached");
        }

        String key = remaining[0];
        ensureChildKeyAllowed(parentIsRoot, key, options);
        Map<String, Object> leaf = buildLeafObject(block, order, tag, options);

        if (remaining.length == 1) {
            Object existing = parent.get(key);
            if (existing instanceof Map<?, ?> map && objectHasMeta(castObject(map), options)) {
                List<Object> items = new ArrayList<>();
                items.add(castObject(map));
                items.add(leaf);
                parent.put(key, items);
                return;
            }
            if (existing instanceof Map<?, ?> map) {
                applyBlockMeta(castObject(map), block, order, tag, options);
                return;
            }
            if (existing instanceof List<?> list) {
                castList(list).add(leaf);
                return;
            }
            if (existing != null) {
                throw new PmlError(0, PmlErrorKind.INVALID_TREE, "path `" + key + "` does not resolve to an object");
            }
            parent.put(key, leaf);
            return;
        }

        Map<String, Object> branch = buildChainValue(slice(remaining, 1, remaining.length), leaf, options);
        Object existing = parent.get(key);
        if (existing instanceof Map<?, ?> map) {
            attachRemainingPath(castObject(map), false, slice(remaining, 1, remaining.length), block, order, tag, options);
            return;
        }
        if (existing instanceof List<?> list) {
            castList(list).add(branch);
            return;
        }
        if (existing != null) {
            throw new PmlError(0, PmlErrorKind.INVALID_TREE, "path `" + key + "` does not resolve to an object");
        }
        parent.put(key, branch);
    }

    private static Map<String, Object> buildLeafObject(PmlBlock block, int order, String tag, PmlTreeOptions options) {
        Map<String, Object> object = new LinkedHashMap<>();
        applyBlockMeta(object, block, order, tag, options);
        return object;
    }

    private static Map<String, Object> buildChainValue(String[] tail, Map<String, Object> leaf, PmlTreeOptions options) {
        Map<String, Object> current = leaf;
        for (int index = tail.length - 1; index >= 0; index -= 1) {
            String segment = tail[index];
            ensureChildKeyAllowed(false, segment, options);
            Map<String, Object> next = new LinkedHashMap<>();
            next.put(segment, current);
            current = next;
        }
        return current;
    }

    private static void applyBlockMeta(Map<String, Object> object, PmlBlock block, int order, String tag, PmlTreeOptions options) {
        insertMetaField(object, metaKey(options, META_TYPE), block.ty);
        insertMetaField(object, metaKey(options, META_CONTENT), block.content);
        insertMetaField(object, metaKey(options, META_ORDER), order);
        if (tag != null) {
            insertMetaField(object, metaKey(options, META_TAG), tag);
        }
    }

    private static void insertMetaField(Map<String, Object> object, String key, Object value) {
        if (object.containsKey(key)) {
            if (!Objects.equals(object.get(key), value)) {
                throw new PmlError(0, PmlErrorKind.INVALID_TREE, "duplicate meta field `" + key + "`");
            }
            return;
        }
        object.put(key, value);
    }

    private static boolean objectHasMeta(Map<String, Object> object, PmlTreeOptions options) {
        return object.containsKey(metaKey(options, META_TYPE))
            || object.containsKey(metaKey(options, META_CONTENT))
            || object.containsKey(metaKey(options, META_TAG))
            || object.containsKey(metaKey(options, META_ORDER));
    }

    private static String metaKey(PmlTreeOptions options, String name) {
        return options.metaPrefix + name;
    }

    private static void ensureChildKeyAllowed(boolean parentIsRoot, String key, PmlTreeOptions options) {
        if (!parentIsRoot && isReservedChildKey(key, options)) {
            throw new PmlError(0, PmlErrorKind.META_FIELD_CONFLICT, key);
        }
    }

    private static boolean isReservedChildKey(String key, PmlTreeOptions options) {
        return key.equals(metaKey(options, META_TYPE))
            || key.equals(metaKey(options, META_CONTENT))
            || key.equals(metaKey(options, META_TAG))
            || key.equals(metaKey(options, META_ORDER));
    }

    private static void collectBlocksFromObject(
        Map<String, Object> object,
        boolean isRoot,
        List<String> path,
        List<CollectedBlock> output,
        int[] sequence,
        PmlTreeOptions options
    ) {
        if (!isRoot) {
            ExtractedMeta meta = extractMeta(object, options);
            if (meta.hasAny()) {
                String baseName = String.join(".", path);
                String name = meta.tag() == null ? baseName : baseName + "#" + meta.tag();
                validateNameOrThrow(name, 0);
                String ty = normalizeType(meta.ty() == null ? "" : meta.ty(), 0);
                String content = normalizeNewlines(meta.content() == null ? "" : meta.content());
                output.add(new CollectedBlock(
                    new PmlBlock(name, ty, content, false),
                    meta.order(),
                    sequence[0]
                ));
                sequence[0] += 1;
            }
        }

        for (Map.Entry<String, Object> entry : object.entrySet()) {
            String key = entry.getKey();
            if (isRoot) {
                if (isReservedChildKey(key, options)) {
                    throw new PmlError(0, PmlErrorKind.INVALID_TREE, "root object cannot contain meta field `" + key + "`");
                }
            } else if (isReservedChildKey(key, options)) {
                continue;
            }

            path.add(key);
            collectBlocksFromValue(entry.getValue(), path, output, sequence, options);
            path.remove(path.size() - 1);
        }
    }

    private static void collectBlocksFromValue(
        Object value,
        List<String> path,
        List<CollectedBlock> output,
        int[] sequence,
        PmlTreeOptions options
    ) {
        if (value instanceof Map<?, ?> map) {
            collectBlocksFromObject(castObject(map), false, path, output, sequence, options);
            return;
        }
        if (value instanceof List<?> list) {
            for (Object item : list) {
                if (item instanceof Map<?, ?> map) {
                    collectBlocksFromObject(castObject(map), false, path, output, sequence, options);
                } else if (item instanceof List<?>) {
                    collectBlocksFromValue(item, path, output, sequence, options);
                } else {
                    throw new PmlError(0, PmlErrorKind.INVALID_TREE, "array at `" + String.join(".", path) + "` must contain objects");
                }
            }
            return;
        }
        throw new PmlError(0, PmlErrorKind.INVALID_TREE, "path `" + String.join(".", path) + "` must resolve to an object or array");
    }

    private static ExtractedMeta extractMeta(Map<String, Object> object, PmlTreeOptions options) {
        String type = null;
        String content = null;
        String tag = null;
        Integer order = null;
        boolean hasAny = false;

        String typeKey = metaKey(options, META_TYPE);
        if (object.containsKey(typeKey)) {
            type = expectTreeString(object.get(typeKey), typeKey);
            hasAny = true;
        }

        String contentKey = metaKey(options, META_CONTENT);
        if (object.containsKey(contentKey)) {
            content = expectTreeString(object.get(contentKey), contentKey);
            hasAny = true;
        }

        String tagKey = metaKey(options, META_TAG);
        if (object.containsKey(tagKey)) {
            tag = expectTreeString(object.get(tagKey), tagKey);
            hasAny = true;
        }

        String orderKey = metaKey(options, META_ORDER);
        if (object.containsKey(orderKey)) {
            order = expectTreeInteger(object.get(orderKey), orderKey);
            if (order < 0) {
                throw new PmlError(0, PmlErrorKind.INVALID_TREE, "meta field `" + orderKey + "` must be a non-negative integer");
            }
            hasAny = true;
        }

        return new ExtractedMeta(type, content, tag, order, hasAny);
    }

    private static String expectTreeString(Object value, String key) {
        if (value instanceof String text) {
            return text;
        }
        throw new PmlError(0, PmlErrorKind.INVALID_TREE, "meta field `" + key + "` must be a string");
    }

    private static Integer expectTreeInteger(Object value, String key) {
        if (value instanceof Integer number) {
            return number;
        }
        if (value instanceof Long number) {
            return Math.toIntExact(number);
        }
        throw new PmlError(0, PmlErrorKind.INVALID_TREE, "meta field `" + key + "` must be an integer");
    }

    private static Map<String, Object> castObject(Map<?, ?> map) {
        @SuppressWarnings("unchecked")
        Map<String, Object> casted = (Map<String, Object>) map;
        return casted;
    }

    private static List<Object> castList(List<?> list) {
        @SuppressWarnings("unchecked")
        List<Object> casted = (List<Object>) list;
        return casted;
    }

    private static String[] slice(String[] values, int start, int end) {
        String[] slice = new String[end - start];
        System.arraycopy(values, start, slice, 0, end - start);
        return slice;
    }

    private static String joinSegments(String[] segments, int length) {
        if (length <= 0) {
            return "";
        }
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < length; i += 1) {
            if (i > 0) {
                out.append('.');
            }
            out.append(segments[i]);
        }
        return out.toString();
    }
}
