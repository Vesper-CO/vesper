#!/bin/sh
# Runs the agent tests (after ./build.sh): the patched call reaches the overlay, and the in-game editor saves a drag.
set -e
cd "$(dirname "$0")"
mkdir -p out/test
javac -nowarn -cp out/classes:out/stubs -d out/test test/AgentMain.java test/EditTest.java test/dev/vesper/hud/overlay/RenderTest.java 2>&1 | grep -v "^Note" || true
T=$(mktemp -d); mkdir -p "$T/game/config"
java -Dvesper.hud.config="$T/game/config/vesper-hud.json" -javaagent:out/vesper-hud-agent.jar -cp out/test AgentMain out/stubs/ | tail -2
java -Dvesper.hud.config="$T/game/config/vesper-hud.json" -javaagent:out/vesper-hud-agent.jar -cp out/test EditTest out/stubs/ | tail -2
rm -rf "$T"
