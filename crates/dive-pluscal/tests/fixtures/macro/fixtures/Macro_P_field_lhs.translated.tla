---- MODULE Macro_P_field_lhs ----
EXTENDS Integers, TLC
(* --algorithm A variables x=[a |-> 0], y=0; macro Put(v, a) begin v := a; end macro; begin L: Put(x.a,2); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "94ac6685" /\ chksum(tla) = "e99b1207")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = [a |-> 0]
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [x EXCEPT !.a = 2]
     /\ pc' = "E"
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
