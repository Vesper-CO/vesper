#!/bin/sh
# Builds the HUD agent jar and copies it to src-tauri/resources (the launcher embeds it).
# Needs a JDK 17+ and ASM 9.x (Debian/Ubuntu: apt install libasm-java; or download asm and asm-commons from Maven Central).
set -e
cd "$(dirname "$0")"
ASM=${ASM:-/usr/share/java/asm-9.7.jar}
ASM_COMMONS=${ASM_COMMONS:-/usr/share/java/asm-commons-9.7.jar}
ASM_TREE=${ASM_TREE:-/usr/share/java/asm-tree-9.7.jar}
rm -rf out && mkdir -p out/stubs out/classes out/tools
javac --release 8 -nowarn -d out/stubs $(find stubs -name '*.java')
javac --release 8 -nowarn -cp "$ASM:out/stubs" -d out/classes $(find src -name '*.java')
cp src/dev/vesper/hud/overlay/font.bin out/classes/dev/vesper/hud/overlay/
javac -cp "$ASM:$ASM_COMMONS:$ASM_TREE" -d out/tools tools/Relocate.java
mkdir -p out/extra/META-INF
cp LICENSE-ASM.txt out/extra/META-INF/LICENSE-ASM.txt
java -cp "out/tools:$ASM:$ASM_COMMONS:$ASM_TREE" Relocate out/classes "$ASM" out/extra out/vesper-hud-agent.jar
mkdir -p ../src-tauri/resources
cp out/vesper-hud-agent.jar ../src-tauri/resources/vesper-hud-agent.jar
echo "built $(wc -c < out/vesper-hud-agent.jar) bytes"
