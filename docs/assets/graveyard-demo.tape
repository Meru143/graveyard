Output docs/assets/graveyard-demo.gif

Set FontSize 18
Set Width 1400
Set Height 760
Set Padding 24
Set Theme "GitHub Dark"
Set TypingSpeed 65ms
Set Shell bash

Hide
Type "bash ./scripts/prepare-hero-demo.sh >/dev/null"
Enter
Sleep 500ms
Type "cd /tmp/graveyard-demo-hero"
Enter
Sleep 300ms
Type "export PATH=$PWD:$PATH"
Enter
Sleep 300ms
Show

Type "graveyard scan --min-age 30d --min-confidence 0.80"
Enter
Sleep 3s

Type "graveyard baseline diff --baseline .graveyard-baseline.json --ci"
Enter
Sleep 2s

Type "echo $?"
Enter
Sleep 2s
