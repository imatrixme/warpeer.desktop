on run arguments
    set volumeName to item 1 of arguments
    set mountPath to item 2 of arguments

    tell application "Finder"
        tell disk volumeName
            open
            tell container window
                set current view to icon view
                set toolbar visible to false
                set statusbar visible to false
                set pathbar visible to false
                set sidebar width to 0
                set bounds to {180, 140, 900, 590}
            end tell

            tell icon view options of container window
                set arrangement to not arranged
                set icon size to 128
                set text size to 13
                set label position to bottom
                set background color to {61952, 62976, 63744}
            end tell

            set position of item "Warpeer.app" to {190, 215}
            set position of item "Applications" to {530, 215}
            delay 2
            close container window
            delay 2
        end tell
    end tell
end run
