---- MODULE MacroFull ----
EXTENDS Integers
(* --algorithm A
variable x = 0;
macro Set(v, a) begin v := a; end macro;
begin Start: Set(x, 1 + 2); Finish: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d36ef1e1" /\ chksum(tla) = "a9295d1a")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Start"

Start == /\ pc = "Start"
         /\ x' = 1 + 2
         /\ pc' = "Finish"

Finish == /\ pc = "Finish"
          /\ TRUE
          /\ pc' = "Done"
          /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Start \/ Finish
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
