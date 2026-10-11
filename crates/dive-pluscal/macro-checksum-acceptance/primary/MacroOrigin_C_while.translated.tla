---- MODULE MacroOrigin_C_while ----
EXTENDS Integers, TLC
(* --algorithm A { variables x=0, y=0; macro Put(v, a) { while (v<a) { v:=v+1; }; }; macro Outer(v,a) { Put(v,a); }; { Outer(x, 1); skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f61a712" /\ chksum(tla) = "a2856bc6")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x<1
               THEN /\ x' = x+1
                    /\ pc' = "Lbl_1"
               ELSE /\ TRUE
                    /\ pc' = "Done"
                    /\ x' = x
         /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
