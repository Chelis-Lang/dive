---- MODULE TfAliasHolds ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Identity(p) == p
Relay(p) == LET Saved == p IN Identity(Saved)
Claim == Relay([]<>x)
Alias == Claim
====
