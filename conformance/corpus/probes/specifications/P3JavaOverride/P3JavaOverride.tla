---- MODULE P3JavaOverride ----
\* Probe: an operator whose TLA+ definition is a placeholder and which only
\* works through TLC's Java override (CommunityModules Json!ToJson is defined
\* as CHOOSE over STRING). A tool without the override must refuse; evaluating
\* the TLA+ body literally cannot terminate or yields an arbitrary string.
EXTENDS Integers, Sequences, Json
VARIABLE x
Init == x = ToJson(<<1, 2>>)
Next == UNCHANGED x
Inv == x = "[1,2]"
====
