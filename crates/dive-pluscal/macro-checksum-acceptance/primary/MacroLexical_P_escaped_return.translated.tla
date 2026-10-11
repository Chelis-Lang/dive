---- MODULE MacroLexical_P_escaped_return ----
EXTENDS Integers, TLC
(* 🦀 outside quoted expression *)
(* --algorithm A variable x=0; macro Put(v,a) begin v:=a; end macro; begin L: Put(x,"\r"); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1b0fd156" /\ chksum(tla) = "620d0140")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = "\r"
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
