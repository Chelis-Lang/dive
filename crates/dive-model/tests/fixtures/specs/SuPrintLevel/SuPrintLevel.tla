---- MODULE SuPrintLevel ----
EXTENDS Naturals, TLC
VARIABLE x
NotConst == Print("level", TLCGet("level"))
Const == Print("config", TLCGet("config").deadlock)
Init == x = 0
Next == x' = x
====
