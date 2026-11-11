package dev.vesper.hud;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.render.RenderTickCounter;
import net.minecraft.entity.EquipmentSlot;
import net.minecraft.entity.effect.StatusEffectInstance;
import net.minecraft.item.ItemStack;

import java.awt.Color;
import java.time.LocalTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayDeque;
import java.util.Locale;
import java.util.function.Consumer;

/** Lays out and draws every enabled element. Positions are fractions of the free space, like the launcher's preview. */
final class HudRenderer {
    private static final String[] ORDER = { "fps", "cps", "ping", "coords", "direction", "memory", "clock", "keystrokes", "armor", "potions" };
    private static final ArrayDeque<Long> LEFT = new ArrayDeque<>(), RIGHT = new ArrayDeque<>();
    private static boolean wasLeft, wasRight;

    /** A laid-out element: its size in GUI units and how to draw its content at (0, 0). */
    private record Block(int w, int h, boolean boxed, Consumer<DrawContext> draw) {}

    static void render(DrawContext ctx, RenderTickCounter tick) {
        MinecraftClient mc = MinecraftClient.getInstance();
        HudConfig cfg = HudConfig.get();
        trackClicks(mc);
        if (!cfg.enabled || mc.player == null || mc.options.hudHidden || mc.getDebugHud().shouldShowDebugHud()) return;
        int sw = mc.getWindow().getScaledWidth(), sh = mc.getWindow().getScaledHeight();
        for (String id : ORDER) {
            HudConfig.Element e = cfg.elements.get(id);
            if (e == null || !e.enabled) continue;
            Block b = build(id, e, mc, ctx);
            if (b == null) continue;
            float s = Math.max(0.3f, e.scale * cfg.scale);
            float px = clamp01(e.x) * Math.max(0, sw - b.w * s), py = clamp01(e.y) * Math.max(0, sh - b.h * s);
            ctx.getMatrices().push();
            ctx.getMatrices().translate(px, py, 0f);
            ctx.getMatrices().scale(s, s, 1f);
            if (b.boxed) box(ctx, e, 0, 0, b.w, b.h);
            b.draw.accept(ctx);
            ctx.getMatrices().pop();
        }
    }

    private static float clamp01(float v) { return Math.max(0f, Math.min(1f, v)); }

    // ----- building each element -----

    private static Block build(String id, HudConfig.Element e, MinecraftClient mc, DrawContext ctx) {
        TextRenderer tr = mc.textRenderer;
        switch (id) {
            case "fps": return text(e, tr, String.valueOf(mc.getCurrentFps()));
            case "cps": return text(e, tr, cps(LEFT) + " | " + cps(RIGHT));
            case "ping": return text(e, tr, ping(mc));
            case "coords": return text(e, tr, (int) Math.floor(mc.player.getX()) + " / " + (int) Math.floor(mc.player.getY()) + " / " + (int) Math.floor(mc.player.getZ()));
            case "direction": { String d = mc.player.getHorizontalFacing().asString(); return text(e, tr, Character.toUpperCase(d.charAt(0)) + d.substring(1)); }
            case "memory": { Runtime r = Runtime.getRuntime(); double used = (r.totalMemory() - r.freeMemory()) / 1073741824.0, max = r.maxMemory() / 1073741824.0; return text(e, tr, String.format(Locale.ROOT, "%.1f / %.1f GB", used, max)); }
            case "clock": return text(e, tr, LocalTime.now().format(DateTimeFormatter.ofPattern("HH:mm")));
            case "keystrokes": return keystrokes(e, tr, mc);
            case "armor": return armor(e, tr, mc);
            case "potions": return potions(e, tr, mc);
            default: return null;
        }
    }

    /** "label value suffix": the template with {v} replaced, label and value in their own colours. */
    private static Block text(HudConfig.Element e, TextRenderer tr, String value) {
        String t = e.template == null ? "{v}" : e.template;
        int i = t.indexOf("{v}");
        String pre = i < 0 ? t : t.substring(0, i), post = i < 0 ? "" : t.substring(i + 3), val = i < 0 ? "" : value;
        int pad = Math.round(e.padding), w = tr.getWidth(pre) + tr.getWidth(val) + tr.getWidth(post) + 2 * (pad + 1), h = 8 + 2 * pad;
        return new Block(w, h, true, c -> parts(c, tr, e, pad + 1, pad, pre, val, post));
    }

    private static void parts(DrawContext c, TextRenderer tr, HudConfig.Element e, int x, int y, String pre, String val, String post) {
        int label = color(e, e.labelColor, 0xC9BFFF), value = color(e, e.valueColor, 0xFFFFFF);
        c.drawText(tr, pre, x, y, label, e.shadow);
        x += tr.getWidth(pre);
        c.drawText(tr, val, x, y, value, e.shadow);
        x += tr.getWidth(val);
        c.drawText(tr, post, x, y, label, e.shadow);
    }

    private static Block keystrokes(HudConfig.Element e, TextRenderer tr, MinecraftClient mc) {
        final int u = 9, g = 2, kh = 18;
        var o = mc.options;
        // label, first column, columns spanned, row, pressed
        Object[][] keys = {
            { "W", 2, 2, 0, o.forwardKey.isPressed() }, { "A", 0, 2, 1, o.leftKey.isPressed() }, { "S", 2, 2, 1, o.backKey.isPressed() }, { "D", 4, 2, 1, o.rightKey.isPressed() },
            { "LMB", 0, 3, 2, o.attackKey.isPressed() }, { "RMB", 3, 3, 2, o.useKey.isPressed() }, { "SPACE", 0, 6, 3, o.jumpKey.isPressed() },
        };
        int w = 6 * u + 5 * g, h = 4 * kh + 3 * g;
        return new Block(w, h, false, c -> {
            for (Object[] k : keys) {
                int col = (int) k[1], span = (int) k[2], row = (int) k[3];
                int x = col * (u + g), y = row * (kh + g), kw = span * u + (span - 1) * g;
                box(c, e, x, y, kw, kh);
                if ((boolean) k[4]) fillRounded(c, x, y, kw, kh, Math.round(e.radius), HudConfig.argb(e.valueColor, 0.38f, 0xFFFFFF));
                String label = (String) k[0];
                c.drawText(tr, label, x + (kw - tr.getWidth(label)) / 2, y + (kh - 8) / 2, color(e, e.valueColor, 0xFFFFFF), e.shadow);
            }
        });
    }

    private static Block armor(HudConfig.Element e, TextRenderer tr, MinecraftClient mc) {
        EquipmentSlot[] slots = { EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET };
        ItemStack[] stacks = new ItemStack[4];
        int rows = 0, textW = 0;
        for (int i = 0; i < 4; i++) {
            stacks[i] = mc.player.getEquippedStack(slots[i]);
            if (!stacks[i].isEmpty()) { rows++; textW = Math.max(textW, tr.getWidth(durability(stacks[i]))); }
        }
        if (rows == 0) return null;
        int pad = Math.round(e.padding), w = 2 * (pad + 1) + 16 + 3 + textW, h = 2 * pad + rows * 16;
        return new Block(w, h, true, c -> {
            int y = pad;
            for (ItemStack s : stacks) {
                if (s.isEmpty()) continue;
                c.drawItem(s, pad + 1, y);
                c.drawText(tr, durability(s), pad + 1 + 16 + 3, y + 4, color(e, e.valueColor, 0xFFFFFF), e.shadow);
                y += 16;
            }
        });
    }

    private static String durability(ItemStack s) { return s.isDamageable() ? String.valueOf(s.getMaxDamage() - s.getDamage()) : ""; }

    private static Block potions(HudConfig.Element e, TextRenderer tr, MinecraftClient mc) {
        var effects = mc.player.getStatusEffects();
        if (effects.isEmpty()) return null;
        int pad = Math.round(e.padding), w = 0, n = 0;
        String[] names = new String[effects.size()], times = new String[effects.size()];
        for (StatusEffectInstance fx : effects) {
            String name = fx.getEffectType().value().getName().getString() + (fx.getAmplifier() > 0 ? " " + roman(fx.getAmplifier() + 1) : "");
            int secs = fx.getDuration() / 20;
            names[n] = name; times[n] = fx.isInfinite() ? "**" : String.format(Locale.ROOT, "%d:%02d", secs / 60, secs % 60);
            w = Math.max(w, tr.getWidth(name + " " + times[n]));
            n++;
        }
        final int count = n;
        return new Block(w + 2 * (pad + 1), 2 * pad + count * 10 - 2, true, c -> {
            for (int i = 0; i < count; i++) parts(c, tr, e, pad + 1, pad + i * 10, names[i] + " ", times[i], "");
        });
    }

    // ----- values -----

    private static String roman(int n) { String[] r = { "", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X" }; return n < r.length ? r[n] : String.valueOf(n); }

    private static String ping(MinecraftClient mc) {
        var nh = mc.getNetworkHandler();
        if (nh == null || mc.isInSingleplayer()) return "-";
        var entry = nh.getPlayerListEntry(mc.player.getUuid());
        return entry == null ? "-" : entry.getLatency() + " ms";
    }

    /** Count a click each time the mouse button goes down (checked every frame). */
    private static void trackClicks(MinecraftClient mc) {
        long now = System.currentTimeMillis();
        boolean l = mc.mouse.wasLeftButtonClicked(), r = mc.mouse.wasRightButtonClicked();
        if (l && !wasLeft) LEFT.addLast(now);
        if (r && !wasRight) RIGHT.addLast(now);
        wasLeft = l; wasRight = r;
    }

    private static int cps(ArrayDeque<Long> clicks) {
        long cutoff = System.currentTimeMillis() - 1000;
        while (!clicks.isEmpty() && clicks.peekFirst() < cutoff) clicks.pollFirst();
        return clicks.size();
    }

    // ----- drawing helpers -----

    private static int color(HudConfig.Element e, String hex, int fallback) {
        if (e.rainbow) return Color.HSBtoRGB((System.currentTimeMillis() % 4000) / 4000f, 0.7f, 1f) | 0xFF000000;
        return 0xFF000000 | HudConfig.rgb(hex, fallback);
    }

    /** Background, border and rounded corners of one box. Fully transparent when the opacity is 0. */
    private static void box(DrawContext c, HudConfig.Element e, int x, int y, int w, int h) {
        int r = Math.round(e.radius), bw = Math.round(e.borderWidth);
        fillRounded(c, x, y, w, h, r, HudConfig.argb(e.bg, e.bgOpacity, 0x0B0B17));
        int bc = HudConfig.argb(e.border, e.borderOpacity, 0x8F7CFF);
        if (bw > 0 && (bc >>> 24) != 0) {
            c.fill(x + r, y, x + w - r, y + bw, bc);               // top
            c.fill(x + r, y + h - bw, x + w - r, y + h, bc);       // bottom
            c.fill(x, y + r, x + bw, y + h - r, bc);               // left
            c.fill(x + w - bw, y + r, x + w, y + h - r, bc);       // right
        }
    }

    private static void fillRounded(DrawContext c, int x, int y, int w, int h, int r, int color) {
        if ((color >>> 24) == 0) return;
        r = Math.max(0, Math.min(r, Math.min(w, h) / 2));
        if (r == 0) { c.fill(x, y, x + w, y + h, color); return; }
        c.fill(x, y + r, x + w, y + h - r, color);
        for (int i = 0; i < r; i++) {
            double dy = r - i - 0.5;
            int inset = r - (int) Math.round(Math.sqrt(r * r - dy * dy));
            c.fill(x + inset, y + i, x + w - inset, y + i + 1, color);
            c.fill(x + inset, y + h - i - 1, x + w - inset, y + h - i, color);
        }
    }
}
