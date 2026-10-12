---- MODULE Snapshot ----
EXTENDS Integers,TLC
CONSTANT Token
VARIABLE x
Init == x=0
Next == x'=1-x
StateHelper == x=0
Danger == IF StateHelper THEN PrintT("ERASED_STATE_MARKER") ELSE TRUE
Hook == TRUE
====
