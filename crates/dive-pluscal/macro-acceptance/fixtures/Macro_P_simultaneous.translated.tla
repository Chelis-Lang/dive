---- MODULE Macro_P_simultaneous ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(a, b) begin x := <<a,b>>; end macro; begin L: Put(b, 3); E: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "6569fd9a" /\ chksum(tla) = "f3308f0f")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = <<b,3>>
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
