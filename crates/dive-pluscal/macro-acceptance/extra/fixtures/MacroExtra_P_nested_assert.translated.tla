---- MODULE MacroExtra_P_nested_assert ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin assert a>0; v := a; end macro;macro Outer(v, a) begin Put(v,a+1); end macro; begin L: Outer(x,2); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "3e45acb9" /\ chksum(tla) = "43b370fd")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Assert((2+1)>0, 
               "Failure of assertion at line 3, column 60 of macro called at line 3, column 146.")
     /\ x' = 2+1
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
