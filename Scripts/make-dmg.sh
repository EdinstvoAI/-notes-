#!/bin/zsh
# Установщик: собирает Release, кладёт приложение и ярлык «Программы» в окно с фоном
# (Scripts/dmg-background.swift) и сжимает в build/notes.dmg. Release-копию потом удаляет:
# у Макара должна оставаться одна копия приложения — в /Applications.
set -e
cd "$(dirname "$0")/.."
APP="build/Build/Products/Release/<notes>.app"
xcodebuild -project "<notes>.xcodeproj" -scheme "<notes>" -configuration Release -destination 'platform=macOS' \
    -derivedDataPath build $XCODE_ARGS build | grep -E "BUILD (SUCCEEDED|FAILED)"
rm -rf build/dmg build/rw.dmg && mkdir -p build/dmg/.background
cp -R "$APP" build/dmg/ && ln -s /Applications build/dmg/Applications
swift Scripts/dmg-background.swift build/dmg/.background/bg.png
hdiutil create -volname "<notes>" -srcfolder build/dmg -ov -format UDRW build/rw.dmg -quiet
DEV=$(hdiutil attach build/rw.dmg -readwrite -noverify -noautoopen | awk '/Apple_HFS|APFS/ {print $1; exit}')
sleep 1
chflags hidden "/Volumes/<notes>/.background" "/Volumes/<notes>/.fseventsd" 2>/dev/null || true  # иначе видны при «показывать скрытые»
osascript <<'OSA'
tell application "Finder"
    tell disk "<notes>"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set bounds of container window to {200, 120, 840, 548}
        set opts to icon view options of container window
        set arrangement of opts to not arranged
        set icon size of opts to 96
        set text size of opts to 12
        set background picture of opts to file ".background:bg.png"
        set position of item "<notes>.app" of container window to {160, 200}
        set position of item "Applications" of container window to {480, 200}
        -- Если в Finder включён показ скрытых — служебное уносим за край окна.
        repeat with junk in {".background", ".fseventsd", ".DS_Store"}
            try
                set position of item junk of container window to {900, 900}
            end try
        end repeat
        update without registering applications
        delay 1
        close
    end tell
end tell
OSA
sync; hdiutil detach "$DEV" -quiet
hdiutil convert build/rw.dmg -format UDZO -ov -o build/notes.dmg -quiet
rm -rf build/dmg build/rw.dmg "$APP"
ls -lh build/notes.dmg | awk '{print $5, $9}'
