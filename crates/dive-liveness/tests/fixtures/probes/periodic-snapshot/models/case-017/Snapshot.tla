---- MODULE Snapshot ----
EXTENDS Integers,TLC
CONSTANT Token
VARIABLE x
Init == x=0
Next == x'=1-x
Unused == PrintT("STARTUP_MARKER")
Hook == TRUE
HookArg(v) == v
====
