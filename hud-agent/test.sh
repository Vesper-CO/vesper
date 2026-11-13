#!/bin/sh
# Runs the agent tests (after ./build.sh): the patched call reaches the overlay, and the in-game editor saves a drag.
set -e
cd "$(dirname "$0")"
mkdir -p out/test
javac -nowarn -cp out/classes:out/stubs -d out/test test/AgentMain.java test/EditTest.java test/dev/vesper/hud/overlay/RenderTest.java test/dev/vesper/hud/overlay/PanelTest.java 2>&1 | grep -v "^Note" || true
T=$(mktemp -d); mkdir -p "$T/game/config"
for P in glfw sdl; do
  rm -rf "$T/game"; mkdir -p "$T/game/config"
  java -Dvesper.hud.config="$T/game/config/vesper-hud.json" -javaagent:out/vesper-hud-agent.jar -cp out/test AgentMain out/stubs/ $P | tail -2
  rm -rf "$T/game"; mkdir -p "$T/game/config"
  java -Dvesper.hud.config="$T/game/config/vesper-hud.json" -javaagent:out/vesper-hud-agent.jar -cp out/test EditTest out/stubs/ $P | tail -2
done
java -Djava.awt.headless=true -cp out/test:out/classes:out/stubs dev.vesper.hud.overlay.PanelTest src/dev/vesper/hud/overlay/font.bin | tail -1
rm -rf "$T"
