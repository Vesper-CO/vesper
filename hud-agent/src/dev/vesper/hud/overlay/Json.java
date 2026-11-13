package dev.vesper.hud.overlay;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** A small JSON reader (objects, arrays, strings, numbers, booleans, null), so the agent needs no libraries. */
final class Json {
    private final String s;
    private int i;

    private Json(String s) { this.s = s; }

    /** Pretty-print what parse() returns (maps, lists, strings, numbers, booleans). */
    static String write(Object o) {
        StringBuilder b = new StringBuilder();
        put(b, o, 0);
        return b.append('\n').toString();
    }

    @SuppressWarnings("unchecked")
    private static void put(StringBuilder b, Object o, int ind) {
        if (o == null) b.append("null");
        else if (o instanceof String) quote(b, (String) o);
        else if (o instanceof Boolean) b.append(o);
        else if (o instanceof Number) {
            double d = ((Number) o).doubleValue();
            if (d == Math.rint(d) && Math.abs(d) < 1e9) b.append((long) d); else b.append(Math.round(d * 10000.0) / 10000.0);
        } else if (o instanceof Map) {
            Map<String, Object> m = (Map<String, Object>) o;
            if (m.isEmpty()) { b.append("{}"); return; }
            b.append("{\n");
            int n = 0;
            for (Map.Entry<String, Object> e : m.entrySet()) {
                pad(b, ind + 1); quote(b, e.getKey()); b.append(": "); put(b, e.getValue(), ind + 1);
                b.append(++n < m.size() ? ",\n" : "\n");
            }
            pad(b, ind); b.append('}');
        } else if (o instanceof List) {
            List<Object> l = (List<Object>) o;
            b.append('[');
            for (int i = 0; i < l.size(); i++) { if (i > 0) b.append(", "); put(b, l.get(i), ind); }
            b.append(']');
        } else quote(b, String.valueOf(o));
    }

    private static void pad(StringBuilder b, int n) { for (int i = 0; i < n; i++) b.append("  "); }

    private static void quote(StringBuilder b, String s) {
        b.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '"' || c == '\\') b.append('\\').append(c);
            else if (c == '\n') b.append("\\n");
            else if (c < 32) b.append(String.format("\\u%04x", (int) c));
            else b.append(c);
        }
        b.append('"');
    }

    static Object parse(String text) {
        Json j = new Json(text);
        j.ws();
        Object v = j.value();
        j.ws();
        if (j.i != j.s.length()) throw new IllegalArgumentException("trailing data");
        return v;
    }

    private void ws() { while (i < s.length() && Character.isWhitespace(s.charAt(i))) i++; }

    private Object value() {
        if (i >= s.length()) throw new IllegalArgumentException("unexpected end");
        char c = s.charAt(i);
        if (c == '{') return object();
        if (c == '[') return array();
        if (c == '"') return string();
        if (s.startsWith("true", i)) { i += 4; return Boolean.TRUE; }
        if (s.startsWith("false", i)) { i += 5; return Boolean.FALSE; }
        if (s.startsWith("null", i)) { i += 4; return null; }
        int st = i;
        while (i < s.length() && "+-0123456789.eE".indexOf(s.charAt(i)) >= 0) i++;
        if (st == i) throw new IllegalArgumentException("bad value at " + i);
        return Double.valueOf(s.substring(st, i));
    }

    private Map<String, Object> object() {
        Map<String, Object> m = new LinkedHashMap<String, Object>();
        i++; ws();
        if (s.charAt(i) == '}') { i++; return m; }
        while (true) {
            ws();
            String k = string();
            ws();
            if (s.charAt(i++) != ':') throw new IllegalArgumentException("expected ':'");
            ws();
            m.put(k, value());
            ws();
            char c = s.charAt(i++);
            if (c == '}') return m;
            if (c != ',') throw new IllegalArgumentException("expected ',' or '}'");
        }
    }

    private List<Object> array() {
        List<Object> l = new ArrayList<Object>();
        i++; ws();
        if (s.charAt(i) == ']') { i++; return l; }
        while (true) {
            ws();
            l.add(value());
            ws();
            char c = s.charAt(i++);
            if (c == ']') return l;
            if (c != ',') throw new IllegalArgumentException("expected ',' or ']'");
        }
    }

    private String string() {
        if (s.charAt(i++) != '"') throw new IllegalArgumentException("expected string");
        StringBuilder b = new StringBuilder();
        while (true) {
            char c = s.charAt(i++);
            if (c == '"') return b.toString();
            if (c != '\\') { b.append(c); continue; }
            char e = s.charAt(i++);
            switch (e) {
                case 'n': b.append('\n'); break;
                case 't': b.append('\t'); break;
                case 'r': b.append('\r'); break;
                case 'b': b.append('\b'); break;
                case 'f': b.append('\f'); break;
                case 'u': b.append((char) Integer.parseInt(s.substring(i, i + 4), 16)); i += 4; break;
                default: b.append(e);
            }
        }
    }
}
