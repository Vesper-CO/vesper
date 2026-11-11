package dev.vesper.hud;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;

/** Draws the HUD that is styled in the Vesper launcher (config/vesper-hud.json). */
public final class VesperHudMod implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        HudRenderCallback.EVENT.register(HudRenderer::render);
    }
}
