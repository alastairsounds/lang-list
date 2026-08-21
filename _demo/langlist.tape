# langlist demo: language breakdown of a cloned repo, with the git remote
# shown for context. Static output (nothing animates), so we skip the gif
# and grab a screenshot directly.
# Run: vhs langlist.tape (from this directory).

Set Shell "zsh"
Set FontFamily "FiraCode Nerd Font"
Set FontSize 20
Set Width 1200
Set Height 500
Set Theme "Catppuccin Mocha"
Set TypingSpeed 30ms
Set Padding 20

Type "clear && cd ~/Code/Clones/linguist"
Enter
Sleep 500ms
Type "git remote -v"
Enter
Sleep 500ms
Type "langlist . && tput civis"
Enter
Sleep 1s
Screenshot langlist.png
Sleep 500ms
