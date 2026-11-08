---- MODULE TmLevel ----
EXTENDS Naturals, TLC
VARIABLE x
ASSUME PrintT(<<"assume", TLCGet("level")>>)
Init == x = 0 /\ PrintT(<<"init", TLCGet("level")>>)
Next == x < 3 /\ PrintT(<<"next", x, TLCGet("level")>>) /\ x' = x + 1
Inv == PrintT(<<"inv", x, TLCGet("level")>>)
Con == PrintT(<<"con", x, TLCGet("level")>>)
Act == PrintT(<<"act", x, x', TLCGet("level")>>)
Prop == [][Act]_x
====
